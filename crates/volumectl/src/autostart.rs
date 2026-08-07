//! Launch-at-login registration using each OS's native mechanism.
//!
//! Best practice per platform, all per-user and admin-free:
//!
//! - **Windows**: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
//!   registry value (`REG_SZ`) — the same list Task Manager's "Startup apps"
//!   reads.
//! - **macOS**: a LaunchAgent plist with `RunAtLoad` in
//!   `~/Library/LaunchAgents`; `launchctl` activates it immediately when
//!   available, and the plist alone still starts the app at the next login.
//! - **Linux**: an XDG autostart `.desktop` entry in
//!   `~/.config/autostart` (or `$XDG_CONFIG_HOME/autostart`).
//!
//! Every operation is idempotent. The app refuses to register a development
//! build (an executable under a Cargo `target/` directory) so a dev binary
//! never takes over the user's autostart.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// Windows `Run` registry value name.
pub const RUN_VALUE_NAME: &str = "VolumeControl";
/// macOS LaunchAgent label and plist filename stem.
pub const LAUNCH_AGENT_LABEL: &str = "com.dt418.volumecontrol";
/// Linux XDG autostart entry filename.
pub const XDG_ENTRY_NAME: &str = "volume-control.desktop";

/// Autostart registration failures.
#[derive(Debug)]
pub enum AutostartError {
    /// The current executable is a Cargo development build; registering it
    /// would pollute the user's startup with a dev binary.
    DevBuild(String),
    Io(io::Error),
}

impl fmt::Display for AutostartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DevBuild(path) => write!(
                f,
                "refusing to autostart a development build ({path}); \
                 install the app instead"
            ),
            Self::Io(error) => write!(f, "autostart I/O failed: {error}"),
        }
    }
}

impl std::error::Error for AutostartError {}

impl From<io::Error> for AutostartError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// True when `exe` lives under a Cargo `target/` build directory.
pub fn is_dev_build(exe: &Path) -> bool {
    exe.components().any(|part| part.as_os_str() == "target")
}

/// The absolute path of the running executable, rejecting dev builds.
pub fn executable_path() -> Result<PathBuf, AutostartError> {
    let exe = std::env::current_exe()?;
    if is_dev_build(&exe) {
        return Err(AutostartError::DevBuild(exe.display().to_string()));
    }
    Ok(exe)
}

/// The quoted command line to register (Windows `Run` value / XDG `Exec`).
pub fn command_line() -> Result<String, AutostartError> {
    Ok(format!("\"{}\"", executable_path()?.display()))
}

/// Escape XML special characters in plist/desktop values.
pub fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// LaunchAgent plist content for `exe` (pure; unit-tested on every target).
pub fn launch_agent_plist(exe: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>ProcessType</key>
    <string>Interactive</string>
</dict>
</plist>
"#,
        label = LAUNCH_AGENT_LABEL,
        exe = xml_escape(&exe.display().to_string()),
    )
}

/// XDG autostart `.desktop` content for `command_line` (pure).
pub fn xdg_desktop_entry(command_line: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=VolumeControl\n\
         Comment=Native volume controller with global hotkeys\n\
         Exec={command_line}\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n\
         Hidden=false\n"
    )
}

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
        RegSetValueExW, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ,
    };

    /// HKCU Run key — the per-user startup list shown in Task Manager.
    pub(crate) const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    fn win32_error(code: u32) -> AutostartError {
        AutostartError::Io(io::Error::from_raw_os_error(code as i32))
    }

    pub fn set_enabled(enabled: bool) -> Result<(), AutostartError> {
        // Resolve the registration value first so a dev-build refusal never
        // leaves a registry key open.
        let registration = if enabled { Some(command_line()?) } else { None };

        let mut hkey = 0;
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide(RUN_KEY).as_ptr(),
                0,
                std::ptr::null(),
                0,
                KEY_SET_VALUE,
                std::ptr::null(),
                &mut hkey,
                std::ptr::null_mut(),
            )
        };
        if status != 0 {
            return Err(win32_error(status));
        }

        let result = match registration {
            Some(value) => {
                let value = wide(&value);
                unsafe {
                    RegSetValueExW(
                        hkey,
                        wide(RUN_VALUE_NAME).as_ptr(),
                        0,
                        REG_SZ,
                        value.as_ptr() as *const u8,
                        (value.len() * 2) as u32,
                    )
                }
            }
            None => unsafe { RegDeleteValueW(hkey, wide(RUN_VALUE_NAME).as_ptr()) },
        };
        unsafe { RegCloseKey(hkey) };

        if result != 0 {
            return Err(win32_error(result));
        }
        Ok(())
    }

    pub fn is_enabled() -> bool {
        let mut hkey = 0;
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                wide(RUN_KEY).as_ptr(),
                0,
                KEY_QUERY_VALUE,
                &mut hkey,
            )
        };
        if status != 0 {
            return false;
        }
        let query = unsafe {
            RegQueryValueExW(
                hkey,
                wide(RUN_VALUE_NAME).as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        unsafe { RegCloseKey(hkey) };
        query == 0
    }
}

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::*;
    use std::fs;

    pub(crate) fn launch_agents_dir() -> PathBuf {
        std::env::var("HOME")
            .map(|home| PathBuf::from(home).join("Library").join("LaunchAgents"))
            .unwrap_or_else(|_| PathBuf::from("."))
    }

    fn plist_path() -> PathBuf {
        launch_agents_dir().join(format!("{LAUNCH_AGENT_LABEL}.plist"))
    }

    pub fn set_enabled(enabled: bool) -> Result<(), AutostartError> {
        let plist = plist_path();
        if enabled {
            let exe = executable_path()?;
            if let Some(dir) = plist.parent() {
                fs::create_dir_all(dir)?;
            }
            fs::write(&plist, launch_agent_plist(&exe))?;
            // Activate immediately; `launchctl` may be unavailable (e.g. in a
            // sandbox), but the plist alone still starts the app at the next
            // login, so a failure here is only logged.
            let _ = std::process::Command::new("launchctl")
                .args(["load", "-w"])
                .arg(&plist)
                .status();
        } else {
            let _ = std::process::Command::new("launchctl")
                .arg("unload")
                .arg(&plist)
                .status();
            let _ = fs::remove_file(&plist);
        }
        Ok(())
    }

    pub fn is_enabled() -> bool {
        plist_path().exists()
    }
}

