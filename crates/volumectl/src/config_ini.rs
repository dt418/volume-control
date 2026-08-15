//! Typed, human-readable INI persistence for [`crate::config::Config`].

use crate::config::{
    self, Config, ConfigError, ConfigValidationError, HotkeyBindings, HotkeyModifier,
};
use crate::ui::{AccentMode, MaterialMode, MotionMode, ThemeMode};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// The result of attempting one-way migration from the legacy JSON file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationOutcome {
    /// JSON was validated and written to the absent INI path.
    Migrated,
    /// An INI file was already present, so the JSON file was untouched.
    AlreadyPresent,
    /// Neither migration nor an existing INI file was needed because JSON was absent.
    NoLegacyFile,
}

/// Parse the supported typed INI schema.
pub fn parse_ini(text: &str) -> Result<Config, ConfigError> {
    let mut config = Config {
        hotkeys: None,
        ..Config::default()
    };
    // The INI schema stores the legacy modifier only. Let normalization derive
    // the corresponding preset rather than retaining Config::default()'s
    // Ctrl+Alt bindings when another modifier is selected.
    let mut section: Option<String> = None;
    let mut seen = HashSet::new();
    let mut blacklist_items = Vec::new();
    let mut blacklist_indexes = HashSet::new();

    for (line_number, raw_line) in text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        if line.starts_with('[') {
            if !line.ends_with(']') || line.len() < 3 {
                return Err(ini_error(
                    "ini",
                    format!("line {} has an invalid section", line_number + 1),
                ));
            }
            let name = line[1..line.len() - 1].trim();
            if name.is_empty() || name.contains('[') || name.contains(']') {
                return Err(ini_error(
                    "ini",
                    format!("line {} has an invalid section", line_number + 1),
                ));
            }
            if !matches!(
                name,
                "general"
                    | "hotkeys"
                    | "appearance"
                    | "feedback"
                    | "color_thresholds"
                    | "blacklist"
            ) {
                log::warn!("ignoring unknown INI section [{name}]");
            }
            section = Some(name.to_owned());
            continue;
        }

        let Some((raw_key, raw_value)) = line.split_once('=') else {
            return Err(ini_error(
                "ini",
                format!("line {} must contain a key=value pair", line_number + 1),
            ));
        };
        let key = raw_key.trim();
        let value = raw_value.trim();
        if key.is_empty() {
            return Err(ini_error(
                "ini",
                format!("line {} has an empty key", line_number + 1),
            ));
        }
        let Some(section_name) = section.as_deref() else {
            return Err(ini_error(
                "ini",
                format!("line {} has a key before its section", line_number + 1),
            ));
        };

        match section_name {
            "general" => match key {
                "volume_step" => {
                    ensure_unique(&mut seen, section_name, key)?;
                    config.volume_step = parse_decimal(value, "volume_step")?;
                }
                "volume_step_large" => {
                    ensure_unique(&mut seen, section_name, key)?;
                    config.volume_step_large = parse_decimal(value, "volume_step_large")?;
                }
                "overlay_duration_ms" => {
                    ensure_unique(&mut seen, section_name, key)?;
                    config.overlay_duration_ms = parse_decimal(value, "overlay_duration_ms")?;
                }
                "modifier" => {
                    ensure_unique(&mut seen, section_name, key)?;
                    config.modifier = parse_modifier(value)?;
                }
                "autostart" => {
                    ensure_unique(&mut seen, section_name, key)?;
                    config.autostart = parse_bool(value, "autostart")?;
                }
                _ => unknown_key(section_name, key),
            },
            "hotkeys" => match key {
                "volume_up" | "volume_down" | "volume_up_large" | "volume_down_large"
                | "toggle_mute" | "reset_50" | "open_mixer" | "open_menu" => {
                    ensure_unique(&mut seen, section_name, key)?;
                    let bindings = config
                        .hotkeys
                        .get_or_insert_with(|| HotkeyBindings::for_modifier(config.modifier));
                    match key {
                        "volume_up" => bindings.volume_up = value.to_owned(),
                        "volume_down" => bindings.volume_down = value.to_owned(),
                        "volume_up_large" => bindings.volume_up_large = value.to_owned(),
                        "volume_down_large" => bindings.volume_down_large = value.to_owned(),
                        "toggle_mute" => bindings.toggle_mute = value.to_owned(),
                        "reset_50" => bindings.reset_50 = value.to_owned(),
                        "open_mixer" => bindings.open_mixer = value.to_owned(),
                        "open_menu" => bindings.open_menu = value.to_owned(),
                        _ => unreachable!("matched hotkey key above"),
                    }
                }
                _ => unknown_key(section_name, key),
            },
            "appearance" => match key {
                "theme" => {
                    ensure_unique(&mut seen, section_name, key)?;
                    config.appearance.theme = parse_theme(value)?;
                }
                "material" => {
                    ensure_unique(&mut seen, section_name, key)?;
                    config.appearance.material = parse_material(value)?;
                }
                "motion" => {
                    ensure_unique(&mut seen, section_name, key)?;
                    config.appearance.motion = parse_motion(value)?;
                }
                "accent" => {
                    ensure_unique(&mut seen, section_name, key)?;
                    config.appearance.accent = parse_accent(value)?;
                }
                _ => unknown_key(section_name, key),
            },
            "feedback" => match key {
                "enabled" => {
                    ensure_unique(&mut seen, section_name, key)?;
                    config.beep.enabled = parse_bool(value, "beep.enabled")?;
                }
                "blocked_freq" => {
                    ensure_unique(&mut seen, section_name, key)?;
                    config.beep.blocked_freq = parse_decimal(value, "beep.blocked_freq")?;
                }
                "blocked_duration_ms" => {
                    ensure_unique(&mut seen, section_name, key)?;
                    config.beep.blocked_duration_ms =
                        parse_decimal(value, "beep.blocked_duration_ms")?;
                }
                "limit_freq" => {
                    ensure_unique(&mut seen, section_name, key)?;
                    config.beep.limit_freq = parse_decimal(value, "beep.limit_freq")?;
                }
                "limit_duration_ms" => {
                    ensure_unique(&mut seen, section_name, key)?;
                    config.beep.limit_duration_ms = parse_decimal(value, "beep.limit_duration_ms")?;
                }
                _ => unknown_key(section_name, key),
            },
            "color_thresholds" => match key {
                "green_up_to" => {
                    ensure_unique(&mut seen, section_name, key)?;
                    config.color_thresholds.green_up_to =
                        parse_decimal(value, "color_thresholds.green_up_to")?;
                }
                "blue_up_to" => {
                    ensure_unique(&mut seen, section_name, key)?;
                    config.color_thresholds.blue_up_to =
                        parse_decimal(value, "color_thresholds.blue_up_to")?;
                }
                "orange_up_to" => {
                    ensure_unique(&mut seen, section_name, key)?;
                    config.color_thresholds.orange_up_to =
                        parse_decimal(value, "color_thresholds.orange_up_to")?;
                }
                _ => unknown_key(section_name, key),
            },
            "blacklist" if key.starts_with("item.") => {
                let suffix = key.strip_prefix("item.").unwrap_or_default();
                let index = parse_decimal::<u64>(suffix, "blacklist.item")?;
                if !blacklist_indexes.insert(index) {
                    return Err(ini_error(
                        "blacklist.item",
                        format!("duplicate numeric item index {index}"),
                    ));
                }
                blacklist_items.push((index, value.to_owned()));
            }
            "blacklist" => unknown_key(section_name, key),
            _ => unknown_key(section_name, key),
        }
    }

    blacklist_items.sort_by_key(|(index, _)| *index);
    config.blacklist = blacklist_items
        .into_iter()
        .map(|(_, value)| value)
        .collect();
    config::validate(&config).map_err(ConfigError::Validation)?;
    Ok(config::normalize(config))
}

