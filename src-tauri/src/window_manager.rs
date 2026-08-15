use std::collections::HashSet;
use std::sync::Mutex;

use tauri::{
    AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalRect, PhysicalSize, Position,
    Size, WebviewUrl, WebviewWindowBuilder,
};

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

    pub fn title(&self) -> &'static str {
        match self {
            Self::Mixer => "Volume Mixer",
            Self::Settings => "VolumeControl Settings",
            Self::Help => "VolumeControl Help",
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

/// Legacy window geometry — logical design sizes of the native surfaces
/// (mixer 400x224, settings 760x620 min 620x520, help 520x500).
pub const MIXER_SIZE: (f64, f64) = (400.0, 224.0);
pub const SETTINGS_SIZE: (f64, f64) = (760.0, 620.0);
pub const SETTINGS_MIN_SIZE: (f64, f64) = (620.0, 520.0);
pub const HELP_SIZE: (f64, f64) = (520.0, 500.0);

/// Placement constants mirrored from the legacy native surfaces:
/// - the volume overlay is 88px tall at a 20px/40px margin from the
///   work-area right/bottom; the mixer shares its right edge and sits 16px
///   above its top (legacy `place_mixer_above_overlay`).
/// - Help sits at a 24px/48px margin (legacy `place_overlay` margins).
const OVERLAY_H: f64 = 88.0;
const OVERLAY_MARGIN_X: f64 = 20.0;
const OVERLAY_MARGIN_Y: f64 = 40.0;
const MIXER_OVERLAY_GAP: f64 = 16.0;
const HELP_MARGIN_X: f64 = 24.0;
const HELP_MARGIN_Y: f64 = 48.0;

/// Compute the physical target rect for a surface inside a monitor work
/// area, replicating the legacy native placements (bottom-right above the
/// overlay for the mixer, centered for settings, bottom-right for help).
/// Margins are physical px like the legacy `place_overlay`/`place_mixer_above_overlay`.
pub fn place_surface(
    surface: SurfaceId,
    work_area: PhysicalRect<i32, u32>,
    scale: f64,
) -> PhysicalRect<i32, u32> {
    let wa_x = work_area.position.x as f64;
    let wa_y = work_area.position.y as f64;
    let wa_w = work_area.size.width as f64;
    let wa_h = work_area.size.height as f64;

    let (left, top, width, height) = match surface {
        SurfaceId::Mixer => {
            let w = MIXER_SIZE.0 * scale;
            let h = MIXER_SIZE.1 * scale;
            // Overlay occupies the bottom-right corner; the mixer is above it.
            let overlay_h = OVERLAY_H * scale;
            let right = wa_x + wa_w - OVERLAY_MARGIN_X;
            let overlay_top = wa_y + wa_h - OVERLAY_MARGIN_Y - overlay_h;
            let bottom = overlay_top - MIXER_OVERLAY_GAP;
            (right - w, (bottom - h).max(wa_y), w, h)
        }
        SurfaceId::Settings => {
            let w = SETTINGS_SIZE.0 * scale;
            let h = SETTINGS_SIZE.1 * scale;
            // Centered, clamped so the surface never leaves the work area.
            let left = wa_x + ((wa_w - w) / 2.0).max(0.0);
            let top = wa_y + ((wa_h - h) / 2.0).max(0.0);
            (left, top, w, h)
        }
        SurfaceId::Help => {
            let w = HELP_SIZE.0 * scale;
            let h = HELP_SIZE.1 * scale;
            let left = wa_x + wa_w - HELP_MARGIN_X - w;
            let top = wa_y + wa_h - HELP_MARGIN_Y - h;
            (left, top, w, h)
        }
    };

    PhysicalRect {
        position: PhysicalPosition::new(left.round() as i32, top.round() as i32),
        size: PhysicalSize::new(width.round() as u32, height.round() as u32),
    }
}

/// Owns the lazy lifecycle of the three webview surface windows. Windows are
/// created on demand (`open`), destroyed on `close`, and never created in the
/// Tauri builder (the app starts with zero webviews to protect idle RAM).
pub struct WindowManager {
    app: AppHandle,
    active: Mutex<HashSet<SurfaceId>>,
    /// Thread that owns the Tauri runtime event loop. WebView2 windows must
    /// be created/destroyed on this thread (wry requirement), so every public
    /// surface operation marshals here when called from an IPC/poll thread.
    main_thread: std::thread::ThreadId,
}

impl WindowManager {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            active: Mutex::new(HashSet::new()),
            main_thread: std::thread::current().id(),
        }
    }

    pub fn is_open(&self, surface: SurfaceId) -> bool {
        self.active.lock().unwrap().contains(&surface)
    }

    pub fn open(&self, surface: SurfaceId) -> Result<(), String> {
        self.on_main(move |manager| manager.open_impl(surface))?
    }

    fn open_impl(&self, surface: SurfaceId) -> Result<(), String> {
        if self.is_open(surface) {
            if let Some(window) = self.app.get_webview_window(surface.label()) {
                if let Ok(Some(monitor)) = window.current_monitor() {
                    apply_placement(&window, surface, &monitor);
                } else if let Ok(Some(monitor)) = window.primary_monitor() {
                    apply_placement(&window, surface, &monitor);
                }
                window.show().map_err(|e| e.to_string())?;
                let _ = window.set_focus();
            }
            return Ok(());
        }
        log::debug!("window_manager: opening {surface:?} (new webview)");
        let mut builder = WebviewWindowBuilder::new(
            &self.app,
            surface.label(),
            WebviewUrl::App(surface.entry().into()),
        );
        #[cfg(feature = "e2e-wdio")]
        if let Some(script) = crate::debug_guest_bridge_script() {
            builder = builder.initialization_script(script);
        }
        let (w, h) = match surface {
            SurfaceId::Mixer => MIXER_SIZE,
            SurfaceId::Settings => SETTINGS_SIZE,
            SurfaceId::Help => HELP_SIZE,
        };
        builder = builder
            .title(surface.title())
            .inner_size(w, h)
            .visible(false);
        match surface {
            SurfaceId::Mixer => {
                builder = builder.decorations(false);
                // Tauri's transparent window builder API is not available on
                // macOS; the webview surface remains readable there through
                // its native material/background fallback.
                #[cfg(not(target_os = "macos"))]
                {
                    builder = builder.transparent(true);
                }
                builder = builder
                    .always_on_top(true)
                    .skip_taskbar(true)
                    .resizable(false);
            }
            SurfaceId::Settings => {
                builder = builder
                    .resizable(true)
                    .min_inner_size(SETTINGS_MIN_SIZE.0, SETTINGS_MIN_SIZE.1);
            }
            SurfaceId::Help => {
                builder = builder.resizable(false);
            }
        }
        let window = builder.build().map_err(|e| e.to_string())?;
        log::debug!(
            "window_manager: created {} url={:?}",
            surface.label(),
            window
                .url()
                .map(|url| url.to_string())
                .unwrap_or_else(|error| format!("<url error: {error}>"))
        );
        #[cfg(feature = "e2e-wdio")]
        if cfg!(debug_assertions) && std::env::var("VOLUMECTL_E2E_DEBUG").as_deref() == Ok("1") {
            log::info!(
                "E2E surface {} created with URL {}",
                surface.label(),
                window
                    .url()
                    .map(|url| url.to_string())
                    .unwrap_or_else(|error| format!("<url error: {error}>"))
            );
        }

        // Legacy placement parity: the monitor work area hosting the window
        // determines the surface position (mixer bottom-right above the
        // overlay, settings centered, help bottom-right). Fail-soft: on a
        // broken monitor query the window keeps its default position.
        if let Ok(Some(monitor)) = window.current_monitor() {
            apply_placement(&window, surface, &monitor);
        } else if let Ok(Some(monitor)) = window.primary_monitor() {
            apply_placement(&window, surface, &monitor);
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

    /// Reveal a surface after its frontend has applied the bootstrap
    /// appearance. Geometry is applied once more immediately before showing
    /// so the user never sees the OS default position or size.
    pub fn surface_ready(&self, surface: SurfaceId) -> Result<(), String> {
        self.on_main(move |manager| manager.surface_ready_impl(surface))?
    }

    fn surface_ready_impl(&self, surface: SurfaceId) -> Result<(), String> {
        let window = self
            .app
            .get_webview_window(surface.label())
            .ok_or_else(|| format!("surface {} is not open", surface.label()))?;
        log::debug!("window_manager: surface_ready for {surface:?}");

        if let Ok(Some(monitor)) = window.current_monitor() {
            apply_placement(&window, surface, &monitor);
        } else if let Ok(Some(monitor)) = window.primary_monitor() {
            apply_placement(&window, surface, &monitor);
        }
        window.show().map_err(|e| {
            log::warn!("window_manager: show {surface:?} failed: {e}");
            e.to_string()
        })?;
        let _ = window.set_focus();
        log::debug!("window_manager: open_impl {surface:?} done");
        Ok(())
    }

    pub fn close(&self, surface: SurfaceId) -> Result<(), String> {
        self.on_main(move |manager| manager.close_impl(surface))?
    }

    fn close_impl(&self, surface: SurfaceId) -> Result<(), String> {
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

    /// Run a window operation on the Tauri main thread.
    ///
    /// Tray menu events and the setup hook already run on the main thread and
    /// call through directly. IPC commands and the hotkey/wheel poll threads
    /// run elsewhere; wry/WebView2 requires window creation and destruction
    /// on the thread owning the event loop, so those calls are marshalled
    /// with `run_on_main_thread` and the caller blocks for the result.
    fn on_main<T: Send + 'static>(
        &self,
        operation: impl FnOnce(&WindowManager) -> T + Send + 'static,
    ) -> Result<T, String> {
        if std::thread::current().id() == self.main_thread {
            return Ok(operation(self));
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = self.app.clone();
        let handle_inner = handle.clone();
        handle
            .run_on_main_thread(move || {
                let manager = handle_inner.state::<WindowManager>();
                let _ = tx.send(operation(&manager));
            })
            .map_err(|error| error.to_string())?;
        rx.recv_timeout(std::time::Duration::from_secs(10))
            .map_err(|_| "window manager operation timed out".to_string())
    }
}

fn apply_placement(window: &tauri::WebviewWindow, surface: SurfaceId, monitor: &Monitor) {
    let rect = place_surface(surface, *monitor.work_area(), monitor.scale_factor());
    let _ = window.set_size(Size::Physical(rect.size));
    let _ = window.set_position(Position::Physical(rect.position));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wa(x: i32, y: i32, w: u32, h: u32) -> PhysicalRect<i32, u32> {
        PhysicalRect {
            position: PhysicalPosition::new(x, y),
            size: PhysicalSize::new(w, h),
        }
    }

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

    /// Mixer: bottom-right of the work area, sharing the overlay's right edge
    /// and sitting 16px above its top (legacy `place_mixer_above_overlay`).
    #[test]
    fn mixer_places_bottom_right_above_overlay() {
        // 2560x1400 work area at 100% scale (the legacy test work area).
        let rect = place_surface(SurfaceId::Mixer, wa(0, 0, 2560, 1400), 1.0);
        // Overlay right = 2560 - 20 = 2540; overlay top = 1400 - 40 - 88 = 1272;
        // mixer bottom = 1272 - 16 = 1256; top = 1256 - 224 = 1032.
        assert_eq!(rect.position, PhysicalPosition::new(2140, 1032));
        assert_eq!(rect.size, PhysicalSize::new(400, 224));
    }

    /// Mixer at 150% DPI: sizes and the overlay stack scale with `scale`,
    /// margins stay physical px.
    #[test]
    fn mixer_scales_with_dpi() {
        let rect = place_surface(SurfaceId::Mixer, wa(0, 0, 3840, 2100), 1.5);
        assert_eq!(rect.size, PhysicalSize::new(600, 336));
        // Overlay right = 3840 - 20 = 3820; overlay top = 2100 - 40 - 132 = 1928;
        // mixer bottom = 1928 - 16 = 1912; top = 1912 - 336 = 1576.
        assert_eq!(rect.position, PhysicalPosition::new(3220, 1576));
    }

    #[test]
    fn mixer_clamps_to_work_area_when_stack_is_taller_than_work_area() {
        let rect = place_surface(SurfaceId::Mixer, wa(0, 0, 800, 200), 1.0);
        assert_eq!(rect.position.y, 0);
        assert_eq!(rect.size, PhysicalSize::new(400, 224));
    }

    /// Settings: centered in the work area, clamped when larger than it.
    #[test]
    fn settings_centers_in_work_area() {
        let rect = place_surface(SurfaceId::Settings, wa(0, 0, 2560, 1400), 1.0);
        assert_eq!(rect.size, PhysicalSize::new(760, 620));
        assert_eq!(rect.position, PhysicalPosition::new(900, 390));
    }

    #[test]
    fn settings_clamps_to_work_area_when_larger() {
        let rect = place_surface(SurfaceId::Settings, wa(0, 0, 800, 600), 1.0);
        assert_eq!(rect.size, PhysicalSize::new(760, 620));
        // Height 620 > 600: clamped to the work-area top.
        assert_eq!(rect.position, PhysicalPosition::new(20, 0));
    }

    /// Help: bottom-right at a 24/48px margin (legacy `place_overlay`).
    #[test]
    fn help_places_bottom_right() {
        let rect = place_surface(SurfaceId::Help, wa(0, 0, 2560, 1400), 1.0);
        assert_eq!(rect.size, PhysicalSize::new(520, 500));
        assert_eq!(rect.position, PhysicalPosition::new(2016, 852));
    }

    /// Negative-origin work areas (multi-monitor left of primary).
    #[test]
    fn placements_work_for_negative_origin_work_area() {
        let work = wa(-1920, 0, 1920, 1080);
        let mixer = place_surface(SurfaceId::Mixer, work, 1.0);
        // Right edge = -1920 + 1920 - 20 = -20; overlay top = 0 + 1080 - 40 - 88 = 952;
        // mixer bottom = 952 - 16 = 936; left = -20 - 400 = -420; top = 936 - 224 = 712.
        assert_eq!(mixer.position, PhysicalPosition::new(-420, 712));

        let settings = place_surface(SurfaceId::Settings, work, 1.0);
        assert_eq!(settings.position, PhysicalPosition::new(-1920 + 580, 230));
    }
}
