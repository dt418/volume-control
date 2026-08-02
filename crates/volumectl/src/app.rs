//! Windows application shell — the single message loop that ties together
//! audio, hotkeys, the mouse-wheel hook, tray, overlay, mixer and help.
//!
//! A hidden host window receives everything as messages:
//!   - `WM_HOTKEY`        → custom keyboard combo fired
//!   - `WM_APP_WHEEL`     → `Mod+Scroll` fired (from the low-level mouse hook)
//!   - `WM_APP_MIXER_*`   → mixer slider / button interaction
//!   - `WM_TIMER` (150ms) → config live reload, tray menu events, external
//!     volume sync, mixer UI sync
//!
//! Custom hotkeys are gated by the app blacklist (foreground process name)
//! with a configurable blocked beep; pressing volume at 0% / 100% beeps the
//! limit tone. A single-instance mutex prevents duplicates.

use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM, LRESULT, WPARAM},
    System::Diagnostics::Debug::Beep,
    System::LibraryLoader::GetModuleHandleW,
    System::Threading::{
        CreateMutexW, OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    },
    UI::Input::KeyboardAndMouse::{keybd_event, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, VK_MENU},
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetForegroundWindow, GetMessageW,
        GetWindowLongPtrW, GetWindowThreadProcessId, PostQuitMessage, RegisterClassW, SetTimer,
        SetWindowLongPtrW, TranslateMessage, CS_OWNDC, CW_USEDEFAULT, GWLP_USERDATA, MSG,
        WM_DESTROY, WM_HOTKEY, WM_TIMER, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
    },
};

use crate::audio::{AudioBackend, VolumeState};
use crate::audio_windows::WindowsAudio;
use crate::config::Config;
use crate::help::Help;
use crate::hotkeys::HotkeyAction;
use crate::hotkeys_win32::{
    hotkey_action, install_wheel_hook, uninstall_wheel_hook, Win32Hotkeys, WM_APP_WHEEL,
};
use crate::mixer::{Mixer, WM_APP_MIXER_CHANGE, WM_APP_MIXER_MUTE, WM_APP_MIXER_RESET};
use crate::overlay::Overlay;
use crate::tray::{Tray, TrayCommand};

const ID_TIMER_POLL: usize = 1;
const POLL_MS: u32 = 150;

/// Last-modified time of the config file (None if it doesn't exist yet).
fn config_mtime() -> Option<std::time::SystemTime> {
    std::fs::metadata(crate::config::config_path())
        .and_then(|m| m.modified())
        .ok()
}

/// Reload the config when the file changed on disk. Re-registers hotkeys when
/// the modifier combo changed. Returns true if a reload happened.
fn reload_config_if_changed(ctx: &mut AppContext) -> bool {
    let mtime = config_mtime();
    if mtime == ctx.last_config_mtime {
        return false;
    }
    ctx.last_config_mtime = mtime;

    let new_cfg = crate::config::load();
    let modifier_changed = new_cfg.modifier != ctx.config.modifier;
    log::info!(
        "config reloaded (step={}, step_large={}, overlay_ms={}, modifier={:?})",
        new_cfg.volume_step,
        new_cfg.volume_step_large,
        new_cfg.overlay_duration_ms,
        new_cfg.modifier
    );
    ctx.config = new_cfg;

    if modifier_changed {
        if let Err(e) = ctx.hotkeys.register(ctx.config.modifier) {
            log::warn!("hotkey re-register after config change failed: {e}");
        }
    }
    true
}

/// Heap-allocated state that lives in the window's GWLP_USERDATA.
struct AppContext {
    audio: WindowsAudio,
    hotkeys: Win32Hotkeys,
    config: Config,
    last_state: VolumeState,
    last_config_mtime: Option<std::time::SystemTime>,
    overlay: Overlay,
    mixer: Mixer,
    help: Help,
    tray: Tray,
}

/// Prevent two instances (VolumePro's `#SingleInstance Force` equivalent).
fn ensure_single_instance() -> bool {
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

/// Lowercase base name (e.g. `code.exe`) of the foreground window's process.
fn foreground_process() -> Option<String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd == 0 {
            return None;
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return None;
        }
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle == 0 {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(handle);
        if ok == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        let base = path.rsplit('\\').next().unwrap_or(&path);
        Some(base.to_lowercase())
    }
}

fn beep_blocked(cfg: &Config) {
    if cfg.beep.enabled {
        unsafe {
            Beep(cfg.beep.blocked_freq, cfg.beep.blocked_duration_ms);
        }
    }
}

