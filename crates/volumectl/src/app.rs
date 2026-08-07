//! Windows application shell — the single message loop that ties together
//! audio, hotkeys, the mouse-wheel hook, tray, overlay, mixer and help.
//!
//! A hidden host window receives everything as messages:
//!   - `WM_APP_WHEEL`     → `Mod+Scroll` fired (from the low-level mouse hook)
//!   - `WM_APP_MIXER_*`   → mixer slider / button interaction
//!   - `WM_TIMER` (150ms) → config live reload, tray menu events, external
//!     volume sync, mixer UI sync
//!
//! Custom hotkeys are gated by the app blacklist (foreground process name)
//! with a configurable blocked beep; pressing volume at 0% / 100% beeps the
//! limit tone. A single-instance mutex prevents duplicates.

#[cfg(target_os = "windows")]
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
        WM_DESTROY, WM_TIMER, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
    },
};

use crate::audio::{AudioBackend, VolumeState};
use crate::audio_windows::WindowsAudio;
use crate::config::Config;
use crate::help::{Help, HelpAppearance, WM_APP_HELP_OPEN_CONFIG, WM_APP_HELP_SETTINGS};
use crate::hotkeys::{hotkey_from_id, HotkeyAction, HotkeyRegResult};
use crate::hotkeys_rdev::RdevHotkeys;
use crate::mixer::{
    Mixer, MixerAppearance, WM_APP_MIXER_CHANGE, WM_APP_MIXER_MUTE, WM_APP_MIXER_RESET,
};
use crate::overlay::Overlay;
use crate::settings::{
    Settings, SettingsAppearance, WM_APP_SETTINGS_APPLY, WM_APP_SETTINGS_CANCEL,
    WM_APP_SETTINGS_OPEN_CONFIG, WM_APP_SETTINGS_RESET,
};
use crate::tray::{Tray, TrayCommand};
use crate::ui::{AppAction, SurfaceId, SurfaceVisibility};
use crate::wheel_win32::{
    install_wheel_hook, set_modifier as set_wheel_modifier, uninstall_wheel_hook, WM_APP_WHEEL,
};

const ID_TIMER_POLL: usize = 1;
const ID_TIMER_HOTKEY: usize = 2;
const POLL_MS: u32 = 150;
const HOTKEY_POLL_MS: u32 = 15;

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
    // An open settings window adopts the new baseline (preserving any
    // in-progress edits) so it never shows a stale config.
    if ctx.settings.is_open() {
        ctx.settings.reload(&new_cfg);
    }
    ctx.config = new_cfg;

    if modifier_changed {
        ctx.hotkeys.set_modifier(ctx.config.modifier);
        set_wheel_modifier(ctx.config.modifier);
        // Keep the per-action registration status fresh for the UI.
        ctx.hotkey_status = ctx.hotkeys.status().to_vec();
    }
    true
}

/// Heap-allocated state that lives in the window's GWLP_USERDATA.
struct AppContext {
    audio: WindowsAudio,
    hotkeys: RdevHotkeys,
    config: Config,
    last_state: VolumeState,
    last_config_mtime: Option<std::time::SystemTime>,
    overlay: Overlay,
    mixer: Mixer,
    settings: Settings,
    help: Help,
    tray: Tray,
    /// Per-action global-listener status exposed to the Help surface.
    hotkey_status: Vec<HotkeyRegResult>,
    /// Shared confirmed UI state published to every renderer by
    /// [`publish_confirmed_state`].
    ui_state: crate::ui::AppState,
    /// Display-session capabilities measured once at startup. Consumed by the
    /// adaptive renderers (Tasks 7/8).
    caps: crate::ui::UiCapabilities,
}

/// Prevent two instances (VolumePro's `#SingleInstance Force` equivalent).
#[cfg(target_os = "windows")]
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

/// Stub for non-Windows platforms (no single-instance enforcement).
#[cfg(not(target_os = "windows"))]
fn ensure_single_instance() -> bool {
    true
}

/// Lowercase base name (e.g. `code.exe`) of the foreground window's process.
#[cfg(target_os = "windows")]
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

/// Lowercase base name of the foreground window's process on macOS.
#[cfg(target_os = "macos")]
fn foreground_process() -> Option<String> {
    use std::process::Command;
    // Use AppleScript to get the frontmost application
    let output = Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to get name of first application process whose frontmost is true"
        ])
        .output()
        .ok()?;

    if output.status.success() {
        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if name.is_empty() {
            None
        } else {
            // Normalize: lowercase and ensure .app suffix for consistency with blacklist
            Some(crate::config::normalize_blacklist_entry(
                &name.to_lowercase(),
            ))
        }
    } else {
        None
    }
}

