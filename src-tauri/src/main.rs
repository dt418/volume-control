// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Sole binary entry. On non-Windows platforms an explicit CLI command
/// (`get` / `set <0-100>` / `mute`) runs the one-shot utility; otherwise the
/// Tauri host starts (the GUI app on Windows, the hotkey+webview host on
/// Linux/macOS). On Windows the binary is GUI-only, as before.
fn main() -> std::process::ExitCode {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    // `--background` is a host launch mode (the login autostart registration
    // uses it), not a CLI verb: the host must start tray-only instead of the
    // one-shot CLI exiting before any window can open.
    let host_launch = args.is_empty()
        || (args.len() == 1 && args[0] == volumectl_lib::autostart::BACKGROUND_FLAG);
    if !host_launch {
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