fn beep_limit(cfg: &Config) {
    if cfg.beep.enabled {
        unsafe {
            Beep(cfg.beep.limit_freq, cfg.beep.limit_duration_ms);
        }
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    if !ensure_single_instance() {
        return Ok(()); // another instance handles it
    }

    let config = crate::config::load();
    let last_config_mtime = config_mtime();
    let audio = WindowsAudio::new()?;
    let overlay = Overlay::new()?;
    let tray = Tray::new()?;
    let last_state = audio.get_state().unwrap_or(VolumeState {
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
            0, // hwndParent
            0, // hmenu
            hinst,
            std::ptr::null(),
        )
    };
    if hwnd == 0 {
        return Err("CreateWindowEx host failed".into());
    }
    log::debug!("host hwnd=0x{:x}", hwnd);

    // Hotkeys + mouse-wheel hook, both targeted at the host window.
    let hotkeys = Win32Hotkeys::new(hwnd, config.modifier)?;
    install_wheel_hook(hwnd)?;

    let mixer = Mixer::new(hwnd)?;
    let help = Help::new()?;

    // Store context in GWLP_USERDATA.
    let ctx = Box::into_raw(Box::new(AppContext {
        audio,
        hotkeys,
        config,
        last_state,
        last_config_mtime,
        overlay,
        mixer,
        help,
        tray,
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

    // Message pump.
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    uninstall_wheel_hook();

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
    let ctx = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut AppContext;
    match msg {
        // ── Custom hotkey (keyboard) fired ───────────────────────────────
        WM_HOTKEY => {
            if let Some(action) = hotkey_action(wparam as i32) {
                if !ctx.is_null() {
                    apply(&mut *ctx, action);
                }
            }
            0
        }
        // ── Mod+Scroll fired (mouse hook posts the same action ids) ──────
        WM_APP_WHEEL => {
            if let Some(action) = hotkey_action(wparam as i32) {
                if !ctx.is_null() {
                    apply(&mut *ctx, action);
                }
            }
            0
        }
        // ── Mixer: slider moved ──────────────────────────────────────────
        WM_APP_MIXER_CHANGE => {
            if !ctx.is_null() {
                let ctx = &mut *ctx;
                let pct_raw = (wparam as u32).min(100);
                let pct = pct_raw as f32 / 100.0;
                log::debug!("mixer change: request={}%%", pct_raw);
                if let Err(e) = ctx.audio.set_volume(pct) {
                    log::warn!("{e}");
                }
                refresh_ui(ctx);
            }
            0
        }
        WM_APP_MIXER_MUTE => {
            if !ctx.is_null() {
                let ctx = &mut *ctx;
                if let Err(e) = ctx.audio.toggle_mute() {
                    log::warn!("{e}");
                }
                refresh_ui(ctx);
            }
            0
        }
        WM_APP_MIXER_RESET => {
            if !ctx.is_null() {
                let ctx = &mut *ctx;
                if let Err(e) = ctx.audio.set_volume(0.5) {
                    log::warn!("{e}");
                }
                refresh_ui(ctx);
            }
            0
        }
        // ── Periodic poll: config reload + tray + external sync ──────────
        WM_TIMER => {
            if !ctx.is_null() {
                let ctx = &mut *ctx;

                // Live config reload (mtime watch).
                let reloaded = reload_config_if_changed(ctx);

                // Tray menu commands.
                while let Some(cmd) = ctx.tray.poll() {
                    handle_tray_command(ctx, cmd);
                }

                // External volume changes (media keys, other apps).
                if let Ok(st) = ctx.audio.get_state() {
                    if reloaded {
                        ctx.overlay.show(&st, &ctx.config);
                    }
                    ctx.tray.set_volume(&st);
                    if ctx.mixer.is_open() {
                        ctx.mixer.sync(&st);
                    }
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

/// Apply a hotkey action (keyboard or wheel). Blacklist + limit beeps happen
/// here, matching VolumePro's behaviour.
fn apply(ctx: &mut AppContext, action: HotkeyAction) {
    use HotkeyAction as A;

    // Actions that don't change volume bypass the blacklist gate.
    match action {
        A::OpenMenu => {
            // Background processes cannot SetForegroundWindow directly
            // (Windows foreground lock); simulating Alt unlocks it.
            unsafe {
                keybd_event(VK_MENU as u8, 0, KEYEVENTF_EXTENDEDKEY, 0);
                keybd_event(VK_MENU as u8, 0, KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP, 0);
            }
            ctx.tray.show_menu();
            return;
        }
        A::OpenMixer => {
            if ctx.mixer.is_open() {
                ctx.mixer.toggle();
            } else {
                if let Ok(st) = ctx.audio.get_state() {
                    ctx.mixer.sync(&st);
                }
                ctx.mixer.toggle();
            }
            return;
        }
        _ => {}
    }

    // Blacklist gate: suppress hotkeys while a blacklisted app is focused.
    if let Some(proc) = foreground_process() {
        if crate::config::is_blacklisted(&ctx.config.blacklist, &proc) {
            log::debug!("hotkey blocked by blacklist ({proc})");
            beep_blocked(&ctx.config);
            return;
        }
    }

    let old = ctx.last_state;
    log::debug!("hotkey: {:?} (current {}%)", action, old.percent());

    // Volume actions: apply, then beep if we were already at the limit.
    let step = ctx.config.volume_step as f32;
    let step_large = ctx.config.volume_step_large as f32;
    let target = match action {
        A::VolumeUp => Some(crate::core::step_volume(old.volume, step)),
        A::VolumeDown => Some(crate::core::step_volume(old.volume, -step)),
        A::VolumeUpLarge => Some(crate::core::step_volume(old.volume, step_large)),
        A::VolumeDownLarge => Some(crate::core::step_volume(old.volume, -step_large)),
        A::Reset50 => Some(0.5),
        A::ToggleMute => None,
        _ => None,
    };

    match target {
        Some(v) => {
            if let Err(e) = ctx.audio.set_volume(v) {
                log::warn!("{e}");
            }
            // Limit beep: no change and already at a boundary.
            if v == old.volume && (old.volume == 0.0 || old.volume == 1.0) {
                beep_limit(&ctx.config);
            }
        }
        None => {
            if let Err(e) = ctx.audio.toggle_mute() {
                log::warn!("{e}");
            }
        }
    }

    refresh_ui(ctx);
}

/// Re-read the audio state and push it into overlay, tray and mixer.
fn refresh_ui(ctx: &mut AppContext) {
    if let Ok(st) = ctx.audio.get_state() {
        log::debug!(
            "refresh_ui: state={}%% muted={} mixer_open={}",
            st.percent(),
            st.muted,
            ctx.mixer.is_open()
        );
        ctx.last_state = st;
        ctx.overlay.show(&st, &ctx.config);
        ctx.tray.set_volume(&st);
        if ctx.mixer.is_open() {
            ctx.mixer.sync(&st);
        }
    }
}

/// Apply a command from the tray menu.
fn handle_tray_command(ctx: &mut AppContext, cmd: TrayCommand) {
    use TrayCommand as C;
    match cmd {
        C::ToggleMute => {
            if let Err(e) = ctx.audio.toggle_mute() {
                log::warn!("{e}");
            }
            refresh_ui(ctx);
        }
        C::Reset50 => {
            if let Err(e) = ctx.audio.set_volume(0.5) {
                log::warn!("{e}");
            }
            refresh_ui(ctx);
        }
        C::OpenMixer => {
            ctx.mixer.toggle();
            if let Ok(st) = ctx.audio.get_state() {
                ctx.mixer.sync(&st);
            }
        }
        C::Help => {
            ctx.help.show(&ctx.config);
        }
        C::EditConfig => {
            crate::config::open_in_editor();
            ctx.overlay
                .show_text("Editing config — changes reload automatically", &ctx.config);
        }
        C::ReloadConfig => {
            ctx.last_config_mtime = None; // force
            reload_config_if_changed(ctx);
            ctx.overlay.show_text("Config reloaded", &ctx.config);
        }
        C::ApplyBlacklist => {
            let recommended = crate::config::recommended_blacklist(ctx.config.modifier);
            let added = crate::config::apply_recommended_blacklist(&mut ctx.config);
            ctx.last_config_mtime = None; // force reload from disk
            reload_config_if_changed(ctx);
            let msg = if recommended.is_empty() {
                format!("No blacklist needed for {:?}", ctx.config.modifier)
            } else if added > 0 {
                format!("Blacklist: +{added} app(s) applied")
            } else {
                "Blacklist already up to date".to_string()
            };
            log::info!("{msg}");
            ctx.overlay.show_text(&msg, &ctx.config);
        }
        C::Exit => unsafe {
            PostQuitMessage(0);
        },
    }
}
