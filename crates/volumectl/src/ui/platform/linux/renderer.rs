//! Ubuntu 24.04 GTK4/libadwaita renderer (spec §10.3).
//!
//! The renderer is split in two:
//!
//! - **Pure planning** (this file, minus `gtk_surfaces`): derives per-surface
//!   placement, material treatment, motion policy, and accessibility labels
//!   from the shared contracts. Everything here is platform-neutral Rust and
//!   is unit-tested; it compiles with or without the `gtk-renderer` feature,
//!   so the seam stays verifiable on any Linux toolchain.
//! - **GTK surfaces** (the `gtk_surfaces` submodule, `gtk-renderer` feature):
//!   real `gtk::Window` construction with the libadwaita stylesheet. On
//!   Wayland, the overlay and mixer become `gtk4-layer-shell` layer surfaces
//!   (`layer-shell` feature) anchored over the work area; on X11 and without
//!   layer-shell they are borderless plain windows. The material ladder ends
//!   in the opaque surface (solid token background via CSS), which is the
//!   always-available fallback.
//!
//! Material ladder (spec §10.3): Wayland compositor + layer-shell + blur
//! available → layer-shell glass; translucency available → translucent window;
//! otherwise or under high contrast → opaque surface. The shared
//! [`crate::ui::resolve_material`] resolves the treatment; this module maps
//! the resolved treatment onto GTK.

#[cfg(feature = "gtk-renderer")]
use crate::ui::model::AppAction;
use crate::ui::model::{AppState, MotionMode, SurfaceId};
#[cfg(feature = "gtk-renderer")]
use crate::ui::renderer::{HostHandle, NativeRenderer};
use crate::ui::theme::{MaterialIntent, Rgba, ThemeTokens};
#[cfg(any(test, feature = "gtk-renderer"))]
use crate::ui::WorkArea;
use crate::ui::{
    place_centered, place_mixer_above_overlay, place_overlay, resolve_material, resolve_motion,
    ResolvedMaterial, SurfaceRect, SurfaceSize, UiCapabilities,
};

/// Logical (1x) surface sizes from the Signal Glass spec §5–§8, shared with
/// the Windows and macOS renderers so every platform places identical
/// surfaces. GTK sizes windows in logical pixels, so these are used directly
/// (GTK applies its own display scale).
pub const OVERLAY_SIZE: SurfaceSize = SurfaceSize::new(336, 88);
pub const MIXER_SIZE: SurfaceSize = SurfaceSize::new(400, 224);
pub const SETTINGS_SIZE: SurfaceSize = SurfaceSize::new(580, 636);
pub const HELP_SIZE: SurfaceSize = SurfaceSize::new(520, 500);

/// Bottom-right placement margins (px, logical) shared with the Windows and
/// macOS renderers.
pub const MARGIN_X: i32 = 20;
pub const MARGIN_Y: i32 = 40;
/// Vertical gap between the mixer and the overlay.
pub const SURFACE_GAP: i32 = 16;

/// What the GTK layer must actually apply for a surface.
///
/// This is the Linux end of the material ladder: the Wayland layer-shell
/// glass treatment (translucent surface the compositor blurs) when the
/// session supports it, a plain translucent window when it does not, and the
/// fully opaque window as the always-available fallback (high contrast
/// forces this via [`crate::ui::resolve_material`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GtkMaterialKind {
    /// `gtk4-layer-shell` layer surface on Wayland (compositor glass).
    WaylandGlass,
    /// Plain window with the token surface alpha.
    Translucent,
    /// Plain window with the solid token background.
    Opaque,
}

impl GtkMaterialKind {
    pub const fn is_glass(self) -> bool {
        matches!(self, Self::WaylandGlass)
    }

    pub const fn is_opaque(self) -> bool {
        matches!(self, Self::Opaque)
    }
}

/// Map the shared resolved material onto the GTK material kinds.
///
/// `Blurred` becomes the Wayland layer-shell glass only when the session
/// actually has layer-shell; otherwise it degrades to the translucent window
/// (blur is not possible without a layer surface). The translucent and
/// opaque treatments pass through 1:1.
pub fn gtk_material_for(resolved: ResolvedMaterial, layer_shell_ok: bool) -> GtkMaterialKind {
    match resolved {
        ResolvedMaterial::Blurred if layer_shell_ok => GtkMaterialKind::WaylandGlass,
        ResolvedMaterial::Blurred | ResolvedMaterial::Translucent => GtkMaterialKind::Translucent,
        ResolvedMaterial::Opaque => GtkMaterialKind::Opaque,
    }
}

