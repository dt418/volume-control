use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::Manager;

use volumectl_lib::audio::AudioBackend;
use volumectl_lib::host_core::AppCore;

use commands::{
    adjust_volume, close_surface, get_audio_sessions, get_bootstrap, mute_session, open_surface,
    reset_volume, save_config, set_modifier, set_session_volume, set_volume, toggle_mute,
    update_settings,
};
use events_sink::TauriSink;
use window_manager::WindowManager;

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
        get_audio_sessions,
        set_session_volume,
        mute_session,
        open_surface,
        close_surface,
    ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() -> tauri::Result<()> {
    volumectl_lib::init_logging();
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            app.manage(WindowManager::new(handle.clone()));

            let audio = create_audio_backend()?;
            let config = volumectl_lib::config::load();
            let modifier = config.modifier;
            let sink = Arc::new(TauriSink::new(handle));
            let core = AppCore::new(audio, config, modifier, sink)
                .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

            // AppCore is shared behind a Mutex so commands can mutate it and
            // the poll thread can drain hotkeys concurrently.
            let shared = Arc::new(Mutex::new(core));
            app.manage(shared.clone());

            // Drain the global-hotkey channel on a background thread. The
            // channel is filled by the platform listener threads started
            // inside GlobalHotkeys::new.
            std::thread::spawn(move || loop {
                let _ = shared.lock().map(|mut core| core.poll_hotkeys());
                std::thread::sleep(Duration::from_millis(20));
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
            get_audio_sessions,
            set_session_volume,
            mute_session,
            open_surface,
            close_surface,
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
