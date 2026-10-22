//! Tauri commands: the webview ↔ AppCore bridge. Every command returns
//! `Result<T, String>` so IPC failures surface as inline toasts in the UI.

use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};

use volumectl_lib::autostart::AutostartStatus;
use volumectl_lib::config::HotkeyModifier;
use volumectl_lib::host_core::{
    AppCore, AudioSessionInfo, BootstrapPayload, RunningApp, SettingsPatch,
};
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
        .and_then(|mut core| core.toggle_mute())
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
    app: tauri::AppHandle,
    core: State<'_, Arc<Mutex<AppCore>>>,
    patch: SettingsPatch,
) -> Result<(), String> {
    let language_patch = patch.language.is_some();
    let mut core = core.lock().map_err(|e| e.to_string())?;
    core.update_settings(patch)?;
    core.save_config()?;
    let language = core.config().language;
    drop(core);

    if language_patch {
        apply_language(&app, language);
    }
    Ok(())
}

/// Relabel every tray so a language change is visible without a restart.
pub(crate) fn apply_language(app: &tauri::AppHandle, language: volumectl_lib::config::Language) {
    #[cfg(target_os = "windows")]
    if let Some(native) = app.try_state::<Arc<crate::native_win32::NativeWin32>>() {
        native.apply_language(language);
    }
    #[cfg(not(target_os = "windows"))]
    if let Some(tray) = app.try_state::<crate::tauri_tray::TauriTray>() {
        tray.apply_language(language);
    }
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

/// Localized native window title for the current surface (Settings/Help).
/// The frontend knows the active language; the OS titlebar must follow it.
#[tauri::command]
pub fn set_window_title(window: WebviewWindow, title: String) -> Result<(), String> {
    window.set_title(&title).map_err(|error| error.to_string())
}

/// Ask the OS for notification authorization before the first alert.
///
/// The Tauri desktop notification plugin reports `granted` unconditionally on
/// desktop, so macOS needs the native `UNUserNotificationCenter` request;
/// Windows and Linux have no per-app permission prompt and return `true`.
#[tauri::command]
pub async fn request_notification_permission() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        return tauri::async_runtime::spawn_blocking(|| {
            use block2::RcBlock;
            use objc2_foundation::NSError;
            use objc2_user_notifications::{UNAuthorizationOptions, UNUserNotificationCenter};
            use std::sync::mpsc;

            let (tx, rx) = mpsc::channel();
            let handler = RcBlock::new(move |granted: bool, _error: *mut NSError| {
                let _ = tx.send(granted);
            });
            let center = UNUserNotificationCenter::currentNotificationCenter();
            center.requestAuthorizationWithOptions_completionHandler(
                UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
                &handler,
            );
            rx.recv_timeout(Duration::from_secs(30))
                .map_err(|_| "notification permission request timed out".to_string())
        })
        .await
        .map_err(|error| error.to_string())?;
    }
    #[cfg(not(target_os = "macos"))]
    Ok(true)
}

#[tauri::command]
pub fn running_apps() -> Result<Vec<RunningApp>, String> {
    Ok(volumectl_lib::host_core::running_apps())
}

#[tauri::command]
pub fn resolve_apps(processes: Vec<String>) -> Result<Vec<RunningApp>, String> {
    Ok(volumectl_lib::host_core::resolve_apps(&processes))
}

/// Open a trusted GitHub release page in the user's default browser.
///
/// Wry does not reliably forward `target="_blank"` popups on every desktop,
/// so the banner uses an explicit opener. The URL prefix check keeps this
/// command from becoming a generic external-command launcher.
#[tauri::command]
pub fn open_release(url: String) -> Result<(), String> {
    const RELEASE_PREFIX: &str = "https://github.com/dt418/volume-control/releases";
    let tail = url
        .strip_prefix(RELEASE_PREFIX)
        .ok_or("open_release: only VolumeControl release URLs are allowed")?;
    if !tail.is_empty() && !tail.starts_with('/') {
        return Err("open_release: only VolumeControl release URLs are allowed".into());
    }
    let mut command = if cfg!(target_os = "windows") {
        let mut command = std::process::Command::new("explorer");
        command.arg(&url);
        command
    } else if cfg!(target_os = "macos") {
        let mut command = std::process::Command::new("open");
        command.arg(&url);
        command
    } else {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(&url);
        command
    };
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("open_release failed: {error}"))
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
    core: State<'_, Arc<Mutex<AppCore>>>,
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
pub async fn open_surface(app: AppHandle, surface: String) -> Result<(), String> {
    let surface = SurfaceId::from_label(&surface).ok_or("unknown surface")?;
    let (tx, mut rx) = tauri::async_runtime::channel(1);
    let handle = app.clone();
    let handle_inner = handle.clone();
    handle
        .run_on_main_thread(move || {
            let result = handle_inner.state::<WindowManager>().open(surface);
            let _ = tx.blocking_send(result);
        })
        .map_err(|error| error.to_string())?;
    rx.recv()
        .await
        .ok_or_else(|| "window manager closed".to_string())?
}