/// Serialize a validated [`Config`] using the stable five-section schema.
pub fn serialize_ini(config: &Config) -> Result<String, ConfigError> {
    config::validate(config).map_err(ConfigError::Validation)?;
    for (index, item) in config.blacklist.iter().enumerate() {
        if item.contains(['\r', '\n']) {
            return Err(ini_error(
                "blacklist",
                format!("item {index} contains a line break"),
            ));
        }
    }

    let mut output = String::new();
    output.push_str("[general]\n");
    output.push_str(&format!("volume_step={}\n", config.volume_step));
    output.push_str(&format!("volume_step_large={}\n", config.volume_step_large));
    output.push_str(&format!(
        "overlay_duration_ms={}\n",
        config.overlay_duration_ms
    ));
    output.push_str(&format!("modifier={}\n", modifier_name(config.modifier)));
    output.push_str(&format!("autostart={}\n", config.autostart));

    let default_hotkeys = HotkeyBindings::default();
    let hotkeys = config.hotkeys.as_ref().unwrap_or(&default_hotkeys);
    output.push_str("\n[hotkeys]\n");
    output.push_str(&format!("volume_up={}\n", hotkeys.volume_up));
    output.push_str(&format!("volume_down={}\n", hotkeys.volume_down));
    output.push_str(&format!("volume_up_large={}\n", hotkeys.volume_up_large));
    output.push_str(&format!(
        "volume_down_large={}\n",
        hotkeys.volume_down_large
    ));
    output.push_str(&format!("toggle_mute={}\n", hotkeys.toggle_mute));
    output.push_str(&format!("reset_50={}\n", hotkeys.reset_50));
    output.push_str(&format!("open_mixer={}\n", hotkeys.open_mixer));
    output.push_str(&format!("open_menu={}\n", hotkeys.open_menu));

    output.push_str("\n[appearance]\n");
    output.push_str(&format!("theme={}\n", theme_name(config.appearance.theme)));
    output.push_str(&format!(
        "material={}\n",
        material_name(config.appearance.material)
    ));
    output.push_str(&format!(
        "motion={}\n",
        motion_name(config.appearance.motion)
    ));
    output.push_str(&format!(
        "accent={}\n",
        accent_name(config.appearance.accent)
    ));

    output.push_str("\n[feedback]\n");
    output.push_str(&format!("enabled={}\n", config.beep.enabled));
    output.push_str(&format!("blocked_freq={}\n", config.beep.blocked_freq));
    output.push_str(&format!(
        "blocked_duration_ms={}\n",
        config.beep.blocked_duration_ms
    ));
    output.push_str(&format!("limit_freq={}\n", config.beep.limit_freq));
    output.push_str(&format!(
        "limit_duration_ms={}\n",
        config.beep.limit_duration_ms
    ));

    output.push_str("\n[color_thresholds]\n");
    output.push_str(&format!(
        "green_up_to={}\n",
        config.color_thresholds.green_up_to
    ));
    output.push_str(&format!(
        "blue_up_to={}\n",
        config.color_thresholds.blue_up_to
    ));
    output.push_str(&format!(
        "orange_up_to={}\n",
        config.color_thresholds.orange_up_to
    ));

    output.push_str("\n[blacklist]\n");
    for (index, item) in config.blacklist.iter().enumerate() {
        output.push_str(&format!("item.{index}={item}\n"));
    }
    Ok(output)
}