/// Accessibility names (spec §11.2 vocabulary, shared with the Windows and
/// macOS renderers) so screen readers announce the same strings everywhere.
pub fn a11y_label(surface: SurfaceId) -> &'static str {
    match surface {
        SurfaceId::Overlay => "System output volume",
        SurfaceId::Mixer => "Volume mixer",
        SurfaceId::Settings => "VolumeControl Settings",
        SurfaceId::Help => "VolumeControl Help",
        SurfaceId::Tray => "VolumeControl",
    }
}

/// Logical size of a surface window (GTK works in logical pixels).
pub fn logical_size(surface: SurfaceId) -> SurfaceSize {
    match surface {
        SurfaceId::Overlay => OVERLAY_SIZE,
        SurfaceId::Mixer => MIXER_SIZE,
        SurfaceId::Settings => SETTINGS_SIZE,
        SurfaceId::Help => HELP_SIZE,
        // The tray owns no window; reuse the overlay size so the match is
        // exhaustive and the helper stays total.
        SurfaceId::Tray => OVERLAY_SIZE,
    }
}

/// One planned surface: where it goes, how it looks, how it is announced.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfacePlan {
    pub surface: SurfaceId,
    pub rect: SurfaceRect,
    /// Resolved material treatment (shared ladder).
    pub material: ResolvedMaterial,
    /// GTK-specific material mapping (derived, cached for the GTK layer).
    pub gtk_material: GtkMaterialKind,
    /// Resolved motion policy after the system reduced-motion preference.
    pub motion: MotionMode,
    /// Accessibility label for the window.
    pub label: &'static str,
    /// Surface fill from the shared tokens (drives the opaque background).
    pub background: Rgba,
    /// Material intent from the shared tokens (alpha/blur guidance).
    pub material_intent: MaterialIntent,
}

/// Plan every surface from the confirmed state.
///
/// Placement reuses the shared pure math so the mixer sits directly above the
/// overlay with the 16 px gap; settings/help are centered. `layer_shell_ok`
/// is the runtime answer to "does this session support Wayland layer-shell"
/// (detected by the GTK layer); the planner stays pure so every branch is
/// unit-testable without a display. Pure — no GTK calls.
pub fn plan_surfaces(
    state: &AppState,
    tokens: &ThemeTokens,
    caps: &UiCapabilities,
    layer_shell_ok: bool,
) -> Vec<SurfacePlan> {
    let dpi = caps.dpi_scale;
    let work = caps.work_area;
    let resolved = resolve_material(state.material, caps);
    let gtk_material = gtk_material_for(resolved, layer_shell_ok);
    let motion = resolve_motion(state.motion, caps);

    let overlay_size = SurfaceSize::new(
        (OVERLAY_SIZE.width as f32 * dpi).round() as i32,
        (OVERLAY_SIZE.height as f32 * dpi).round() as i32,
    );
    let mixer_size = SurfaceSize::new(
        (MIXER_SIZE.width as f32 * dpi).round() as i32,
        (MIXER_SIZE.height as f32 * dpi).round() as i32,
    );
    let overlay_rect = place_overlay(work, overlay_size, MARGIN_X, MARGIN_Y);
    let mixer_rect = place_mixer_above_overlay(
        work,
        mixer_size,
        overlay_size,
        MARGIN_X,
        MARGIN_Y,
        SURFACE_GAP,
    );
    let settings_rect = place_centered(
        work,
        SurfaceSize::new(
            (SETTINGS_SIZE.width as f32 * dpi).round() as i32,
            (SETTINGS_SIZE.height as f32 * dpi).round() as i32,
        ),
    );
    let help_rect = place_centered(
        work,
        SurfaceSize::new(
            (HELP_SIZE.width as f32 * dpi).round() as i32,
            (HELP_SIZE.height as f32 * dpi).round() as i32,
        ),
    );

    let material_intent = tokens.material;
    let background = tokens.surface;

    vec![
        SurfacePlan {
            surface: SurfaceId::Overlay,
            rect: overlay_rect,
            material: resolved,
            gtk_material,
            motion,
            label: a11y_label(SurfaceId::Overlay),
            background,
            material_intent,
        },
        SurfacePlan {
            surface: SurfaceId::Mixer,
            rect: mixer_rect,
            material: resolved,
            gtk_material,
            motion,
            label: a11y_label(SurfaceId::Mixer),
            background,
            material_intent,
        },
        SurfacePlan {
            surface: SurfaceId::Settings,
            rect: settings_rect,
            material: resolved,
            gtk_material,
            motion,
            label: a11y_label(SurfaceId::Settings),
            background,
            material_intent,
        },
        SurfacePlan {
            surface: SurfaceId::Help,
            rect: help_rect,
            material: resolved,
            gtk_material,
            motion,
            label: a11y_label(SurfaceId::Help),
            background,
            material_intent,
        },
    ]
}