#[tauri::command]
pub async fn close_surface(app: AppHandle, surface: String) -> Result<(), String> {
    let surface = SurfaceId::from_label(&surface).ok_or("unknown surface")?;
    let (tx, mut rx) = tauri::async_runtime::channel(1);
    let handle = app.clone();
    let handle_inner = handle.clone();
    handle
        .run_on_main_thread(move || {
            let result = handle_inner.state::<WindowManager>().close(surface);
            let _ = tx.blocking_send(result);
        })
        .map_err(|error| error.to_string())?;
    rx.recv()
        .await
        .ok_or_else(|| "window manager closed".to_string())?
}

#[tauri::command]
pub async fn surface_ready(app: AppHandle, window: WebviewWindow) -> Result<(), String> {
    let surface = SurfaceId::from_label(window.label()).ok_or("unknown surface")?;
    let (tx, mut rx) = tauri::async_runtime::channel(1);
    let handle = app.clone();
    let handle_inner = handle.clone();
    handle
        .run_on_main_thread(move || {
            let result = handle_inner.state::<WindowManager>().surface_ready(surface);
            let _ = tx.blocking_send(result);
        })
        .map_err(|error| error.to_string())?;
    rx.recv()
        .await
        .ok_or_else(|| "window manager closed".to_string())?
}
const UPDATE_CACHE_TTL: Duration = Duration::from_secs(10 * 60);
static UPDATE_CACHE: LazyLock<
    Mutex<
        Option<(
            Instant,
            volumectl_lib::config::Language,
            volumectl_lib::updates::UpdateInfo,
        )>,
    >,
> = LazyLock::new(|| Mutex::new(None));

fn cached_update(
    language: volumectl_lib::config::Language,
) -> Option<volumectl_lib::updates::UpdateInfo> {
    let guard = UPDATE_CACHE.lock().ok()?;
    let (checked_at, cached_language, info) = guard.as_ref()?;
    (checked_at.elapsed() < UPDATE_CACHE_TTL && *cached_language == language).then(|| info.clone())
}

fn store_update(
    language: volumectl_lib::config::Language,
    info: &volumectl_lib::updates::UpdateInfo,
) {
    if let Ok(mut guard) = UPDATE_CACHE.lock() {
        *guard = Some((Instant::now(), language, info.clone()));
    }
}

/// Check GitHub for the latest release. Automatic mixer checks share a
/// process-wide 10-minute cache so reopening the mixer cannot exhaust the
/// unauthenticated GitHub rate limit; the Settings button passes
/// `force: true` for an explicit refresh.
#[tauri::command]
pub async fn check_for_updates(
    app: AppHandle,
    core: State<'_, Arc<Mutex<AppCore>>>,
    force: Option<bool>,
) -> Result<volumectl_lib::updates::UpdateInfo, String> {
    let language = {
        let core = core.lock().map_err(|e| e.to_string())?;
        core.config().language
    };
    if !force.unwrap_or(false) {
        if let Some(info) = cached_update(language) {
            let _ = app.emit("state://update", &info);
            return Ok(info);
        }
    }

    let info = tauri::async_runtime::spawn_blocking(move || {
        volumectl_lib::updates::check_update_info(language)
    })
    .await
    .map_err(|e| e.to_string())??;

    store_update(language, &info);
    let _ = app.emit("state://update", &info);
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::SurfaceId;

    #[test]
    fn release_opener_rejects_non_release_urls() {
        assert!(super::open_release("https://example.com/v0.1.10".into()).is_err());
        assert!(super::open_release(
            "https://github.com/dt418/volume-control/releases-evil".into()
        )
        .is_err());
    }

    #[test]
    fn automatic_update_cache_is_language_scoped() {
        let info = volumectl_lib::updates::UpdateInfo {
            current_version: "0.1.10".into(),
            latest_version: Some("v0.1.11".into()),
            is_update_available: true,
            release_url: Some(
                "https://github.com/dt418/volume-control/releases/tag/v0.1.11".into(),
            ),
            message: "Update available".into(),
        };
        super::store_update(volumectl_lib::config::Language::En, &info);
        assert_eq!(
            super::cached_update(volumectl_lib::config::Language::En),
            Some(info.clone())
        );
        assert!(super::cached_update(volumectl_lib::config::Language::Vi).is_none());
    }

    /// The command layer resolves surface names through
    /// `SurfaceId::from_label`; these are the exact strings the webview
    /// surfaces and the AppCore sink pass in.
    #[test]
    fn command_surface_names_round_trip() {
        for surface in [
            SurfaceId::Mixer,
            SurfaceId::Settings,
            SurfaceId::Help,
            SurfaceId::Overlay,
        ] {
            let label = surface.label();
            assert_eq!(SurfaceId::from_label(label), Some(surface));
        }
        assert_eq!(SurfaceId::from_label("window-nope"), None);
    }
}