/// Read and parse an INI configuration file.
pub fn load_ini(path: &Path) -> Result<Config, ConfigError> {
    let text = fs::read_to_string(path)?;
    parse_ini(&text)
}

/// Write a validated INI configuration using a sibling temporary file and atomic replacement.
pub fn save_ini_atomic(config: &Config, path: &Path) -> Result<(), ConfigError> {
    let text = serialize_ini(config)?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let temporary = temporary_path(path);

    let result = (|| -> io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(text.as_bytes())?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        replace_file(&temporary, path)
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(ConfigError::Io)
}

/// Migrate a validated legacy JSON configuration, without deleting the backup.
pub fn migrate_json_to_ini(
    json_path: &Path,
    ini_path: &Path,
) -> Result<MigrationOutcome, ConfigError> {
    if ini_path.exists() {
        return Ok(MigrationOutcome::AlreadyPresent);
    }
    if !json_path.exists() {
        return Ok(MigrationOutcome::NoLegacyFile);
    }

    let json = fs::read_to_string(json_path)?;
    let config: Config = serde_json::from_str(&json)?;
    config::validate(&config).map_err(ConfigError::Validation)?;
    save_ini_atomic(&config, ini_path)?;
    Ok(MigrationOutcome::Migrated)
}

fn ensure_unique(
    seen: &mut HashSet<(String, String)>,
    section: &str,
    key: &str,
) -> Result<(), ConfigError> {
    if !seen.insert((section.to_owned(), key.to_owned())) {
        return Err(ini_error("ini", format!("duplicate key [{section}] {key}")));
    }
    Ok(())
}

fn unknown_key(section: &str, key: &str) {
    log::warn!("ignoring unknown INI key [{section}] {key}");
}

fn ini_error(field: &'static str, message: impl Into<String>) -> ConfigError {
    ConfigError::Validation(ConfigValidationError {
        field,
        message: message.into(),
    })
}

fn parse_decimal<T>(value: &str, field: &'static str) -> Result<T, ConfigError>
where
    T: FromStr,
{
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ini_error(field, "must be a decimal integer"));
    }
    value
        .parse()
        .map_err(|_| ini_error(field, "is outside the supported range"))
}

