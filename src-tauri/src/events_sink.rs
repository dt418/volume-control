//! Tauri implementation of the [`EventSink`] contract: pushes `state://*`
//! events to open webviews, routes surface open/close to the lazy
//! [`WindowManager`], and drives the native HUD overlay/tray on Windows.

use tauri::{AppHandle, Emitter, Manager};

use volumectl_lib::audio::VolumeState;
use volumectl_lib::config::Config;
use volumectl_lib::host_core::{AudioSessionInfo, BackendStatus, EventSink};
use volumectl_lib::hotkeys::HotkeyRegResult;

#[cfg(target_os = "windows")]
use std::sync::Arc;

#[cfg(not(target_os = "windows"))]
use crate::tauri_tray::TauriTray;
use crate::window_manager::{SurfaceId, WindowManager};

/// Build the `state://overlay` payload shared by every webview HUD consumer.
///
/// Pure function (no app handle) so the payload shape is unit-testable.
/// Compiled on Windows only under `cfg(test)` because the webview overlay
/// path itself is non-Windows; the payload-shape test still runs everywhere.
#[cfg(any(not(target_os = "windows"), test))]
fn overlay_payload(
    text: Option<String>,
    state: &VolumeState,
    config: &Config,
) -> serde_json::Value {
    serde_json::json!({
        "text": text,
        "pct": state.percent(),
        "muted": state.muted,
        "green_up_to": config.color_thresholds.green_up_to,
        "blue_up_to": config.color_thresholds.blue_up_to,
        "orange_up_to": config.color_thresholds.orange_up_to,
        "theme_resolved": volumectl_lib::host_core::resolved_theme_str(config.appearance.theme),
        "material": format!("{:?}", config.appearance.material),
        "motion": format!("{:?}", config.appearance.motion),
        "accent": format!("{:?}", config.appearance.accent),
    })
}

/// Bridges [`volumectl_lib::host_core::AppCore`] notifications to the Tauri
/// runtime. Constructed in the builder setup with the app handle.
pub struct TauriSink {
    app: AppHandle,
    #[cfg(not(target_os = "windows"))]
    overlay_seq: std::sync::Arc<std::sync::Mutex<u64>>,
}

impl TauriSink {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            #[cfg(not(target_os = "windows"))]
            overlay_seq: std::sync::Arc::new(std::sync::Mutex::new(0)),
        }
    }
}

impl EventSink for TauriSink {
    fn volume(&self, pct: u8, muted: bool) {
        let _ = self.app.emit(
            "state://volume",
            serde_json::json!({ "pct": pct, "muted": muted }),
        );
        // Keep the tray tooltip/menu volume display in sync.
        #[cfg(target_os = "windows")]
        if let Some(native) = self
            .app
            .try_state::<Arc<crate::native_win32::NativeWin32>>()
        {
            native.set_tray_volume(&VolumeState {
                volume: pct as f32 / 100.0,
                muted,
            });
        }
        #[cfg(not(target_os = "windows"))]
        if let Some(tray) = self.app.try_state::<TauriTray>() {
            tray.set_volume(&VolumeState {
                volume: pct as f32 / 100.0,
                muted,
            });
        }
    }

    fn hotkeys(&self, status: &[HotkeyRegResult]) {
        let _ = self.app.emit("state://hotkeys", status);
    }

    fn sessions(&self, sessions: &[AudioSessionInfo]) {
        let _ = self.app.emit("state://sessions", sessions);
    }

    fn backend(&self, status: &BackendStatus) {
        let _ = self.app.emit("state://backend", status);
    }

