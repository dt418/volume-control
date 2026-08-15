//! Tauri commands: the webview ↔ AppCore bridge. Every command returns
//! `Result<T, String>` so IPC failures surface as inline toasts in the UI.

use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, State, WebviewWindow};

use volumectl_lib::autostart::AutostartStatus;
use volumectl_lib::config::HotkeyModifier;
use volumectl_lib::host_core::{AppCore, AudioSessionInfo, BootstrapPayload, SettingsPatch};
use volumectl_lib::ui::AppAction;

use crate::window_manager::{SurfaceId, WindowManager};

#[tauri::command]
pub fn get_bootstrap(core: State<'_, Arc<Mutex<AppCore>>>) -> Result<BootstrapPayload, String> {
    if cfg!(debug_assertions)
        && std::env::var("VOLUMECTL_E2E_BOOTSTRAP_FAILURE").as_deref() == Ok("1")
    {
        return Err("E2E bootstrap failure".to_string());
    }
    // A poisoned lock should not take the mixer surface down permanently. A
    // worker panic is already recorded by Rust's panic hook; recovering the
    // guard lets the read-only bootstrap path render a useful surface and
    // keeps the process alive for the user to retry.
    let mut core = core.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    Ok(core.bootstrap())
}

#[tauri::command]
pub fn adjust_volume(
    core: State<'_, Arc<Mutex<AppCore>>>,
    delta_percent: i16,
) -> Result<(), String> {
    core.lock()
        .map_err(|e| e.to_string())
        .map(|mut core| core.handle_action(AppAction::AdjustVolume { delta_percent }))
}

#[tauri::command]
pub fn set_volume(core: State<'_, Arc<Mutex<AppCore>>>, percent: u8) -> Result<(), String> {
    core.lock().map_err(|e| e.to_string()).map(|mut core| {
        core.handle_action(AppAction::SetVolumePercent {
            percent: percent as u16,
        })
    })
}

#[tauri::command]
pub fn toggle_mute(core: State<'_, Arc<Mutex<AppCore>>>) -> Result<(), String> {
    core.lock()
        .map_err(|e| e.to_string())
        .map(|mut core| core.handle_action(AppAction::ToggleMute))
}

#[tauri::command]
pub fn reset_volume(core: State<'_, Arc<Mutex<AppCore>>>) -> Result<(), String> {
    core.lock()
        .map_err(|e| e.to_string())
        .map(|mut core| core.handle_action(AppAction::ResetVolume))
}

#[tauri::command]
pub fn set_modifier(
    core: State<'_, Arc<Mutex<AppCore>>>,
    modifier: HotkeyModifier,
) -> Result<(), String> {
    core.lock()
        .map_err(|e| e.to_string())?
        .set_modifier(modifier)
}

/// Apply a partial settings patch (step sizes, appearance) and persist it.
/// Mutation happens in [`AppCore::update_settings`]; persistence re-adopts
/// the config (publishes state, refreshes appearance) on success.
#[tauri::command]
pub fn update_settings(
    core: State<'_, Arc<Mutex<AppCore>>>,
    patch: SettingsPatch,
) -> Result<(), String> {
    let mut core = core.lock().map_err(|e| e.to_string())?;
    core.update_settings(patch)?;
    core.save_config()
}

/// Read the current-user startup registration. This is deliberately separate
/// from AppCore's draft config because the registry is an immediate side
/// effect and must report the platform's read-back state.
#[tauri::command]
pub fn get_autostart() -> Result<AutostartStatus, String> {
    volumectl_lib::autostart::status()
}

/// Enable or disable startup registration and return the verified state.
#[tauri::command]
pub fn set_autostart(enabled: bool) -> Result<AutostartStatus, String> {
    volumectl_lib::autostart::set_enabled(enabled)
}

/// Read-only: the recommended blacklist presets for the current modifier
/// (feeds the Blacklist editor's "Apply Recommended" draft merge).
#[tauri::command]
pub fn recommended_blacklist(core: State<'_, Arc<Mutex<AppCore>>>) -> Result<Vec<String>, String> {
    core.lock()
        .map_err(|e| e.to_string())
        .map(|core| core.recommended_blacklist())
}

/// Read-only: the canonical on-disk INI config path for the Storage section.
#[tauri::command]
pub fn config_path() -> Result<String, String> {
    Ok(volumectl_lib::config::config_path()
        .to_string_lossy()
        .into_owned())
}

/// Open the config file in the default editor (the host shows the
/// "Editing config — changes reload automatically" overlay).
#[tauri::command]
pub fn open_config_location(core: State<'_, Arc<Mutex<AppCore>>>) -> Result<(), String> {
    core.lock()
        .map_err(|e| e.to_string())
        .map(|mut core| core.handle_action(AppAction::OpenConfigLocation))
}

#[tauri::command]
pub fn save_config(core: State<'_, Arc<Mutex<AppCore>>>) -> Result<(), String> {
    core.lock().map_err(|e| e.to_string())?.save_config()
}

#[tauri::command]
pub fn get_audio_sessions(
    core: State<'_, Arc<Mutex<AppCore>>>,
) -> Result<Vec<AudioSessionInfo>, String> {
    core.lock()
        .map_err(|e| e.to_string())
        .map(|mut core| core.sessions())
}

#[tauri::command]
pub fn set_session_volume(
    app: AppHandle,
    core: State<'_, Arc<Mutex<AppCore>>>,
    id: String,
    pct: u8,
) -> Result<(), String> {
    let mut core = core.lock().map_err(|e| e.to_string())?;
    let result = core.set_session_volume(&id, pct);
    if result.is_err() {
        // Spec §9.6: the session went stale mid-flight — push the fresh list
        // so the frontend drops the dead row instead of erroring the view.
        let sessions = core.sessions();
        let _ = app.emit("state://sessions", sessions);
    }
    result
}

#[tauri::command]
pub fn mute_session(
    app: AppHandle,
    core: State<'_, Mutex<AppCore>>,
    id: String,
) -> Result<(), String> {
    let mut core = core.lock().map_err(|e| e.to_string())?;
    let result = core.mute_session(&id);
    if result.is_err() {
        let sessions = core.sessions();
        let _ = app.emit("state://sessions", sessions);
    }
    result
}

#[tauri::command]
pub fn open_surface(
    window_manager: State<'_, WindowManager>,
    surface: String,
) -> Result<(), String> {
    let surface = SurfaceId::from_label(&surface).ok_or("unknown surface")?;
    window_manager.open(surface)
}

#[tauri::command]
pub fn close_surface(
    window_manager: State<'_, WindowManager>,
    surface: String,
) -> Result<(), String> {
    let surface = SurfaceId::from_label(&surface).ok_or("unknown surface")?;
    window_manager.close(surface)
}

#[tauri::command]
pub fn surface_ready(
    window: WebviewWindow,
    window_manager: State<'_, WindowManager>,
) -> Result<(), String> {
    let surface = SurfaceId::from_label(window.label()).ok_or("unknown surface")?;
    window_manager.surface_ready(surface)
}

#[cfg(test)]
mod tests {
    use super::SurfaceId;

    /// The command layer resolves surface names through
    /// `SurfaceId::from_label`; these are the exact strings the webview
    /// surfaces and the AppCore sink pass in.
    #[test]
    fn command_surface_names_round_trip() {
        for surface in [SurfaceId::Mixer, SurfaceId::Settings, SurfaceId::Help] {
            let label = surface.label();
            assert_eq!(SurfaceId::from_label(label), Some(surface));
        }
        assert_eq!(SurfaceId::from_label("window-overlay"), None);
    }
}
