// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
fn attach_parent_console_if_needed() {
    use windows_sys::Win32::Storage::FileSystem::{GetFileType, FILE_TYPE_DISK, FILE_TYPE_PIPE};
    use windows_sys::Win32::System::Console::{
        AttachConsole, GetStdHandle, ATTACH_PARENT_PROCESS, STD_OUTPUT_HANDLE,
    };
    unsafe {
        let stdout_handle = GetStdHandle(STD_OUTPUT_HANDLE);
        let file_type = GetFileType(stdout_handle);
        if file_type != FILE_TYPE_DISK && file_type != FILE_TYPE_PIPE {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}

#[cfg(not(windows))]
fn attach_parent_console_if_needed() {}

/// Sole binary entry. On non-Windows platforms an explicit CLI command
/// (`get` / `set <0-100>` / `mute`) runs the one-shot utility; otherwise the
/// Tauri host starts (the GUI app on Windows, the hotkey+webview host on
/// Linux/macOS). On Windows the binary is GUI-only, as before.
fn main() -> std::process::ExitCode {
    let args = std::env::args().skip(1).collect::<Vec<_>>();

    // `--version` prints compiled version and exits immediately without config/audio loading.
    if args
        .iter()
        .any(|a| a == "--version" || a == "-v" || a == "-V")
    {
        attach_parent_console_if_needed();
        println!("VolumeControl {}", env!("CARGO_PKG_VERSION"));
        return std::process::ExitCode::SUCCESS;
    }

    // `--quit` requests deliberate exit of the running instance.
    if args.iter().any(|a| a == "--quit" || a == "-q") {
        attach_parent_console_if_needed();
        #[cfg(windows)]
        {
            let signaled = volumecontrol_tauri_lib::native_win32::request_exit_event();
            if signaled {
                println!("VolumeControl exit requested.");
            } else {
                println!("No running VolumeControl instance found.");
            }
            return std::process::ExitCode::SUCCESS;
        }
        #[cfg(not(windows))]
        {
            println!("VolumeControl exit requested via signal.");
            return std::process::ExitCode::SUCCESS;
        }
    }

    // `--background` is a host launch mode (the login autostart registration
    // uses it), not a CLI verb: the host must start tray-only instead of the
    // one-shot CLI exiting before any window can open.
    if !volumectl_lib::autostart::is_host_launch(&args) {
        attach_parent_console_if_needed();
        return match volumectl_lib::cli::run() {
            Ok(code) => code,
            Err(e) => {
                eprintln!("volumectl: {e}");
                std::process::ExitCode::FAILURE
            }
        };
    }
    volumecontrol_tauri_lib::run().expect("VolumeControl Tauri host failed to start");
    std::process::ExitCode::SUCCESS
}
