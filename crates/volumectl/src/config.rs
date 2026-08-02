//! Persistent configuration.
//!
//! The app auto-generates a `config.json` on first run in the user's config
//! directory, mirrors the setting surface of VolumePro (`VolumePro.ini`) but
//! in a format the native backends can share. Overlay duration, step sizes,
//! hotkey modifier and blacklist are the user-tunable knobs.
//!
//! The app silently watches the file (mtime) and reloads it while running,
//! so tweaking config in a text editor takes effect without restart.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Modifier combos for the custom hotkeys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub enum HotkeyModifier {
    #[default]
    CtrlAlt,
    Alt,
    Ctrl,
    #[serde(rename = "CapsLock")]
    CapsLock,
}

/// App-level configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Small step percent (default 2).
    pub volume_step: u32,
    /// Large step percent for Shift variants (default 10).
    pub volume_step_large: u32,
    /// Overlay visible time in ms (default 1800).
    pub overlay_duration_ms: u64,
    /// Modifier used for custom hotkeys (default CtrlAlt).
    pub modifier: HotkeyModifier,
    /// Window titles / executable names where custom hotkeys are suppressed.
    pub blacklist: Vec<String>,
    /// Percent thresholds for the colour legend: [gray<green<blue<orange].
    pub color_thresholds: ColorThresholds,
}

/// Colour legend thresholds, matching VolumePro's (0 grey, ≤40 green,
/// ≤75 blue, else orange-red).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColorThresholds {
    pub green_up_to: u8,
    pub blue_up_to: u8,
    pub orange_up_to: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            volume_step: 2,
            volume_step_large: 10,
            overlay_duration_ms: 1800,
            modifier: HotkeyModifier::CtrlAlt,
            blacklist: Vec::new(),
            color_thresholds: ColorThresholds {
                green_up_to: 40,
                blue_up_to: 75,
                orange_up_to: 100,
            },
        }
    }
}

/// Compute the config file path (user config dir + `volume-control/config.json`).
pub fn config_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    let base = std::env::var("APPDATA").unwrap_or_else(|_| "." .into());
    #[cfg(target_os = "macos")]
    let base = {
        std::env::var("HOME")
            .map(|h| h + "/Library/Application Support")
            .unwrap_or_else(|_| ".".into())
    };
    #[cfg(target_os = "linux")]
    let base = {
        std::env::var("XDG_CONFIG_HOME")
            .unwrap_or_else(|_| std::env::var("HOME").unwrap_or_else(|_| ".".into()) + "/.config")
    };

    PathBuf::from(base).join("volume-control").join("config.json")
}

/// Load the config; on absence/parse/validation failure write default and re-save.
///
/// Only writes the file back when normalisation actually changed values, so
/// the app's live-reload watcher (mtime based) doesn't loop on its own writes.
pub fn load() -> Config {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(raw) => match serde_json::from_str::<Config>(&raw) {
            Ok(orig) => {
                let cfg = validate(orig.clone());
                if cfg != orig {
                    let _ = save(&cfg);
                }
                cfg
            }
            Err(e) => {
                log::warn!("config parse error ({e}); using defaults");
                let cfg = validate(Config::default());
                let _ = save(&cfg);
                cfg
            }
        },
        Err(_) => {
            log::info!("no config yet — writing defaults to {}", path.display());
            let cfg = validate(Config::default());
            let _ = save(&cfg);
            cfg
        }
    }
}

fn validate(mut cfg: Config) -> Config {
    cfg.volume_step = cfg.volume_step.clamp(1, 50);
    cfg.volume_step_large = cfg.volume_step_large.clamp(cfg.volume_step + 1, 50);
    cfg.overlay_duration_ms = cfg.overlay_duration_ms.clamp(200, 10_000);
    cfg
}

/// Persist the config; creates the parent dir if absent.
pub fn save(cfg: &Config) -> std::io::Result<()> {
    let path = config_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(cfg)?;
    std::fs::write(&path, text)
}

/// Open the config in the user's text editor.
#[cfg(target_os = "windows")]
pub fn open_in_editor() {
    let path = config_path();
    // `cmd /c start` opens with the associated editor (Notepad default).
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", &path.display().to_string()])
        .spawn();
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
pub fn open_in_editor() {
    // No-op stub for non-Windows until the GUI frontend lands.
}