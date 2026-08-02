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
use crate::ui::{AppAction, SurfaceId, SurfaceVisibility};

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
    /// Shared confirmed UI state published to every renderer by
    /// [`publish_confirmed_state`].
    ui_state: crate::ui::AppState,
    /// Display-session capabilities measured once at startup. Consumed by the
    /// adaptive renderers (Tasks 7/8).
    #[allow(dead_code)]
    caps: crate::ui::UiCapabilities,
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
        help,
        tray,
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
                    apply_hotkey(&mut *ctx, action);
                }
            }
            0
        }
        // ── Mod+Scroll fired (mouse hook posts the same action ids) ──────
        WM_APP_WHEEL => {
            if let Some(action) = hotkey_action(wparam as i32) {
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

    log::debug!(
        "publish: state={}%% muted={} mixer_open={} show_overlay={}",
        st.percent(),
        st.muted,
        ctx.mixer.is_open(),
        show_overlay
    );

    if show_overlay {
        ctx.ui_state.surfaces.overlay = SurfaceVisibility::Visible;
        ctx.overlay.show(&st, &ctx.config);
    }
    ctx.tray.set_volume(&st);
    if ctx.mixer.is_open() {
        ctx.mixer.sync(&st);
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
        C::EditConfig => AppAction::OpenConfigLocation,
        C::ReloadConfig => AppAction::ReloadConfig,
        C::ApplyBlacklist => AppAction::ApplyRecommendedBlacklist,
        C::Exit => AppAction::Exit,
    }
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
                    ctx.mixer.sync(&st);
                }
                ctx.mixer.toggle();
            }
            publish_confirmed_state(ctx, false);
        }
        A::HideSurface(S::Mixer) => {
            if ctx.mixer.is_open() {
                ctx.mixer.toggle();
            }
            publish_confirmed_state(ctx, false);
        }
        A::ToggleSurface(S::Mixer) => {
            // Sync before showing so the mixer reflects current audio state
            // when it appears (mirrors the original hotkey/tray behavior).
            if !ctx.mixer.is_open() {
                if let Ok(st) = ctx.audio.get_state() {
                    ctx.mixer.sync(&st);
                }
            }
            ctx.mixer.toggle();
            publish_confirmed_state(ctx, false);
        }
        A::ShowSurface(S::Help) => {
            ctx.help.show(&ctx.config);
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
                ctx.help.show(&ctx.config);
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
            ctx.overlay
                .show_text("Editing config — changes reload automatically", &ctx.config);
        }
        A::ReloadConfig => {
            ctx.last_config_mtime = None; // force
            reload_config_if_changed(ctx);
            ctx.overlay.show_text("Config reloaded", &ctx.config);
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
            ctx.overlay.show_text(&msg, &ctx.config);
        }
        A::Exit => unsafe {
            PostQuitMessage(0);
        },
        // Settings draft/appearance intents are for Tasks 9/10; not wired yet.
        A::ApplyConfig
        | A::CancelConfig
        | A::ResetConfig
        | A::SetTheme(_)
        | A::SetMaterial(_)
        | A::SetMotion(_)
        | A::AddBlacklistEntry(_)
        | A::RemoveBlacklistEntry(_)
        | A::ClearBlacklist => {
            log::debug!("action stubbed for Settings (Tasks 9/10): {action:?}");
        }
        // Surface toggles without a native surface yet (Settings/Overlay/Tray
        // show-hide from future renderers).
        A::ShowSurface(S::Settings)
        | A::HideSurface(S::Settings)
        | A::ToggleSurface(S::Settings)
        | A::ShowSurface(S::Overlay)
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
            tray_command_to_action(C::EditConfig),
            AppAction::OpenConfigLocation
        );
        assert_eq!(
            tray_command_to_action(C::ReloadConfig),
            AppAction::ReloadConfig
        );
        assert_eq!(
            tray_command_to_action(C::ApplyBlacklist),
            AppAction::ApplyRecommendedBlacklist
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
            C::EditConfig,
            C::ReloadConfig,
            C::ApplyBlacklist,
            C::Exit,
        ] {
            let action = tray_command_to_action(cmd);
            assert_ne!(action, AppAction::OpenTrayMenu);
            assert_ne!(action, AppAction::SetVolumePercent { percent: 0 });
        }
    }
}
