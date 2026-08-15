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
    use super::{quote_command_path, AutostartStatus};
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
        if value_type != REG_SZ || byte_len == 0 || !byte_len.is_multiple_of(2) {
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
        if value_type != REG_SZ || byte_len == 0 || !byte_len.is_multiple_of(2) {
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
            let command = quote_command_path(&executable);
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

    #[cfg(test)]
    mod tests {
        use super::{command_matches_executable, command_token};
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

#[cfg(test)]
mod tests {
    use super::quote_command_path;
    use std::path::Path;

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
