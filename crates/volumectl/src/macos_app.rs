//! Native macOS application host.
//!
//! The host owns CoreAudio, configuration, global hotkeys, confirmed UI state,
//! and the AppKit renderer. AppKit work stays on the process main thread while
//! the rdev listener continues to receive events on its existing worker thread.
//! Renderer intent crosses the boundary through [`crate::ui::HostHandle`].

#![cfg(target_os = "macos")]

use std::sync::mpsc::{self, Receiver};
use std::time::SystemTime;

use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSEventMask, NSScreen};
use objc2_foundation::NSDate;

use crate::audio::AudioBackend;
use crate::config::{self, Config};
use crate::core;
use crate::hotkeys::HotkeyAction;
use crate::hotkeys_rdev::RdevHotkeys;
use crate::ui::platform::macos::renderer::MacosRenderer;
use crate::ui::{
    tokens_for, AppAction, AppState, HostHandle, NativeRenderer, SurfaceId, SurfaceVisibility,
    UiCapabilities, UiStatus, WorkArea,
};

const POLL_INTERVAL_SECONDS: f64 = 0.150;
const FALLBACK_WORK_AREA: WorkArea = WorkArea::new(0, 0, 1600, 900);

/// Everything required to process one host poll on the AppKit main thread.
struct HostCtx {
    audio: Box<dyn AudioBackend>,
    hotkeys: RdevHotkeys,
    renderer: MacosRenderer,
    config: Config,
    last_config_mtime: Option<SystemTime>,
    state: AppState,
    caps: UiCapabilities,
    quit_requested: bool,
}

/// Translate a global listener action into the shared host action contract.
fn hotkey_to_action(action: HotkeyAction, config: &Config) -> AppAction {
    use HotkeyAction as H;

    match action {
        H::VolumeUp => AppAction::AdjustVolume {
            delta_percent: config.volume_step as i16,
        },
        H::VolumeDown => AppAction::AdjustVolume {
            delta_percent: -(config.volume_step as i16),
        },
        H::VolumeUpLarge => AppAction::AdjustVolume {
            delta_percent: config.volume_step_large as i16,
        },
        H::VolumeDownLarge => AppAction::AdjustVolume {
            delta_percent: -(config.volume_step_large as i16),
        },
        H::ToggleMute => AppAction::ToggleMute,
        H::Reset50 => AppAction::ResetVolume,
        H::OpenMixer => AppAction::ToggleSurface(SurfaceId::Mixer),
        H::OpenMenu => AppAction::OpenTrayMenu,
    }
}

fn config_mtime() -> Option<SystemTime> {
    std::fs::metadata(config::config_path())
        .and_then(|metadata| metadata.modified())
        .ok()
}

fn detect_caps() -> UiCapabilities {
    let Some(mtm) = MainThreadMarker::new() else {
        return UiCapabilities {
            compositor: true,
            blur: true,
            high_contrast: false,
            reduced_motion: false,
            dpi_scale: 1.0,
            work_area: FALLBACK_WORK_AREA,
        };
    };

    let Some(screen) = NSScreen::mainScreen(mtm) else {
        return UiCapabilities {
            compositor: true,
            blur: true,
            high_contrast: false,
            reduced_motion: false,
            dpi_scale: 1.0,
            work_area: FALLBACK_WORK_AREA,
        };
    };

    let scale = screen.backingScaleFactor() as f32;
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let frame = screen.visibleFrame();
    let work_area = WorkArea::new(
        (frame.origin.x as f32 * scale).round() as i32,
        (frame.origin.y as f32 * scale).round() as i32,
        (frame.size.width as f32 * scale).round().max(1.0) as i32,
        (frame.size.height as f32 * scale).round().max(1.0) as i32,
    );

    UiCapabilities {
        compositor: true,
        blur: true,
        // Optional accessibility preference queries are deliberately non-fatal
        // in this first host slice. Renderer fallback remains deterministic.
        high_contrast: false,
        reduced_motion: false,
        dpi_scale: scale,
        work_area,
    }
}

fn set_appearance(state: &mut AppState, config: &Config) {
    state.theme = config.appearance.theme;
    state.material = config.appearance.material;
    state.motion = config.appearance.motion;
}

