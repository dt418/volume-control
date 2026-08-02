//! Persistent configuration.
//!
//! The app auto-generates a `config.json` on first run in the user's config
//! directory, mirrors the setting surface of VolumePro (`VolumePro.ini`) but
//! in a format the native backends can share. Overlay duration, step sizes,
//! hotkey modifier and blacklist are the user-tunable knobs.
//!
//! The app silently watches the file (mtime) and reloads it while running,
//! so tweaking config in a text editor takes effect without restart.

use crate::ui::{AccentMode, MaterialMode, MotionMode, ThemeMode};
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

const MIN_VOLUME_STEP: u32 = 1;
const MAX_VOLUME_STEP: u32 = 50;
const MIN_OVERLAY_DURATION_MS: u64 = 200;
const MAX_OVERLAY_DURATION_MS: u64 = 10_000;

/// A field-specific configuration validation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValidationError {
    pub field: &'static str,
    pub message: String,
}

impl fmt::Display for ConfigValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for ConfigValidationError {}

/// Errors returned when validated persistence cannot complete.
#[derive(Debug)]
pub enum ConfigError {
    Validation(ConfigValidationError),
    Io(std::io::Error),
    Serialization(serde_json::Error),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => error.fmt(f),
            Self::Io(error) => write!(f, "config I/O failed: {error}"),
            Self::Serialization(error) => write!(f, "config serialization failed: {error}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for ConfigError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}

fn validation(field: &'static str, message: impl Into<String>) -> ConfigValidationError {
    ConfigValidationError {
        field,
        message: message.into(),
    }
}

/// Persisted preferences shared by all UI renderers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppearanceConfig {
    pub theme: ThemeMode,
    pub material: MaterialMode,
    pub motion: MotionMode,
    pub accent: AccentMode,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            theme: ThemeMode::System,
            material: MaterialMode::Auto,
            motion: MotionMode::Full,
            accent: AccentMode::System,
        }
    }
}

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
    /// Appearance preferences shared by all UI surfaces.
    #[serde(default)]
    pub appearance: AppearanceConfig,
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
            appearance: AppearanceConfig::default(),
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
                let cfg = normalize(orig.clone());
                if cfg != orig {
                    let _ = save(&cfg);
                }
                cfg
            }
            Err(e) => {
                log::warn!("config parse error ({e}); using defaults");
                let cfg = normalize(Config::default());
                let _ = save(&cfg);
                cfg
            }
        },
        Err(_) => {
            log::info!("no config yet — writing defaults to {}", path.display());
            let cfg = normalize(Config::default());
            let _ = save(&cfg);
            cfg
        }
    }
}

/// Validate raw configuration values without changing them.
///
/// This is intentionally strict for callers such as Settings Apply. The live
/// loader uses [`normalize`] instead so an older or hand-edited file still has
/// the historical fallback behavior.
pub fn validate(cfg: &Config) -> Result<(), ConfigValidationError> {
    if !(MIN_VOLUME_STEP..=MAX_VOLUME_STEP).contains(&cfg.volume_step) {
        return Err(validation(
            "volume_step",
            format!("must be between {MIN_VOLUME_STEP} and {MAX_VOLUME_STEP}"),
        ));
    }
    if !(MIN_VOLUME_STEP..=MAX_VOLUME_STEP).contains(&cfg.volume_step_large) {
        return Err(validation(
            "volume_step_large",
            format!("must be between {MIN_VOLUME_STEP} and {MAX_VOLUME_STEP}"),
        ));
    }
    if cfg.volume_step_large <= cfg.volume_step {
        return Err(validation(
            "volume_step_large",
            "must be greater than volume_step",
        ));
    }
    if !(MIN_OVERLAY_DURATION_MS..=MAX_OVERLAY_DURATION_MS).contains(&cfg.overlay_duration_ms) {
        return Err(validation(
            "overlay_duration_ms",
            format!("must be between {MIN_OVERLAY_DURATION_MS} and {MAX_OVERLAY_DURATION_MS}"),
        ));
    }
    if !(37..=32_767).contains(&cfg.beep.blocked_freq) {
        return Err(validation(
            "beep.blocked_freq",
            "must be between 37 and 32767",
        ));
    }
    if !(10..=2_000).contains(&cfg.beep.blocked_duration_ms) {
        return Err(validation(
            "beep.blocked_duration_ms",
            "must be between 10 and 2000",
        ));
    }
    if !(37..=32_767).contains(&cfg.beep.limit_freq) {
        return Err(validation(
            "beep.limit_freq",
            "must be between 37 and 32767",
        ));
    }
    if !(10..=2_000).contains(&cfg.beep.limit_duration_ms) {
        return Err(validation(
            "beep.limit_duration_ms",
            "must be between 10 and 2000",
        ));
    }
    Ok(())
}

