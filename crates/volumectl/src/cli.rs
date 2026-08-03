//! Non-Windows CLI fallback.
//!
//! Until the native macOS/Linux backends land, the binary doubles as a simple
//! volume utility: `volumes get` / `set <0-100>` / `mute`.

use std::process::ExitCode;

pub fn run() -> Result<ExitCode, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("get");

    // The `volumecontrol` crate provides a cross-platform default device.
    let device = volumecontrol::AudioDevice::from_default()
        .map_err(|e| format!("cannot open default audio device: {e}"))?;

    match cmd {
        "get" => {
            let vol = device
                .get_vol()
                .map_err(|e| format!("get_vol failed: {e}"))?;
            println!("{}%", (vol * 100.0).round() as i32);
            Ok(ExitCode::SUCCESS)
        }
        "set" => {
            let pct = args
                .get(1)
                .ok_or("usage: volumectl set <0-100>")?
                .parse::<i32>()
                .map_err(|_| "volume must be 0-100")?;
            device
                .set_vol(pct.clamp(0, 100) as f32 / 100.0)
                .map_err(|e| format!("set_vol failed: {e}"))?;
            Ok(ExitCode::SUCCESS)
        }
        other => Err(format!("unknown command '{other}' (try: get, set <0-100>)")),
    }
}
