//! Windows-only native surfaces for the Tauri host: the HUD overlay, the tray
//! icon/menu and the mouse-wheel hotkey bridge.
//!
//! The wheel hook (`wheel_win32`) posts `WM_APP_WHEEL` to a hidden bridge
//! window; its window proc converts the payload back to a [`HotkeyAction`]
//! and pushes it into a channel the host poll thread drains.

use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::time::Duration;

use volumectl_lib::audio::VolumeState;
use volumectl_lib::config::Config;
use volumectl_lib::hotkeys::{hotkey_from_id, HotkeyAction};
use volumectl_lib::overlay::{Overlay, OverlayAppearance};
use volumectl_lib::tray::Tray;
use volumectl_lib::ui::UiCapabilities;
use volumectl_lib::wheel_win32;

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM, LRESULT, WAIT_OBJECT_0, WPARAM,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{
    CreateEventW, CreateMutexW, OpenEventW, SetEvent, WaitForSingleObject, EVENT_MODIFY_STATE,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    keybd_event, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, VK_MENU,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW, RegisterClassW,
    SetWindowLongPtrW, CW_USEDEFAULT, GWLP_USERDATA, WNDCLASSW, WS_EX_TOOLWINDOW, WS_OVERLAPPED,
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
// wheel channel is an `mpsc::Receiver`. The overlay is the only cross-thread
// data path: `show_overlay`/`show_overlay_text` run on the hotkey poll / IPC
// command threads and write `OverlayData` under `Mutex<Overlay>` before
// posting a paint message; the read happens in the overlay's `WM_PAINT` on
// the main thread, ordered after the write by the win32 message queue's
// cross-thread posting edge (the same documented trust boundary the legacy
// host used). This mirrors the existing `unsafe impl Send + Sync` for
// `GlobalHotkeys`/`WindowsAudio` in the volumectl crate.
unsafe impl Send for NativeWin32 {}
unsafe impl Sync for NativeWin32 {}

/// Named auto-reset event the primary instance waits on. A second launch
/// (desktop icon, Start menu, winget) signals it instead of exiting silently,
/// so the already-running tray host can reveal the mixer.
const SHOW_MIXER_EVENT_NAME: windows_sys::core::PCWSTR =
    windows_sys::core::w!("Local\\VolumeControl.ShowMixer");
static SHOW_MIXER_EVENT_HANDLE: AtomicIsize = AtomicIsize::new(0);

const EXIT_EVENT_NAME: windows_sys::core::PCWSTR =
    windows_sys::core::w!("Local\\VolumeControl.Exit");
static EXIT_EVENT_HANDLE: AtomicIsize = AtomicIsize::new(0);
/// Prevent two app instances (VolumePro's `#SingleInstance Force`
/// equivalent). Mirrors the legacy host's named-mutex guard verbatim.
/// Returns `false` when another instance already holds the mutex. A foreground
/// second launch asks the running host to show the mixer; a background launch
/// exits tray-only. The mutex handle is intentionally kept for the owning
/// instance's lifetime.
pub fn ensure_single_instance(show_mixer: bool) -> bool {
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
            if show_mixer {
                request_show_mixer();
            }
            return false;
        }
        create_show_mixer_event();
        create_exit_event();
        true
    }
}

/// Create the event the fast poll thread watches for "show the mixer" requests.
fn create_show_mixer_event() {
    unsafe {
        let event = CreateEventW(std::ptr::null(), 0, 0, SHOW_MIXER_EVENT_NAME);
        if event == 0 {
            log::warn!("could not create the show-mixer event; second launches will stay silent");
            return;
        }
        SHOW_MIXER_EVENT_HANDLE.store(event as isize, Ordering::SeqCst);
    }
}

/// Create the event the fast poll thread watches for deliberate "exit" requests.
fn create_exit_event() {
    unsafe {
        let event = CreateEventW(std::ptr::null(), 0, 0, EXIT_EVENT_NAME);
        if event == 0 {
            log::warn!("could not create the exit event; quit requests will stay silent");
            return;
        }
        EXIT_EVENT_HANDLE.store(event as isize, Ordering::SeqCst);
    }
}