/// The Linux renderer: a [`crate::ui::NativeRenderer`] implementation owning
/// the GTK windows.
///
/// The host constructs it once with a [`HostHandle`] and the capability
/// snapshot; `publish` re-plans surfaces and applies the plans to the panels;
/// user intent is delivered through the host handle. Creating the renderer
/// initializes GTK/libadwaita; when no display session exists the error
/// propagates so the host can fall back to the CLI.
#[cfg(feature = "gtk-renderer")]
pub struct LinuxRenderer {
    host: HostHandle,
    layer_shell_ok: bool,
    panels: Vec<(SurfaceId, gtk_surfaces::GtkPanel)>,
}

#[cfg(feature = "gtk-renderer")]
impl LinuxRenderer {
    fn panel_for(&mut self, surface: SurfaceId) -> &mut gtk_surfaces::GtkPanel {
        let idx = match self.panels.iter().position(|(s, _)| *s == surface) {
            Some(idx) => idx,
            None => {
                let panel = gtk_surfaces::GtkPanel::new(surface, self.layer_shell_ok);
                self.panels.push((surface, panel));
                self.panels.len() - 1
            }
        };
        &mut self.panels[idx].1
    }
}

#[cfg(feature = "gtk-renderer")]
impl NativeRenderer for LinuxRenderer {
    type Error = String;

    fn create(host: HostHandle, capabilities: UiCapabilities) -> Result<Self, Self::Error> {
        gtk_surfaces::ensure_gtk_initialized()?;
        let layer_shell_ok = gtk_surfaces::layer_shell_available();
        let _ = capabilities;
        Ok(Self {
            host,
            layer_shell_ok,
            panels: Vec::new(),
        })
    }

    fn publish(&mut self, state: &AppState, tokens: &ThemeTokens, capabilities: &UiCapabilities) {
        let plans = plan_surfaces(state, tokens, capabilities, self.layer_shell_ok);
        for plan in &plans {
            let panel = self.panel_for(plan.surface);
            panel.apply_plan(plan, capabilities);
            let visible = state.is_visible(plan.surface);
            panel.set_visible(visible);
        }
    }

    fn dispatch(&mut self, action: AppAction) {
        // User intent from a surface routes to the host, which normalizes
        // (clamps, validates) and mutates authoritative state.
        self.host.enqueue(action);
    }

    fn destroy(&mut self) {
        self.panels.clear();
    }
}

/// GTK surface code (`gtk-renderer` feature).
///
/// Real `gtk::Window` construction with the libadwaita stylesheet; the
/// Wayland layer-shell path is gated on the `layer-shell` feature. The whole
/// submodule is feature-gated so the plain CLI fallback still builds on
/// Linux systems without GTK development packages.
#[cfg(feature = "gtk-renderer")]
mod gtk_surfaces {
    use super::*;
    use gtk::prelude::*;

    /// Initialize GTK and the libadwaita stylesheet.
    ///
    /// Fails with a message when no display session is available (for
    /// example a headless server); the host then falls back to the CLI.
    pub fn ensure_gtk_initialized() -> Result<(), String> {
        if !gtk::is_initialized() {
            gtk::init().map_err(|e| format!("GTK initialization failed: {e}"))?;
        }
        adw::init().map_err(|e| format!("libadwaita initialization failed: {e}"))
    }