/// Lowercase base name of the foreground window's process on Linux.
#[cfg(target_os = "linux")]
fn foreground_process() -> Option<String> {
    use std::process::Command;

    // Method 1: Try xdotool first (most reliable if available)
    if let Ok(output) = Command::new("xdotool")
        .args(["getactivewindow", "--pid"])
        .output()
    {
        if output.status.success() {
            let pid_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if let Ok(pid) = pid_str.parse::<u32>() {
                if let Ok(comm) = std::fs::read_to_string(format!("/proc/{}/comm", pid)) {
                    let name = comm.trim().to_lowercase();
                    log::debug!("foreground_process: xdotool found PID {} -> {}", pid, name);
                    return Some(crate::config::normalize_blacklist_entry(&name));
                }
            }
        }
    }

    // Method 2: Fallback to xprop + wmctrl
    let output = Command::new("xprop")
        .args(["-root", "_NET_ACTIVE_WINDOW"])
        .output()
        .ok()?;

    if !output.status.success() {
        log::debug!("foreground_process: xprop failed");
        return None;
    }

    let window_id = String::from_utf8_lossy(&output.stdout);
    let window_id = window_id.split('#').nth(1)?.trim();

    let wmctrl_output = Command::new("wmctrl").arg("-lp").output().ok()?;

    if wmctrl_output.status.success() {
        let lines = String::from_utf8_lossy(&wmctrl_output.stdout);
        for line in lines.lines() {
            if line.contains(window_id) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    if let Ok(pid) = parts[1].parse::<u32>() {
                        if let Ok(comm) = std::fs::read_to_string(format!("/proc/{}/comm", pid)) {
                            let name = comm.trim().to_lowercase();
                            log::debug!("foreground_process: wmctrl found PID {} -> {}", pid, name);
                            return Some(crate::config::normalize_blacklist_entry(&name));
                        }
                    }
                }
            }
        }
    }

    // Method 3: Direct X11 query via x11rb (no CLI dependencies needed)
    // This replaces the broken /proc enumeration which returned arbitrary processes
    if let Some(pid) = get_window_pid_x11() {
        if let Ok(comm) = std::fs::read_to_string(format!("/proc/{}/comm", pid)) {
            let name = comm.trim().to_lowercase();
            log::debug!("foreground_process: x11rb found PID {} -> {}", pid, name);
            return Some(crate::config::normalize_blacklist_entry(&name));
        }
    }

    // All methods failed - return None (better than wrong answer)
    log::warn!("foreground_process: could not determine foreground process on Linux");
    None
}

/// Get the PID of the active window using X11 directly via x11rb.
/// This is a pure-Rust implementation that doesn't require external CLI tools.
#[cfg(target_os = "linux")]
fn get_window_pid_x11() -> Option<u32> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};

    // Connect to X11 display
    let (conn, screen_num) = x11rb::connect(None).ok()?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;

    // Get _NET_ACTIVE_WINDOW property from root window
    let active_win_prop = conn
        .get_property(
            false,
            root,
            AtomEnum::_NET_ACTIVE_WINDOW,
            AtomEnum::WINDOW,
            0,
            1,
        )
        .ok()?
        .reply()
        .ok()?;

    if active_win_prop.value.len() < 4 {
        return None;
    }

    let active_window = u32::from_ne_bytes([
        active_win_prop.value[0],
        active_win_prop.value[1],
        active_win_prop.value[2],
        active_win_prop.value[3],
    ]);

    if active_window == 0 {
        return None;
    }

    // Get _NET_WM_PID property from the active window
    let pid_prop = conn
        .get_property(
            false,
            active_window,
            AtomEnum::_NET_WM_PID,
            AtomEnum::CARDINAL,
            0,
            1,
        )
        .ok()?
        .reply()
        .ok()?;

    if pid_prop.value.len() < 4 {
        return None;
    }

    let pid = u32::from_ne_bytes([
        pid_prop.value[0],
        pid_prop.value[1],
        pid_prop.value[2],
        pid_prop.value[3],
    ]);

    Some(pid)
}

