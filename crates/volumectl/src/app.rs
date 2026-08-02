//! Windows application shell.
//!
//! Creates a single hidden window that owns both [`WindowsAudio`] and
//! [`Win32Hotkeys`]. The window proc handles `WM_HOTKEY` (custom combos) and
//! a 150 ms `WM_TIMER` (external volume sync / media-key detection) in one
//! message loop.
//!
//! The hidden window is purely a message pump; it is never shown. When volume
//! changes via custom hotkeys the change is applied to the system audio
//! backend; external changes (media keys, tray, Bluetooth) are picked up by
//! the timer and logged (tray/overlay attach here in later milestones).

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM, FALSE},
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PostQuitMessage,
        RegisterClassW, SetTimer, TranslateMessage, MSG, WNDCLASSW, WM_DESTROY, WM_HOTKEY,
        WM_TIMER, GWLP_USERDATA, SetWindowLongPtrW, GetWindowLongPtrW, CW_USEDEFAULT, CS_OWNDC,
        WS_POPUP, WS_EX_TOOLWINDOW,
    },
};

use crate::audio::{AudioBackend, VolumeState};
use crate::audio_windows::WindowsAudio;
use crate::config::Config;
use crate::hotkeys::HotkeyAction;
use crate::hotkeys_win32::{Win32Hotkeys, hotkey_action};

const ID_TIMER_POLL: usize = 1;
const POLL_MS: u32 = 150;

/// Heap-allocated state that lives in the window's GWLP_USERDATA.
struct AppContext {
    audio: WindowsAudio,
    _hotkeys: Win32Hotkeys,
    config: Config,
    last_state: VolumeState,
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = crate::config::load();
    let audio = WindowsAudio::new()?;
    let last_state = audio
        .get_state()
        .unwrap_or(VolumeState {
            volume: 0.5,
            muted: false,
        });

    // Hidden host window.
    let hinst = unsafe { GetModuleHandleW(std::ptr::null()) };
    let class = windows_sys::core::w!("VolCtlHost");
    unsafe {
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.style = CS_OWNDC;
        wc.lpfnWndProc = Some(host_wndproc);
        wc.hInstance = hinst;
        wc.lpszClassName = class;
        RegisterClassW(&wc);
    }
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class,
            windows_sys::core::w!("VolumeControl"),
            WS_POPUP,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            0,
            0,
            0, // hwndParent: HWND = isize
            0, // hmenu: HMENU = isize
            hinst,
            std::ptr::null(), // lpParam
        )
    };
    if hwnd == 0 {
        return Err("CreateWindowEx host failed".into());
    }
    log::debug!("host hwnd=0x{:x}", hwnd);

    // Hotkeys are registered against the host window.
    let _hotkeys = Win32Hotkeys::new(hwnd, config.modifier)?;

    // Store context in GWLP_USERDATA.
    let ctx = Box::into_raw(Box::new(AppContext {
        audio,
        _hotkeys,
        config,
        last_state,
    }));
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, ctx as isize);
    }

    // Poll timer (150 ms).
    unsafe {
        let timer_ok = SetTimer(hwnd, ID_TIMER_POLL, POLL_MS, None);
        log::debug!("SetTimer -> {}", timer_ok);
    }

    log::info!("volumectl {} running", crate::VERSION);

    // Message pump: GetMessageW returns 0 on WM_QUIT, -1 on error.
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        let mut seen: usize = 0;
        while GetMessageW(&mut msg, 0, 0, 0) > FALSE {
            seen += 1;
            if seen <= 20 {
                log::trace!(
                    "msg 0x{:04x} hwnd={:x} wparam={}",
                    msg.message,
                    msg.hwnd,
                    msg.wParam
                );
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    // Cleanup the context.
    if !ctx.is_null() {
        unsafe {
            drop(Box::from_raw(ctx));
        }
    }

    Ok(())
}

unsafe extern "system" fn host_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        // ── Custom hotkey fired ──────────────────────────────────────────
        WM_HOTKEY => {
            let ctx = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppContext;
            if let Some(action) = hotkey_action(wparam as i32) {
                if !ctx.is_null() {
                    apply(&mut *ctx, action);
                }
            }
            0
        }
        // ── Periodic poll for external changes ──────────────────────────
        WM_TIMER => {
            let ctx = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppContext;
            if !ctx.is_null() {
                let ctx = &mut *ctx;
                if let Ok(st) = ctx.audio.get_state() {
                    if st != ctx.last_state {
                        log::debug!("ext change: {}% muted={}", st.percent(), st.muted);
                        ctx.last_state = st;
                    }
                }
            }
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn apply(ctx: &mut AppContext, action: HotkeyAction) {
    let old = ctx.last_state;
    log::debug!("hotkey: {:?} (current {}%)", action, old.percent());
    use HotkeyAction as A;
    match action {
        A::VolumeUp => {
            let v = crate::core::step_volume(old.volume, ctx.config.volume_step as f32);
            if let Err(e) = ctx.audio.set_volume(v) {
                log::warn!("{e}");
            }
        }
        A::VolumeDown => {
            let v = crate::core::step_volume(old.volume, -(ctx.config.volume_step as f32));
            if let Err(e) = ctx.audio.set_volume(v) {
                log::warn!("{e}");
            }
        }
        A::VolumeUpLarge => {
            let v = crate::core::step_volume(old.volume, ctx.config.volume_step_large as f32);
            if let Err(e) = ctx.audio.set_volume(v) {
                log::warn!("{e}");
            }
        }
        A::VolumeDownLarge => {
            let v = crate::core::step_volume(old.volume, -(ctx.config.volume_step_large as f32));
            if let Err(e) = ctx.audio.set_volume(v) {
                log::warn!("{e}");
            }
        }
        A::ToggleMute => {
            if let Err(e) = ctx.audio.toggle_mute() {
                log::warn!("{e}");
            }
        }
        A::Reset50 => {
            if let Err(e) = ctx.audio.set_volume(0.5) {
                log::warn!("{e}");
            }
        }
        A::OpenMixer => {
            log::info!("OpenMixer — not wired yet");
            return;
        }
    }
    // Re-read and store the new state.
    if let Ok(st) = ctx.audio.get_state() {
        ctx.last_state = st;
    }
}