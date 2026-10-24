//! Native Winit/WGPU runtime host for Liquid Glass surfaces.
//!
//! Consumes authoritative [`volumectl::ui::AppState`] publications through
//! [`liquid_glass::LiquidGlassSurfaceController`] and renders each surface
//! with WGPU. User intent is emitted as [`volumectl::ui::AppAction`] values.
//!
//! Usage: `volumecontrol-host [--smoke]` — `--smoke` presents every surface
//! once and exits through the production loop.

mod app;
mod hotkeys;
mod paint;
mod placement;
mod sync;
mod vibrancy;
use std::time::SystemTime;

use winit::event_loop::EventLoop;

use app::HostApp;
use hotkeys::GlobalHotkeySource;
use volumectl::linux_host_core::HostCore;
use volumectl::ui::{UiCapabilities, WorkArea};

fn detect_caps() -> UiCapabilities {
    #[cfg(target_os = "windows")]
    {
        UiCapabilities {
            compositor: volumectl::ui::platform::windows::primitives::compositor_enabled(),
            blur: volumectl::ui::platform::windows::primitives::compositor_enabled(),
            high_contrast: volumectl::ui::platform::windows::primitives::high_contrast_enabled(),
            reduced_motion: volumectl::ui::platform::windows::primitives::reduced_motion_enabled(),
            reduced_transparency:
                volumectl::ui::platform::windows::primitives::reduced_transparency_enabled(),
            dpi_scale: 1.0,
            work_area: WorkArea::new(0, 0, 1600, 900),
        }
    }
    #[cfg(not(target_os = "windows"))]
    UiCapabilities {
        compositor: true,
        blur: true,
        high_contrast: false,
        reduced_motion: false,
        reduced_transparency: false,
        dpi_scale: 1.0,
        work_area: WorkArea::new(0, 0, 1600, 900),
    }
}

fn config_mtime() -> Option<SystemTime> {
    std::fs::metadata(volumectl::config::config_path())
        .and_then(|metadata| metadata.modified())
        .ok()
}

fn main() -> Result<(), String> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init()
        .ok();

    let smoke = std::env::args().any(|arg| arg == "--smoke");
    let smoke_ms = std::env::args().find_map(|arg| {
        arg.strip_prefix("--smoke-ms=")
            .and_then(|ms| ms.parse::<u64>().ok())
    });
    let show_all = std::env::args().any(|arg| arg == "--show-all");
    let target_surface = std::env::args().find_map(|arg| {
        arg.strip_prefix("--surface=")
            .and_then(|name| match name.to_lowercase().as_str() {
                "mixer" => Some(volumectl::ui::SurfaceId::Mixer),
                "settings" => Some(volumectl::ui::SurfaceId::Settings),
                "help" => Some(volumectl::ui::SurfaceId::Help),
                "overlay" => Some(volumectl::ui::SurfaceId::Overlay),
                _ => None,
            })
    });
    let config = volumectl::config::load();
    let audio = volumectl::audio::default_backend().ok();
    let hotkeys = match volumectl::hotkeys_global::GlobalHotkeys::new(config.modifier) {
        Ok(hotkeys) => Some(Box::new(GlobalHotkeySource::new(hotkeys))
            as Box<dyn volumectl::linux_host_core::HotkeySource>),
        Err(error) => {
            log::warn!("global hotkeys unavailable: {error}");
            None
        }
    };

    let core = HostCore::new(audio, hotkeys, config, detect_caps());
    let mut core = core;
    #[cfg(target_os = "windows")]
    core.set_sessions_source(Box::new(volumectl::audio_sessions_win32::WindowsSessions));
    #[cfg(target_os = "linux")]
    core.set_sessions_source(Box::new(
        volumectl::audio_sessions_linux::PulseSessions::new(),
    ));
    let mut app = HostApp::new(
        core,
        smoke,
        show_all,
        target_surface,
        config_mtime(),
        smoke_ms.unwrap_or(2_500),
    );

    let event_loop = EventLoop::builder()
        .build()
        .map_err(|error| format!("event loop creation failed: {error}"))?;
    event_loop
        .run_app(&mut app)
        .map_err(|error| format!("event loop failed: {error}"))?;
    app.smoke_result()
}
