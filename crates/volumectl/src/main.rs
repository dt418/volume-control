//! VolumeControl binary entry point.
//!
//! On Windows this runs the tray + overlay + hotkey app. On other platforms
//! it falls back to a small CLI (prints/controls volume) until the native
//! backends are completed on those OSes.

#[cfg(target_os = "windows")]
fn main() {
    volumectl_lib::init_logging();
    if let Err(e) = volumectl_lib::app::run() {
        log::error!("volumectl: {e}");
        std::process::exit(1);
    }
}

/// Linux with the GTK renderer: with no arguments the GUI host runs; explicit
/// CLI commands (`get` / `set <0-100>` / `mute`) go to the CLI utility even on
/// a GUI build. Without a display session the host reports the reason and
/// falls back to the CLI.
#[cfg(all(target_os = "linux", feature = "gtk-renderer"))]
fn main() -> std::process::ExitCode {
    volumectl_lib::init_logging();
    if std::env::args().nth(1).is_some() {
        return match volumectl_lib::cli::run() {
            Ok(code) => code,
            Err(e) => {
                eprintln!("volumectl: {e}");
                std::process::ExitCode::FAILURE
            }
        };
    }
    match volumectl_lib::linux_app::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("volumectl: GUI host unavailable ({e}); falling back to CLI");
            match volumectl_lib::cli::run() {
                Ok(code) => code,
                Err(cli_err) => {
                    eprintln!("volumectl: {cli_err}");
                    std::process::ExitCode::FAILURE
                }
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
#[cfg(not(all(target_os = "linux", feature = "gtk-renderer")))]
fn main() -> std::process::ExitCode {
    volumectl_lib::init_logging();
    match volumectl_lib::cli::run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("volumectl: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