/// Read authoritative CoreAudio state and publish one confirmed renderer frame.
fn refresh_from_audio(ctx: &mut HostCtx) {
    match ctx.audio.get_state() {
        Ok(volume) => {
            ctx.state.volume_percent = volume.percent();
            ctx.state.muted = volume.muted;
            ctx.state.status = UiStatus::Ready;
        }
        Err(error) => {
            log::warn!("macOS audio readback failed: {error}");
            ctx.state.status = UiStatus::Error(error.to_string());
        }
    }

    set_appearance(&mut ctx.state, &ctx.config);
    let tokens = tokens_for(
        ctx.config.appearance.theme,
        ctx.caps.high_contrast,
        ctx.config.appearance.accent,
        || None,
    );
    ctx.renderer.publish(&ctx.state, &tokens, &ctx.caps);
}

fn log_deferred(action: &AppAction) {
    log::info!("macOS host defers {action:?}; the host remains running");
}

/// Apply one host action. Audio errors are logged and confirmed on the next
/// readback; a single failed mutation does not terminate the host.
fn apply_action(ctx: &mut HostCtx, action: &AppAction) {
    match action {
        AppAction::SetVolumePercent { percent } => {
            let target = (*percent).min(100) as f32 / 100.0;
            if let Err(error) = ctx.audio.set_volume(target) {
                log::warn!("macOS set volume failed: {error}");
            }
        }
        AppAction::AdjustVolume { delta_percent } => {
            let current = ctx
                .audio
                .get_state()
                .map(|state| state.volume)
                .unwrap_or(ctx.state.volume_percent as f32 / 100.0);
            let target = core::step_volume(current, *delta_percent as f32);
            if let Err(error) = ctx.audio.set_volume(target) {
                log::warn!("macOS adjust volume failed: {error}");
            }
        }
        AppAction::ToggleMute => {
            if let Err(error) = ctx.audio.toggle_mute() {
                log::warn!("macOS toggle mute failed: {error}");
            }
        }
        AppAction::SetMute { muted } => {
            if let Err(error) = ctx.audio.set_mute(*muted) {
                log::warn!("macOS set mute failed: {error}");
            }
        }
        AppAction::ResetVolume => {
            if let Err(error) = ctx.audio.set_volume(0.5) {
                log::warn!("macOS reset volume failed: {error}");
            }
        }
        AppAction::ShowSurface(surface) => ctx
            .state
            .set_visibility(*surface, SurfaceVisibility::Visible),
        AppAction::HideSurface(surface) => ctx
            .state
            .set_visibility(*surface, SurfaceVisibility::Hidden),
        AppAction::ToggleSurface(surface) => {
            let visibility = ctx.state.toggle(*surface);
            log::debug!("macOS toggled {surface:?} -> {visibility:?}");
        }
        AppAction::OpenTrayMenu => {
            log::info!("macOS tray/menu unavailable in this host");
        }
        AppAction::OpenConfigLocation => {
            log::info!("macOS config-file integration is deferred in this host");
        }
        AppAction::ReloadConfig => {
            ctx.last_config_mtime = None;
            let _ = reload_config_if_changed(ctx);
        }
        AppAction::SetTheme(theme) => {
            ctx.config.appearance.theme = *theme;
            ctx.state.theme = *theme;
        }
        AppAction::SetMaterial(material) => {
            ctx.config.appearance.material = *material;
            ctx.state.material = *material;
        }
        AppAction::SetMotion(motion) => {
            ctx.config.appearance.motion = *motion;
            ctx.state.motion = *motion;
        }
        AppAction::ApplyConfig
        | AppAction::CancelConfig
        | AppAction::ResetConfig
        | AppAction::AddBlacklistEntry(_)
        | AppAction::RemoveBlacklistEntry(_)
        | AppAction::ClearBlacklist
        | AppAction::ApplyRecommendedBlacklist => log_deferred(action),
        AppAction::Exit => {
            ctx.quit_requested = true;
        }
    }
}

/// Reload persisted preferences after an mtime change.
fn reload_config_if_changed(ctx: &mut HostCtx) -> bool {
    let mtime = config_mtime();
    if mtime == ctx.last_config_mtime && mtime.is_some() {
        return false;
    }

    let new_config = match config::load_existing() {
        Ok(config) => config,
        Err(error) => {
            log::warn!("macOS configuration reload failed: {error}");
            return false;
        }
    };
    let modifier_changed = new_config.modifier != ctx.config.modifier;
    ctx.config = new_config;
    ctx.last_config_mtime = mtime;
    set_appearance(&mut ctx.state, &ctx.config);

    if modifier_changed {
        ctx.hotkeys.set_modifier(ctx.config.modifier);
    }
    log::info!("macOS configuration reloaded");
    true
}