/// Return a normalized copy while preserving the existing config bounds.
pub fn normalize(mut cfg: Config) -> Config {
    // There must be room for a strictly larger large step within the existing
    // 1..=50 bounds, so the small step's effective maximum is 49.
    cfg.volume_step = cfg
        .volume_step
        .clamp(MIN_VOLUME_STEP, MAX_VOLUME_STEP.saturating_sub(1));
    cfg.volume_step_large = cfg
        .volume_step_large
        .clamp(cfg.volume_step.saturating_add(1), MAX_VOLUME_STEP);

    // Keeping the relationship repair here (rather than relying on callers to
    // validate first) makes normalization safe for hand-edited legacy files.
    cfg.overlay_duration_ms = cfg
        .overlay_duration_ms
        .clamp(MIN_OVERLAY_DURATION_MS, MAX_OVERLAY_DURATION_MS);
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

/// Validate, normalize, persist, and return the saved configuration.
pub fn save_validated(cfg: &Config) -> Result<Config, ConfigError> {
    validate(cfg).map_err(ConfigError::Validation)?;
    let normalized = normalize(cfg.clone());
    save_at_path(&normalized, &config_path())?;
    Ok(normalized)
}

fn save_at_path(cfg: &Config, path: &Path) -> Result<(), ConfigError> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(cfg)?;
    let temp_path = temporary_path(path);

    let write_result = (|| -> io::Result<()> {
        let mut temp = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        temp.write_all(text.as_bytes())?;
        temp.flush()?;
        temp.sync_all()?;
        drop(temp);
        replace_file(&temp_path, path)
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result.map_err(ConfigError::Io)
}

fn temporary_path(path: &Path) -> PathBuf {
    let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("config");
    path.with_file_name(format!(".{file_name}.tmp-{}-{counter}", std::process::id()))
}

#[cfg(not(target_os = "windows"))]
fn replace_file(temp_path: &Path, path: &Path) -> io::Result<()> {
    fs::rename(temp_path, path)
}

#[cfg(target_os = "windows")]
fn replace_file(temp_path: &Path, path: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, ReplaceFileW, MOVEFILE_REPLACE_EXISTING,
    };

    let temp: Vec<u16> = temp_path.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let replaced = unsafe {
        ReplaceFileW(
            destination.as_ptr(),
            temp.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if replaced != 0 {
        return Ok(());
    }

    // ReplaceFileW requires an existing destination. MoveFileExW preserves the
    // same-directory atomic replacement behavior for a newly created config.
    let moved = unsafe {
        MoveFileExW(
            temp.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING,
        )
    };
    if moved != 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(unsafe { GetLastError() } as i32))
    }
}

#[cfg(test)]
fn save_at_path_for_test(cfg: &Config, path: &Path) -> Result<(), ConfigError> {
    save_at_path(cfg, path)
}

/// Backwards-compatible persistence entry point.
pub fn save(cfg: &Config) -> std::io::Result<()> {
    save_at_path(cfg, &config_path()).map_err(|error| match error {
        ConfigError::Io(error) => error,
        ConfigError::Serialization(error) => {
            std::io::Error::new(std::io::ErrorKind::InvalidData, error)
        }
        ConfigError::Validation(error) => {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, error)
        }
    })
}

