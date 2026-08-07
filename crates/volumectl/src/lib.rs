//! VolumeControl — native multiplatform volume controller.
//!
//! Core platform-agnostic logic lives here; the Windows system services
//! (tray, overlay, hotkey window) are compiled only on `target_os="windows"`
//! and wired in by `main.rs` via [`app`].

pub mod audio;
#[cfg(target_os = "linux")]
pub mod audio_linux;
#[cfg(target_os = "macos")]
pub mod audio_macos;
pub mod autostart;
pub mod config;
pub mod core;
pub mod hotkeys;
pub mod hotkeys_rdev;
#[cfg(all(target_os = "linux", feature = "gtk-renderer"))]
pub mod linux_app;
pub mod ui;

#[cfg(target_os = "windows")]
pub mod app;
/// Windows-only backends.
#[cfg(target_os = "windows")]
pub mod audio_windows;
#[cfg(target_os = "windows")]
pub mod help;
#[cfg(target_os = "windows")]
pub mod mixer;
#[cfg(target_os = "windows")]
pub mod overlay;
#[cfg(target_os = "windows")]
pub mod settings;
#[cfg(target_os = "windows")]
pub mod tray;
#[cfg(target_os = "windows")]
pub mod wheel_win32;

/// CLI fallback for non-Windows platforms (also handy for testing).
#[cfg(not(target_os = "windows"))]
pub mod cli;

/// Legacy application state retained for callers of the original public API.
/// New UI code should use [`ui::AppState`].
#[derive(Debug, Clone, Copy, Default)]
pub struct AppState {
    pub volume: f32,
    pub muted: bool,
}

impl AppState {
    pub fn percent(&self) -> u8 {
        (self.volume.clamp(0.0, 1.0) * 100.0).round() as u8
    }
}

/// Logging initialisation helper. Respects `RUST_LOG`; defaults to `info`.
pub fn init_logging() {
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());
    pretty_env_logger::formatted_builder()
        .parse_filters(&filter)
        .init();
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