fn parse_bool(value: &str, field: &'static str) -> Result<bool, ConfigError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(ini_error(field, "must be true or false")),
    }
}

fn parse_modifier(value: &str) -> Result<HotkeyModifier, ConfigError> {
    match value {
        "CtrlAlt" => Ok(HotkeyModifier::CtrlAlt),
        "Alt" => Ok(HotkeyModifier::Alt),
        "Ctrl" => Ok(HotkeyModifier::Ctrl),
        "CapsLock" => Ok(HotkeyModifier::CapsLock),
        _ => Err(ini_error("modifier", "is not a supported modifier")),
    }
}

fn parse_theme(value: &str) -> Result<ThemeMode, ConfigError> {
    match value {
        "System" => Ok(ThemeMode::System),
        "Light" => Ok(ThemeMode::Light),
        "Dark" => Ok(ThemeMode::Dark),
        _ => Err(ini_error("appearance.theme", "is not a supported theme")),
    }
}

fn parse_material(value: &str) -> Result<MaterialMode, ConfigError> {
    match value {
        "Auto" => Ok(MaterialMode::Auto),
        "Translucent" => Ok(MaterialMode::Translucent),
        "Opaque" => Ok(MaterialMode::Opaque),
        _ => Err(ini_error(
            "appearance.material",
            "is not a supported material",
        )),
    }
}

fn parse_motion(value: &str) -> Result<MotionMode, ConfigError> {
    match value {
        "Full" => Ok(MotionMode::Full),
        "Reduced" => Ok(MotionMode::Reduced),
        "Disabled" => Ok(MotionMode::Disabled),
        _ => Err(ini_error(
            "appearance.motion",
            "is not a supported motion mode",
        )),
    }
}

fn parse_accent(value: &str) -> Result<AccentMode, ConfigError> {
    match value {
        "System" => Ok(AccentMode::System),
        "Blue" => Ok(AccentMode::Blue),
        "Green" => Ok(AccentMode::Green),
        "Purple" => Ok(AccentMode::Purple),
        "Orange" => Ok(AccentMode::Orange),
        _ => Err(ini_error("appearance.accent", "is not a supported accent")),
    }
}

fn modifier_name(value: HotkeyModifier) -> &'static str {
    match value {
        HotkeyModifier::CtrlAlt => "CtrlAlt",
        HotkeyModifier::Alt => "Alt",
        HotkeyModifier::Ctrl => "Ctrl",
        HotkeyModifier::CapsLock => "CapsLock",
    }
}

fn theme_name(value: ThemeMode) -> &'static str {
    match value {
        ThemeMode::System => "System",
        ThemeMode::Light => "Light",
        ThemeMode::Dark => "Dark",
    }
}

fn material_name(value: MaterialMode) -> &'static str {
    match value {
        MaterialMode::Auto => "Auto",
        MaterialMode::Translucent => "Translucent",
        MaterialMode::Opaque => "Opaque",
    }
}

fn motion_name(value: MotionMode) -> &'static str {
    match value {
        MotionMode::Full => "Full",
        MotionMode::Reduced => "Reduced",
        MotionMode::Disabled => "Disabled",
    }
}

fn accent_name(value: AccentMode) -> &'static str {
    match value {
        AccentMode::System => "System",
        AccentMode::Blue => "Blue",
        AccentMode::Green => "Green",
        AccentMode::Purple => "Purple",
        AccentMode::Orange => "Orange",
    }
}

fn temporary_path(path: &Path) -> PathBuf {
    let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config.ini");
    path.with_file_name(format!(".{file_name}.tmp-{}-{counter}", std::process::id()))
}

#[cfg(not(target_os = "windows"))]
fn replace_file(temporary: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(temporary, destination)
}

