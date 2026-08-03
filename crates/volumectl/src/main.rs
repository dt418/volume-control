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

#[cfg(not(target_os = "windows"))]
fn main() {
    volumectl_lib::init_logging();
    let code = match volumectl_lib::cli::run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("volumectl: {e}");
            std::process::exit(1);
        }
    };
    std::process::exit(code.code().unwrap_or(0) as i32);
}
