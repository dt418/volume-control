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
    /// Executable names (lowercase, with `.exe`) where custom hotkeys are
    /// suppressed while that process has the foreground window.
    pub blacklist: Vec<String>,
    /// Percent thresholds for the colour legend: [gray<green<blue<orange].
    pub color_thresholds: ColorThresholds,
    /// Audible feedback for blocked hotkeys and volume limits.
    pub beep: BeepConfig,
}

/// Beep feedback settings (mirrors VolumePro's `[Beep]` section).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BeepConfig {
    /// Master switch.
    pub enabled: bool,
    /// Frequency (Hz) when a hotkey is blocked by the blacklist.
    pub blocked_freq: u32,
    /// Duration (ms) of the blocked beep.
    pub blocked_duration_ms: u32,
    /// Frequency (Hz) when volume is already at 0% / 100%.
    pub limit_freq: u32,
    /// Duration (ms) of the limit beep.
    pub limit_duration_ms: u32,
}

impl Default for BeepConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            blocked_freq: 400,
            blocked_duration_ms: 80,
            limit_freq: 600,
            limit_duration_ms: 60,
        }
    }
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
            beep: BeepConfig::default(),
        }
    }
}

/// Recommended blacklist presets per modifier (VolumePro parity).
///
/// `CtrlAlt` / `CapsLock` have no known conflicts → empty list.
/// `Alt` conflicts with move-line in editors → editors.
/// `Ctrl` conflicts with paste/zoom/reload in browsers, editors, terminals.
pub fn recommended_blacklist(modifier: HotkeyModifier) -> Vec<String> {
    let list: &[&str] = match modifier {
        HotkeyModifier::CtrlAlt | HotkeyModifier::CapsLock => &[],
        HotkeyModifier::Alt => &[
            "Code.exe",
            "idea64.exe",
            "webstorm64.exe",
            "phpstorm64.exe",
            "sublime_text.exe",
            "notepad++.exe",
            "cursor.exe",
        ],
        HotkeyModifier::Ctrl => &[
            "chrome.exe",
            "msedge.exe",
            "firefox.exe",
            "brave.exe",
            "opera.exe",
            "vivaldi.exe",
            "Code.exe",
            "idea64.exe",
            "webstorm64.exe",
            "phpstorm64.exe",
            "sublime_text.exe",
            "notepad++.exe",
            "cursor.exe",
            "WindowsTerminal.exe",
        ],
    };
    // Entries are lowercased so they match the lowercase process-name check.
    list.iter()
        .map(|s| s.to_lowercase())
        .filter(|s| s.ends_with(".exe"))
        .collect()
}

/// Merge the recommended blacklist for `modifier` into the config, preserving
/// the user's custom entries. Returns the number of apps added (0 if none).
pub fn apply_recommended_blacklist(cfg: &mut Config) -> usize {
    let rec = recommended_blacklist(cfg.modifier);
    let mut seen: std::collections::HashSet<String> = cfg.blacklist.iter().cloned().collect();
    let mut added = 0;
    for app in rec {
        if seen.insert(app.clone()) {
            cfg.blacklist.push(app);
            added += 1;
        }
    }
    if added > 0 {
        let _ = save(cfg);
    }
    added
}

/// Compute the config file path (user config dir + `volume-control/config.json`).
pub fn config_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
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

    PathBuf::from(base)
        .join("volume-control")
        .join("config.json")
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
    // Blacklist entries are lowercased + trimmed at load so the hotkey gate
    // can do an exact lowercase match against the foreground process name.
    cfg.blacklist = cfg
        .blacklist
        .iter()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| s.ends_with(".exe"))
        .collect();
    cfg.beep.blocked_freq = cfg.beep.blocked_freq.clamp(37, 32767);
    cfg.beep.blocked_duration_ms = cfg.beep.blocked_duration_ms.clamp(10, 2000);
    cfg.beep.limit_freq = cfg.beep.limit_freq.clamp(37, 32767);
    cfg.beep.limit_duration_ms = cfg.beep.limit_duration_ms.clamp(10, 2000);
    cfg
}

/// Check whether the given lowercase process name is blacklisted.
pub fn is_blacklisted(blacklist: &[String], process_lower: &str) -> bool {
    blacklist.iter().any(|b| b == process_lower)
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
