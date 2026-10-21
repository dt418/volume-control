use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::Manager;

#[cfg(debug_assertions)]
use volumectl_lib::audio::E2eAudio;
use volumectl_lib::audio::{AudioBackend, UnavailableAudio};
use volumectl_lib::host_core::tray_command_to_action;
use volumectl_lib::host_core::AppCore;
use volumectl_lib::tray_common::TrayCommand;

use commands::{
    adjust_volume, check_for_updates, close_surface, config_path, get_audio_sessions,
    get_autostart, get_bootstrap, mute_session, open_config_location, open_surface,
    recommended_blacklist, reset_volume, save_config, set_autostart, set_modifier,
    set_session_volume, set_volume, surface_ready, toggle_mute, update_settings,
};
use events_sink::TauriSink;
use window_manager::{SurfaceId, WindowManager};

#[cfg(not(target_os = "windows"))]
pub mod native_headless;
#[cfg(target_os = "windows")]
mod native_win32;

mod commands;
mod events_sink;
#[cfg(not(target_os = "windows"))]
mod tauri_tray;
mod window_manager;

pub fn builder() -> tauri::Builder<tauri::Wry> {
    let builder = register_debug_plugins(tauri::Builder::default());
    install_menu_event_handler(builder).invoke_handler(tauri::generate_handler![
        get_bootstrap,
        get_autostart,
        adjust_volume,
        set_volume,
        toggle_mute,
        reset_volume,
        set_modifier,
        save_config,
        update_settings,
        set_autostart,
        recommended_blacklist,
        config_path,
        open_config_location,
        get_audio_sessions,
        set_session_volume,
        mute_session,
        open_surface,
        close_surface,
        surface_ready,
        check_for_updates,
    ])
}

/// Route native tray commands through Tauri's menu event bridge.
///
/// Tauri installs a process-wide `muda::MenuEvent` handler when its runtime
/// starts. That intentionally disables `muda::MenuEvent::receiver()`, which
/// made the previous background polling path silently see an empty channel.
/// Registering here keeps the tray callback on Tauri's supported event path
/// and dispatches into the same `AppCore` action handler as hotkeys/IPC.
fn install_menu_event_handler(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    #[cfg(target_os = "windows")]
    {
        builder.on_menu_event(|app, event| {
            let Some(command) = TrayCommand::from_menu_id(event.id().as_ref()) else {
                return;
            };
            dispatch_tray_command(app, command);
        })
    }

    #[cfg(not(target_os = "windows"))]
    {
        builder
    }
}

/// Shared tray-command dispatch (Windows global handler and the macOS/Linux
/// tray callback both route here).
fn dispatch_tray_command(app: &tauri::AppHandle, command: TrayCommand) {
    // The update check performs a blocking network fetch; run it off the
    // core lock so volume/hotkey handling stays responsive while it flies.
    if command == TrayCommand::CheckUpdates {
        let handle = app.clone();
        // Immediate feedback: the fetch takes seconds, so the overlay shows
        // the in-progress card before the result replaces it.
        if let Some(shared) = app.try_state::<Arc<Mutex<AppCore>>>() {
            let mut core = shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let in_progress =
                volumectl_lib::locale::strings(core.config().language).update_check_in_progress;
            core.show_overlay_text(in_progress.to_string());
        }
        std::thread::spawn(move || {
            let language = handle
                .try_state::<Arc<Mutex<AppCore>>>()
                .map(|shared| {
                    shared
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .config()
                        .language
                })
                .unwrap_or_default();
            let message = volumectl_lib::updates::check_for_updates(language);
            if let Some(shared) = handle.try_state::<Arc<Mutex<AppCore>>>() {
                let mut core = shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                core.show_overlay_text(message);
            }
        });
        return;
    }
    let Some(shared) = app.try_state::<Arc<Mutex<AppCore>>>() else {
        log::warn!(
            "tray command {:?} received before AppCore was managed",
            command
        );
        return;
    };
    log::debug!("tray command: {command:?}");
    let mut core = shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    core.handle_action(tray_command_to_action(command));
}