/// Check whether the given lowercase process name is blacklisted.
pub fn is_blacklisted(blacklist: &[String], process_lower: &str) -> bool {
    blacklist.iter().any(|b| b == process_lower)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{AccentMode, MaterialMode, MotionMode, ThemeMode};

    #[test]
    fn old_json_without_appearance_uses_appearance_defaults() {
        let cfg: Config = serde_json::from_str(
            r#"{
                "volume_step": 2,
                "volume_step_large": 10,
                "overlay_duration_ms": 1800,
                "modifier": "CtrlAlt",
                "blacklist": [],
                "color_thresholds": {
                    "green_up_to": 40,
                    "blue_up_to": 75,
                    "orange_up_to": 100
                },
                "beep": {
                    "enabled": true,
                    "blocked_freq": 400,
                    "blocked_duration_ms": 80,
                    "limit_freq": 600,
                    "limit_duration_ms": 60
                }
            }"#,
        )
        .expect("legacy config remains readable");

        assert_eq!(cfg.appearance, AppearanceConfig::default());
    }

    #[test]
    fn appearance_defaults_are_system_auto_full_system() {
        let appearance = AppearanceConfig::default();

        assert_eq!(appearance.theme, ThemeMode::System);
        assert_eq!(appearance.material, MaterialMode::Auto);
        assert_eq!(appearance.motion, MotionMode::Full);
        assert_eq!(appearance.accent, AccentMode::System);
    }

    #[test]
    fn strict_validation_reports_invalid_step_relationship_by_field() {
        let mut cfg = Config::default();
        cfg.volume_step = 20;
        cfg.volume_step_large = 10;

        let error = validate(&cfg).expect_err("large step must be greater");

        assert_eq!(error.field, "volume_step_large");
        assert!(error.to_string().contains("volume_step_large"));
    }

    #[test]
    fn normalize_preserves_bounds_and_repairs_step_relationship() {
        let mut cfg = Config::default();
        cfg.volume_step = 0;
        cfg.volume_step_large = 0;
        cfg.overlay_duration_ms = u64::MAX;
        cfg.beep.blocked_freq = 0;
        cfg.beep.blocked_duration_ms = 0;

        let normalized = normalize(cfg);

        assert_eq!(normalized.volume_step, 1);
        assert_eq!(normalized.volume_step_large, 2);
        assert_eq!(normalized.overlay_duration_ms, 10_000);
        assert_eq!(normalized.beep.blocked_freq, 37);
        assert_eq!(normalized.beep.blocked_duration_ms, 10);
        validate(&normalized).expect("normalized config is valid");
    }

    #[test]
    fn normalize_repairs_maximum_step_boundary() {
        let mut cfg = Config::default();
        cfg.volume_step = 50;
        cfg.volume_step_large = 50;

        let normalized = normalize(cfg);

        assert_eq!(normalized.volume_step, 49);
        assert_eq!(normalized.volume_step_large, 50);
        validate(&normalized).expect("maximum normalized config is valid");
    }

    #[test]
    fn save_at_path_preserves_existing_file_contents() {
        let temp_dir = std::env::temp_dir().join(format!(
            "volumectl-config-test-{}-{}",
            std::process::id(),
            TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&temp_dir).expect("create temporary test directory");
        let path = temp_dir.join("config.json");
        std::fs::write(&path, "existing config").expect("seed existing config");

        save_at_path_for_test(&Config::default(), &path).expect("safe save succeeds");

        assert_ne!(std::fs::read_to_string(&path).unwrap(), "existing config");
        assert!(std::fs::read_dir(&temp_dir)
            .unwrap()
            .filter_map(Result::ok)
            .all(|entry| entry.path() == path));
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn save_at_path_does_not_truncate_existing_directory_destination() {
        let temp_dir = std::env::temp_dir().join(format!(
            "volumectl-config-directory-destination-{}-{}",
            std::process::id(),
            TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&temp_dir).expect("create temporary test directory");
        let path = temp_dir.join("config.json");
        std::fs::create_dir(&path).expect("create directory destination");

        let result = save_at_path_for_test(&Config::default(), &path);

        assert!(result.is_err());
        assert!(path.is_dir());
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn save_validated_rejects_invalid_config_before_writing() {
        let mut cfg = Config::default();
        cfg.volume_step = 30;
        cfg.volume_step_large = 29;

        let error = save_validated(&cfg).expect_err("invalid relationship must not save");

        assert!(matches!(error, ConfigError::Validation(_)));
    }
}
