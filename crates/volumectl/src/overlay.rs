//! Volume overlay — native Win32 status capsule shown briefly on volume changes.
//!
//! A 336x88 (logical) bottom-right, always-on-top, click-through
//! (`WS_EX_TRANSPARENT | WS_EX_LAYERED`), tool window (no taskbar entry) that
//! answers current volume, output identity, and mute state at a glance (Signal
//! Glass spec §5). Paints through the Task 3 resource-safe [`PaintCanvas`]
//! (Direct2D when available, GDI otherwise) using the shared semantic tokens;
//! the volume visualization is the Task 4 shared [`SignalRail`] with its
//! threshold fill and thumb/diamond markers. Auto-hides after
//! `config.overlay_duration_ms` (default 1800 ms) via a one-shot timer.
//!
//! Placement is the bottom-right of the *monitor work area* hosting the window
//! (taskbar excluded), computed through [`crate::ui::surface::place_overlay`].
//! The `OVERLAY_*` geometry constants remain the single source of truth that
//! the mixer placement consumes, so the 16px mixer/overlay gap is preserved by
//! construction.
//!
//! The overlay is created once at startup (hidden) and shown on demand by the
//! app when a custom hotkey changes the volume. Media keys keep the native
//! Windows flyout, so the overlay never fights it.

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    Graphics::Gdi::InvalidateRect,
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindowLongPtrW, KillTimer,
        RegisterClassW, SetLayeredWindowAttributes, SetTimer, SetWindowLongPtrW, SetWindowPos,
        ShowWindow, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, HWND_TOPMOST, LWA_ALPHA, SWP_NOACTIVATE,
        SWP_SHOWWINDOW, SW_HIDE, WM_ERASEBKGND, WM_PAINT, WM_TIMER, WNDCLASSW, WS_EX_LAYERED,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
    },
};

use crate::audio::VolumeState;
use crate::config::{ColorThresholds, Config};
use crate::ui::platform::windows::text::{TextAlign, TextLayout};
use crate::ui::primitives::{
    apply_backdrop, work_area_for, DpiMetrics, PaintCanvas, PointF, RectF,
};
use crate::ui::{
    place_overlay, rail_geometry, resolve_material, resolve_motion, tokens_for, AccentMode,
    MarkerGeometry, MotionMode, ResolvedMaterial, SignalRail, SurfaceSize, ThemeMode, ThemeTokens,
    TrackRect, UiCapabilities,
};

/// Logical width of the status capsule (spec §5.1).
pub(crate) const OVERLAY_WIDTH: i32 = 336;
/// Logical height of the status capsule (spec §5.1).
pub(crate) const OVERLAY_HEIGHT: i32 = 88;
pub(crate) const OVERLAY_MARGIN_X: i32 = 20;
pub(crate) const OVERLAY_MARGIN_Y: i32 = 40;
const TIMER_ID: usize = 1;

/// Rail thumb diameter 12px → 6px radius (spec §5.2).
const THUMB_RADIUS: f32 = 6.0;
/// Muted diamond half-size 6px — same extent as the thumb (spec §5.2).
const MUTED_DIAMOND_HALF_SIZE: f32 = 6.0;

/// Adaptive appearance resolved by the host and applied by the overlay.
///
/// The host resolves this once per `show`/`show_text` from the confirmed
/// appearance preferences and the display-session capability snapshot, so the
/// overlay stays a dumb consumer with a single resolution point in `app.rs`.
#[derive(Debug, Clone, Copy)]
pub struct OverlayAppearance {
    /// Resolved palette tokens (theme + high-contrast + accent).
    pub tokens: ThemeTokens,
    /// Capability-resolved material treatment (blur/translucent/opaque).
    pub material: ResolvedMaterial,
    /// Capability-resolved motion policy. `caps.reduced_motion` downgrades
    /// [`MotionMode::Full`] to [`MotionMode::Reduced`]; explicit Reduced and
    /// Disabled are preserved (see [`resolve_motion`]).
    ///
    /// The overlay currently has no animation at all: every motion mode
    /// presents the final frame immediately. Reduced and Disabled therefore
    /// behave identically today — they simply must never gain perpetual or
    /// decorative motion (spec §5.4: reduced motion respected, no perpetual
    /// animation). When entry animation lands, this field is what gates it.
    pub motion: MotionMode,
}