/// Enable test-only automation plugins only for an explicitly marked debug run.
///
/// The feature gates keep the optional crates out of the normal production
/// dependency graph. The environment marker is a second runtime boundary so a
/// developer cannot accidentally expose a WebDriver/Pilot endpoint by setting a
/// Cargo feature alone. Release builds always ignore the marker.
fn register_debug_plugins(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    if !cfg!(debug_assertions) || std::env::var("VOLUMECTL_E2E_DEBUG").as_deref() != Ok("1") {
        return builder;
    }

    #[cfg(any(feature = "e2e-wdio", feature = "e2e-pilot"))]
    let mut builder = builder;

    #[cfg(feature = "e2e-wdio")]
    {
        builder = builder.plugin(tauri_plugin_wdio::init());
        builder = builder.plugin(tauri_plugin_wdio_webdriver::init());
    }

    #[cfg(feature = "e2e-pilot")]
    {
        builder = builder.plugin(tauri_plugin_pilot::init());
    }

    builder
}

/// Install the tiny native-side compatibility bridge required by the
/// embedded WebDriver service. The official `@wdio/tauri-plugin` is still
/// injected into the debug frontend, but the initialization script makes the
/// original Tauri core discoverable before a page/module can run. This is
/// compiled only for the explicit E2E feature and runtime marker.
#[cfg(feature = "e2e-wdio")]
pub(crate) fn debug_guest_bridge_script() -> Option<&'static str> {
    if !cfg!(debug_assertions) || std::env::var("VOLUMECTL_E2E_DEBUG").as_deref() != Ok("1") {
        return None;
    }

    Some(
        r#"(() => {
  const tauri = globalThis.__TAURI__;
  if (tauri?.core?.invoke) {
    globalThis.__wdio_original_tauri__ ??= tauri;
    globalThis.__wdio_original_core__ ??= tauri.core;
  }
})();"#,
    )
}

/// The hotkey channel poll cadence (keeps the first key press responsive).
const FAST_POLL_MS: u64 = 20;
/// The config-reload / tray / external-sync poll cadence (mirrors the old
/// host's 150 ms `WM_TIMER`).
const SLOW_POLL_MS: u64 = 150;

