//! macOS AppKit renderer (spec §10.2).
//!
//! The renderer is split in two:
//!
//! - **Pure planning** (this file, minus `appkit`): derives per-surface
//!   placement, material treatment, motion policy, and VoiceOver labels from
//!   the shared contracts. Everything here is platform-neutral Rust and is
//!   unit-tested; it runs identically in CI (no window server needed).
//! - **AppKit surfaces** (the `appkit` submodule, macOS only): real
//!   `NSPanel`/`NSWindow` construction with `NSVisualEffectView` material
//!   backing, VoiceOver labels, and level/opacity configuration. Material
//!   APIs are availability-gated at runtime (class existence checks); no
//!   private APIs.
//!
//! Material ladder (spec §10.2): public native material available →
//! `NSVisualEffectView` glass; material unavailable → translucent surface;
//! translucency unavailable or high contrast → opaque surface. The shared
//! [`crate::ui::resolve_material`] resolves the treatment; this module maps
//! the resolved treatment onto AppKit.

use crate::ui::model::{AppAction, AppState, MotionMode, SurfaceId};
use crate::ui::renderer::{HostHandle, NativeRenderer};
use crate::ui::theme::{MaterialIntent, Rgba, ThemeTokens};
use crate::ui::{
    place_centered, place_mixer_above_overlay, place_overlay, resolve_material, resolve_motion,
    ResolvedMaterial, SurfaceRect, SurfaceSize, UiCapabilities, WorkArea,
};

/// Logical (1x) surface sizes from the Signal Glass spec §5–§8.
pub const OVERLAY_SIZE: SurfaceSize = SurfaceSize::new(336, 88);
pub const MIXER_SIZE: SurfaceSize = SurfaceSize::new(400, 224);
pub const SETTINGS_SIZE: SurfaceSize = SurfaceSize::new(580, 636);
pub const HELP_SIZE: SurfaceSize = SurfaceSize::new(520, 500);

/// Bottom-right placement margins (px, logical) shared with the Windows
/// renderer so both platforms place surfaces identically.
pub const MARGIN_X: i32 = 20;
pub const MARGIN_Y: i32 = 40;
/// Vertical gap between the mixer and the overlay.
pub const SURFACE_GAP: i32 = 16;

/// What the AppKit layer must actually apply for a surface.
///
/// This is the macOS end of the material ladder: the native `NSVisualEffectView`
/// glass treatment when the runtime supports it, a plain translucent surface
/// when it does not, and the fully opaque surface as the always-available
/// fallback (high contrast forces this via [`crate::ui::resolve_material`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppKitMaterialKind {
    /// `NSVisualEffectView` with a system glass material.
    NativeGlass,
    /// Translucent `NSWindow` without an effect view.
    Translucent,
    /// Fully opaque window with the token background.
    Opaque,
}

impl AppKitMaterialKind {
    pub const fn is_glass(self) -> bool {
        matches!(self, Self::NativeGlass)
    }

    pub const fn is_opaque(self) -> bool {
        matches!(self, Self::Opaque)
    }
}

/// Map the shared resolved material onto the AppKit material kinds.
///
/// `Blurred` (compositor + blur available) becomes the native glass; the
/// translucent and opaque treatments pass through 1:1.
pub fn appkit_material_for(resolved: ResolvedMaterial) -> AppKitMaterialKind {
    match resolved {
        ResolvedMaterial::Blurred => AppKitMaterialKind::NativeGlass,
        ResolvedMaterial::Translucent => AppKitMaterialKind::Translucent,
        ResolvedMaterial::Opaque => AppKitMaterialKind::Opaque,
    }
}

/// VoiceOver labels (spec §11.2 accessibility names).
///
/// The surface windows carry the surface names; the mixer controls carry the
/// exact §11.2 strings verified on the Windows renderer so screen readers
/// announce the same vocabulary on every platform.
pub fn a11y_label(surface: SurfaceId) -> &'static str {
    match surface {
        SurfaceId::Overlay => "System output volume",
        SurfaceId::Mixer => "Volume mixer",
        SurfaceId::Settings => "VolumeControl Settings",
        SurfaceId::Help => "VolumeControl Help",
        SurfaceId::Tray => "VolumeControl",
    }
}

