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

/// Test helper: plan one surface with the default dark theme at 100% DPI.
///
/// Lives outside `mod tests` so the AppKit smoke tests (in `mod appkit`) and
/// the pure planning tests can both assert against the same shared math.
#[cfg(test)]
fn plan_for(surface: &SurfaceId) -> SurfacePlan {
    let tokens = crate::ui::tokens_for(
        crate::ui::ThemeMode::Dark,
        false,
        crate::ui::AccentMode::System,
        || Some(true),
    );
    let caps = UiCapabilities {
        compositor: true,
        blur: true,
        high_contrast: false,
        reduced_motion: false,
        dpi_scale: 1.0,
        work_area: WorkArea::new(0, 0, 2560, 1400),
    };
    let state = AppState::from_audio(50, false, Some("Speakers".into()));
    plan_surfaces(&state, &tokens, &caps)
        .into_iter()
        .find(|p| &p.surface == surface)
        .expect("planned surface")
}

/// AppKit surface code (macOS only).
///
/// Real `NSPanel` construction with `NSVisualEffectView` material backing.
/// The whole submodule is gated to `target_os = "macos"`; on any other target
/// the renderer planning above still compiles and tests.
#[cfg(target_os = "macos")]
mod appkit {
    use super::*;
    #[cfg(test)]
    use core::ffi::{c_int, c_void};
    #[cfg(test)]
    use core::ptr::addr_of;
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

    fn to_ns_rect(rect: SurfaceRect, work_area: WorkArea) -> NSRect {
        // AppKit uses a bottom-left origin; the shared math uses top-left.
        let height = work_area.height;
        let top = work_area.y;
        NSRect::new(
            NSPoint::new(rect.left as f64, (top + height - rect.bottom) as f64),
            NSSize::new(rect.width() as f64, rect.height() as f64),
        )
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
            let rect = to_ns_rect(plan.rect, caps.work_area);
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
    }

    /// Run `f` on the main thread.
    ///
    /// `cargo test` runs test bodies on worker threads, and AppKit requires
    /// the main thread. libSystem's GCD is a stable public API; we hand-declare
    /// the two symbols we need plus the Apple block ABI for a stack block.
    /// `dispatch_sync` copies the block (always) and runs it on the main
    /// queue before returning, so the stack block stays valid for its whole
    /// lifetime. The closure must capture only `Copy` data: `Block_copy` on a
    /// helper-less block is a plain bitwise copy, which is only safe when the
    /// captures are `Copy`.
    ///
    /// `f` must not panic: a panic would unwind across the C `dispatch_sync`
    /// frame. Callers collect results and assert after dispatch returns.
    #[cfg(test)]
    unsafe fn on_main_thread<F: FnOnce() + Copy>(f: F) {
        #[cfg(test)]
        {
            if MainThreadMarker::new().is_some() {
                f();
                return;
            }

            #[repr(C)]
            struct BlockHeader {
                isa: *const c_void,
                flags: c_int,
                reserved: c_int,
                invoke: unsafe extern "C" fn(*mut c_void),
                descriptor: *const BlockDescriptor,
            }
            #[repr(C)]
            struct BlockDescriptor {
                reserved: usize,
                size: usize,
            }
            #[repr(C)]
            struct Payload<F> {
                header: BlockHeader,
                payload: F,
            }

            unsafe extern "C" {
                // Apple block runtime + libdispatch, both in libSystem.
                static _NSConcreteStackBlock: c_void;
                fn dispatch_get_main_queue() -> *mut c_void;
                fn dispatch_sync(queue: *mut c_void, block: *const c_void);
            }

            unsafe extern "C" fn trampoline<F: FnOnce() + Copy>(block: *mut c_void) {
                let payload = &*block.cast::<Payload<F>>();
                (payload.payload)();
            }

            let payload = Payload {
                header: BlockHeader {
                    isa: addr_of!(_NSConcreteStackBlock).cast(),
                    flags: 0,
                    reserved: 0,
                    invoke: trampoline::<F>,
                    descriptor: &BlockDescriptor {
                        reserved: 0,
                        size: core::mem::size_of::<Payload<F>>(),
                    },
                },
                payload: f,
            };
            dispatch_sync(dispatch_get_main_queue(), addr_of!(payload).cast());
        }
    }