/// True once the user explicitly asked to exit (tray "Exit VolumeControl").
///
/// Tauri's desktop runtime exits by default when the last webview surface is
/// closed. This app is a tray-resident host: closing the mixer/settings/help
/// surface must leave the tray alive. The flag distinguishes that implicit
/// teardown from a deliberate exit so `RunEvent::ExitRequested` can be
/// prevented only for the former.
static EXIT_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Mark that the process exit is intentional (tray exit command).
pub(crate) fn mark_exit_requested() {
    EXIT_REQUESTED.store(true, Ordering::SeqCst);
}

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
    install_menu_event_handler(register_debug_plugins(tauri::Builder::default()))
        .setup(|app| {
            // Startup stage trace (env-gated): `VOLUMECTL_STARTUP_TRACE=1`
            // logs per-stage elapsed milliseconds so startup impact is
            // measured, not guessed. The marks are cheap no-ops otherwise.
            let startup_trace = std::env::var("VOLUMECTL_STARTUP_TRACE").as_deref() == Ok("1");
            let startup_start = std::time::Instant::now();
            macro_rules! startup_mark {
                ($label:literal) => {
                    if startup_trace {
                        log::info!(
                            "startup: {:<22} {} ms",
                            $label,
                            startup_start.elapsed().as_millis()
                        );
                    }
                };
            }
            startup_mark!("setup begin");

            // Prevent two instances (mirrors the legacy host's named-mutex
            // guard: a second instance would double-apply hotkeys/wheel and
            // create a second tray icon). The check runs before any native
            // surface or managed state is created.
            #[cfg(target_os = "windows")]
            let debug_e2e_instance = cfg!(debug_assertions)
                && std::env::var("VOLUMECTL_E2E_DEBUG").as_deref() == Ok("1");
            #[cfg(target_os = "windows")]
            if !debug_e2e_instance && !native_win32::ensure_single_instance() {
                log::warn!("another VolumeControl instance is already running");
                std::process::exit(0);
            }
            startup_mark!("single instance");

            let handle = app.handle().clone();
            app.manage(WindowManager::new(handle.clone()));

            // Loaded before any surface so native trays build localized menus.
            let (config, config_notice) = volumectl_lib::config::load_with_notice();
            startup_mark!("config loaded");

            // Older autostart entries lack `--background`; without this an
            // upgraded user gets the mixer popping up at every sign-in.
            if let Err(error) = volumectl_lib::autostart::ensure_background_flag() {
                log::warn!("could not refresh the auto-start command: {error}");
            }

            // Tauri-managed tray (macOS/Linux only). Creation failure is
            // non-fatal: keep the host alive with the webview surfaces.
            #[cfg(not(target_os = "windows"))]
            match tauri_tray::TauriTray::create(&handle, config.language) {
                Ok(tray) => {
                    app.manage(tray);
                    log::info!("tray created");
                }
                Err(error) => log::warn!("tray unavailable; keeping host alive: {error}"),
            }

            // Native surfaces. Windows: HUD overlay + tray + wheel bridge.
            // Linux/macOS: none (the headless host is AppCore alone).
            #[cfg(target_os = "windows")]
            let native = {
                let native = Arc::new(native_win32::NativeWin32::new(config.language)?);
                app.manage(native.clone());
                native
            };
            startup_mark!("native surfaces");

            let audio = match create_audio_backend() {
                Ok(audio) => audio,
                Err(error) => {
                    log::warn!("audio backend unavailable; keeping host alive: {error}");
                    Box::new(UnavailableAudio::new(error)) as Box<dyn AudioBackend>
                }
            };
            let modifier = config.modifier;
            let sink = Arc::new(TauriSink::new(handle));
            let core = AppCore::new_with_notice(audio, config, modifier, sink, config_notice)
                .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            startup_mark!("audio + core");

            // AppCore is shared behind a Mutex so commands can mutate it and
            // the poll threads can drain hotkeys/wheel/config concurrently.
            let shared = Arc::new(Mutex::new(core));
            app.manage(shared.clone());
            startup_mark!("state managed");

            // A user launch (desktop/tray shortcut, Start menu, winget
            // `winget install` completion) must show something: the tray icon
            // alone sits in the overflow flyout, so clicking the icon looked
            // dead. The login autostart entry passes `--background` and stays
            // tray-only. The E2E verify marker owns its own surface choice.
            let background_launch = std::env::args()
                .skip(1)
                .any(|arg| arg == volumectl_lib::autostart::BACKGROUND_FLAG);
            if !background_launch && std::env::var("VOLUMECTL_VERIFY_SURFACE").is_err() {
                if let Err(error) = app.state::<WindowManager>().open(SurfaceId::Mixer) {
                    log::warn!("could not open the mixer on launch: {error}");
                }
            }

            // Diagnostic verification opens through the same WindowManager
            // path as production, but only after every command dependency is
            // managed so bootstrap can render a real surface.
            if let Ok(raw_surface) = std::env::var("VOLUMECTL_VERIFY_SURFACE") {
                if let Some(surface) = parse_verify_surface(&raw_surface)? {
                    log::info!("E2E verify marker opening {}", surface.label());
                    if let Err(error) = app.state::<WindowManager>().open(surface) {
                        log::error!(
                            "E2E verify marker could not open {}: {error}",
                            surface.label()
                        );
                        return Err(error.into());
                    }
                    log::info!("E2E verify marker opened {}", surface.label());
                }
            }

            // Fast poll (20 ms): drain the global-hotkey channel and the
            // wheel-bridge channel into AppCore.apply_hotkey.
            let fast_shared = shared.clone();
            #[cfg(target_os = "windows")]
            let fast_native = native.clone();
            std::thread::spawn(move || loop {
                // Never queue an IPC command behind a poll: when a command
                // holds the core (volume slider, bootstrap, tray action),
                // skip this cycle instead of blocking. Commands are the
                // interactive path — polls are best-effort by design.
                let Ok(mut core) = fast_shared.try_lock() else {
                    std::thread::sleep(Duration::from_millis(FAST_POLL_MS));
                    continue;
                };
                core.poll_hotkeys();
                #[cfg(target_os = "windows")]
                if native_win32::take_show_mixer_request() {
                    // A second launch (desktop icon while the tray host is
                    // already running) asked for the mixer through the named
                    // event: the process itself exits, so this is the only
                    // visible response.
                    core.handle_action(volumectl_lib::ui::AppAction::ShowSurface(
                        volumectl_lib::ui::SurfaceId::Mixer,
                    ));
                }
                #[cfg(target_os = "windows")]
                while let Some(action) = fast_native.try_recv_wheel() {
                    core.apply_hotkey(action);
                }
                drop(core);
                std::thread::sleep(Duration::from_millis(FAST_POLL_MS));
            });

            // Slow poll (150 ms): live config reload (mtime watch) and the
            // external audio-state sync (the reload path re-reads and
            // publishes the confirmed state). Tauri dispatches tray menu
            // commands through `on_menu_event` on the runtime event loop.
            let slow_shared = shared.clone();
            std::thread::spawn(move || loop {
                // Same non-blocking contract as the fast poll: the audio
                // probe (PulseAudio/CoreAudio/WASAPI get_state) can block on
                // a slow or lost device, and the 150 ms cadence must never
                // stall an in-flight command.
                let Ok(mut core) = slow_shared.try_lock() else {
                    std::thread::sleep(Duration::from_millis(SLOW_POLL_MS));
                    continue;
                };
                core.reload_config_if_changed();
                // External audio-state sync: volume changed outside the
                // app (media keys, other apps) — keeps the tray tooltip
                // and open webviews fresh (legacy 150 ms host timer).
                core.sync_external_state();
                drop(core);
                std::thread::sleep(Duration::from_millis(SLOW_POLL_MS));
            });

            startup_mark!("poll threads");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_bootstrap,
            get_autostart,
            adjust_volume,
            set_volume,
            toggle_mute,
            reset_volume,
            set_modifier,
            save_config,
            update_settings,
            set_autostart,
            recommended_blacklist,
            config_path,
            open_config_location,
            get_audio_sessions,
            set_session_volume,
            mute_session,
            open_surface,
            close_surface,
            surface_ready,
            check_for_updates,
        ])
        .build(tauri::generate_context!())?
        .run(|_app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if !EXIT_REQUESTED.load(Ordering::SeqCst) {
                    // All webview surfaces closed (or the OS asked to close);
                    // keep the tray host alive. The tray Exit command sets the
                    // flag first, so a deliberate exit still terminates.
                    log::debug!("all surfaces closed; keeping the tray host alive");
                    api.prevent_exit();
                }
            }
        });
    Ok(())
}

