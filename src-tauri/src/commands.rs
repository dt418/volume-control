//! Tauri commands: the webview ↔ AppCore bridge. Every command returns
//! `Result<T, String>` so IPC failures surface as inline toasts in the UI.

use std::sync::Mutex;

use tauri::State;

use volumectl_lib::config::HotkeyModifier;
use volumectl_lib::host_core::{AppCore, AudioSessionInfo, BootstrapPayload};
use volumectl_lib::ui::AppAction;

use crate::window_manager::{SurfaceId, WindowManager};

#[tauri::command]
pub fn get_bootstrap(core: State<'_, Mutex<AppCore>>) -> Result<BootstrapPayload, String> {
    core.lock()
        .map_err(|e| e.to_string())
        .map(|mut core| core.bootstrap())
}

#[tauri::command]
pub fn adjust_volume(core: State<'_, Mutex<AppCore>>, delta_percent: i16) -> Result<(), String> {
    core.lock()
        .map_err(|e| e.to_string())
        .map(|mut core| core.handle_action(AppAction::AdjustVolume { delta_percent }))
}

#[tauri::command]
pub fn set_volume(core: State<'_, Mutex<AppCore>>, percent: u8) -> Result<(), String> {
    core.lock().map_err(|e| e.to_string()).map(|mut core| {
        core.handle_action(AppAction::SetVolumePercent {
            percent: percent as u16,
        })
    })
}

#[tauri::command]
pub fn toggle_mute(core: State<'_, Mutex<AppCore>>) -> Result<(), String> {
    core.lock()
        .map_err(|e| e.to_string())
        .map(|mut core| core.handle_action(AppAction::ToggleMute))
}

#[tauri::command]
pub fn reset_volume(core: State<'_, Mutex<AppCore>>) -> Result<(), String> {
    core.lock()
        .map_err(|e| e.to_string())
        .map(|mut core| core.handle_action(AppAction::ResetVolume))
}

#[tauri::command]
pub fn set_modifier(
    core: State<'_, Mutex<AppCore>>,
    modifier: HotkeyModifier,
) -> Result<(), String> {
    core.lock()
        .map_err(|e| e.to_string())?
        .set_modifier(modifier)
}

#[tauri::command]
pub fn save_config(core: State<'_, Mutex<AppCore>>) -> Result<(), String> {
    core.lock().map_err(|e| e.to_string())?.save_config()
}

#[tauri::command]
pub fn get_audio_sessions(
    core: State<'_, Mutex<AppCore>>,
) -> Result<Vec<AudioSessionInfo>, String> {
    core.lock()
        .map_err(|e| e.to_string())
        .map(|mut core| core.sessions())
}

#[tauri::command]
pub fn set_session_volume(
    core: State<'_, Mutex<AppCore>>,
    id: String,
    pct: u8,
) -> Result<(), String> {
    core.lock()
        .map_err(|e| e.to_string())?
        .set_session_volume(&id, pct)
}

#[tauri::command]
pub fn mute_session(core: State<'_, Mutex<AppCore>>, id: String) -> Result<(), String> {
    core.lock().map_err(|e| e.to_string())?.mute_session(&id)
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
