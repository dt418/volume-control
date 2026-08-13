// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Sole binary entry. On non-Windows platforms an explicit CLI command
/// (`get` / `set <0-100>` / `mute`) runs the one-shot utility; otherwise the
/// Tauri host starts (the GUI app on Windows, the hotkey+webview host on
/// Linux/macOS). On Windows the binary is GUI-only, as before.
fn main() -> std::process::ExitCode {
    #[cfg(not(target_os = "windows"))]
    if std::env::args().nth(1).is_some() {
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