    fn overlay(&self, text: Option<String>, state: VolumeState, config: Config) {
        #[cfg(target_os = "windows")]
        if let Some(native) = self
            .app
            .try_state::<Arc<crate::native_win32::NativeWin32>>()
        {
            match text {
                None => native.show_overlay(&state, &config),
                Some(text) => native.show_overlay_text(&text, &config),
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let wm = self.app.state::<WindowManager>();
            if let Err(e) = wm.open(SurfaceId::Overlay) {
                log::warn!("overlay open failed: {e}");
                return;
            }
            let _ = self
                .app
                .emit("state://overlay", overlay_payload(text, &state, &config));
            let seq = {
                let mut s = self.overlay_seq.lock().unwrap_or_else(|p| p.into_inner());
                *s += 1;
                *s
            };
            let duration = config.overlay_duration_ms.clamp(200, 10_000);
            let handle = self.app.clone();
            let seq_arc = self.overlay_seq.clone();
            tauri::async_runtime::spawn(async move {
                tauri::async_runtime::sleep(std::time::Duration::from_millis(duration)).await;
                let current = seq_arc.lock().unwrap_or_else(|p| p.into_inner());
                if *current == seq {
                    let wm = handle.state::<WindowManager>();
                    if let Err(e) = wm.close(SurfaceId::Overlay) {
                        log::warn!("overlay auto-hide failed: {e}");
                    }
                }
            });
        }
    }

    fn show_tray_menu(&self) {
        #[cfg(target_os = "windows")]
        if let Some(native) = self
            .app
            .try_state::<Arc<crate::native_win32::NativeWin32>>()
        {
            native.show_tray_menu();
        }
        #[cfg(not(target_os = "windows"))]
        if let Some(tray) = self.app.try_state::<TauriTray>() {
            tray.show_menu();
        } else {
            log::debug!("tray menu unavailable on this platform");
        }
    }

    fn exit(&self) {
        log::info!("exiting via tray command");
        crate::mark_exit_requested();
        // Release native resources first: uninstall the wheel hook and destroy
        // the bridge window (legacy host did the same on exit).
        #[cfg(target_os = "windows")]
        if let Some(native) = self
            .app
            .try_state::<Arc<crate::native_win32::NativeWin32>>()
        {
            native.shutdown();
        }
        self.app.exit(0);
    }

    fn open_surface(&self, label: &str) {
        let Some(surface) = SurfaceId::from_label(label) else {
            log::warn!("open_surface: unknown label {label:?}");
            return;
        };
        if let Some(window_manager) = self.app.try_state::<WindowManager>() {
            if let Err(e) = window_manager.open(surface) {
                log::warn!("open_surface {label} failed: {e}");
            }
        }
    }

    fn close_surface(&self, label: &str) {
        let Some(surface) = SurfaceId::from_label(label) else {
            log::warn!("close_surface: unknown label {label:?}");
            return;
        };
        if let Some(window_manager) = self.app.try_state::<WindowManager>() {
            if let Err(e) = window_manager.close(surface) {
                log::warn!("close_surface {label} failed: {e}");
            }
        }
    }

    fn toggle_surface(&self, label: &str) {
        let Some(surface) = SurfaceId::from_label(label) else {
            log::warn!("toggle_surface: unknown label {label:?}");
            return;
        };
        if let Some(window_manager) = self.app.try_state::<WindowManager>() {
            if let Err(e) = window_manager.toggle(surface) {
                log::warn!("toggle_surface {label} failed: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::overlay_payload;

    #[test]
    fn overlay_payload_carries_state_and_thresholds() {
        let state = volumectl_lib::audio::VolumeState {
            volume: 0.42,
            muted: true,
        };
        let mut config = volumectl_lib::config::Config::default();
        config.appearance.theme = volumectl_lib::ui::ThemeMode::Dark;
        let payload = overlay_payload(None, &state, &config);
        assert_eq!(payload["pct"], 42);
        assert_eq!(payload["muted"], true);
        assert_eq!(payload["text"], serde_json::Value::Null);
        assert_eq!(payload["green_up_to"], config.color_thresholds.green_up_to);
        assert_eq!(payload["blue_up_to"], config.color_thresholds.blue_up_to);
        assert_eq!(
            payload["orange_up_to"],
            config.color_thresholds.orange_up_to
        );
        assert_eq!(
            payload["theme_resolved"], "dark",
            "the overlay payload must carry the Rust-resolved theme"
        );
        assert!(
            payload.get("theme").is_none(),
            "the raw config theme key must not leak into the payload"
        );
    }
}
