//! Tauri implementation of the [`EventSink`] contract: pushes `state://*`
//! events to open webviews and routes surface open/close to the lazy
//! [`WindowManager`].

use tauri::{AppHandle, Emitter, Manager};

use volumectl_lib::host_core::{AudioSessionInfo, EventSink};
use volumectl_lib::hotkeys::HotkeyRegResult;

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
    }

    fn hotkeys(&self, status: &[HotkeyRegResult]) {
        let _ = self.app.emit("state://hotkeys", status);
    }

    fn sessions(&self, sessions: &[AudioSessionInfo]) {
        let _ = self.app.emit("state://sessions", sessions);
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
}