#[cfg(target_os = "windows")]
fn replace_file(temporary: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::GetLastError;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, ReplaceFileW, MOVEFILE_REPLACE_EXISTING,
    };

    let temporary: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let replaced = unsafe {
        ReplaceFileW(
            destination.as_ptr(),
            temporary.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if replaced != 0 {
        return Ok(());
    }
    let moved = unsafe {
        MoveFileExW(
            temporary.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING,
        )
    };
    if moved != 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(
            unsafe { GetLastError() } as i32
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{
            AppearanceConfig, BeepConfig, ColorThresholds, Config, HotkeyBindings, HotkeyModifier,
        },
        ui::{AccentMode, MaterialMode, MotionMode, ThemeMode},
    };
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "volumectl-config-ini-{}-{}-{name}",
            std::process::id(),
            TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn full_config() -> Config {
        Config {
            volume_step: 3,
            volume_step_large: 17,
            overlay_duration_ms: 2_500,
            modifier: HotkeyModifier::Alt,
            autostart: true,
            hotkeys: Some(HotkeyBindings::for_modifier(HotkeyModifier::Alt)),
            blacklist: vec!["game.exe".into(), "chat.exe".into()],
            color_thresholds: ColorThresholds {
                green_up_to: 30,
                blue_up_to: 70,
                orange_up_to: 95,
            },
            beep: BeepConfig {
                enabled: false,
                blocked_freq: 500,
                blocked_duration_ms: 100,
                limit_freq: 700,
                limit_duration_ms: 120,
            },
            appearance: AppearanceConfig {
                theme: ThemeMode::Dark,
                material: MaterialMode::Opaque,
                motion: MotionMode::Reduced,
                accent: AccentMode::Purple,
            },
        }
    }

    #[test]
    fn full_schema_round_trips_to_equal_config() {
        let config = full_config();
        let expected = config::normalize(config.clone());

        let encoded = serialize_ini(&config).expect("serialize full INI config");
        let decoded = parse_ini(&encoded).expect("parse serialized INI config");

        // Parsing adopts the platform blacklist normalization contract.
        assert_eq!(decoded, expected);
    }

    #[test]
    fn custom_hotkey_bindings_round_trip_without_loss() {
        let mut config = full_config();
        config.hotkeys.as_mut().unwrap().volume_up = "Ctrl+Shift+KeyU".to_string();
        let expected = config::normalize(config.clone());

        let encoded = serialize_ini(&config).expect("serialize custom hotkeys");
        let decoded = parse_ini(&encoded).expect("parse custom hotkeys");

        assert_eq!(decoded, expected);
    }

    #[test]
    fn omitted_appearance_and_feedback_fields_use_typed_defaults() {
        let decoded = parse_ini(
            "[general]\nvolume_step=3\nvolume_step_large=10\noverlay_duration_ms=1800\nmodifier=CtrlAlt\n",
        )
        .expect("minimal INI config parses");

        assert_eq!(decoded.appearance, AppearanceConfig::default());
        assert_eq!(decoded.beep, BeepConfig::default());
        assert_eq!(decoded.color_thresholds, Config::default().color_thresholds);
        assert!(!decoded.autostart);
        assert!(decoded.blacklist.is_empty());
    }

    #[test]
    fn repeated_blacklist_items_are_sorted_by_numeric_suffix() {
        let decoded = parse_ini(
            "[general]\nvolume_step=1\nvolume_step_large=10\noverlay_duration_ms=1800\nmodifier=CtrlAlt\n\
[blacklist]\nitem.10=ten.exe\nitem.2=two.exe\nitem.1=one.exe\n",
        )
        .expect("blacklist items parse");

        let expected: Vec<_> = ["one.exe", "two.exe", "ten.exe"]
            .into_iter()
            .map(config::normalize_blacklist_entry)
            .collect();
        assert_eq!(decoded.blacklist, expected);
    }

    #[test]
    fn unknown_keys_and_sections_are_ignored() {
        let decoded = parse_ini(
            "[general]\nvolume_step=1\nvolume_step_large=10\noverlay_duration_ms=1800\nmodifier=CtrlAlt\nfuture=true\n\
[future_section]\nvalue=ignored\n",
        )
        .expect("unknown INI entries are ignored");

        assert_eq!(decoded, Config::default());
    }

    #[test]
    fn malformed_values_are_rejected_without_coercion() {
        let cases = [
            ("volume_step=not-a-number", "integer"),
            ("[feedback]\nenabled=1", "boolean"),
            ("[appearance]\ntheme=dark", "enum"),
        ];

        for (body, label) in cases {
            let text = format!(
                "[general]\nvolume_step=1\nvolume_step_large=10\noverlay_duration_ms=1800\nmodifier=CtrlAlt\n{body}\n"
            );
            assert!(
                parse_ini(&text).is_err(),
                "invalid {label} must be rejected"
            );
        }
    }

    #[test]
    fn malformed_lines_and_duplicate_scalar_keys_are_rejected() {
        let malformed = "[general]\nvolume_step=1\nthis is not a key\n";
        assert!(parse_ini(malformed).is_err());

        let duplicate = "[general]\nvolume_step=1\nvolume_step=2\n";
        assert!(parse_ini(duplicate).is_err());
    }

    #[test]
    fn modifier_derives_the_matching_legacy_hotkey_preset() {
        let config = parse_ini(
            "[general]\nvolume_step=1\nvolume_step_large=10\noverlay_duration_ms=1800\nmodifier=Alt\n",
        )
        .expect("modifier parses");

        assert_eq!(
            config.hotkeys,
            Some(HotkeyBindings::for_modifier(HotkeyModifier::Alt))
        );
    }

    #[test]
    fn atomic_save_preserves_directory_destination_and_cleans_temp_file() {
        let parent = temp_path("atomic-directory");
        fs::create_dir_all(&parent).expect("create temp parent");
        let destination = parent.join("config.ini");
        fs::create_dir(&destination).expect("create directory destination");

        let result = save_ini_atomic(&Config::default(), &destination);

        assert!(result.is_err());
        assert!(destination.is_dir());
        assert!(fs::read_dir(&parent)
            .expect("read temp parent")
            .filter_map(Result::ok)
            .all(|entry| entry.path() == destination));
        fs::remove_dir_all(parent).expect("remove temp parent");
    }

    #[test]
    fn atomic_save_replaces_existing_file_and_load_reads_it_back() {
        let parent = temp_path("atomic-replace");
        fs::create_dir_all(&parent).expect("create temp parent");
        let destination = parent.join("config.ini");
        fs::write(&destination, "old config").expect("seed destination");
        let config = full_config();
        let expected = config::normalize(config.clone());

        save_ini_atomic(&config, &destination).expect("replace destination atomically");

        assert_eq!(load_ini(&destination).expect("load saved INI"), expected);
        assert!(fs::read_dir(&parent)
            .expect("read temp parent")
            .filter_map(Result::ok)
            .all(|entry| entry.path() == destination));
        fs::remove_dir_all(parent).expect("remove temp parent");
    }

    #[test]
    fn json_migration_writes_ini_once_and_keeps_json_backup() {
        let parent = temp_path("migration");
        fs::create_dir_all(&parent).expect("create temp parent");
        let json_path = parent.join("config.json");
        let ini_path = parent.join("config.ini");
        let config = full_config();
        let expected = config::normalize(config.clone());
        let original_json = serde_json::to_string(&config).expect("serialize JSON fixture");
        fs::write(&json_path, &original_json).expect("write JSON fixture");

        assert_eq!(
            migrate_json_to_ini(&json_path, &ini_path).expect("migrate JSON"),
            MigrationOutcome::Migrated
        );
        assert_eq!(
            fs::read_to_string(&json_path).expect("read JSON backup"),
            original_json
        );
        assert_eq!(
            parse_ini(&fs::read_to_string(&ini_path).expect("read migrated INI")).unwrap(),
            expected
        );
        assert_eq!(
            migrate_json_to_ini(&json_path, &ini_path).expect("do not overwrite INI"),
            MigrationOutcome::AlreadyPresent
        );

        fs::remove_dir_all(parent).expect("remove temp parent");
    }

    #[test]
    fn json_migration_reports_missing_legacy_file() {
        let parent = temp_path("no-legacy");
        fs::create_dir_all(&parent).expect("create temp parent");

        assert_eq!(
            migrate_json_to_ini(&parent.join("config.json"), &parent.join("config.ini"))
                .expect("missing JSON is not an error"),
            MigrationOutcome::NoLegacyFile
        );

        fs::remove_dir_all(parent).expect("remove temp parent");
    }
}