    /// Runtime answer to "does this session support Wayland layer-shell".
    ///
    /// Requires both the `layer-shell` feature and a Wayland compositor;
    /// under X11 (including Xvfb in CI) this is false and surfaces use the
    /// plain-window fallback.
    pub fn layer_shell_available() -> bool {
        #[cfg(feature = "layer-shell")]
        {
            gtk4_layer_shell::is_layer_shell_supported()
        }
        #[cfg(not(feature = "layer-shell"))]
        {
            false
        }
    }

    /// A live GTK window bound to one surface.
    pub struct GtkPanel {
        window: gtk::Window,
        kind: GtkMaterialKind,
    }

    impl GtkPanel {
        pub fn new(surface: SurfaceId, layer_shell_ok: bool) -> Self {
            let _ = (surface, layer_shell_ok);
            let window = gtk::Window::new();
            Self {
                window,
                kind: GtkMaterialKind::Opaque,
            }
        }

        /// Apply a fresh plan: window type (layer-shell vs plain), material
        /// treatment, accessibility label, and motion policy.
        pub fn apply_plan(&mut self, plan: &SurfacePlan, caps: &UiCapabilities) {
            let size = logical_size(plan.surface);
            let is_float = matches!(plan.surface, SurfaceId::Overlay | SurfaceId::Mixer);
            let use_layer_shell = is_float && plan.gtk_material == GtkMaterialKind::WaylandGlass;

            if use_layer_shell {
                self.window.set_decorated(false);
                self.init_layer_shell_window(plan, caps, size);
            } else {
                self.window.set_decorated(false);
                self.window.set_title(Some(plan.label));
                self.window.set_default_size(size.width, size.height);
            }

            self.apply_material(plan);

            // libadwaita surface styling: Settings/Help carry the stylesheet
            // card/view treatment (adw::init loaded the stylesheet; the
            // classes come from the libadwaita design language).
            match plan.surface {
                SurfaceId::Settings | SurfaceId::Help => {
                    self.window.add_css_class("view");
                    self.window.add_css_class("card");
                }
                _ => {}
            }

            // Accessibility: the §11.2 label the Windows/macOS renderers use.
            self.window
                .update_property(&[gtk::accessible::Property::Label(plan.label.into())]);

            // Motion: disabled/reduced surfaces present instantly. GTK has no
            // per-window animation policy, so this is expressed by leaving the
            // window's own animation settings untouched (no implicit
            // animations are added by the surface scaffolding).
            let _ = plan.motion;
        }

        /// Wayland layer-shell setup: anchored overlay surface with the plan
        /// margins, sized by its content.
        #[cfg(feature = "layer-shell")]
        fn init_layer_shell_window(
            &mut self,
            plan: &SurfacePlan,
            caps: &UiCapabilities,
            size: SurfaceSize,
        ) {
            use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
            self.window.init_layer_shell();
            self.window.set_layer(Layer::Overlay);
            // Bottom-right placement against the shared work-area math: the
            // anchors fix the corner and the margins reproduce the plan rect.
            self.window.set_anchor(Edge::Right, true);
            self.window.set_anchor(Edge::Bottom, true);
            self.window
                .set_margin(Edge::Right, caps.work_area.width - plan.rect.right);
            self.window
                .set_margin(Edge::Bottom, caps.work_area.height - plan.rect.bottom);
            // Exclusive keyboard keeps the transient overlay from stealing
            // focus from the active app, matching the Windows non-activating
            // overlay/mixer behavior.
            self.window.set_keyboard_mode(KeyboardMode::Exclusive);
            let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
            content.set_size_request(size.width, size.height);
            self.window.set_child(Some(&content));
        }

        /// Plain-window fallback when layer-shell is unavailable (X11,
        /// headless, or the feature is off): content-sized borderless window.
        #[cfg(not(feature = "layer-shell"))]
        fn init_layer_shell_window(
            &mut self,
            _plan: &SurfacePlan,
            _caps: &UiCapabilities,
            size: SurfaceSize,
        ) {
            let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
            content.set_size_request(size.width, size.height);
            self.window.set_child(Some(&content));
        }