/// Convert physical shared geometry into AppKit points.
fn appkit_rect_values(
    rect: SurfaceRect,
    work_area: WorkArea,
    dpi_scale: f32,
) -> (f64, f64, f64, f64) {
    let scale = if dpi_scale.is_finite() && dpi_scale > 0.0 {
        dpi_scale as f64
    } else {
        1.0
    };
    let height = work_area.height as f64;
    let top = work_area.y as f64;
    (
        rect.left as f64 / scale,
        (top + height - rect.bottom as f64) / scale,
        rect.width() as f64 / scale,
        rect.height() as f64 / scale,
    )
}

/// One planned surface: where it goes, how it looks, how it is announced.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfacePlan {
    pub surface: SurfaceId,
    pub rect: SurfaceRect,
    /// Resolved material treatment (shared ladder).
    pub material: ResolvedMaterial,
    /// AppKit-specific material mapping (derived, cached for the appkit layer).
    pub appkit_material: AppKitMaterialKind,
    /// Resolved motion policy after the system reduced-motion preference.
    pub motion: MotionMode,
    /// VoiceOver label for the window.
    pub label: &'static str,
    /// Surface fill from the shared tokens (drives the AppKit background).
    pub background: Rgba,
    /// Material intent from the shared tokens (alpha/blur guidance).
    pub material_intent: MaterialIntent,
}

/// Scale a logical size to physical pixels for the current DPI.
fn physical(size: SurfaceSize, dpi: f32) -> SurfaceSize {
    SurfaceSize::new(
        (size.width as f32 * dpi).round() as i32,
        (size.height as f32 * dpi).round() as i32,
    )
}

/// Plan every surface from the confirmed state.
///
/// Placement reuses the shared pure math so the mixer sits directly above the
/// overlay with the 16 px gap at any DPI; settings/help are centered. The
/// material ladder is resolved per the spec and the tokens, and motion is
/// resolved against the system preference. Pure — no AppKit calls.
pub fn plan_surfaces(
    state: &AppState,
    tokens: &ThemeTokens,
    caps: &UiCapabilities,
) -> Vec<SurfacePlan> {
    let dpi = caps.dpi_scale;
    let work = caps.work_area;
    let resolved = resolve_material(state.material, caps);
    let appkit_material = appkit_material_for(resolved);
    let motion = resolve_motion(state.motion, caps);

    let overlay_size = physical(OVERLAY_SIZE, dpi);
    let mixer_size = physical(MIXER_SIZE, dpi);
    let overlay_rect = place_overlay(work, overlay_size, MARGIN_X, MARGIN_Y);
    let mixer_rect = place_mixer_above_overlay(
        work,
        mixer_size,
        overlay_size,
        MARGIN_X,
        MARGIN_Y,
        SURFACE_GAP,
    );
    let settings_rect = place_centered(work, physical(SETTINGS_SIZE, dpi));
    let help_rect = place_centered(work, physical(HELP_SIZE, dpi));

    let material_intent = tokens.material;
    let background = tokens.surface;

    vec![
        SurfacePlan {
            surface: SurfaceId::Overlay,
            rect: overlay_rect,
            material: resolved,
            appkit_material,
            motion,
            label: a11y_label(SurfaceId::Overlay),
            background,
            material_intent,
        },
        SurfacePlan {
            surface: SurfaceId::Mixer,
            rect: mixer_rect,
            material: resolved,
            appkit_material,
            motion,
            label: a11y_label(SurfaceId::Mixer),
            background,
            material_intent,
        },
        SurfacePlan {
            surface: SurfaceId::Settings,
            rect: settings_rect,
            material: resolved,
            appkit_material,
            motion,
            label: a11y_label(SurfaceId::Settings),
            background,
            material_intent,
        },
        SurfacePlan {
            surface: SurfaceId::Help,
            rect: help_rect,
            material: resolved,
            appkit_material,
            motion,
            label: a11y_label(SurfaceId::Help),
            background,
            material_intent,
        },
    ]
}

/// The macOS renderer: a [`crate::ui::NativeRenderer`] implementation owning
/// the AppKit panels.
///
/// The host constructs it once with a [`HostHandle`] and the capability
/// snapshot; `publish` re-plans surfaces and applies the plans to the panels;
/// user intent is delivered through the host handle.
#[cfg(target_os = "macos")]
pub struct MacosRenderer {
    host: HostHandle,
    panels: Vec<(SurfaceId, appkit::Panel)>,
}

#[cfg(target_os = "macos")]
impl MacosRenderer {
    fn panel_for(&mut self, surface: SurfaceId) -> &mut appkit::Panel {
        let idx = match self.panels.iter().position(|(s, _)| *s == surface) {
            Some(idx) => idx,
            None => {
                let panel = appkit::Panel::new();
                self.panels.push((surface, panel));
                self.panels.len() - 1
            }
        };
        &mut self.panels[idx].1
    }
}

