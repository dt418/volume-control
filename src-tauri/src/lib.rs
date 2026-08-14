use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::Manager;

use volumectl_lib::audio::AudioBackend;
use volumectl_lib::host_core::AppCore;

use commands::{
    adjust_volume, close_surface, config_path, get_audio_sessions, get_bootstrap, mute_session,
    open_config_location, open_surface, recommended_blacklist, reset_volume, save_config,
    set_modifier, set_session_volume, set_volume, surface_ready, toggle_mute, update_settings,
};
use events_sink::TauriSink;
use window_manager::{SurfaceId, WindowManager};

#[cfg(not(target_os = "windows"))]
pub mod native_headless;
#[cfg(target_os = "windows")]
mod native_win32;

mod commands;
mod events_sink;
mod window_manager;

pub fn builder() -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default().invoke_handler(tauri::generate_handler![
        get_bootstrap,
        adjust_volume,
        set_volume,
        toggle_mute,
        reset_volume,
        set_modifier,
        save_config,
        update_settings,
        recommended_blacklist,
        config_path,
        open_config_location,
        get_audio_sessions,
        set_session_volume,
        mute_session,
        open_surface,
        close_surface,
        surface_ready,
    ])
}

/// The hotkey channel poll cadence (keeps the first key press responsive).
const FAST_POLL_MS: u64 = 20;
/// The config-reload / tray / external-sync poll cadence (mirrors the old
/// host's 150 ms `WM_TIMER`).
const SLOW_POLL_MS: u64 = 150;

fn parse_verify_surface(value: &str) -> Result<Option<SurfaceId>, String> {
    if value.is_empty() {
        Ok(None)
    } else {
        SurfaceId::from_label(value)
            .map(Some)
            .ok_or_else(|| "unknown surface".to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() -> tauri::Result<()> {
    volumectl_lib::init_logging();
    tauri::Builder::default()
        .setup(|app| {
            // Prevent two instances (mirrors the legacy host's named-mutex
            // guard: a second instance would double-apply hotkeys/wheel and
            // create a second tray icon). The check runs before any native
            // surface or managed state is created.
            #[cfg(target_os = "windows")]
            if !native_win32::ensure_single_instance() {
                log::warn!("another VolumeControl instance is already running");
                std::process::exit(0);
            }

            let handle = app.handle().clone();
            app.manage(WindowManager::new(handle.clone()));

            // Native surfaces. Windows: HUD overlay + tray + wheel bridge.
            // Linux/macOS: none (the headless host is AppCore alone).
            #[cfg(target_os = "windows")]
            let native = {
                let native = Arc::new(native_win32::NativeWin32::new()?);
                app.manage(native.clone());
                native
            };

            let audio = create_audio_backend()?;
            let config = volumectl_lib::config::load();
            let modifier = config.modifier;
            let sink = Arc::new(TauriSink::new(handle));
            let core = AppCore::new(audio, config, modifier, sink)
                .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

            // AppCore is shared behind a Mutex so commands can mutate it and
            // the poll threads can drain hotkeys/wheel/config concurrently.
            let shared = Arc::new(Mutex::new(core));
            app.manage(shared.clone());

            // Diagnostic verification opens through the same WindowManager
            // path as production, but only after every command dependency is
            // managed so bootstrap can render a real surface.
            if let Ok(raw_surface) = std::env::var("VOLUMECTL_VERIFY_SURFACE") {
                if let Some(surface) = parse_verify_surface(&raw_surface)? {
                    app.state::<WindowManager>().open(surface)?;
                }
            }

            // Fast poll (20 ms): drain the global-hotkey channel and the
            // wheel-bridge channel into AppCore.apply_hotkey.
            let fast_shared = shared.clone();
            #[cfg(target_os = "windows")]
            let fast_native = native.clone();
            std::thread::spawn(move || loop {
                if let Ok(mut core) = fast_shared.lock() {
                    core.poll_hotkeys();
                    #[cfg(target_os = "windows")]
                    while let Some(action) = fast_native.try_recv_wheel() {
                        core.apply_hotkey(action);
                    }
                }
                std::thread::sleep(Duration::from_millis(FAST_POLL_MS));
            });

            // Slow poll (150 ms): live config reload (mtime watch), tray
            // menu commands, and the external audio-state sync (the reload
            // path re-reads and publishes the confirmed state).
            let slow_shared = shared.clone();
            #[cfg(target_os = "windows")]
            let slow_native = native.clone();
            std::thread::spawn(move || loop {
                if let Ok(mut core) = slow_shared.lock() {
                    core.reload_config_if_changed();
                    // External audio-state sync: volume changed outside the
                    // app (media keys, other apps) — keeps the tray tooltip
                    // and open webviews fresh (legacy 150 ms host timer).
                    core.sync_external_state();
                    #[cfg(target_os = "windows")]
                    while let Some(cmd) = slow_native.poll_tray() {
                        core.handle_action(volumectl_lib::host_core::tray_command_to_action(cmd));
                    }
                }
                std::thread::sleep(Duration::from_millis(SLOW_POLL_MS));
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_bootstrap,
            adjust_volume,
            set_volume,
            toggle_mute,
            reset_volume,
            set_modifier,
            save_config,
            update_settings,
            recommended_blacklist,
            config_path,
            open_config_location,
            get_audio_sessions,
            set_session_volume,
            mute_session,
            open_surface,
            close_surface,
            surface_ready,
        ])
        .run(tauri::generate_context!())
}

/// Construct the platform audio backend.
#[cfg(target_os = "windows")]
fn create_audio_backend() -> Result<Box<dyn AudioBackend>, String> {
    volumectl_lib::audio_windows::WindowsAudio::new()
        .map(|a| Box::new(a) as Box<dyn AudioBackend>)
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "linux")]
fn create_audio_backend() -> Result<Box<dyn AudioBackend>, String> {
    volumectl_lib::audio_linux::LinuxAudio::new()
        .map(|a| Box::new(a) as Box<dyn AudioBackend>)
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
fn create_audio_backend() -> Result<Box<dyn AudioBackend>, String> {
    volumectl_lib::audio_macos::MacAudio::new()
        .map(|a| Box::new(a) as Box<dyn AudioBackend>)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::window_manager::SurfaceId;

    #[test]
    fn verify_surface_parser_accepts_only_webview_labels() {
        assert_eq!(
            super::parse_verify_surface("window-mixer").unwrap(),
            Some(SurfaceId::Mixer)
        );
        assert_eq!(
            super::parse_verify_surface("window-settings").unwrap(),
            Some(SurfaceId::Settings)
        );
        assert_eq!(
            super::parse_verify_surface("window-help").unwrap(),
            Some(SurfaceId::Help)
        );
        assert_eq!(
            super::parse_verify_surface("window-overlay"),
            Err("unknown surface".to_string())
        );
        assert_eq!(super::parse_verify_surface(""), Ok(None));
    }
}