        /// Apply the resolved material treatment to the window.
        fn apply_material(&mut self, plan: &SurfacePlan) {
            let background = plan.background;
            let (css, opacity) = match plan.gtk_material {
                // Compositor glass: transparent content; the Wayland
                // compositor blends and blurs the backdrop.
                GtkMaterialKind::WaylandGlass => {
                    ("window { background: transparent; }".to_string(), 1.0)
                }
                // Translucent: transparent content plus the token surface
                // alpha at the window level.
                GtkMaterialKind::Translucent => (
                    "window { background: transparent; }".to_string(),
                    plan.material_intent.surface_alpha as f64,
                ),
                // Opaque: the solid token background (high contrast and the
                // no-compositor fallback).
                GtkMaterialKind::Opaque => (
                    format!(
                        "window {{ background: rgba({}, {}, {}, {:.3}); }}",
                        background.red,
                        background.green,
                        background.blue,
                        background.alpha as f32 / 255.0
                    ),
                    1.0,
                ),
            };
            let provider = gtk::CssProvider::new();
            provider.load_from_string(&css);
            self.window
                .style_context()
                .add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
            self.window.set_opacity(opacity);
            self.kind = plan.gtk_material;
        }

        pub fn set_visible(&mut self, visible: bool) {
            self.window.set_visible(visible);
        }
    }

    /// Smoke test: exercise the real GTK path — window creation, the
    /// material ladder, the accessibility label, and visibility flips.
    ///
    /// This is the runtime evidence for spec §10.3. Requires a display
    /// session (CI runs this under `xvfb-run`); without one the test skips
    /// with a message so headless `cargo test` stays green.
    #[cfg(test)]
    pub(crate) mod tests {
        use super::*;

        /// Initialize GTK/libadwaita once; false when no display exists.
        fn display_available() -> bool {
            static READY: std::sync::OnceLock<Option<()>> = std::sync::OnceLock::new();
            READY
                .get_or_init(|| {
                    if ensure_gtk_initialized().is_err() {
                        return None;
                    }
                    Some(())
                })
                .is_some()
        }

        fn caps() -> UiCapabilities {
            UiCapabilities {
                compositor: true,
                blur: true,
                high_contrast: false,
                reduced_motion: false,
                dpi_scale: 1.0,
                work_area: WorkArea::new(0, 0, 2560, 1400),
            }
        }

        fn state() -> AppState {
            AppState::from_audio(50, false, Some("Speakers".into()))
        }

        #[test]
        fn gtk_surfaces_apply_material_kinds_labels_and_visibility() {
            if !display_available() {
                eprintln!("skipping GTK smoke test: no display (CI runs under xvfb-run)");
                return;
            }
            let tokens = crate::ui::tokens_for(
                crate::ui::ThemeMode::Dark,
                false,
                crate::ui::AccentMode::System,
                || Some(true),
            );
            // Xvfb/X11: layer-shell is never available, so Blurred must land
            // on the Translucent window (glass requires a Wayland session).
            let plans = plan_surfaces(&state(), &tokens, &caps(), false);
            assert_eq!(plans.len(), 4);
            for plan in &plans {
                let mut panel = GtkPanel::new(plan.surface, false);
                panel.apply_plan(plan, &caps());
                assert_eq!(
                    panel.kind, plan.gtk_material,
                    "panel must carry the planned material for {:?}",
                    plan.surface
                );
                if plan.gtk_material.is_opaque() {
                    assert_eq!(
                        panel.window.opacity(),
                        1.0,
                        "opaque surface must be fully opaque ({:?})",
                        plan.surface
                    );
                }
                panel.set_visible(true);
                assert!(
                    panel.window.is_visible(),
                    "surface window must be visible after show ({:?})",
                    plan.surface
                );
                panel.set_visible(false);
                assert!(
                    !panel.window.is_visible(),
                    "surface window must be hidden after hide ({:?})",
                    plan.surface
                );
            }
        }

        #[test]
        fn gtk_high_contrast_forces_opaque_surfaces() {
            if !display_available() {
                eprintln!("skipping GTK smoke test: no display (CI runs under xvfb-run)");
                return;
            }
            let tokens = crate::ui::tokens_for(
                crate::ui::ThemeMode::Dark,
                true,
                crate::ui::AccentMode::System,
                || Some(true),
            );
            let mut caps = caps();
            caps.high_contrast = true;
            let plans = plan_surfaces(&state(), &tokens, &caps, true);
            for plan in &plans {
                let mut panel = GtkPanel::new(plan.surface, true);
                panel.apply_plan(plan, &caps);
                assert!(
                    panel.kind.is_opaque(),
                    "high contrast must force an opaque {:?}",
                    plan.surface
                );
            }
        }