#[cfg(target_os = "windows")]
fn beep_blocked(cfg: &Config) {
    if cfg.beep.enabled {
        unsafe {
            Beep(cfg.beep.blocked_freq, cfg.beep.blocked_duration_ms);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn beep_blocked(_cfg: &Config) {
    // No-op on non-Windows (no Win32 Beep API)
}

#[cfg(target_os = "windows")]
fn beep_limit(cfg: &Config) {
    if cfg.beep.enabled {
        unsafe {
            Beep(cfg.beep.limit_freq, cfg.beep.limit_duration_ms);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn beep_limit(_cfg: &Config) {
    // No-op on non-Windows (no Win32 Beep API)
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    if !ensure_single_instance() {
        return Ok(()); // another instance handles it
    }

    let config = crate::config::load();
    if config.autostart && !crate::autostart::is_enabled() {
        log::warn!(
            "config requests autostart but the OS registration is missing — \
             re-enable it in Settings or the app will not launch at login"
        );
    }
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

    // Cross-platform rdev keyboard listener + the Windows-only wheel bridge.
    // The listener emits into a channel; audio mutations remain on this host
    // message-loop thread.
    let hotkeys = RdevHotkeys::new(config.modifier)?;
    set_wheel_modifier(config.modifier);
    // The global listener owns every action, so the Help card can show the
    // same active status for all configured shortcuts.
    let hotkey_status = hotkeys.status().to_vec();
    install_wheel_hook(hwnd)?;

    let mixer = Mixer::new(hwnd)?;
    let settings = Settings::new(hwnd)?;
    let help = Help::new(hwnd)?;

    // Shared confirmed UI state starts from the audio snapshot and the
    // appearance preferences loaded from config; `publish_confirmed_state`
    // keeps it fresh after that.
    let mut ui_state =
        crate::ui::AppState::from_audio(last_state.percent(), last_state.muted, None);
    ui_state.theme = config.appearance.theme;
    ui_state.material = config.appearance.material;
    ui_state.motion = config.appearance.motion;
    let caps = crate::ui::primitives::detect_capabilities(hwnd);

    // Store context in GWLP_USERDATA.
    let ctx = Box::into_raw(Box::new(AppContext {
        audio,
        hotkeys,
        config,
        last_state,
        last_config_mtime,
        overlay,
        mixer,
        settings,
        help,
        tray,
        hotkey_status,
        ui_state,
        caps,
    }));
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, ctx as isize);
    }

    // Poll timer (150 ms).
    unsafe {
        let timer_ok = SetTimer(hwnd, ID_TIMER_POLL, POLL_MS, None);
        log::debug!("SetTimer -> {}", timer_ok);
        let hotkey_timer_ok = SetTimer(hwnd, ID_TIMER_HOTKEY, HOTKEY_POLL_MS, None);
        log::debug!("SetTimer(hotkey) -> {}", hotkey_timer_ok);
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
        // ── Mod+Scroll fired (mouse hook posts the same action ids) ──────
        WM_APP_WHEEL => {
            if let Some(action) = hotkey_from_id(wparam as i32) {
                if !ctx.is_null() {
                    apply_hotkey(&mut *ctx, action);
                }
            }
            0
        }
        // ── Mixer: slider moved → shared action, host owns the mutation ──
        WM_APP_MIXER_CHANGE => {
            if !ctx.is_null() {
                let pct = (wparam as u32).min(100) as u16;
                log::debug!("mixer change: request={}%%", pct);
                handle_action(&mut *ctx, AppAction::SetVolumePercent { percent: pct });
            }
            0
        }
        WM_APP_MIXER_MUTE => {
            if !ctx.is_null() {
                handle_action(&mut *ctx, AppAction::ToggleMute);
            }
            0
        }
        WM_APP_MIXER_RESET => {
            if !ctx.is_null() {
                handle_action(&mut *ctx, AppAction::ResetVolume);
            }
            0
        }
        // ── Settings: the window posts draft intents; the host owns every
        //    config/hotkey mutation and reports the result back. ──────────
        WM_APP_SETTINGS_APPLY => {
            if !ctx.is_null() {
                handle_action(&mut *ctx, AppAction::ApplyConfig);
            }
            0
        }
        WM_APP_SETTINGS_CANCEL => {
            if !ctx.is_null() {
                handle_action(&mut *ctx, AppAction::CancelConfig);
            }
            0
        }
        WM_APP_SETTINGS_RESET => {
            if !ctx.is_null() {
                handle_action(&mut *ctx, AppAction::ResetConfig);
            }
            0
        }
        WM_APP_SETTINGS_OPEN_CONFIG => {
            if !ctx.is_null() {
                handle_action(&mut *ctx, AppAction::OpenConfigLocation);
            }
            0
        }
        // ── Help: the card posts its affordances; the host owns the action. ─
        WM_APP_HELP_OPEN_CONFIG => {
            if !ctx.is_null() {
                handle_action(&mut *ctx, AppAction::OpenConfigLocation);
            }
            0
        }
        WM_APP_HELP_SETTINGS => {
            if !ctx.is_null() {
                handle_action(&mut *ctx, AppAction::ShowSurface(SurfaceId::Settings));
            }
            0
        }
        // ── Periodic poll: config reload + tray + external sync ──────────
        WM_TIMER => {
            if !ctx.is_null() {
                let ctx = &mut *ctx;

                if wparam == ID_TIMER_HOTKEY {
                    drain_hotkeys(ctx);
                    return 0;
                }

                // Live config reload (mtime watch).
                let reloaded = reload_config_if_changed(ctx);

                // Tray menu commands.
                while let Some(cmd) = ctx.tray.poll() {
                    handle_tray_command(ctx, cmd);
                }

                // External volume changes (media keys, other apps). Only re-show
                // the overlay when the config just reloaded, so the native
                // media-key flyout stays authoritative.
                let before = ctx.last_state;
                publish_confirmed_state(ctx, reloaded);
                if ctx.last_state != before {
                    log::debug!(
                        "ext change: {}% muted={}",
                        ctx.last_state.percent(),
                        ctx.last_state.muted
                    );
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

/// Drain rdev actions on a fast timer independent from the slower config and
/// audio synchronization poll. This keeps the first key press responsive.
fn drain_hotkeys(ctx: &mut AppContext) {
    while let Some(action) = ctx.hotkeys.try_recv() {
        apply_hotkey(ctx, action);
    }
}

/// Apply a hotkey action (keyboard or wheel). The blacklist gate and the limit
/// beep are hotkey-origin concerns and live here, matching VolumePro's
/// behaviour; tray-origin commands bypass the gate.
fn apply_hotkey(ctx: &mut AppContext, action: HotkeyAction) {
    use HotkeyAction as H;

    // Actions that don't change volume bypass the blacklist gate.
    if matches!(action, H::OpenMenu | H::OpenMixer) {
        let step = ctx.config.volume_step as i16;
        let step_large = ctx.config.volume_step_large as i16;
        handle_action(ctx, hotkey_to_action(action, step, step_large));
        return;
    }

    // Blacklist gate: suppress hotkeys while a blacklisted app is focused.
    if let Some(proc) = foreground_process() {
        if crate::config::is_blacklisted(&ctx.config.blacklist, &proc) {
            log::debug!("hotkey blocked by blacklist ({proc})");
            beep_blocked(&ctx.config);
            return;
        }
    }

    log::debug!("hotkey: {action:?} (current {}%)", ctx.last_state.percent());
    let step = ctx.config.volume_step as i16;
    let step_large = ctx.config.volume_step_large as i16;
    handle_action(ctx, hotkey_to_action(action, step, step_large));
}

/// Map a hotkey action to the shared action contract, resolving the configured
/// step sizes. Deliberately pure and testable: the blacklist gate is NOT part
/// of this mapping — it is a host concern applied only to hotkey/wheel origin.
fn hotkey_to_action(action: HotkeyAction, step: i16, step_large: i16) -> AppAction {
    use HotkeyAction as H;
    match action {
        H::VolumeUp => AppAction::AdjustVolume {
            delta_percent: step,
        },
        H::VolumeDown => AppAction::AdjustVolume {
            delta_percent: -step,
        },
        H::VolumeUpLarge => AppAction::AdjustVolume {
            delta_percent: step_large,
        },
        H::VolumeDownLarge => AppAction::AdjustVolume {
            delta_percent: -step_large,
        },
        H::ToggleMute => AppAction::ToggleMute,
        H::Reset50 => AppAction::ResetVolume,
        H::OpenMixer => AppAction::ToggleSurface(SurfaceId::Mixer),
        H::OpenMenu => AppAction::OpenTrayMenu,
    }
}

/// Resolve the overlay's adaptive appearance from the confirmed appearance
/// preferences and the startup display-session capability snapshot. One
/// resolution point for the overlay: it stays a dumb consumer of the resolved
/// tokens + material.
fn overlay_appearance(ctx: &AppContext) -> crate::overlay::OverlayAppearance {
    crate::overlay::OverlayAppearance::resolve(
        &ctx.config,
        &ctx.caps,
        crate::ui::primitives::system_theme,
    )
}

/// Resolve the mixer's adaptive appearance the same way — one resolution point
/// in the host, consumed blindly by the mixer (Task 8, mirrors the overlay).
fn mixer_appearance(ctx: &AppContext) -> MixerAppearance {
    MixerAppearance::resolve(&ctx.config, &ctx.caps, crate::ui::primitives::system_theme)
}

/// Resolve the settings window's adaptive appearance — one resolution point in
/// the host, consumed blindly by the settings window (Task 10).
fn settings_appearance(ctx: &AppContext) -> SettingsAppearance {
    SettingsAppearance::resolve(&ctx.config, &ctx.caps, crate::ui::primitives::system_theme)
}

/// Resolve the Help card's adaptive appearance — one resolution point in the
/// host, consumed blindly by the card (Task 11, mirrors the other surfaces).
fn help_appearance(ctx: &AppContext) -> HelpAppearance {
    HelpAppearance::resolve(&ctx.config, &ctx.caps, crate::ui::primitives::system_theme)
}

/// Re-read the audio state, update the audio-truth cache and the shared
/// confirmed [`crate::ui::AppState`], then push the snapshot into every
/// renderer (overlay, tray, mixer). `show_overlay` controls whether the
/// transient volume overlay is re-shown — the external-sync path only shows it
/// when the config just reloaded, so the native media-key flyout stays
/// authoritative.
fn publish_confirmed_state(ctx: &mut AppContext, show_overlay: bool) {
    let Ok(st) = ctx.audio.get_state() else {
        return;
    };
    ctx.last_state = st;

    ctx.ui_state.volume_percent = st.percent();
    ctx.ui_state.muted = st.muted;
    ctx.ui_state.theme = ctx.config.appearance.theme;
    ctx.ui_state.material = ctx.config.appearance.material;
    ctx.ui_state.motion = ctx.config.appearance.motion;
    ctx.ui_state.surfaces.mixer = if ctx.mixer.is_open() {
        SurfaceVisibility::Visible
    } else {
        SurfaceVisibility::Hidden
    };
    ctx.ui_state.surfaces.settings = if ctx.settings.is_open() {
        SurfaceVisibility::Visible
    } else {
        SurfaceVisibility::Hidden
    };

    log::debug!(
        "publish: state={}%% muted={} mixer_open={} settings_open={} show_overlay={}",
        st.percent(),
        st.muted,
        ctx.mixer.is_open(),
        ctx.settings.is_open(),
        show_overlay
    );

    if show_overlay {
        ctx.ui_state.surfaces.overlay = SurfaceVisibility::Visible;
        ctx.overlay
            .show(&st, &ctx.config, &overlay_appearance(&*ctx));
    }
    ctx.tray.set_volume(&st);
    if ctx.mixer.is_open() {
        // The rail fill honours the user's colour-band boundaries; they
        // travel on their own seam so the mixer's sync/toggle signatures
        // stay unchanged (the overlay receives them per-show via config).
        ctx.mixer
            .set_thresholds(ctx.config.color_thresholds.clone());
        ctx.mixer.sync(&st, &mixer_appearance(&*ctx));
    }
    if ctx.settings.is_open() {
        ctx.settings.set_appearance(&settings_appearance(ctx));
    }
}

/// Apply a command from the tray menu. Tray-origin commands bypass the
/// blacklist gate by design (the gate only suppresses hotkey/wheel input).
fn handle_tray_command(ctx: &mut AppContext, cmd: TrayCommand) {
    handle_action(ctx, tray_command_to_action(cmd));
}

/// Map a tray menu command to the shared action contract. Pure and testable;
/// like the hotkey mapping it carries no blacklist gate.
fn tray_command_to_action(cmd: TrayCommand) -> AppAction {
    use TrayCommand as C;
    match cmd {
        C::ToggleMute => AppAction::ToggleMute,
        C::Reset50 => AppAction::ResetVolume,
        C::OpenMixer => AppAction::ToggleSurface(SurfaceId::Mixer),
        C::Help => AppAction::ShowSurface(SurfaceId::Help),
        C::Settings => AppAction::ToggleSurface(SurfaceId::Settings),
        C::EditConfig => AppAction::OpenConfigLocation,
        C::ReloadConfig => AppAction::ReloadConfig,
        C::Exit => AppAction::Exit,
    }
}

/// Adopt a saved config as the running config, resync the mtime watch (so the
/// 150ms poll does not immediately reload our own write), re-register hotkeys
/// only when the modifier changed, and refresh every renderer.
fn adopt_saved_config(ctx: &mut AppContext, saved: Config) {
    let modifier_changed = saved.modifier != ctx.config.modifier;
    let autostart_changed = saved.autostart != ctx.config.autostart;
    ctx.config = saved;
    ctx.last_config_mtime = config_mtime();
    if modifier_changed {
        log::info!("config: modifier changed — updating global listener");
        ctx.hotkeys.set_modifier(ctx.config.modifier);
        set_wheel_modifier(ctx.config.modifier);
        // Keep the per-action registration status fresh for the UI.
        ctx.hotkey_status = ctx.hotkeys.status().to_vec();
    }
    if autostart_changed {
        match crate::autostart::set_enabled(ctx.config.autostart) {
            Ok(()) => log::info!(
                "autostart: {} (system registration updated)",
                if ctx.config.autostart {
                    "enabled"
                } else {
                    "disabled"
                }
            ),
            Err(e) => log::warn!("autostart update failed: {e}"),
        }
    }
    publish_confirmed_state(ctx, false);
}

/// Central handler for every [`AppAction`] emitted by any surface. Owns all
/// audio/config/hotkey mutation; renderers only ever emit actions and never
/// mutate anything themselves.
fn handle_action(ctx: &mut AppContext, action: AppAction) {
    use AppAction as A;
    use SurfaceId as S;

    match action {
        A::SetVolumePercent { percent } => {
            let pct = (percent.min(100) as f32) / 100.0;
            if let Err(e) = ctx.audio.set_volume(pct) {
                log::warn!("{e}");
            }
            publish_confirmed_state(ctx, true);
        }
        A::AdjustVolume { delta_percent } => {
            // Step actions use the cached audio-truth as the base so the limit
            // beep compares against the same reference as before the mutation.
            let old = ctx.last_state;
            let target = crate::core::step_volume(old.volume, delta_percent as f32);
            log::debug!(
                "action: adjust {delta_percent}% ({}% -> {:.0}%)",
                old.percent(),
                target * 100.0
            );
            if let Err(e) = ctx.audio.set_volume(target) {
                log::warn!("{e}");
            }
            // Limit beep: no change and already at a boundary.
            if target == old.volume && (old.volume == 0.0 || old.volume == 1.0) {
                beep_limit(&ctx.config);
            }
            publish_confirmed_state(ctx, true);
        }
        A::ToggleMute => {
            if let Err(e) = ctx.audio.toggle_mute() {
                log::warn!("{e}");
            }
            publish_confirmed_state(ctx, true);
        }
        A::SetMute { muted } => {
            if let Err(e) = ctx.audio.set_mute(muted) {
                log::warn!("{e}");
            }
            publish_confirmed_state(ctx, true);
        }
        A::ResetVolume => {
            let old = ctx.last_state;
            if let Err(e) = ctx.audio.set_volume(0.5) {
                log::warn!("{e}");
            }
            // VolumePro parity: a reset cannot land on a boundary, so the limit
            // beep never fires here; the check is kept for symmetry with the
            // step path.
            if 0.5 == old.volume && (old.volume == 0.0 || old.volume == 1.0) {
                beep_limit(&ctx.config);
            }
            publish_confirmed_state(ctx, true);
        }
        A::ShowSurface(S::Mixer) => {
            if !ctx.mixer.is_open() {
                if let Ok(st) = ctx.audio.get_state() {
                    ctx.mixer.sync(&st, &mixer_appearance(ctx));
                }
                ctx.mixer.toggle(&mixer_appearance(ctx));
            }
            publish_confirmed_state(ctx, false);
        }
        A::HideSurface(S::Mixer) => {
            if ctx.mixer.is_open() {
                ctx.mixer.toggle(&mixer_appearance(ctx));
            }
            publish_confirmed_state(ctx, false);
        }
        A::ToggleSurface(S::Mixer) => {
            // Sync before showing so the mixer reflects current audio state
            // when it appears (mirrors the original hotkey/tray behavior).
            if !ctx.mixer.is_open() {
                if let Ok(st) = ctx.audio.get_state() {
                    ctx.mixer.sync(&st, &mixer_appearance(ctx));
                }
            }
            ctx.mixer.toggle(&mixer_appearance(ctx));
            publish_confirmed_state(ctx, false);
        }
        A::ShowSurface(S::Help) => {
            ctx.help
                .show(&ctx.config, &ctx.hotkey_status, &help_appearance(ctx));
            ctx.ui_state.surfaces.help = SurfaceVisibility::Visible;
        }
        A::HideSurface(S::Help) => {
            ctx.help.hide();
            ctx.ui_state.surfaces.help = SurfaceVisibility::Hidden;
        }
        A::ToggleSurface(S::Help) => {
            if ctx.ui_state.surfaces.help.is_visible() {
                ctx.help.hide();
                ctx.ui_state.surfaces.help = SurfaceVisibility::Hidden;
            } else {
                ctx.help
                    .show(&ctx.config, &ctx.hotkey_status, &help_appearance(ctx));
                ctx.ui_state.surfaces.help = SurfaceVisibility::Visible;
            }
        }
        A::OpenTrayMenu | A::ToggleSurface(S::Tray) => {
            // Background processes cannot SetForegroundWindow directly
            // (Windows foreground lock); simulating Alt unlocks it.
            unsafe {
                keybd_event(VK_MENU as u8, 0, KEYEVENTF_EXTENDEDKEY, 0);
                keybd_event(VK_MENU as u8, 0, KEYEVENTF_EXTENDEDKEY | KEYEVENTF_KEYUP, 0);
            }
            ctx.tray.show_menu();
            ctx.ui_state.surfaces.tray = SurfaceVisibility::Visible;
        }
        A::OpenConfigLocation => {
            crate::config::open_in_editor();
            ctx.overlay.show_text(
                "Editing config — changes reload automatically",
                &ctx.config,
                &overlay_appearance(&*ctx),
            );
        }
        A::ReloadConfig => {
            ctx.last_config_mtime = None; // force
            reload_config_if_changed(ctx);
            ctx.overlay
                .show_text("Config reloaded", &ctx.config, &overlay_appearance(&*ctx));
        }
        A::ApplyRecommendedBlacklist => {
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
            ctx.overlay
                .show_text(&msg, &ctx.config, &overlay_appearance(&*ctx));
        }
        A::Exit => unsafe {
            PostQuitMessage(0);
        },
        // ── Settings (Task 10): the host owns every config/hotkey mutation.
        //    The window posts draft intents; the host drives the commit,
        //    adopts the saved config, re-registers hotkeys only after a
        //    successful modifier change, and reports the result back. ─────
        A::ApplyConfig => {
            let result = ctx.settings.apply();
            match result {
                Ok(saved) => {
                    ctx.settings.on_apply_result(&Ok(saved.clone()));
                    adopt_saved_config(ctx, saved);
                }
                Err(e) => {
                    log::warn!("settings apply failed: {e}");
                    ctx.settings.on_apply_result(&Err(e));
                }
            }
        }
        A::CancelConfig => {
            ctx.settings.cancel();
            ctx.ui_state.surfaces.settings = SurfaceVisibility::Hidden;
        }
        A::ResetConfig => {
            ctx.settings.reset();
        }
        // Direct appearance intents for external callers (e.g. a future tray
        // preview). The settings window drives appearance through ApplyConfig,
        // which persists every appearance field together.
        A::SetTheme(theme) => {
            ctx.config.appearance.theme = theme;
            match crate::config::save_validated(&ctx.config) {
                Ok(saved) => adopt_saved_config(ctx, saved),
                Err(e) => log::warn!("theme persist failed: {e}"),
            }
        }
        A::SetMaterial(material) => {
            ctx.config.appearance.material = material;
            match crate::config::save_validated(&ctx.config) {
                Ok(saved) => adopt_saved_config(ctx, saved),
                Err(e) => log::warn!("material persist failed: {e}"),
            }
        }
        A::SetMotion(motion) => {
            ctx.config.appearance.motion = motion;
            match crate::config::save_validated(&ctx.config) {
                Ok(saved) => adopt_saved_config(ctx, saved),
                Err(e) => log::warn!("motion persist failed: {e}"),
            }
        }
        // Blacklist edits from external callers update the settings window's
        // in-memory draft; nothing touches disk until the user applies.
        A::AddBlacklistEntry(name) => ctx.settings.add_blacklist_entry(&name),
        A::RemoveBlacklistEntry(name) => ctx.settings.remove_blacklist_entry(&name),
        A::ClearBlacklist => ctx.settings.clear_blacklist(),
        // ── Settings surface show/hide/toggle ────────────────────────────
        A::ShowSurface(S::Settings) => {
            if !ctx.settings.is_open() {
                ctx.settings.show(&ctx.config, &settings_appearance(ctx));
                ctx.ui_state.surfaces.settings = SurfaceVisibility::Visible;
            }
        }
        A::HideSurface(S::Settings) => {
            if ctx.settings.is_open() {
                ctx.settings.hide();
            }
            ctx.ui_state.surfaces.settings = SurfaceVisibility::Hidden;
        }
        A::ToggleSurface(S::Settings) => {
            if ctx.settings.is_open() {
                ctx.settings.hide();
                ctx.ui_state.surfaces.settings = SurfaceVisibility::Hidden;
            } else {
                ctx.settings.show(&ctx.config, &settings_appearance(ctx));
                ctx.ui_state.surfaces.settings = SurfaceVisibility::Visible;
            }
        }
        // Surface toggles without a native surface yet (Overlay/Tray show-hide
        // from future renderers).
        A::ShowSurface(S::Overlay)
        | A::HideSurface(S::Overlay)
        | A::ToggleSurface(S::Overlay)
        | A::ShowSurface(S::Tray)
        | A::HideSurface(S::Tray) => {
            log::debug!("action stubbed (no native surface yet): {action:?}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEP: i16 = 2;
    const STEP_LARGE: i16 = 10;

    fn adjust(delta: i16) -> AppAction {
        AppAction::AdjustVolume {
            delta_percent: delta,
        }
    }

    #[test]
    fn every_hotkey_maps_to_a_sensible_shared_action() {
        use HotkeyAction as H;
        assert_eq!(hotkey_to_action(H::VolumeUp, STEP, STEP_LARGE), adjust(2));
        assert_eq!(
            hotkey_to_action(H::VolumeDown, STEP, STEP_LARGE),
            adjust(-2)
        );
        assert_eq!(
            hotkey_to_action(H::VolumeUpLarge, STEP, STEP_LARGE),
            adjust(10)
        );
        assert_eq!(
            hotkey_to_action(H::VolumeDownLarge, STEP, STEP_LARGE),
            adjust(-10)
        );
        assert_eq!(
            hotkey_to_action(H::ToggleMute, STEP, STEP_LARGE),
            AppAction::ToggleMute
        );
        assert_eq!(
            hotkey_to_action(H::Reset50, STEP, STEP_LARGE),
            AppAction::ResetVolume
        );
        assert_eq!(
            hotkey_to_action(H::OpenMixer, STEP, STEP_LARGE),
            AppAction::ToggleSurface(SurfaceId::Mixer)
        );
        assert_eq!(
            hotkey_to_action(H::OpenMenu, STEP, STEP_LARGE),
            AppAction::OpenTrayMenu
        );
    }

    #[test]
    fn hotkey_mapping_uses_configured_step_sizes() {
        use HotkeyAction as H;
        // Large steps come from the config, not hardcoded.
        assert_eq!(hotkey_to_action(H::VolumeUpLarge, STEP, 25), adjust(25));
        assert_eq!(hotkey_to_action(H::VolumeDownLarge, STEP, 25), adjust(-25));
    }

    #[test]
    fn hotkey_mapping_never_embeds_blacklist_gating() {
        use HotkeyAction as H;
        // The mapping has no blacklist input and always yields the plain
        // volume action; the gate is a host concern in `apply_hotkey` that only
        // applies to hotkey/wheel origin.
        assert_eq!(hotkey_to_action(H::VolumeUp, STEP, STEP_LARGE), adjust(2));
        assert_eq!(
            hotkey_to_action(H::ToggleMute, STEP, STEP_LARGE),
            AppAction::ToggleMute
        );
        assert_eq!(
            hotkey_to_action(H::OpenMenu, STEP, STEP_LARGE),
            AppAction::OpenTrayMenu
        );
    }

    #[test]
    fn every_tray_command_maps_to_intended_action() {
        use TrayCommand as C;
        // Together with `tray_commands_bypass_the_blacklist_gate` below, every
        // remaining `TrayCommand` variant is enumerated (8/8), so a separate
        // exhaustiveness test would be redundant.
        assert_eq!(tray_command_to_action(C::ToggleMute), AppAction::ToggleMute);
        assert_eq!(tray_command_to_action(C::Reset50), AppAction::ResetVolume);
        assert_eq!(
            tray_command_to_action(C::OpenMixer),
            AppAction::ToggleSurface(SurfaceId::Mixer)
        );
        assert_eq!(
            tray_command_to_action(C::Help),
            AppAction::ShowSurface(SurfaceId::Help)
        );
        assert_eq!(
            tray_command_to_action(C::Settings),
            AppAction::ToggleSurface(SurfaceId::Settings)
        );
        assert_eq!(
            tray_command_to_action(C::EditConfig),
            AppAction::OpenConfigLocation
        );
        assert_eq!(
            tray_command_to_action(C::ReloadConfig),
            AppAction::ReloadConfig
        );
        assert_eq!(tray_command_to_action(C::Exit), AppAction::Exit);
    }

    #[test]
    fn tray_commands_bypass_the_blacklist_gate() {
        use TrayCommand as C;
        // Every tray command maps to a plain action the central handler
        // applies unconditionally; none route through the hotkey gate.
        for cmd in [
            C::ToggleMute,
            C::Reset50,
            C::OpenMixer,
            C::Help,
            C::Settings,
            C::EditConfig,
            C::ReloadConfig,
            C::Exit,
        ] {
            let action = tray_command_to_action(cmd);
            assert_ne!(action, AppAction::OpenTrayMenu);
            assert_ne!(action, AppAction::SetVolumePercent { percent: 0 });
        }
    }
}