/// Ask the running primary instance to exit gracefully.
pub fn request_exit_event() -> bool {
    unsafe {
        for _ in 0..100 {
            let event = OpenEventW(EVENT_MODIFY_STATE, 0, EXIT_EVENT_NAME);
            if event != 0 {
                SetEvent(event);
                CloseHandle(event);
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        log::warn!("running instance did not expose the exit event");
        false
    }
}

/// Consume a pending exit request. Auto-reset event + zero timeout.
pub fn take_exit_request() -> bool {
    let handle = EXIT_EVENT_HANDLE.load(Ordering::Acquire);
    if handle == 0 {
        return false;
    }
    unsafe {
        WaitForSingleObject(handle as windows_sys::Win32::Foundation::HANDLE, 0) == WAIT_OBJECT_0
    }
}
/// Ask the running primary instance to reveal the mixer. The event may not be
/// created yet while the primary is still between the mutex and the event
/// (startup microseconds), so retry briefly instead of dropping the request.
fn request_show_mixer() {
    unsafe {
        for _ in 0..100 {
            let event = OpenEventW(EVENT_MODIFY_STATE, 0, SHOW_MIXER_EVENT_NAME);
            if event != 0 {
                SetEvent(event);
                CloseHandle(event);
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        log::warn!("running instance did not expose the show-mixer event");
    }
}

/// Consume a pending show-mixer request. Auto-reset event + zero timeout:
/// the signal is cleared only when this returns `true`, preserving one pending
/// request for the next poll.
pub fn take_show_mixer_request() -> bool {
    let handle = SHOW_MIXER_EVENT_HANDLE.load(Ordering::Acquire);
    if handle == 0 {
        return false;
    }
    unsafe {
        WaitForSingleObject(handle as windows_sys::Win32::Foundation::HANDLE, 0) == WAIT_OBJECT_0
    }
}
impl NativeWin32 {
    /// Create the hidden wheel-bridge window, install the wheel hook, and
    /// create the native overlay + tray.
    pub fn new(lang: volumectl_lib::config::Language) -> Result<Self, String> {
        let (wheel_tx, wheel_rx) = channel();
        let wheel_hwnd = unsafe { create_wheel_bridge_window(wheel_tx)? };
        let caps = volumectl_lib::ui::primitives::detect_capabilities(wheel_hwnd);
        let overlay = Overlay::new().map_err(|e| format!("create overlay: {e}"))?;
        let tray = Tray::new(lang).map_err(|e| format!("create tray: {e}"))?;
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
        let appearance = self.overlay_appearance(config);
        self.overlay
            .lock()
            .expect("overlay mutex poisoned")
            .show(state, config, &appearance);
    }

    /// Show a short text card on the overlay (e.g. "Config reloaded").
    pub fn show_overlay_text(&self, text: &str, config: &Config) {
        let appearance = self.overlay_appearance(config);
        self.overlay
            .lock()
            .expect("overlay mutex poisoned")
            .show_text(text, config, &appearance);
    }

    /// Relabel the native tray after the configured language changes.
    pub fn apply_language(&self, lang: volumectl_lib::config::Language) {
        self.tray.apply_language(lang);
    }

    /// Single resolution point for the native overlay's adaptive appearance
    /// (config theme/material/motion/accent + the startup capability snapshot).
    fn overlay_appearance(&self, config: &Config) -> OverlayAppearance {
        OverlayAppearance::resolve(
            config,
            &self.caps,
            volumectl_lib::ui::primitives::system_theme,
        )
    }

    /// Release native resources on shutdown: uninstall the low-level wheel
    /// hook (restores the legacy host's `uninstall_wheel_hook` exit path) and
    /// destroy the hidden wheel-bridge window. The overlay/tray windows are
    /// OS-reclaimed at process exit.
    pub fn shutdown(&self) {
        wheel_win32::uninstall_wheel_hook();
        unsafe {
            DestroyWindow(self._wheel_hwnd);
        }
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

#[cfg(test)]
mod tests {
    use super::{
        create_exit_event, create_show_mixer_event, request_exit_event, request_show_mixer,
        take_exit_request, take_show_mixer_request,
    };

    #[test]
    fn show_mixer_event_is_auto_reset() {
        create_show_mixer_event();
        assert!(!take_show_mixer_request(), "fresh event must be clear");
        request_show_mixer();
        assert!(
            take_show_mixer_request(),
            "signalled event must be observed"
        );
        assert!(
            !take_show_mixer_request(),
            "auto-reset event must clear after one wait"
        );
    }

    #[test]
    fn exit_event_is_auto_reset() {
        create_exit_event();
        assert!(!take_exit_request(), "fresh exit event must be clear");
        assert!(request_exit_event(), "signalling exit event must succeed");
        assert!(take_exit_request(), "signalled exit event must be observed");
        assert!(
            !take_exit_request(),
            "auto-reset exit event must clear after one wait"
        );
    }
}
