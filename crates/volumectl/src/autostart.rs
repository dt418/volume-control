//! Current-user login autostart integration.
//!
//! Windows uses the per-user Run key so enabling this feature never needs
//! elevation and does not affect other users. Linux and macOS deliberately
//! return an explicit unsupported error until native adapters are approved;
//! callers must not silently claim that startup registration succeeded.

use serde::Serialize;
use std::path::Path;

#[cfg(not(windows))]
const UNSUPPORTED_ERROR: &str = "auto-start is unsupported on this platform";

/// The typed state exposed to the Settings surface and Tauri commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AutostartStatus {
    pub enabled: bool,
    pub command: Option<String>,
}

/// Host launch mode used by the login autostart entry: start tray-resident
/// without opening a surface. A user launch (desktop/tray shortcut) has no
/// argument and opens the mixer, so clicking the icon never looks dead.
pub const BACKGROUND_FLAG: &str = "--background";

/// True when any host argument requests the tray-only launch mode.
pub fn is_background_launch(args: &[String]) -> bool {
    args.iter().any(|arg| arg.as_str() == BACKGROUND_FLAG)
}

/// True when the argument list is a host launch rather than a CLI verb.
/// An empty list is a foreground launch; only background-flag tokens are
/// treated as host arguments, so `--background foo` stays a CLI invocation.
pub fn is_host_launch(args: &[String]) -> bool {
    args.is_empty() || args.iter().all(|arg| arg.as_str() == BACKGROUND_FLAG)
}

/// The Run-key command for the login entry, with the background flag.
pub fn startup_command(executable: &Path) -> String {
    format!("{} {BACKGROUND_FLAG}", quote_command_path(executable))
}

/// True when a Run-key command already carries the background launch flag.
pub fn has_background_flag(command: &str) -> bool {
    command
        .split_whitespace()
        .any(|token| token == BACKGROUND_FLAG)
}

/// Quote one executable path as a standalone Windows command-line argument.
///
/// This follows the Windows command-line escaping rules: embedded quotes are
/// escaped and backslashes immediately before a quote or the closing quote
/// are doubled. It is also deterministic on non-Windows hosts, which lets the
/// pure helper be tested in the regular Linux CI job.
pub fn quote_command_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');

    let mut backslashes = 0usize;
    for ch in text.chars() {
        match ch {
            '\\' => backslashes += 1,
            '"' => {
                quoted.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                quoted.push('"');
                backslashes = 0;
            }
            _ => {
                quoted.extend(std::iter::repeat_n('\\', backslashes));
                quoted.push(ch);
                backslashes = 0;
            }
        }
    }

    // Backslashes before the closing quote must be doubled so they remain
    // part of the path rather than escaping the delimiter.
    quoted.extend(std::iter::repeat_n('\\', backslashes * 2));
    quoted.push('"');
    quoted
}

/// Read the current user's startup registration.
#[cfg(not(windows))]
pub fn status() -> Result<AutostartStatus, String> {
    Err(UNSUPPORTED_ERROR.to_string())
}

/// Enable or disable the current user's startup registration.
#[cfg(not(windows))]
pub fn set_enabled(_enabled: bool) -> Result<AutostartStatus, String> {
    Err(UNSUPPORTED_ERROR.to_string())
}