/// Construct the platform audio backend.
#[cfg(debug_assertions)]
fn debug_audio_backend() -> Option<Box<dyn AudioBackend>> {
    if std::env::var("VOLUMECTL_E2E_DEBUG").as_deref() == Ok("1")
        && std::env::var("VOLUMECTL_E2E_AUDIO").as_deref() == Ok("virtual")
    {
        return Some(Box::new(E2eAudio::new()));
    }
    None
}

#[cfg(not(debug_assertions))]
fn debug_audio_backend() -> Option<Box<dyn AudioBackend>> {
    None
}

#[cfg(target_os = "windows")]
fn create_audio_backend() -> Result<Box<dyn AudioBackend>, String> {
    if let Some(audio) = debug_audio_backend() {
        return Ok(audio);
    }
    volumectl_lib::audio_windows::WindowsAudio::new()
        .map(|a| Box::new(a) as Box<dyn AudioBackend>)
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "linux")]
fn create_audio_backend() -> Result<Box<dyn AudioBackend>, String> {
    if let Some(audio) = debug_audio_backend() {
        return Ok(audio);
    }
    volumectl_lib::audio_linux::LinuxAudio::new()
        .map(|a| Box::new(a) as Box<dyn AudioBackend>)
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
fn create_audio_backend() -> Result<Box<dyn AudioBackend>, String> {
    if let Some(audio) = debug_audio_backend() {
        return Ok(audio);
    }
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
            super::parse_verify_surface("window-overlay").unwrap(),
            Some(SurfaceId::Overlay)
        );
        assert_eq!(super::parse_verify_surface(""), Ok(None));
    }
}