#[cfg(target_os = "linux")]
mod linux_impl {
    use super::*;
    use std::fs;

    pub(crate) fn xdg_autostart_dir() -> PathBuf {
        let base = std::env::var("XDG_CONFIG_HOME")
            .unwrap_or_else(|_| std::env::var("HOME").unwrap_or_else(|_| ".".into()) + "/.config");
        PathBuf::from(base).join("autostart")
    }

    fn entry_path() -> PathBuf {
        xdg_autostart_dir().join(XDG_ENTRY_NAME)
    }

    pub fn set_enabled(enabled: bool) -> Result<(), AutostartError> {
        let entry = entry_path();
        if enabled {
            if let Some(dir) = entry.parent() {
                fs::create_dir_all(dir)?;
            }
            fs::write(&entry, xdg_desktop_entry(&command_line()?))?;
        } else {
            let _ = fs::remove_file(&entry);
        }
        Ok(())
    }

    pub fn is_enabled() -> bool {
        entry_path().exists()
    }
}

#[cfg(target_os = "linux")]
pub use linux_impl::{is_enabled, set_enabled};
#[cfg(target_os = "macos")]
pub use macos_impl::{is_enabled, set_enabled};
#[cfg(target_os = "windows")]
pub use windows_impl::{is_enabled, set_enabled};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_build_detection_handles_windows_and_unix_layouts() {
        assert!(is_dev_build(Path::new(
            r"C:\work\volume-control\target\debug\volumectl.exe"
        )));
        assert!(is_dev_build(Path::new(
            "/home/user/volume-control/target/release/volumectl"
        )));
        assert!(!is_dev_build(Path::new(
            r"C:\Program Files\VolumeControl\volumectl.exe"
        )));
        assert!(!is_dev_build(Path::new("/usr/local/bin/volumectl")));
        assert!(!is_dev_build(Path::new(r"C:\volume-control\volumectl.exe")));
    }

    #[test]
    fn launch_agent_plist_marks_run_at_load_and_uses_the_label() {
        let plist = launch_agent_plist(Path::new(
            "/Applications/VolumeControl.app/Contents/MacOS/volumectl",
        ));
        assert!(plist.contains("<key>RunAtLoad</key>"));
        assert!(plist.contains("<string>com.dt418.volumecontrol</string>"));
        assert!(plist
            .contains("<string>/Applications/VolumeControl.app/Contents/MacOS/volumectl</string>"));
        assert!(plist.contains("<key>ProgramArguments</key>"));
    }

    #[test]
    fn launch_agent_plist_escapes_xml_special_characters() {
        let plist = launch_agent_plist(Path::new("/opt/App & Co/<dev>/volumectl"));
        assert!(plist.contains("/opt/App &amp; Co/&lt;dev&gt;/volumectl"));
        assert!(!plist.contains("<dev>"));
    }

    #[test]
    fn xdg_desktop_entry_is_a_valid_autostart_marker() {
        let entry = xdg_desktop_entry("\"C:\\Program Files\\VolumeControl\\volumectl.exe\"");
        assert!(entry.starts_with("[Desktop Entry]\n"));
        assert!(entry.contains("Type=Application\n"));
        assert!(entry.contains("Exec=\"C:\\Program Files\\VolumeControl\\volumectl.exe\""));
        assert!(entry.contains("X-GNOME-Autostart-enabled=true"));
        assert!(entry.contains("Hidden=false"));
        assert!(entry.contains("Terminal=false"));
    }

    #[test]
    fn xml_escape_covers_the_four_special_characters() {
        assert_eq!(xml_escape(r#"a&b<c>d"e"#), "a&amp;b&lt;c&gt;d&quot;e");
    }
}