#[cfg(windows)]
mod windows_registry {
    use super::{has_background_flag, startup_command, AutostartStatus};
    use std::{path::Path, ptr};
    use windows_sys::Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
        System::Registry::{
            RegCloseKey, RegCreateKeyW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
            RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_SZ,
        },
    };

    const RUN_SUBKEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    const RUN_VALUE: &str = "VolumeControl";

    struct RegistryKey(HKEY);

    impl Drop for RegistryKey {
        fn drop(&mut self) {
            unsafe {
                let _ = RegCloseKey(self.0);
            }
        }
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn registry_error(operation: &str, code: u32) -> String {
        format!("auto-start registry {operation} failed (Win32 error {code})")
    }

    fn open_run_key(access: u32) -> Result<Option<RegistryKey>, String> {
        let subkey = wide(RUN_SUBKEY);
        let mut raw = 0;
        let result =
            unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, access, &mut raw) };
        if result == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if result != ERROR_SUCCESS {
            return Err(registry_error("open", result));
        }
        Ok(Some(RegistryKey(raw)))
    }

    fn create_run_key() -> Result<RegistryKey, String> {
        let subkey = wide(RUN_SUBKEY);
        let mut raw = 0;
        let result = unsafe { RegCreateKeyW(HKEY_CURRENT_USER, subkey.as_ptr(), &mut raw) };
        if result != ERROR_SUCCESS {
            return Err(registry_error("create", result));
        }
        Ok(RegistryKey(raw))
    }

    fn open_or_create_run_key() -> Result<RegistryKey, String> {
        if let Some(key) = open_run_key(KEY_SET_VALUE)? {
            return Ok(key);
        }
        create_run_key()
    }

    fn read_run_value(key: &RegistryKey) -> Result<Option<String>, String> {
        let value_name = wide(RUN_VALUE);
        let mut value_type = 0;
        let mut byte_len = 0u32;
        let result = unsafe {
            RegQueryValueExW(
                key.0,
                value_name.as_ptr(),
                ptr::null(),
                &mut value_type,
                ptr::null_mut(),
                &mut byte_len,
            )
        };
        if result == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        if result != ERROR_SUCCESS {
            return Err(registry_error("query", result));
        }
        if value_type != REG_SZ || byte_len == 0 || byte_len % 2 != 0 {
            return Err("auto-start registry value is not a valid UTF-16 string".to_string());
        }

        let mut bytes = vec![0u8; byte_len as usize];
        let result = unsafe {
            RegQueryValueExW(
                key.0,
                value_name.as_ptr(),
                ptr::null(),
                &mut value_type,
                bytes.as_mut_ptr(),
                &mut byte_len,
            )
        };
        if result != ERROR_SUCCESS {
            return Err(registry_error("query", result));
        }
        if value_type != REG_SZ || byte_len == 0 || byte_len % 2 != 0 {
            return Err("auto-start registry value changed to an invalid type".to_string());
        }

        let mut units = bytes[..byte_len as usize]
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        if let Some(nul) = units.iter().position(|unit| *unit == 0) {
            if units[nul + 1..].iter().any(|unit| *unit != 0) {
                return Err("auto-start registry value contains embedded NUL data".to_string());
            }
            units.truncate(nul);
        }
        String::from_utf16(&units)
            .map(Some)
            .map_err(|_| "auto-start registry value contains invalid UTF-16".to_string())
    }

    fn command_token(command: &str) -> Option<&str> {
        let command = command.trim();
        if command.is_empty() {
            return None;
        }
        if let Some(rest) = command.strip_prefix('"') {
            return rest.split('"').next().filter(|value| !value.is_empty());
        }
        command.split_whitespace().next()
    }

    fn normalized_path(text: &str) -> String {
        text.replace('/', "\\")
            .trim_end_matches('\\')
            .to_ascii_lowercase()
    }

    fn command_matches_executable(command: &str, executable: &Path) -> bool {
        command_token(command)
            .map(|token| normalized_path(token) == normalized_path(&executable.to_string_lossy()))
            .unwrap_or(false)
    }

    fn should_migrate(command: &str, executable: &Path) -> bool {
        !has_background_flag(command) && command_matches_executable(command, executable)
    }

    pub(super) fn status() -> Result<AutostartStatus, String> {
        let Some(key) = open_run_key(KEY_READ)? else {
            return Ok(AutostartStatus {
                enabled: false,
                command: None,
            });
        };
        let Some(command) = read_run_value(&key)? else {
            return Ok(AutostartStatus {
                enabled: false,
                command: None,
            });
        };
        if command.trim().is_empty() {
            return Ok(AutostartStatus {
                enabled: false,
                command: None,
            });
        }

        let executable = std::env::current_exe()
            .map_err(|error| format!("could not determine current executable: {error}"))?;
        if command_matches_executable(&command, &executable) {
            Ok(AutostartStatus {
                enabled: true,
                command: Some(command),
            })
        } else {
            // Do not claim enabled for a stale or manually replaced command.
            Ok(AutostartStatus {
                enabled: false,
                command: None,
            })
        }
    }

    pub(super) fn set_enabled(enabled: bool) -> Result<AutostartStatus, String> {
        if enabled {
            let executable = std::env::current_exe()
                .map_err(|error| format!("could not determine current executable: {error}"))?;
            let command = startup_command(&executable);
            let key = open_or_create_run_key()?;
            let value_name = wide(RUN_VALUE);
            let value = wide(&command);
            let result = unsafe {
                RegSetValueExW(
                    key.0,
                    value_name.as_ptr(),
                    0,
                    REG_SZ,
                    value.as_ptr().cast(),
                    (value.len() * std::mem::size_of::<u16>()) as u32,
                )
            };
            if result != ERROR_SUCCESS {
                return Err(registry_error("write", result));
            }
        } else if let Some(key) = open_run_key(KEY_SET_VALUE)? {
            let value_name = wide(RUN_VALUE);
            let result = unsafe { RegDeleteValueW(key.0, value_name.as_ptr()) };
            if result != ERROR_SUCCESS && result != ERROR_FILE_NOT_FOUND {
                return Err(registry_error("delete", result));
            }
        }

        status()
    }

    /// Rewrite an existing login entry that predates the background flag so
    /// an upgraded user does not get the mixer popping up at every sign-in.
    /// Returns `true` only when this launch rewrote the value, which the host
    /// uses to keep the current login launch tray-only.
    pub(super) fn ensure_background_flag() -> Result<bool, String> {
        let Some(key) = open_run_key(KEY_READ)? else {
            return Ok(false);
        };
        let Some(command) = read_run_value(&key)? else {
            return Ok(false);
        };
        drop(key);
        let executable = std::env::current_exe()
            .map_err(|error| format!("could not determine current executable: {error}"))?;
        if !should_migrate(&command, &executable) {
            return Ok(false);
        }
        set_enabled(true)?;
        Ok(true)
    }

    #[cfg(test)]
    mod tests {
        use super::{command_matches_executable, command_token, should_migrate};
        use std::path::Path;

        #[test]
        fn command_token_handles_quoted_path_and_arguments() {
            assert_eq!(
                command_token(r#""C:\Program Files\VolumeControl.exe" --background"#),
                Some(r"C:\Program Files\VolumeControl.exe")
            );
            assert_eq!(
                command_token(r#"C:\VolumeControl.exe --background"#),
                Some(r"C:\VolumeControl.exe")
            );
        }

        #[test]
        fn command_matching_is_case_insensitive_and_ignores_arguments() {
            assert!(command_matches_executable(
                r#""c:\VOLUMECONTROL.EXE" --background"#,
                Path::new(r"C:\VolumeControl.exe"),
            ));
            assert!(!command_matches_executable(
                r#""C:\Other.exe""#,
                Path::new(r"C:\VolumeControl.exe"),
            ));
        }

        #[test]
        fn migration_targets_only_unflagged_matching_commands() {
            let executable = Path::new(r"C:\VolumeControl.exe");
            assert!(should_migrate(r#""C:\VolumeControl.exe""#, executable));
            assert!(!should_migrate(
                r#""C:\VolumeControl.exe" --background"#,
                executable
            ));
            assert!(!should_migrate(r#""C:\Other.exe""#, executable));
        }
    }
}

#[cfg(windows)]
pub fn status() -> Result<AutostartStatus, String> {
    windows_registry::status()
}

#[cfg(windows)]
pub fn set_enabled(enabled: bool) -> Result<AutostartStatus, String> {
    windows_registry::set_enabled(enabled)
}

/// Ensure an existing login entry launches tray-only. Non-Windows hosts have
/// no login registration, so this is a no-op there.
#[cfg(windows)]
pub fn ensure_background_flag() -> Result<bool, String> {
    windows_registry::ensure_background_flag()
}

#[cfg(not(windows))]
pub fn ensure_background_flag() -> Result<bool, String> {
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::{has_background_flag, quote_command_path, startup_command};
    use std::path::Path;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn launch_mode_accepts_only_known_host_arguments() {
        assert!(super::is_host_launch(&args(&[])));
        assert!(super::is_host_launch(&args(&["--background"])));
        assert!(super::is_host_launch(&args(&[
            "--background",
            "--background"
        ])));
        assert!(!super::is_host_launch(&args(&["get"])));
        assert!(!super::is_host_launch(&args(&["--background", "get"])));
    }

    #[test]
    fn background_launch_detects_the_flag_anywhere() {
        assert!(!super::is_background_launch(&args(&[])));
        assert!(super::is_background_launch(&args(&["--background"])));
        assert!(super::is_background_launch(&args(&[
            "--profile",
            "--background"
        ])));
        assert!(!super::is_background_launch(&args(&["--background-lite"])));
    }

    #[test]
    fn startup_command_appends_background_flag() {
        assert_eq!(
            startup_command(Path::new(r"C:\Program Files\VolumeControl.exe")),
            r#""C:\Program Files\VolumeControl.exe" --background"#,
        );
        assert!(has_background_flag(&startup_command(Path::new(
            r"C:\VolumeControl.exe"
        ))));
        assert!(!has_background_flag(r#""C:\VolumeControl.exe""#));
        assert!(!has_background_flag(
            r#""C:\VolumeControl.exe" --background-lite"#
        ));
    }

    #[test]
    fn quote_command_path_wraps_paths_with_spaces() {
        assert_eq!(
            quote_command_path(Path::new(r"C:\Program Files\VolumeControl.exe")),
            r#""C:\Program Files\VolumeControl.exe""#,
        );
    }

    #[test]
    fn quote_command_path_escapes_embedded_quotes() {
        assert_eq!(
            quote_command_path(Path::new(r#"C:\Volume"Control.exe"#)),
            r#""C:\Volume\"Control.exe""#,
        );
    }

    #[test]
    fn quote_command_path_doubles_trailing_backslashes() {
        assert_eq!(
            quote_command_path(Path::new(r"C:\VolumeControl\")),
            r#""C:\VolumeControl\\""#,
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_platforms_fail_closed() {
        let error = super::status().expect_err("Linux/macOS autostart is explicit unsupported");
        assert!(error.contains("unsupported"));
        let error = super::set_enabled(true).expect_err("unsupported enable must not succeed");
        assert!(error.contains("unsupported"));
    }
}