impl OverlayAppearance {
    /// Resolve the adaptive appearance from `config.appearance` against `caps`.
    ///
    /// `system_is_dark` is consulted only for [`ThemeMode::System`]; it lets
    /// tests inject the darkness decision while the host passes the shared
    /// [`crate::ui::primitives::system_theme`] helper.
    pub fn resolve(
        config: &Config,
        caps: &UiCapabilities,
        system_is_dark: impl Fn() -> Option<bool>,
    ) -> Self {
        let appearance = &config.appearance;
        let tokens = tokens_for(
            appearance.theme,
            caps.high_contrast,
            appearance.accent,
            system_is_dark,
        );
        let material = resolve_material(appearance.material, caps);
        let motion = resolve_motion(appearance.motion, caps);
        Self {
            tokens,
            material,
            motion,
        }
    }

    /// Placeholder used before the first `show` (the window is hidden then, so
    /// this is never painted).
    fn placeholder() -> Self {
        Self {
            tokens: tokens_for(ThemeMode::System, false, AccentMode::System, || None),
            material: ResolvedMaterial::Opaque,
            motion: MotionMode::Full,
        }
    }
}

/// State shared with the window proc via GWLP_USERDATA.
struct OverlayData {
    percent: u8,
    muted: bool,
    thresholds: ColorThresholds,
    appearance: OverlayAppearance,
    /// When set, paint this toast text instead of the volume bar.
    toast: Option<String>,
}

pub struct Overlay {
    hwnd: HWND,
    data: *mut OverlayData,
}

impl Overlay {
    /// Create the hidden overlay window (shown on demand).
    pub fn new() -> Result<Overlay, Box<dyn std::error::Error>> {
        unsafe {
            let hinst = GetModuleHandleW(std::ptr::null());
            let class = windows_sys::core::w!("VolCtlOverlay");

            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.style = CS_HREDRAW | CS_VREDRAW;
            wc.lpfnWndProc = Some(overlay_wndproc);
            wc.hInstance = hinst;
            wc.lpszClassName = class;
            RegisterClassW(&wc);

            let data = Box::into_raw(Box::new(OverlayData {
                percent: 0,
                muted: false,
                thresholds: Config::default().color_thresholds,
                appearance: OverlayAppearance::placeholder(),
                toast: None,
            }));

            // Created at the logical design size; `present` re-positions the
            // window at the DPI-scaled physical size before the first show.
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST
                    | WS_EX_TOOLWINDOW
                    | WS_EX_TRANSPARENT
                    | WS_EX_LAYERED
                    | WS_EX_NOACTIVATE,
                class,
                windows_sys::core::w!("VolumeControl Overlay"),
                WS_POPUP,
                0,
                0,
                OVERLAY_WIDTH,
                OVERLAY_HEIGHT,
                0, // hwndParent
                0, // hmenu
                hinst,
                std::ptr::null(),
            );
            if hwnd == 0 {
                drop(Box::from_raw(data));
                return Err("overlay CreateWindowEx failed".into());
            }

            // Full alpha + click-through behaviour from WS_EX_LAYERED|TRANSPARENT.
            // Kept unconditionally: the renderer paints its own (opaque) fill,
            // so the layered window stays click-through on every material
            // fallback.
            SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, data as isize);