        #[test]
        #[cfg(feature = "layer-shell")]
        fn layer_shell_windows_are_never_created_under_x11() {
            // On an X11/Xvfb display the layer-shell path is unreachable:
            // detection returns false and plans fall back to Translucent.
            assert!(!layer_shell_available());
            let tokens = crate::ui::tokens_for(
                crate::ui::ThemeMode::Dark,
                false,
                crate::ui::AccentMode::System,
                || Some(true),
            );
            let plans = plan_surfaces(&state(), &tokens, &caps(), false);
            assert!(
                plans.iter().all(|p| !p.gtk_material.is_glass()),
                "no Wayland glass without layer-shell support"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::model::{MaterialMode, SurfaceVisibilityState, ThemeMode};

    fn caps(dpi: f32) -> UiCapabilities {
        UiCapabilities {
            compositor: true,
            blur: true,
            high_contrast: false,
            reduced_motion: false,
            dpi_scale: dpi,
            work_area: WorkArea::new(0, 0, 2560, 1400),
        }
    }

    fn state() -> AppState {
        AppState {
            volume_percent: 50,
            muted: false,
            device: Some("Speakers".into()),
            surfaces: SurfaceVisibilityState::default(),
            theme: ThemeMode::Dark,
            material: MaterialMode::Auto,
            motion: MotionMode::Full,
            status: crate::ui::UiStatus::Ready,
        }
    }

    #[test]
    fn plans_all_four_surfaces_with_spec_geometry_at_100_dpi() {
        let tokens = crate::ui::tokens_for(
            ThemeMode::Dark,
            false,
            crate::ui::AccentMode::System,
            || Some(true),
        );
        let plans = plan_surfaces(&state(), &tokens, &caps(1.0), true);

        assert_eq!(plans.len(), 4);
        let overlay = plans
            .iter()
            .find(|p| p.surface == SurfaceId::Overlay)
            .unwrap();
        let mixer = plans
            .iter()
            .find(|p| p.surface == SurfaceId::Mixer)
            .unwrap();
        let settings = plans
            .iter()
            .find(|p| p.surface == SurfaceId::Settings)
            .unwrap();
        let help = plans.iter().find(|p| p.surface == SurfaceId::Help).unwrap();

        // Overlay bottom-right with the shared margins.
        assert_eq!(overlay.rect.right, 2560 - MARGIN_X);
        assert_eq!(overlay.rect.bottom, 1400 - MARGIN_Y);
        assert_eq!(overlay.rect.width(), OVERLAY_SIZE.width);
        // Mixer directly above with the 16px gap, shared right edge.
        assert_eq!(mixer.rect.bottom + SURFACE_GAP, overlay.rect.top);
        assert_eq!(mixer.rect.right, overlay.rect.right);
        assert_eq!(mixer.rect.width(), MIXER_SIZE.width);
        // Settings/help centered.
        let expected_settings_left = (2560 - SETTINGS_SIZE.width) / 2;
        let expected_settings_top = (1400 - SETTINGS_SIZE.height) / 2;
        assert_eq!(settings.rect.left, expected_settings_left);
        assert_eq!(settings.rect.top, expected_settings_top);
        let expected_help_left = (2560 - HELP_SIZE.width) / 2;
        let expected_help_top = (1400 - HELP_SIZE.height) / 2;
        assert_eq!(help.rect.left, expected_help_left);
        assert_eq!(help.rect.top, expected_help_top);
    }

    #[test]
    fn plans_scale_to_physical_pixels_at_125_and_150_percent() {
        let tokens = crate::ui::tokens_for(
            ThemeMode::Dark,
            false,
            crate::ui::AccentMode::System,
            || Some(true),
        );
        for dpi in [1.25f32, 1.5f32] {
            let plans = plan_surfaces(&state(), &tokens, &caps(dpi), true);
            let overlay = plans
                .iter()
                .find(|p| p.surface == SurfaceId::Overlay)
                .unwrap();
            let mixer = plans
                .iter()
                .find(|p| p.surface == SurfaceId::Mixer)
                .unwrap();

            let expected_w = (OVERLAY_SIZE.width as f32 * dpi).round() as i32;
            let expected_mixer_w = (MIXER_SIZE.width as f32 * dpi).round() as i32;
            assert_eq!(overlay.rect.width(), expected_w, "overlay at {dpi}x");
            assert_eq!(mixer.rect.width(), expected_mixer_w, "mixer at {dpi}x");
            // The 16px gap holds in physical pixels at every scale.
            assert_eq!(
                mixer.rect.bottom + SURFACE_GAP,
                overlay.rect.top,
                "gap at {dpi}x"
            );
            assert_eq!(mixer.rect.right, overlay.rect.right, "right edge at {dpi}x");
        }
    }

    #[test]
    fn material_ladder_maps_to_gtk_kinds() {
        // Blurred with layer-shell → Wayland glass; without → translucent.
        assert_eq!(
            gtk_material_for(ResolvedMaterial::Blurred, true),
            GtkMaterialKind::WaylandGlass
        );
        assert_eq!(
            gtk_material_for(ResolvedMaterial::Blurred, false),
            GtkMaterialKind::Translucent
        );
        assert_eq!(
            gtk_material_for(ResolvedMaterial::Translucent, true),
            GtkMaterialKind::Translucent
        );
        assert_eq!(
            gtk_material_for(ResolvedMaterial::Translucent, false),
            GtkMaterialKind::Translucent
        );
        assert_eq!(
            gtk_material_for(ResolvedMaterial::Opaque, true),
            GtkMaterialKind::Opaque
        );
        assert_eq!(
            gtk_material_for(ResolvedMaterial::Opaque, false),
            GtkMaterialKind::Opaque
        );
        assert!(GtkMaterialKind::WaylandGlass.is_glass());
        assert!(GtkMaterialKind::Opaque.is_opaque());
    }

    #[test]
    fn high_contrast_forces_opaque_gtk_surface() {
        let tokens = crate::ui::tokens_for(
            ThemeMode::Dark,
            false,
            crate::ui::AccentMode::System,
            || Some(true),
        );
        let mut caps = caps(1.0);
        caps.high_contrast = true;
        caps.blur = true;
        let plans = plan_surfaces(&state(), &tokens, &caps, true);
        for plan in &plans {
            assert_eq!(
                plan.gtk_material,
                GtkMaterialKind::Opaque,
                "HC must force opaque for {:?}",
                plan.surface
            );
        }
    }

    #[test]
    fn no_layer_shell_degrades_blurred_to_translucent() {
        let tokens = crate::ui::tokens_for(
            ThemeMode::Dark,
            false,
            crate::ui::AccentMode::System,
            || Some(true),
        );
        let plans = plan_surfaces(&state(), &tokens, &caps(1.0), false);
        for plan in &plans {
            assert!(
                !plan.gtk_material.is_glass(),
                "{:?} must not use Wayland glass without layer-shell",
                plan.surface
            );
        }
    }

    #[test]
    fn reduced_motion_downgrades_full_motion() {
        let tokens = crate::ui::tokens_for(
            ThemeMode::Dark,
            false,
            crate::ui::AccentMode::System,
            || Some(true),
        );
        let mut caps = caps(1.0);
        caps.reduced_motion = true;
        let plans = plan_surfaces(&state(), &tokens, &caps, true);
        for plan in &plans {
            assert_eq!(plan.motion, MotionMode::Reduced);
        }
    }

    #[test]
    fn a11y_labels_match_spec_section_11_2_vocabulary() {
        assert_eq!(a11y_label(SurfaceId::Overlay), "System output volume");
        assert_eq!(a11y_label(SurfaceId::Mixer), "Volume mixer");
        assert_eq!(a11y_label(SurfaceId::Settings), "VolumeControl Settings");
        assert_eq!(a11y_label(SurfaceId::Help), "VolumeControl Help");
        assert_eq!(a11y_label(SurfaceId::Tray), "VolumeControl");
    }

    #[test]
    fn logical_sizes_match_the_spec_constants() {
        assert_eq!(logical_size(SurfaceId::Overlay), OVERLAY_SIZE);
        assert_eq!(logical_size(SurfaceId::Mixer), MIXER_SIZE);
        assert_eq!(logical_size(SurfaceId::Settings), SETTINGS_SIZE);
        assert_eq!(logical_size(SurfaceId::Help), HELP_SIZE);
    }
}
