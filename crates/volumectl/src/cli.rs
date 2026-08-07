//! Non-Windows CLI fallback.
//!
//! Until the native macOS/Linux GUI host lands, the binary doubles as a simple
//! volume utility: `volumes get` / `set <0-100>` / `mute`. All audio access
//! routes through the shared [`AudioBackend`](crate::audio::AudioBackend)
//! contract via [`crate::audio::default_backend`], so each platform exercises
//! its real backend (PulseAudio on Linux, CoreAudio on macOS).

use std::process::ExitCode;

pub fn run() -> Result<ExitCode, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("get");

    let device = crate::audio::default_backend().map_err(|e| e.to_string())?;

    match cmd {
        "get" => {
            let state = device.get_state().map_err(|e| e.to_string())?;
            println!("{}%", state.percent());
            Ok(ExitCode::SUCCESS)
        }
        "set" => {
            let pct = args
                .get(1)
                .ok_or("usage: volumectl set <0-100>")?
                .parse::<i32>()
                .map_err(|_| "volume must be 0-100")?;
            device
                .set_volume(pct.clamp(0, 100) as f32 / 100.0)
                .map_err(|e| e.to_string())?;
            Ok(ExitCode::SUCCESS)
        }
        "mute" => {
            device.toggle_mute().map_err(|e| e.to_string())?;
            Ok(ExitCode::SUCCESS)
        }
        "autostart" => {
            let action = args.get(1).map(String::as_str).unwrap_or("status");
            match action {
                "on" => {
                    crate::autostart::set_enabled(true).map_err(|e| e.to_string())?;
                    println!("autostart: enabled");
                    Ok(ExitCode::SUCCESS)
                }
                "off" => {
                    crate::autostart::set_enabled(false).map_err(|e| e.to_string())?;
                    println!("autostart: disabled");
                    Ok(ExitCode::SUCCESS)
                }
                "status" => {
                    println!(
                        "autostart: {}",
                        if crate::autostart::is_enabled() {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    );
                    Ok(ExitCode::SUCCESS)
                }
                other => Err(format!(
                    "usage: volumectl autostart <on|off|status> (got '{other}')"
                )),
            }
        }
        other => Err(format!(
            "unknown command '{other}' (try: get, set <0-100>, mute, autostart <on|off|status>)"
        )),
    }
}