            Ok(Overlay { hwnd, data })
        }
    }

    /// Show (or refresh) the overlay for the given volume state.
    pub fn show(&mut self, state: &VolumeState, config: &Config, appearance: &OverlayAppearance) {
        unsafe {
            let d = &mut *self.data;
            d.percent = state.percent();
            d.muted = state.muted;
            d.thresholds = config.color_thresholds.clone();
            d.appearance = *appearance;
            d.toast = None;
        }
        self.present(config);
    }

    /// Show a text toast (e.g. "Config reloaded", "No blacklist needed").
    pub fn show_text(&mut self, text: &str, config: &Config, appearance: &OverlayAppearance) {
        unsafe {
            let d = &mut *self.data;
            d.toast = Some(text.to_string());
            d.appearance = *appearance;
        }
        self.present(config);
    }

    /// Position the window at the bottom-right of the current monitor work
    /// area, apply the resolved material treatment, refresh the surface, and
    /// restart the auto-hide timer.
    fn present(&self, config: &Config) {
        unsafe {
            let d = &*self.data;

            // Material fallback: request the DWM treatment (blur/translucent/
            // opaque). The overlay always paints its own fill, so a missing
            // system backdrop (Windows 10) simply keeps the painted fill.
            let backdrop_active = apply_backdrop(
                self.hwnd,
                d.appearance.material,
                d.appearance.tokens.is_dark,
            );
            log::debug!(
                "overlay material={:?} backdrop_active={}",
                d.appearance.material,
                backdrop_active
            );

            // Bottom-right of the monitor work area hosting the window. The
            // logical design size (OVERLAY_*) is scaled exactly once to
            // physical pixels via the window's DPI metrics; painting uses the
            // same logical coordinates and the canvas scales them the same way.
            let dpi = DpiMetrics::new(crate::ui::primitives::dpi_scale_for(self.hwnd));
            let size = SurfaceSize::new(
                dpi.to_physical(OVERLAY_WIDTH),
                dpi.to_physical(OVERLAY_HEIGHT),
            );
            let work_area = work_area_for(self.hwnd);
            let rect = place_overlay(work_area, size, OVERLAY_MARGIN_X, OVERLAY_MARGIN_Y);
            let ok = SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                rect.left,
                rect.top,
                rect.width(),
                rect.height(),
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            log::debug!(
                "overlay show hwnd={:x} pos=({},{}) size=({}x{}) ok={}",
                self.hwnd,
                rect.left,
                rect.top,
                rect.width(),
                rect.height(),
                ok
            );

            InvalidateRect(self.hwnd, std::ptr::null(), 0);

            // Restart the auto-hide timer.
            KillTimer(self.hwnd, TIMER_ID);
            let ms = config.overlay_duration_ms.min(10_000) as u32;
            SetTimer(self.hwnd, TIMER_ID, ms, None);
        }
    }

    pub fn hide(&self) {
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
        }
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        unsafe {
            if !self.data.is_null() {
                drop(Box::from_raw(self.data));
            }
            DestroyWindow(self.hwnd);
        }
    }
}

unsafe extern "system" fn overlay_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TIMER => {
            // Auto-hide after the configured duration.
            log::debug!("overlay WM_TIMER -> hide");
            ShowWindow(hwnd, SW_HIDE);
            KillTimer(hwnd, TIMER_ID);
            0
        }
        WM_ERASEBKGND => 1, // skip erase for flicker-free painting
        WM_PAINT => {
            // Paint through the resource-safe canvas (Task 3): it owns the
            // BeginPaint/EndPaint pair, selects ONE paint path per frame
            // (Direct2D when available, GDI otherwise), and deletes every
            // per-call GDI object. If BeginPaint itself fails, paint nothing
            // and invalidate so the next WM_PAINT retries — never regress to
            // hand-rolled GDI brushes leaking per paint.
            if let Some(mut canvas) = PaintCanvas::begin_paint(hwnd) {
                let data = &*(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const OverlayData);
                paint(&mut canvas, data);
            } else {
                log::debug!("overlay: BeginPaint failed; invalidating for a retry");
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Logical layout of the volume frame (all values in logical px).
#[derive(Debug, Clone, Copy, PartialEq)]
struct VolumeLayout {
    /// "Volume" title box (label role), left edge at the 16px padding.
    title_rect: RectF,
    /// Live value box ending at `width - 16`; the value is right-aligned
    /// inside it via [`TextAlign::Right`] origin math (`rect.right -
    /// text_width`, never space padding).
    value_rect: RectF,
    /// "System output" caption box at y 44.
    output_rect: RectF,
    /// Signal Rail track: spans x 16..=width-16, 8px tall, vertically centered
    /// in the lower band between the caption and the surface bottom.
    track: TrackRect,
}

impl VolumeLayout {
    /// Build the layout for a `w x h` logical surface.
    ///
    /// Vertical rhythm (spec §5.2): 16px top padding; row 1 (title + 28px
    /// display value) spans y 16..44; row 2 (11px caption) starts at y 44; the
    /// rail's 8px band sits centered in the lower band 55..88 → center 71.5,
    /// track 67.5..75.5, so the 12px marker (65.5..77.5) stays fully inside
    /// the surface.
    fn new(w: f32, h: f32) -> Self {
        let content_right = w - 16.0;
        let output_bottom = 55.0;
        let rail_center_y = (output_bottom + h) * 0.5;
        let rail_half = 4.0;
        Self {
            title_rect: RectF::new(16.0, 16.0, content_right, 32.0),
            value_rect: RectF::new(16.0, 16.0, content_right, output_bottom),
            output_rect: RectF::new(16.0, 44.0, content_right, output_bottom),
            track: TrackRect {
                left: 16.0,
                right: content_right,
                top: rail_center_y - rail_half,
                bottom: rail_center_y + rail_half,
            },
        }
    }
}

/// Pure paint plan for one overlay frame.
///
/// The plan is the single testable decision point: toast vs volume mode, the
/// rail state, and every layout rectangle are resolved here without a window.
/// [`paint`] only executes the plan through the canvas.
struct PaintPlan<'a> {
    width: f32,
    height: f32,
    kind: PaintPlanKind<'a>,
}

enum PaintPlanKind<'a> {
    /// Toast mode: text only, no rail (spec §5.3).
    Toast { text: &'a str, rect: RectF },
    /// Volume mode: title/value/output identity plus the Signal Rail.
    Volume {
        layout: VolumeLayout,
        rail: SignalRail,
        geometry: crate::ui::SignalRailGeometry,
    },
}

/// Resolve the frame plan for `data` in a `w x h` logical surface.
///
/// The rail carries the user's `config.color_thresholds` band boundaries
/// (`green_up_to`/`blue_up_to`) and the token palette, so
/// [`SignalRail::fill_color`] mirrors the authoritative
/// `core::volume_color_rgb` semantics for any config.
fn paint_plan(data: &OverlayData, w: f32, h: f32) -> PaintPlan<'_> {
    if let Some(text) = &data.toast {
        return PaintPlan {
            width: w,
            height: h,
            kind: PaintPlanKind::Toast {
                text,
                // Centered body line (vertical center 44 = surface center).
                rect: RectF::new(16.0, 36.0, w - 16.0, 52.0),
            },
        };
    }
    let layout = VolumeLayout::new(w, h);
    let rail = SignalRail::new(
        data.percent,
        data.muted,
        data.appearance.tokens.volume_threshold,
        data.thresholds.green_up_to,
        data.thresholds.blue_up_to,
    );
    let geometry = rail_geometry(&rail, layout.track, THUMB_RADIUS, MUTED_DIAMOND_HALF_SIZE);
    PaintPlan {
        width: w,
        height: h,
        kind: PaintPlanKind::Volume {
            layout,
            rail,
            geometry,
        },
    }
}

