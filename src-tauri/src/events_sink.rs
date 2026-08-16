//! Tauri implementation of the [`EventSink`] contract: pushes `state://*`
//! events to open webviews, routes surface open/close to the lazy
//! [`WindowManager`], and drives the native HUD overlay/tray on Windows.

use tauri::{AppHandle, Emitter, Manager};

use volumectl_lib::audio::VolumeState;
use volumectl_lib::config::Config;
use volumectl_lib::host_core::{AudioSessionInfo, EventSink};
use volumectl_lib::hotkeys::HotkeyRegResult;

#[cfg(target_os = "windows")]
use std::sync::Arc;

#[cfg(not(target_os = "windows"))]
use crate::tauri_tray::TauriTray;
use crate::window_manager::{SurfaceId, WindowManager};

/// Bridges [`volumectl_lib::host_core::AppCore`] notifications to the Tauri
/// runtime. Constructed in the builder setup with the app handle.
pub struct TauriSink {
    app: AppHandle,
}

impl TauriSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
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
        let _ = (text, state, config); // no native overlay on Linux/macOS
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