fn drain_host_actions(ctx: &mut HostCtx, receiver: &Receiver<AppAction>) -> Option<String> {
    let listener_error = ctx.hotkeys.listener_failure();
    if let Some(error) = &listener_error {
        log::error!("global keyboard listener stopped: {error}");
        ctx.quit_requested = true;
    }

    while let Some(hotkey) = ctx.hotkeys.try_recv() {
        let action = hotkey_to_action(hotkey, &ctx.config);
        apply_action(ctx, &action);
    }
    while let Ok(action) = receiver.try_recv() {
        apply_action(ctx, &action);
    }

    reload_config_if_changed(ctx);
    listener_error
}

/// Run the native macOS host. This function must be called on the process main
/// thread because all AppKit objects are main-thread-only.
pub fn run() -> Result<(), String> {
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| "macOS host must run on the process main thread".to_string())?;
    let app = NSApplication::sharedApplication(mtm);
    app.finishLaunching();

    let config = config::load();
    let last_config_mtime = config_mtime();
    let audio = crate::audio::default_backend().map_err(|error| error.to_string())?;
    let hotkeys = RdevHotkeys::new(config.modifier).map_err(|error| error.to_string())?;
    let caps = detect_caps();

    let (sender, receiver) = mpsc::channel::<AppAction>();
    let host = HostHandle::new(move |action| {
        let _ = sender.send(action);
    });
    let mut renderer = MacosRenderer::create(host, caps).map_err(|error| error.to_string())?;
    let initial_audio = audio.get_state().map_err(|error| error.to_string())?;
    let mut state = AppState::from_audio(initial_audio.percent(), initial_audio.muted, None);
    state.theme = config.appearance.theme;
    state.material = config.appearance.material;
    state.motion = config.appearance.motion;

    let tokens = tokens_for(
        config.appearance.theme,
        caps.high_contrast,
        config.appearance.accent,
        || None,
    );
    renderer.publish(&state, &tokens, &caps);

    let mut ctx = HostCtx {
        audio,
        hotkeys,
        renderer,
        config,
        last_config_mtime,
        state,
        caps,
        quit_requested: false,
    };

    let mut listener_error = None;
    while !ctx.quit_requested {
        let deadline = NSDate::dateWithTimeIntervalSinceNow(POLL_INTERVAL_SECONDS);
        if let Some(event) = app.nextEventMatchingMask_untilDate_inMode_dequeue(
            NSEventMask::Any,
            Some(&deadline),
            unsafe { objc2_foundation::NSDefaultRunLoopMode },
            true,
        ) {
            app.sendEvent(&event);
            app.updateWindows();
        }

        listener_error = drain_host_actions(&mut ctx, &receiver);
        refresh_from_audio(&mut ctx);
    }

    ctx.renderer.destroy();
    drop(ctx);

    match listener_error {
        Some(error) => Err(format!("global keyboard listener stopped: {error}")),
        None => Ok(()),
    }
}

/// Minimal public seam used by the harness-free host smoke binary.
#[doc(hidden)]
pub fn action_requests_shutdown(action: &AppAction) -> bool {
    matches!(action, AppAction::Exit)
}

#[doc(hidden)]
pub fn deferred_tray_menu_is_non_fatal() -> bool {
    let action = AppAction::OpenTrayMenu;
    !action_requests_shutdown(&action)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotkeys_use_configured_small_and_large_steps() {
        let config = Config::default();
        assert_eq!(
            hotkey_to_action(HotkeyAction::VolumeUp, &config),
            AppAction::AdjustVolume { delta_percent: 2 }
        );
        assert_eq!(
            hotkey_to_action(HotkeyAction::VolumeDownLarge, &config),
            AppAction::AdjustVolume { delta_percent: -10 }
        );
        assert_eq!(
            hotkey_to_action(HotkeyAction::ToggleMute, &config),
            AppAction::ToggleMute
        );
        assert_eq!(
            hotkey_to_action(HotkeyAction::Reset50, &config),
            AppAction::ResetVolume
        );
        assert_eq!(
            hotkey_to_action(HotkeyAction::OpenMixer, &config),
            AppAction::ToggleSurface(SurfaceId::Mixer)
        );
        assert_eq!(
            hotkey_to_action(HotkeyAction::OpenMenu, &config),
            AppAction::OpenTrayMenu
        );
    }

    #[test]
    fn deferred_menu_does_not_request_shutdown() {
        assert!(!action_requests_shutdown(&AppAction::OpenTrayMenu));
        assert!(action_requests_shutdown(&AppAction::Exit));
    }
}
