//! VolumeControl — native multiplatform volume controller.
//!
//! Core platform-agnostic logic lives here; the Windows system services
//! (tray, overlay, hotkey window) are compiled only on `target_os="windows"`
//! and wired in by `main.rs` via [`app`].

#![allow(clippy::chunks_exact_to_as_chunks, clippy::manual_is_multiple_of)]

pub mod audio;
#[cfg(target_os = "linux")]
pub mod audio_linux;
#[cfg(target_os = "macos")]
pub mod audio_macos;
#[cfg(target_os = "linux")]
pub mod audio_sessions_linux;
pub mod autostart;
#[cfg(target_os = "windows")]
pub mod com_guard;
pub mod config;
pub mod config_ini;
pub mod core;
pub mod host_core;
pub mod hotkeys;
pub mod hotkeys_global;
#[cfg(all(target_os = "linux", feature = "gtk-renderer"))]
pub mod linux_app;
pub mod linux_host_core;
pub mod locale;
#[cfg(target_os = "macos")]
pub mod macos_app;
pub mod ui;

#[cfg(target_os = "windows")]
pub mod app;
#[cfg(target_os = "windows")]
pub mod audio_sessions_win32;
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
pub mod tray_common;
pub mod updates;
#[cfg(target_os = "windows")]
pub mod wheel_win32;

/// CLI fallback (also provides one-shot commands like check-updates across platforms).
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
///
/// On Windows the logger also appends to
/// `%LOCALAPPDATA%\VolumeControl\logs\runtime.log`. Desktop launches have no
/// visible stderr, so this file is the only way to inspect host/audio errors
/// after the fact. Rotated once to `runtime.log.1` at 2 MiB.
pub fn init_logging() {
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());
    let mut builder = pretty_env_logger::formatted_builder();
    builder.parse_filters(&filter);

    #[cfg(target_os = "windows")]
    if let Some(file) = windows_runtime_log() {
        builder.target(env_logger::Target::Pipe(Box::new(TeeWriter {
            file: std::sync::Mutex::new(file),
        })));
    }

    if let Err(error) = builder.try_init() {
        eprintln!("logging already initialised: {error}");
    }
}

#[cfg(target_os = "windows")]
struct TeeWriter {
    file: std::sync::Mutex<std::fs::File>,
}

#[cfg(target_os = "windows")]
impl std::io::Write for TeeWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = std::io::stderr().write_all(buf);
        if let Ok(mut file) = self.file.lock() {
            let _ = file.write_all(buf);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let _ = std::io::stderr().flush();
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn windows_runtime_log() -> Option<std::fs::File> {
    use std::fs::{self, OpenOptions};
    use std::path::PathBuf;

    let base = std::env::var_os("LOCALAPPDATA")?;
    let dir = PathBuf::from(base).join("VolumeControl").join("logs");
    fs::create_dir_all(&dir).ok()?;
    let path = dir.join("runtime.log");
    if fs::metadata(&path)
        .map(|metadata| metadata.len() > 2 * 1024 * 1024)
        .unwrap_or(false)
    {
        let previous = dir.join("runtime.log.1");
        let _ = fs::remove_file(&previous);
        let _ = fs::rename(&path, &previous);
    }
    OpenOptions::new().create(true).append(true).open(path).ok()
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