/// Draw the overlay contents from the resolved [`PaintPlan`]: adaptive
/// background, optional opaque border, then either the centered toast text or
/// the title/value/output rows plus the Signal Rail. All coordinates are
/// logical; the canvas scales them to physical pixels exactly once via its
/// DPI metrics.
///
/// Motion policy: there is no animation today, so every [`MotionMode`]
/// (Full/Reduced/Disabled) presents the final frame immediately — Reduced and
/// Disabled must not add perpetual or decorative motion, which they don't.
fn paint(canvas: &mut PaintCanvas, data: &OverlayData) {
    let tokens = &data.appearance.tokens;
    let plan = paint_plan(data, OVERLAY_WIDTH as f32, OVERLAY_HEIGHT as f32);

    // Background: always-painted opaque token fill. This renderer fill is the
    // material fallback — the surface stays fully readable even when no DWM
    // backdrop is available (Windows 10 / unsupported backdrop attribute).
    // The capsule corners are rounded by the DWM corner preference requested
    // in `apply_backdrop`.
    canvas.fill_rect(
        RectF::new(0.0, 0.0, plan.width, plan.height),
        tokens.background,
    );

    // 1px border in opaque mode (spec §5.2). Blurred/translucent modes draw no
    // border: the DWM backdrop provides the surface edge, and a painted
    // border would sit on top of the acrylic. High contrast always resolves
    // to opaque, so it always gets the border.
    if data.appearance.material.is_opaque() {
        canvas.stroke_rounded_rect(
            RectF::new(0.5, 0.5, plan.width - 0.5, plan.height - 0.5),
            tokens.radii.surface_px,
            tokens.border,
            1.0,
        );
    }

    match &plan.kind {
        PaintPlanKind::Toast { text, rect } => {
            let drawn = canvas.draw_text(&TextLayout {
                text,
                rect: *rect,
                align: TextAlign::Center,
                role: tokens.typography.body,
                color: tokens.text_primary,
            });
            if !drawn {
                log::debug!("overlay: toast text draw failed");
            }
        }
        PaintPlanKind::Volume {
            layout,
            rail,
            geometry,
        } => {
            // Row 1: title (left) + live value (right-aligned by real
            // alignment math — `TextAlign::Right` origin = rect.right -
            // text_width, never space padding).
            canvas.draw_text(&TextLayout {
                text: "Volume",
                rect: layout.title_rect,
                align: TextAlign::Left,
                role: tokens.typography.label,
                color: tokens.text_secondary,
            });
            let value = if rail.muted {
                "Muted".to_string()
            } else {
                format!("{}%", rail.percent)
            };
            canvas.draw_text(&TextLayout {
                text: &value,
                rect: layout.value_rect,
                align: TextAlign::Right,
                // Muted renders as the `Muted` label in the muted legend
                // colour; the shape cue below (MutedDiamond) pairs with the
                // text so the state never relies on colour alone.
                role: if rail.muted {
                    tokens.typography.label
                } else {
                    tokens.typography.display_value
                },
                color: if rail.muted {
                    tokens.volume_threshold.muted
                } else {
                    tokens.text_primary
                },
            });

            // Row 2: output identity.
            canvas.draw_text(&TextLayout {
                text: "System output",
                rect: layout.output_rect,
                align: TextAlign::Left,
                role: tokens.typography.caption,
                color: tokens.text_secondary,
            });

            // Row 3: Signal Rail — track, threshold fill, marker. The fill
            // honours the user's `config.color_thresholds` band boundaries
            // through the rail (Task 4, proven identical to
            // `core::volume_color_rgb` for any config).
            let t = geometry.track;
            canvas.fill_rect(RectF::new(t.left, t.top, t.right, t.bottom), tokens.border);
            if geometry.fill_right > t.left {
                canvas.fill_rect(
                    RectF::new(t.left, t.top, geometry.fill_right, t.bottom),
                    rail.fill_color(),
                );
            }
            match geometry.marker {
                MarkerGeometry::Thumb {
                    center_x,
                    center_y,
                    radius,
                } => {
                    let center = PointF::new(center_x, center_y);
                    // Filled surface circle with a strong outline: the thumb
                    // stays visible against both the fill and the track.
                    // (`border_strong` lives on the additive signal-glass
                    // state tokens.)
                    canvas.fill_circle(center, radius, tokens.surface);
                    canvas.stroke_circle(center, radius, tokens.signal_glass().border_strong, 1.0);
                }
                MarkerGeometry::MutedDiamond {
                    center_x,
                    center_y,
                    half_size,
                } => {
                    // Outline diamond (◇), never a filled grey copy of the
                    // thumb: the shape carries the muted state in high
                    // contrast. The outline uses text_primary (maximal
                    // contrast on every theme — the muted-grey fill beneath
                    // the left half would swallow a grey outline).
                    canvas.stroke_diamond(
                        PointF::new(center_x, center_y),
                        half_size,
                        tokens.text_primary,
                        1.0,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::platform::windows::text::text_x_origin;
    use crate::ui::{place_mixer_above_overlay, MaterialMode, Rgba, WorkArea};

    fn caps(compositor: bool, high_contrast: bool) -> UiCapabilities {
        UiCapabilities {
            compositor,
            blur: compositor,
            high_contrast,
            reduced_motion: false,
            dpi_scale: 1.0,
            work_area: WorkArea::new(0, 0, 2560, 1400),
        }
    }

    fn appearance(
        theme: ThemeMode,
        material: MaterialMode,
        high_contrast: bool,
    ) -> OverlayAppearance {
        let mut cfg = Config::default();
        cfg.appearance.theme = theme;
        cfg.appearance.material = material;
        OverlayAppearance::resolve(&cfg, &caps(true, high_contrast), || None)
    }

    /// An overlay frame in volume mode with the given state.
    fn volume_plan(
        percent: u8,
        muted: bool,
    ) -> (VolumeLayout, SignalRail, crate::ui::SignalRailGeometry) {
        let cfg = Config::default();
        let data = OverlayData {
            percent,
            muted,
            thresholds: cfg.color_thresholds,
            appearance: appearance(ThemeMode::Dark, MaterialMode::Opaque, false),
            toast: None,
        };
        let plan = paint_plan(&data, OVERLAY_WIDTH as f32, OVERLAY_HEIGHT as f32);
        match plan.kind {
            PaintPlanKind::Volume {
                layout,
                rail,
                geometry,
            } => (layout, rail, geometry),
            PaintPlanKind::Toast { .. } => panic!("expected a volume plan"),
        }
    }

    #[test]
    fn appearance_carries_the_volumepro_threshold_palette() {
        // The bar fill stays VolumePro-derived (`volume_color_rgb` via the
        // rail); the tokens encode the same palette, so the two never
        // disagree.
        let a = appearance(ThemeMode::Dark, MaterialMode::Auto, false);
        assert_eq!(
            a.tokens.volume_threshold.muted,
            Rgba::from_rgb(0x88, 0x88, 0x88)
        );
        assert_eq!(
            a.tokens.volume_threshold.low,
            Rgba::from_rgb(0x27, 0xAE, 0x60)
        );
        assert_eq!(
            a.tokens.volume_threshold.medium,
            Rgba::from_rgb(0x00, 0x78, 0xD4)
        );
        assert_eq!(
            a.tokens.volume_threshold.high,
            Rgba::from_rgb(0xE0, 0x5C, 0x00)
        );
    }

    #[test]
    fn dark_appearance_uses_the_signal_glass_palette() {
        let a = appearance(ThemeMode::Dark, MaterialMode::Auto, false);
        assert_eq!(a.tokens.background, Rgba::from_rgb(0x10, 0x13, 0x1A));
        assert_eq!(a.tokens.text_primary, Rgba::from_rgb(0xF5, 0xF7, 0xFA));
        assert!(a.tokens.is_dark);
    }

    #[test]
    fn appearance_exposes_signal_glass_tokens_to_the_renderer() {
        // The resolved appearance consumed by `paint` must carry the approved
        // Signal Glass state tokens, not just the legacy surface fields.
        let dark = appearance(ThemeMode::Dark, MaterialMode::Auto, false);
        let sg = dark.tokens.signal_glass();
        assert_eq!(sg.surface_subtle, Rgba::from_rgb(0x1C, 0x22, 0x2D));
        assert_eq!(sg.border_strong, Rgba::from_rgb(0x53, 0x62, 0x76));
        assert_eq!(sg.accent_pressed, Rgba::from_rgb(0x19, 0x8F, 0xEA));

        let light = appearance(ThemeMode::Light, MaterialMode::Auto, false);
        let sg = light.tokens.signal_glass();
        assert_eq!(sg.surface_subtle, Rgba::from_rgb(0xF1, 0xF4, 0xF8));
        assert_eq!(sg.border_strong, Rgba::from_rgb(0xAE, 0xB9, 0xC8));
        assert_eq!(sg.accent_pressed, Rgba::from_rgb(0x00, 0x4A, 0x8D));
    }

    #[test]
    fn system_theme_resolves_through_the_system_is_dark_callback() {
        let mut cfg = Config::default();
        cfg.appearance.theme = ThemeMode::System;

        let dark = OverlayAppearance::resolve(&cfg, &caps(true, false), || Some(true));
        assert!(dark.tokens.is_dark);

        let light = OverlayAppearance::resolve(&cfg, &caps(true, false), || Some(false));
        assert!(!light.tokens.is_dark);
    }

    #[test]
    fn high_contrast_forces_opaque_material_and_hc_tokens() {
        let a = appearance(ThemeMode::System, MaterialMode::Auto, true);
        assert!(a.material.is_opaque());
        assert!(a.tokens.high_contrast);
        assert!(a.tokens.background.is_opaque());
        assert!(a.tokens.text_primary.is_opaque());
    }

    #[test]
    fn auto_material_resolves_blurred_with_compositor_support() {
        let a = appearance(ThemeMode::Dark, MaterialMode::Auto, false);
        assert_eq!(a.material, ResolvedMaterial::Blurred);
    }

    #[test]
    fn explicit_opaque_resolves_opaque_even_with_best_capabilities() {
        let a = appearance(ThemeMode::Dark, MaterialMode::Opaque, false);
        assert_eq!(a.material, ResolvedMaterial::Opaque);
    }

    // ── logical geometry / DPI scaling ───────────────────────────────────────

    #[test]
    fn logical_geometry_is_336x88_and_scales_exactly_once_to_physical() {
        // The window logical size is the spec capsule; physical size comes
        // from a single DpiMetrics conversion (the same path `present` and
        // the canvas use).
        assert_eq!((OVERLAY_WIDTH, OVERLAY_HEIGHT), (336, 88));
        let at_100 = DpiMetrics::new(1.0);
        assert_eq!(
            (
                at_100.to_physical(OVERLAY_WIDTH),
                at_100.to_physical(OVERLAY_HEIGHT)
            ),
            (336, 88)
        );
        let at_125 = DpiMetrics::new(1.25);
        assert_eq!(
            (
                at_125.to_physical(OVERLAY_WIDTH),
                at_125.to_physical(OVERLAY_HEIGHT)
            ),
            (420, 110)
        );
        let at_150 = DpiMetrics::new(1.5);
        assert_eq!(
            (
                at_150.to_physical(OVERLAY_WIDTH),
                at_150.to_physical(OVERLAY_HEIGHT)
            ),
            (504, 132)
        );
    }

    // ── layout ───────────────────────────────────────────────────────────────

    #[test]
    fn volume_layout_matches_the_spec_rows() {
        let l = VolumeLayout::new(OVERLAY_WIDTH as f32, OVERLAY_HEIGHT as f32);
        // Padding 16 left/right/top; title left at 16.
        assert_eq!(l.title_rect.left, 16.0);
        assert_eq!(l.title_rect.top, 16.0);
        // Value right-anchored at width - 16, aligned by math, never padded.
        assert_eq!(l.value_rect.right, OVERLAY_WIDTH as f32 - 16.0);
        assert_eq!(
            text_x_origin(l.value_rect, TextAlign::Right, 0.0),
            l.value_rect.right
        );
        assert_eq!(
            text_x_origin(l.value_rect, TextAlign::Right, 42.0),
            l.value_rect.right - 42.0
        );
        // Output caption at y ≈ 44.
        assert_eq!(l.output_rect.top, 44.0);
        // Rail: full content width, 8px tall, centered in the lower band.
        assert_eq!(l.track.left, 16.0);
        assert_eq!(l.track.right, OVERLAY_WIDTH as f32 - 16.0);
        assert_eq!(l.track.height(), 8.0);
        assert_eq!(l.track.center_y(), 71.5);
        assert!(l.track.top >= l.output_rect.bottom);
    }

    // ── rail integration (pure paint plan) ───────────────────────────────────

    #[test]
    fn volume_plan_fill_edge_and_thumb_at_0_50_100() {
        let t = TrackRect {
            left: 16.0,
            right: 320.0,
            top: 67.5,
            bottom: 75.5,
        };
        for (percent, expected) in [
            (0u8, t.left),
            (50, t.left + t.width() * 0.5),
            (100, t.right),
        ] {
            let (_, _rail, geometry) = volume_plan(percent, false);
            assert_eq!(geometry.fill_right, expected, "percent {percent}");
            let MarkerGeometry::Thumb {
                center_x, radius, ..
            } = geometry.marker
            else {
                panic!("percent {percent} must be a thumb");
            };
            assert_eq!(radius, THUMB_RADIUS);
            assert_eq!(center_x, expected.clamp(t.left + radius, t.right - radius));
        }
        // Threshold fill colours for 0/50/100 (default VolumePro bands).
        let (_, rail0, _) = volume_plan(0, false);
        assert_eq!(rail0.fill_color(), rail0.thresholds.muted);
        let (_, rail50, _) = volume_plan(50, false);
        assert_eq!(rail50.fill_color(), rail50.thresholds.medium);
        let (_, rail100, _) = volume_plan(100, false);
        assert_eq!(rail100.fill_color(), rail100.thresholds.high);
    }

    #[test]
    fn muted_plan_uses_diamond_marker_and_muted_fill() {
        // Muted at 50%: the marker must be a diamond (never a thumb), at the
        // same center the thumb would use, with the muted grey fill.
        let (_, _normal, normal_geom) = volume_plan(50, false);
        let (_, muted, muted_geom) = volume_plan(50, true);

        let MarkerGeometry::Thumb { center_x, .. } = normal_geom.marker else {
            panic!("normal marker must be a thumb");
        };
        let MarkerGeometry::MutedDiamond {
            center_x: dx,
            half_size,
            ..
        } = muted_geom.marker
        else {
            panic!("muted marker must be a diamond");
        };
        assert_eq!(dx, center_x, "same marker center as the thumb");
        assert_eq!(half_size, MUTED_DIAMOND_HALF_SIZE);
        assert_eq!(muted.fill_color(), muted.thresholds.muted);
    }

    #[test]
    fn rail_carries_user_threshold_boundaries_into_the_fill() {
        // A custom band config (green 25 / blue 60) must drive the rail fill
        // through the paint plan, matching core::volume_color_rgb semantics —
        // the same agreement Task 4 proves at the rail level.
        let mut cfg = Config::default();
        cfg.color_thresholds.green_up_to = 25;
        cfg.color_thresholds.blue_up_to = 60;
        let data = OverlayData {
            percent: 26, // medium band only under the custom config
            muted: false,
            thresholds: cfg.color_thresholds,
            appearance: appearance(ThemeMode::Dark, MaterialMode::Opaque, false),
            toast: None,
        };
        let plan = paint_plan(&data, 336.0, 88.0);
        let PaintPlanKind::Volume { rail, .. } = &plan.kind else {
            panic!("expected a volume plan");
        };
        assert_eq!(rail.green_up_to, 25);
        assert_eq!(rail.blue_up_to, 60);
        assert_eq!(rail.fill_color(), rail.thresholds.medium);
    }

    // ── toast vs volume mode ─────────────────────────────────────────────────

    #[test]
    fn toast_plan_renders_text_without_a_rail() {
        let data = OverlayData {
            percent: 72,
            muted: false,
            thresholds: Config::default().color_thresholds,
            appearance: appearance(ThemeMode::Dark, MaterialMode::Opaque, false),
            toast: Some("Config reloaded".to_string()),
        };
        let plan = paint_plan(&data, 336.0, 88.0);
        let PaintPlanKind::Toast { text, rect } = &plan.kind else {
            panic!("toast data must produce a toast plan");
        };
        assert_eq!(*text, "Config reloaded");
        // Centered body line: vertical center at the surface center.
        assert_eq!((rect.top + rect.bottom) * 0.5, 44.0);
    }

    #[test]
    fn volume_plan_is_chosen_when_no_toast_is_set() {
        let data = OverlayData {
            percent: 72,
            muted: false,
            thresholds: Config::default().color_thresholds,
            appearance: appearance(ThemeMode::Dark, MaterialMode::Opaque, false),
            toast: None,
        };
        let plan = paint_plan(&data, 336.0, 88.0);
        let PaintPlanKind::Volume { .. } = &plan.kind else {
            panic!("volume data must produce a volume plan (with a rail)");
        };
    }

    // ── motion policy ────────────────────────────────────────────────────────

    #[test]
    fn motion_resolves_from_config_against_capabilities_and_is_carried() {
        let mut cfg = Config::default();
        cfg.appearance.motion = MotionMode::Full;
        let mut caps = caps(true, false);
        caps.reduced_motion = false;
        assert_eq!(
            OverlayAppearance::resolve(&cfg, &caps, || None).motion,
            MotionMode::Full
        );
        caps.reduced_motion = true;
        assert_eq!(
            OverlayAppearance::resolve(&cfg, &caps, || None).motion,
            MotionMode::Reduced,
            "system reduced-motion downgrades Full to Reduced"
        );

        cfg.appearance.motion = MotionMode::Reduced;
        assert_eq!(
            OverlayAppearance::resolve(&cfg, &caps, || None).motion,
            MotionMode::Reduced
        );

        cfg.appearance.motion = MotionMode::Disabled;
        assert_eq!(
            OverlayAppearance::resolve(&cfg, &caps, || None).motion,
            MotionMode::Disabled,
            "Disabled never animates regardless of capabilities"
        );
    }

    // ── placement (16px mixer gap preserved) ─────────────────────────────────

    #[test]
    fn placement_keeps_the_exact_16px_gap_above_the_336x88_overlay() {
        // The mixer consumes the OVERLAY_* constants directly, so the gap
        // survives the size change by construction.
        let work_area = WorkArea::new(0, 0, 2560, 1400);
        let overlay = place_overlay(
            work_area,
            SurfaceSize::new(OVERLAY_WIDTH, OVERLAY_HEIGHT),
            OVERLAY_MARGIN_X,
            OVERLAY_MARGIN_Y,
        );
        assert_eq!(overlay.width(), 336);
        assert_eq!(overlay.height(), 88);
        assert_eq!(overlay.right, work_area.right() - OVERLAY_MARGIN_X);
        assert_eq!(overlay.bottom, work_area.bottom() - OVERLAY_MARGIN_Y);

        let mixer = place_mixer_above_overlay(
            work_area,
            SurfaceSize::new(400, 224),
            SurfaceSize::new(OVERLAY_WIDTH, OVERLAY_HEIGHT),
            OVERLAY_MARGIN_X,
            OVERLAY_MARGIN_Y,
            16,
        );
        assert_eq!(mixer.bottom + 16, overlay.top);
        assert_eq!(mixer.right, overlay.right);
    }
}