#[cfg(target_os = "macos")]
impl NativeRenderer for MacosRenderer {
    type Error = String;

    fn create(host: HostHandle, capabilities: UiCapabilities) -> Result<Self, Self::Error> {
        appkit::ensure_application();
        let _ = capabilities;
        Ok(Self {
            host,
            panels: Vec::new(),
        })
    }

    fn publish(&mut self, state: &AppState, tokens: &ThemeTokens, capabilities: &UiCapabilities) {
        let plans = plan_surfaces(state, tokens, capabilities);
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

/// AppKit surface code (macOS only).
///
/// Real `NSPanel` construction with `NSVisualEffectView` material backing.
/// The whole submodule is gated to `target_os = "macos"`; on any other target
/// the renderer planning above still compiles and tests.
#[cfg(target_os = "macos")]
mod appkit {
    use super::*;
    use objc2::rc::Retained;
    use objc2::runtime::AnyClass;
    use objc2::{MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{
        NSAccessibility, NSAnimatablePropertyContainer, NSApplication, NSAutoresizingMaskOptions,
        NSBackingStoreType, NSColor, NSFloatingWindowLevel, NSPanel, NSVisualEffectBlendingMode,
        NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView, NSWindowStyleMask,
    };
    use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

    /// A live AppKit panel bound to one surface.
    pub struct Panel {
        window: Retained<NSPanel>,
        /// The glass backing view, created lazily under the availability gate.
        effect: Option<Retained<NSVisualEffectView>>,
    }

    /// Ensure the shared application instance exists before creating panels.
    ///
    /// `sharedApplication` is safe to call (the marker proves the main
    /// thread, which the host renderer owns); the returned object is
    /// retained for the process lifetime.
    pub fn ensure_application() {
        let mtm = MainThreadMarker::new().expect("renderer on the main thread");
        let _app = NSApplication::sharedApplication(mtm);
    }

    fn to_ns_rect(rect: SurfaceRect, work_area: WorkArea, dpi_scale: f32) -> NSRect {
        let (left, bottom, width, height) = appkit_rect_values(rect, work_area, dpi_scale);
        NSRect::new(NSPoint::new(left, bottom), NSSize::new(width, height))
    }

    impl Panel {
        pub fn new() -> Self {
            // Main thread (host-owned renderer). The panel is borderless and
            // non-activating so it floats without stealing focus, matching
            // the Windows overlay/mixer behavior. The initializer and level
            // setter are safe in these bindings (marker/plain-property).
            let mtm = MainThreadMarker::new().expect("panel creation on the main thread");
            let window = NSPanel::initWithContentRect_styleMask_backing_defer(
                NSPanel::alloc(mtm),
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(320.0, 200.0)),
                NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
                NSBackingStoreType::Buffered,
                false,
            );
            window.setLevel(NSFloatingWindowLevel);
            Self {
                window,
                effect: None,
            }
        }

        /// Apply a fresh plan: frame, material treatment, VoiceOver label,
        /// and motion policy.
        pub fn apply_plan(&mut self, plan: &SurfacePlan, caps: &UiCapabilities) {
            let rect = to_ns_rect(plan.rect, caps.work_area, caps.dpi_scale);
            // SAFETY: main thread, window retained by `self`.
            unsafe {
                self.window.setFrame_display(rect, true);
                match plan.appkit_material {
                    AppKitMaterialKind::NativeGlass => self.apply_glass(),
                    AppKitMaterialKind::Translucent => self.apply_translucent(),
                    AppKitMaterialKind::Opaque => self.apply_opaque(plan),
                }
                let label = NSString::from_str(plan.label);
                self.window.setAccessibilityLabel(Some(&label));
                match plan.motion {
                    // No animation at all: the panel appears instantly.
                    MotionMode::Disabled | MotionMode::Reduced => {
                        self.window.setAnimations(&NSDictionary::new());
                    }
                    MotionMode::Full => {}
                }
            }
        }

        unsafe fn apply_glass(&mut self) {
            self.window.setOpaque(false);
            self.window.setBackgroundColor(Some(&NSColor::clearColor()));
            if self.effect.is_none() {
                // Availability gate (spec §10.2): the class must exist at
                // runtime; when it does not (headless/exotic hosts) we
                // degrade to the translucent surface.
                if AnyClass::get(c"NSVisualEffectView").is_some() {
                    let mtm = MainThreadMarker::new().expect("main thread");
                    let view = NSVisualEffectView::initWithFrame(
                        NSVisualEffectView::alloc(mtm),
                        self.window.frame(),
                    );
                    view.setAutoresizingMask(
                        NSAutoresizingMaskOptions::ViewWidthSizable
                            | NSAutoresizingMaskOptions::ViewHeightSizable,
                    );
                    view.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
                    view.setState(NSVisualEffectState::Active);
                    // HUDWindow is the oldest supported material (10.10+);
                    // availability follows what the running OS exposes.
                    view.setMaterial(NSVisualEffectMaterial::HUDWindow);
                    self.effect = Some(view);
                }
            }
            if let Some(effect) = &self.effect {
                self.window.setContentView(Some(&**effect));
            } else {
                self.window.setContentView(None);
            }
        }

        fn apply_translucent(&mut self) {
            self.window.setOpaque(false);
            self.window.setBackgroundColor(Some(&NSColor::clearColor()));
            self.effect = None;
            self.window.setContentView(None);
        }

        fn apply_opaque(&mut self, plan: &SurfacePlan) {
            let color = NSColor::colorWithSRGBRed_green_blue_alpha(
                plan.background.red as f64 / 255.0,
                plan.background.green as f64 / 255.0,
                plan.background.blue as f64 / 255.0,
                plan.background.alpha as f64 / 255.0,
            );
            self.window.setOpaque(true);
            self.window.setBackgroundColor(Some(&color));
            self.effect = None;
            self.window.setContentView(None);
        }

        pub fn set_visible(&mut self, visible: bool) {
            if visible {
                self.window.orderFrontRegardless();
            } else {
                self.window.orderOut(None);
            }
        }

        /// Whether the panel is currently opaque (window-level opacity flag).
        ///
        /// Safe property read on the main thread; used by the harness-free
        /// smoke test binary to assert the material ladder, and useful to
        /// hosts that report the applied treatment.
        pub fn is_opaque(&self) -> bool {
            self.window.isOpaque()
        }

        /// Whether the panel carries a VoiceOver accessibility label.
        pub fn has_accessibility_label(&self) -> bool {
            self.window.accessibilityLabel().is_some()
        }
    }
}

/// Re-exports for the harness-free AppKit smoke test binary
/// (`tests/appkit_smoke.rs`), which links the library without `cfg(test)`.
#[cfg(target_os = "macos")]
pub use appkit::{ensure_application, Panel};

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
        let plans = plan_surfaces(&state(), &tokens, &caps(1.0));

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
            let plans = plan_surfaces(&state(), &tokens, &caps(dpi));
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
    fn retina_appkit_frame_converts_physical_pixels_to_points_once() {
        let work_area = WorkArea::new(0, 0, 2880, 1800);
        let rect = SurfaceRect::new(2168, 1624, 2840, 1800);
        let (left, bottom, width, height) = appkit_rect_values(rect, work_area, 2.0);

        // AppKit uses a lower-left origin: a rect whose physical bottom is
        // the work-area bottom maps to y=0 points after one scale conversion.
        assert_eq!((left, bottom, width, height), (1084.0, 0.0, 336.0, 88.0));
    }

    #[test]
    fn material_ladder_maps_to_appkit_kinds() {
        assert_eq!(
            appkit_material_for(ResolvedMaterial::Blurred),
            AppKitMaterialKind::NativeGlass
        );
        assert_eq!(
            appkit_material_for(ResolvedMaterial::Translucent),
            AppKitMaterialKind::Translucent
        );
        assert_eq!(
            appkit_material_for(ResolvedMaterial::Opaque),
            AppKitMaterialKind::Opaque
        );
        assert!(AppKitMaterialKind::NativeGlass.is_glass());
        assert!(AppKitMaterialKind::Opaque.is_opaque());
    }

    #[test]
    fn high_contrast_forces_opaque_appkit_surface() {
        let tokens = crate::ui::tokens_for(
            ThemeMode::Dark,
            false,
            crate::ui::AccentMode::System,
            || Some(true),
        );
        let mut caps = caps(1.0);
        caps.high_contrast = true;
        caps.blur = true;
        let plans = plan_surfaces(&state(), &tokens, &caps);
        for plan in &plans {
            assert_eq!(
                plan.appkit_material,
                AppKitMaterialKind::Opaque,
                "HC must force opaque for {:?}",
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
        let plans = plan_surfaces(&state(), &tokens, &caps);
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
}
