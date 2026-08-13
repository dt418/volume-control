//! Windows-only native surfaces for the Tauri host: the HUD overlay, the tray
//! icon/menu and the mouse-wheel hotkey bridge.
//!
//! The wheel hook (`wheel_win32`) posts `WM_APP_WHEEL` to a hidden bridge
//! window; its window proc converts the payload back to a [`HotkeyAction`]
//! and pushes it into a channel the host poll thread drains.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;

use volumectl_lib::audio::VolumeState;
use volumectl_lib::config::Config;
use volumectl_lib::hotkeys::{hotkey_from_id, HotkeyAction};
use volumectl_lib::overlay::{Overlay, OverlayAppearance};
use volumectl_lib::tray::{Tray, TrayCommand};
use volumectl_lib::ui::UiCapabilities;
use volumectl_lib::wheel_win32;

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM, LRESULT, WPARAM,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    keybd_event, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, VK_MENU,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetWindowLongPtrW, RegisterClassW, SetWindowLongPtrW,
    CW_USEDEFAULT, GWLP_USERDATA, WNDCLASSW, WS_EX_TOOLWINDOW, WS_OVERLAPPED,
};

/// Native surfaces for the Windows host. All state is interior-mutable so the
/// instance can live behind an `Arc` shared with the sink and the poll
/// threads.
pub struct NativeWin32 {
    overlay: Mutex<Overlay>,
    tray: Tray,
    caps: UiCapabilities,
    wheel_rx: Receiver<HotkeyAction>,
    _wheel_hwnd: HWND,
}

// Safety: the native handles (overlay/tray windows, wheel bridge hwnd) are
// only ever touched through their own win32 message queues — ShowWindow,
// tray-icon and wheel-hook calls marshal to the owning threads — and the
// wheel channel is an `mpsc::Receiver`. This mirrors the existing
// `unsafe impl Send + Sync` for `GlobalHotkeys`/`WindowsAudio` in the
// volumectl crate (documented trust boundary, single native-owner per handle).
unsafe impl Send for NativeWin32 {}
unsafe impl Sync for NativeWin32 {}

/// Prevent two app instances (VolumePro's `#SingleInstance Force`
/// equivalent). Mirrors the legacy host's named-mutex guard verbatim.
/// Returns `false` when another instance already holds the mutex (the caller
/// logs the warning and exits); the mutex handle is intentionally kept for
/// the owning instance's lifetime.
pub fn ensure_single_instance() -> bool {
    unsafe {
        let h = CreateMutexW(
            std::ptr::null(),
            1, // bInitialOwner
            windows_sys::core::w!("Local\\VolumeControl.SingleInstance"),
        );
        if h == 0 {
            return true; // could not create — allow (unusual)
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            CloseHandle(h);
            log::warn!("another VolumeControl instance is already running");
            return false;
        }
        true
    }
}
impl NativeWin32 {
    /// Create the hidden wheel-bridge window, install the wheel hook, and
    /// create the native overlay + tray.
    pub fn new() -> Result<Self, String> {
        let (wheel_tx, wheel_rx) = channel();
        let wheel_hwnd = unsafe { create_wheel_bridge_window(wheel_tx)? };
        let caps = volumectl_lib::ui::primitives::detect_capabilities(wheel_hwnd);
        let overlay = Overlay::new().map_err(|e| format!("create overlay: {e}"))?;
        let tray = Tray::new().map_err(|e| format!("create tray: {e}"))?;
        wheel_win32::install_wheel_hook(wheel_hwnd)
            .map_err(|e| format!("install wheel hook: {e}"))?;
        Ok(Self {
            overlay: Mutex::new(overlay),
            tray,
            caps,
            wheel_rx,
            _wheel_hwnd: wheel_hwnd,
        })
    }

    /// Show the volume HUD overlay (volume state + adaptive appearance).
    pub fn show_overlay(&self, state: &VolumeState, config: &Config) {
        let appearance = OverlayAppearance::resolve(
            config,
            &self.caps,
            volumectl_lib::ui::primitives::system_theme,
        );
        self.overlay
            .lock()
            .expect("overlay mutex poisoned")
            .show(state, config, &appearance);
    }

    /// Show a short text card on the overlay (e.g. "Config reloaded").
    pub fn show_overlay_text(&self, text: &str, config: &Config) {
        let appearance = OverlayAppearance::resolve(
            config,
            &self.caps,
            volumectl_lib::ui::primitives::system_theme,
        );
        self.overlay
            .lock()
            .expect("overlay mutex poisoned")
            .show_text(text, config, &appearance);
    }

    /// Open the tray menu. Background processes cannot SetForegroundWindow
    /// directly (Windows foreground lock); simulating an Alt press unlocks it.
    pub fn show_tray_menu(&self) {
        unsafe {
            keybd_event(VK_MENU as u8, 0, KEYEVENTF_EXTENDEDKEY, 0);
            keybd_event(VK_MENU as u8, 0, KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP, 0);
        }
        self.tray.show_menu();
    }

    /// Refresh the tray tooltip/menu volume display.
    pub fn set_tray_volume(&self, state: &VolumeState) {
        self.tray.set_volume(state);
    }

    /// Poll tray menu commands (called from the slow host poll).
    pub fn poll_tray(&self) -> Option<TrayCommand> {
        self.tray.poll()
    }

    /// Non-blocking drain of wheel-hotkey actions.
    pub fn try_recv_wheel(&self) -> Option<HotkeyAction> {
        self.wheel_rx.try_recv().ok()
    }
}

unsafe fn create_wheel_bridge_window(tx: Sender<HotkeyAction>) -> Result<HWND, String> {
    let hinst = GetModuleHandleW(std::ptr::null());
    let class = windows_sys::core::w!("VolCtlWheelBridge");
    let mut wc: WNDCLASSW = std::mem::zeroed();
    wc.lpfnWndProc = Some(wheel_wndproc);
    wc.hInstance = hinst;
    wc.lpszClassName = class;
    RegisterClassW(&wc);
    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW,
        class,
        windows_sys::core::w!("VolumeControlWheel"),
        WS_OVERLAPPED,
        CW_USEDEFAULT,
        0,
        CW_USEDEFAULT,
        0,
        0, // parent
        0, // menu
        hinst,
        std::ptr::null(),
    );
    if hwnd == 0 {
        return Err("create wheel-bridge window failed".into());
    }
    // The sender is leaked for the process lifetime; the window proc reads it
    // via GWLP_USERDATA.
    let boxed: *mut Sender<HotkeyAction> = Box::into_raw(Box::new(tx));
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, boxed as isize);
    Ok(hwnd)
}

unsafe extern "system" fn wheel_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == wheel_win32::WM_APP_WHEEL {
        if let Some(action) = hotkey_from_id(wparam as i32) {
            let sender = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Sender<HotkeyAction>;
            if !sender.is_null() {
                let _ = (*sender).send(action);
            }
        }
        return 0;
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}