    /// Smoke test: exercise the real AppKit path — panel creation, the
    /// material ladder, and the VoiceOver label — on the main thread.
    ///
    /// This is the runtime evidence for spec §10.2. Results are collected
    /// inside the main-thread block (no panics there) and asserted after
    /// dispatch returns.
    #[test]
    fn appkit_panel_applies_material_kinds_and_labels() {
        let mut results: Vec<(SurfaceId, bool, bool)> = Vec::new();
        let results_ptr = &mut results as *mut Vec<(SurfaceId, bool, bool)>;
        // SAFETY: `results_ptr` outlives the dispatch; the closure captures
        // only Copy data; no panics inside the block.
        unsafe {
            on_main_thread(move || {
                ensure_application();
                let caps = UiCapabilities {
                    compositor: true,
                    blur: true,
                    high_contrast: false,
                    reduced_motion: false,
                    dpi_scale: 1.0,
                    work_area: WorkArea::new(0, 0, 2560, 1400),
                };
                let state = AppState::from_audio(50, false, Some("Speakers".into()));
                let tokens = crate::ui::tokens_for(
                    crate::ui::ThemeMode::Dark,
                    false,
                    crate::ui::AccentMode::System,
                    || Some(true),
                );
                let plans = plan_surfaces(&state, &tokens, &caps);
                let out = &mut *results_ptr;
                for plan in &plans {
                    let mut panel = Panel::new();
                    panel.apply_plan(plan, &caps);
                    let opaque = panel.window.isOpaque();
                    let label = panel.window.accessibilityLabel();
                    out.push((plan.surface, opaque, label.is_some()));
                }
            });
        }
        assert_eq!(results.len(), 4, "all four surfaces must produce panels");
        for (surface, opaque, labelled) in results {
            assert!(
                labelled,
                "every surface window must carry a VoiceOver label ({surface:?})"
            );
            // The material ladder: opaque only when the resolved treatment is
            // Opaque; glass/translucent must leave the window translucent.
            let expected = plan_for(&surface).appkit_material.is_opaque();
            assert_eq!(
                opaque, expected,
                "opacity must match the resolved material for {surface:?}"
            );
        }
    }

    #[test]
    fn appkit_high_contrast_forces_opaque_panels() {
        let mut results: Vec<(SurfaceId, bool)> = Vec::new();
        let results_ptr = &mut results as *mut Vec<(SurfaceId, bool)>;
        // SAFETY: as above; no panics inside the block.
        unsafe {
            on_main_thread(move || {
                ensure_application();
                let caps = UiCapabilities {
                    compositor: true,
                    blur: true,
                    high_contrast: true,
                    reduced_motion: false,
                    dpi_scale: 1.0,
                    work_area: WorkArea::new(0, 0, 2560, 1400),
                };
                let state = AppState::from_audio(50, false, Some("Speakers".into()));
                let tokens = crate::ui::tokens_for(
                    crate::ui::ThemeMode::Dark,
                    true,
                    crate::ui::AccentMode::System,
                    || Some(true),
                );
                let plans = plan_surfaces(&state, &tokens, &caps);
                let out = &mut *results_ptr;
                for plan in &plans {
                    let mut panel = Panel::new();
                    panel.apply_plan(plan, &caps);
                    let opaque = panel.window.isOpaque();
                    out.push((plan.surface, opaque));
                }
            });
        }
        for (surface, opaque) in results {
            assert!(opaque, "high contrast must force an opaque {surface:?}");
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
