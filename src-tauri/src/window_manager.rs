use std::collections::HashSet;
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceId {
    Mixer,
    Settings,
    Help,
}

impl SurfaceId {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Mixer => "window-mixer",
            Self::Settings => "window-settings",
            Self::Help => "window-help",
        }
    }

    /// Vite preserves the input path under `dist/`: `frontend/src/mixer/index.html`
    /// builds to `frontend/dist/src/mixer/index.html`.
    pub fn entry(&self) -> &'static str {
        match self {
            Self::Mixer => "src/mixer/index.html",
            Self::Settings => "src/settings/index.html",
            Self::Help => "src/help/index.html",
        }
    }

    pub fn all() -> [SurfaceId; 3] {
        [Self::Mixer, Self::Settings, Self::Help]
    }

    pub fn from_label(label: &str) -> Option<Self> {
        Self::all().into_iter().find(|s| s.label() == label)
    }
}

/// Owns the lazy lifecycle of the three webview surface windows. Windows are
/// created on demand (`open`), destroyed on `close`, and never created in the
/// Tauri builder (the app starts with zero webviews to protect idle RAM).
pub struct WindowManager {
    app: AppHandle,
    active: Mutex<HashSet<SurfaceId>>,
}

impl WindowManager {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            active: Mutex::new(HashSet::new()),
        }
    }

    pub fn is_open(&self, surface: SurfaceId) -> bool {
        self.active.lock().unwrap().contains(&surface)
    }

    pub fn open(&self, surface: SurfaceId) -> Result<(), String> {
        if self.is_open(surface) {
            return Ok(());
        }
        let mut builder = WebviewWindowBuilder::new(
            &self.app,
            surface.label(),
            WebviewUrl::App(surface.entry().into()),
        );
        match surface {
            SurfaceId::Mixer => {
                builder = builder
                    .inner_size(420.0, 580.0)
                    .decorations(false)
                    .transparent(true)
                    .always_on_top(true)
                    .skip_taskbar(true);
            }
            SurfaceId::Settings => {
                builder = builder.inner_size(680.0, 520.0).resizable(true);
            }
            SurfaceId::Help => {
                builder = builder.inner_size(520.0, 420.0).resizable(false);
            }
        }
        let window = builder.build().map_err(|e| e.to_string())?;
        // Take focus so the webview receives keyboard input (Esc close).
        // Fail-soft: some Wayland compositors may deny focus requests.
        if surface == SurfaceId::Mixer {
            let _ = window.set_focus();
        }
        let app_handle = self.app.clone();
        window.on_window_event(move |event| {
            // Mixer auto-closes on losing focus. Rust-level focus events are
            // reliable on Win32 and Wayland; JS blur is not (spec §9.3).
            if let tauri::WindowEvent::Focused(false) = event {
                if surface == SurfaceId::Mixer {
                    let _ = app_handle.emit("close_mixer_request", ());
                }
            }
            // The decorated settings/help windows can be destroyed by the
            // user's OS close button; drop the surface from `active` so it
            // can be reopened (a stale entry would make open() a permanent
            // no-op).
            if let tauri::WindowEvent::Destroyed = event {
                if let Some(wm) = app_handle.try_state::<WindowManager>() {
                    wm.active.lock().unwrap().remove(&surface);
                }
            }
        });
        self.active.lock().unwrap().insert(surface);
        Ok(())
    }

    pub fn close(&self, surface: SurfaceId) -> Result<(), String> {
        if let Some(window) = self.app.get_webview_window(surface.label()) {
            window.destroy().map_err(|e| e.to_string())?;
        }
        self.active.lock().unwrap().remove(&surface);
        Ok(())
    }

    /// Open the surface if it is closed, close it if it is open (the legacy
    /// mixer hotkey toggled; spec §5.1 keeps that behavior for the Mixer).
    pub fn toggle(&self, surface: SurfaceId) -> Result<(), String> {
        if self.is_open(surface) {
            self.close(surface)
        } else {
            self.open(surface)
        }
    }

    /// Part of the planned WindowManager API surface (used by host teardown
    /// in the full integration task); unused by the scaffold commands.
    #[allow(dead_code)]
    pub fn close_all(&self) -> Result<(), String> {
        for surface in SurfaceId::all() {
            self.close(surface)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_labels_and_entries_are_stable() {
        assert_eq!(SurfaceId::Mixer.label(), "window-mixer");
        assert_eq!(SurfaceId::Mixer.entry(), "src/mixer/index.html");
        assert_eq!(SurfaceId::Settings.label(), "window-settings");
        assert_eq!(SurfaceId::Settings.entry(), "src/settings/index.html");
        assert_eq!(SurfaceId::Help.label(), "window-help");
        assert_eq!(SurfaceId::Help.entry(), "src/help/index.html");
    }

    #[test]
    fn from_label_round_trips() {
        assert_eq!(
            SurfaceId::from_label("window-mixer"),
            Some(SurfaceId::Mixer)
        );
        assert_eq!(
            SurfaceId::from_label("window-settings"),
            Some(SurfaceId::Settings)
        );
        assert_eq!(SurfaceId::from_label("window-help"), Some(SurfaceId::Help));
        assert_eq!(SurfaceId::from_label("nope"), None);
    }

    #[test]
    fn all_covers_every_surface() {
        assert_eq!(SurfaceId::all().len(), 3);
        assert!(SurfaceId::all().contains(&SurfaceId::Mixer));
        assert!(SurfaceId::all().contains(&SurfaceId::Settings));
        assert!(SurfaceId::all().contains(&SurfaceId::Help));
    }
}
