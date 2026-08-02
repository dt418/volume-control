//! VolumeControl — native multiplatform volume controller.
//!
//! Core platform-agnostic logic lives here; the Windows system services
//! (tray, overlay, hotkey window) are compiled only on `target_os="windows"`
//! and wired in by `main.rs` via [`app`].

pub mod audio;
pub mod config;
pub mod core;
pub mod hotkeys;

#[cfg(target_os = "windows")]
pub mod app;
/// Windows-only backends.
#[cfg(target_os = "windows")]
pub mod audio_windows;
#[cfg(target_os = "windows")]
pub mod help;
#[cfg(target_os = "windows")]
pub mod hotkeys_win32;
#[cfg(target_os = "windows")]
pub mod mixer;
#[cfg(target_os = "windows")]
pub mod overlay;
#[cfg(target_os = "windows")]
pub mod tray;

/// CLI fallback for non-Windows platforms (also handy for testing).
#[cfg(not(target_os = "windows"))]
pub mod cli;

/// Requested application state — the single place the GUI layer reads from.
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
