//! Windows rendering primitives: adaptive capability detection and surface
//! styling.
//!
//! This module only compiles on `target_os = "windows"` (it is reached through
//! `ui::platform`, which is cfg-gated). Detection is best-effort: every system
//! query degrades to a safe default instead of panicking, so a renderer always
//! receives a usable [`UiCapabilities`] snapshot even on an older or unusual
//! system.
//!
//! All `windows-sys` FFI signatures below were verified against the installed
//! `windows-sys` 0.52 crate source (cargo registry) before use.

use windows_sys::Win32::{
    Foundation::{BOOL, HWND},
    Graphics::Dwm::{
        DwmGetWindowAttribute, DwmIsCompositionEnabled, DwmSetWindowAttribute, DWMSBT_NONE,
        DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE,
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    },
    Graphics::Gdi::{
        GetDC, GetDeviceCaps, GetMonitorInfoW, MonitorFromWindow, ReleaseDC, LOGPIXELSX,
        MONITORINFO, MONITOR_DEFAULTTONEAREST,
    },
    System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD},
    UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
    UI::Controls::SetWindowTheme,
    UI::HiDpi::{GetDpiForSystem, GetDpiForWindow},
    UI::WindowsAndMessaging::{
        GetSystemMetrics, GetWindowLongPtrW, SystemParametersInfoW, GWL_EXSTYLE,
        SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
        SPI_GETCLIENTAREAANIMATION, SPI_GETHIGHCONTRAST, WS_EX_LAYERED,
    },
};

use crate::ui::capabilities::{ResolvedMaterial, UiCapabilities};
use crate::ui::surface::WorkArea;
use crate::ui::theme::Rgba;

/// The registry value that records whether the user prefers light apps.
const LIGHT_THEME_VALUE: u32 = 1;

/// Whether the current user prefers the dark app theme.
///
/// Reads `AppsUseLightTheme` (a DWORD) from
/// `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize`
/// via `RegGetValueW`. Returns:
/// - `Some(true)` when the value reads `0` (dark),
/// - `Some(false)` when it reads non-zero (light),
/// - `None` when the value is missing or unreadable.
///
/// `None` plugs directly into [`crate::ui::theme::tokens_for`]'s
/// `system_is_dark` callback, which falls back to the light palette.
pub fn system_theme() -> Option<bool> {
    unsafe {
        let mut value = 0u32;
        let mut size = std::mem::size_of::<u32>() as u32;
        let ok = RegGetValueW(
            HKEY_CURRENT_USER,
            windows_sys::core::w!(
                "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"
            ),
            windows_sys::core::w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            &mut value as *mut u32 as *mut _,
            &mut size,
        ) == 0;
        ok.then_some(value != LIGHT_THEME_VALUE)
    }
}

/// Convert an [`Rgba`] to a Win32 `COLORREF`.
///
/// `COLORREF` packs `0x00BBGGRR` — the red/blue channels are swapped relative
/// to the RGBA field order. The alpha channel is dropped (Win32 fills have no
/// per-pixel alpha via `COLORREF`; translucency is expressed separately).
pub fn colorref(rgba: Rgba) -> u32 {
    (u32::from(rgba.red)) | (u32::from(rgba.green) << 8) | (u32::from(rgba.blue) << 16)
}

/// Convert a logical (design) pixel count to physical pixels for a DPI scale.
///
/// Rounding: `f32::round`, i.e. round-half-away-from-zero to the nearest
/// integer pixel. A 1.25x scale maps 320 logical px to 400 physical px, and a
/// 2.5 px measure maps to 3 px.
pub fn scale_px(dpi_scale: f32, px: i32) -> i32 {
    (px as f32 * dpi_scale).round() as i32
}

/// Whether the desktop compositor (DWM) is enabled.
pub fn compositor_enabled() -> bool {
    unsafe {
        let mut enabled: BOOL = 0;
        DwmIsCompositionEnabled(&mut enabled) == 0 && enabled != 0
    }
}

/// Whether system high-contrast mode is active.
///
/// Uses `SystemParametersInfoW(SPI_GETHIGHCONTRAST, ...)` and checks
/// `HIGHCONTRAST.dwFlags & HCF_HIGHCONTRASTON`. A query failure reports `false`
/// (treating the session as normal contrast).
pub fn high_contrast_enabled() -> bool {
    unsafe {
        let mut hc: HIGHCONTRASTW = std::mem::zeroed();
        hc.cbSize = std::mem::size_of::<HIGHCONTRASTW>() as u32;
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            hc.cbSize,
            &mut hc as *mut HIGHCONTRASTW as *mut _,
            0,
        ) != 0
            && (hc.dwFlags & HCF_HIGHCONTRASTON) != 0
    }
}

/// Whether the system prefers reduced animation.
///
/// Reads `SPI_GETCLIENTAREAANIMATION` (Windows 11 "Animation effects" toggle
/// for controls and elements inside windows). When the toggle is off the
/// system prefers reduced motion. On Windows 10 the flag is unknown, the
/// query fails, and this reports `false` (animations allowed).
pub fn reduced_motion_enabled() -> bool {
    unsafe {
        let mut enabled: BOOL = 0;
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            &mut enabled as *mut BOOL as *mut _,
            0,
        ) != 0
            && enabled == 0
    }
}

/// DPI scale factor (for example 1.0, 1.25, 1.5) for the display hosting
/// `hwnd`.
///
/// Tries `GetDpiForWindow`, then `GetDpiForSystem`, then falls back to
/// `GetDeviceCaps(LOGPIXELSX)`; the scale is `dpi / 96.0`, floored at 1.0.
pub fn dpi_scale_for(hwnd: HWND) -> f32 {
    let dpi = unsafe {
        let per_window = GetDpiForWindow(hwnd);
        if per_window != 0 {
            per_window
        } else {
            GetDpiForSystem()
        }
    };
    let dpi = if dpi > 0 {
        dpi
    } else {
        // Very old system without per-window/system DPI APIs.
        unsafe {
            let hdc = GetDC(hwnd);
            if hdc != 0 {
                let logical = GetDeviceCaps(hdc, LOGPIXELSX as i32);
                ReleaseDC(hwnd, hdc);
                if logical > 0 {
                    logical as u32
                } else {
                    96
                }
            } else {
                96
            }
        }
    };
    (dpi as f32 / 96.0).max(1.0)
}

/// Whether `hwnd` is a layered window (`WS_EX_LAYERED`).
///
/// Layered windows own a per-window surface (DIB) that the desktop compositor
/// blends with the screen. `ID2D1HwndRenderTarget` presents through a
/// redirection path that does not land in that surface, so D2D-drawn content
/// never becomes visible on layered windows; GDI paints straight into the
/// surface and always works (see [`PaintCanvas::begin_paint`]).
fn is_layered(hwnd: HWND) -> bool {
    unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) & WS_EX_LAYERED as isize != 0 }
}

/// Whether `hwnd` has a DWM system backdrop active (a Windows 11 acrylic /
/// mica attribute such as `DWMSBT_TRANSIENTWINDOW`).
///
/// Backdrop windows are composed by DWM the same way layered windows are —
/// the backdrop is drawn into the window's own composited surface — and
/// `ID2D1HwndRenderTarget` presents do not land there either (live-verified
/// on Windows 11: the mixer with a transient-window backdrop rendered a
/// uniform blur-tinted surface under D2D while the same window painted
/// correctly via GDI). See [`PaintCanvas::begin_paint`] for the shared rule.
fn backdrop_active(hwnd: HWND) -> bool {
    unsafe {
        let mut value: i32 = 0;
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE as u32,
            &mut value as *mut i32 as *mut _,
            std::mem::size_of::<i32>() as u32,
        ) == 0
            && value != DWMSBT_NONE
    }
}

/// Whether D2D hwnd render targets can present into `hwnd` at all.
///
/// DWM-owned surfaces (layered windows and system-backdrop windows) never
/// show `ID2D1HwndRenderTarget` output; GDI paints straight into them and
/// always works, so the canvas selects the GDI path for both (see
/// [`PaintCanvas::begin_paint`]).
fn d2d_present_supported(hwnd: HWND) -> bool {
    !is_layered(hwnd) && !backdrop_active(hwnd)
}

/// Monitor work area for the display nearest `hwnd`.
///
/// Uses `MonitorFromWindow(MONITOR_DEFAULTTONEAREST)` +
/// `GetMonitorInfoW` and converts `rcWork` into a [`WorkArea`]. `RECT`'s
/// right/bottom are exclusive, matching [`WorkArea::right`]/[`WorkArea::bottom`],
/// so `width = rc.right - rc.left` and `height = rc.bottom - rc.top`.
///
/// Falls back to the virtual screen bounds (union of all monitors) if the
/// monitor query fails.
pub fn work_area_for(hwnd: HWND) -> WorkArea {
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if monitor != 0 {
            let mut info: MONITORINFO = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            if GetMonitorInfoW(monitor, &mut info) != 0 {
                let r = info.rcWork;
                return WorkArea::new(r.left, r.top, r.right - r.left, r.bottom - r.top);
            }
        }
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let cx = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let cy = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        WorkArea::new(x, y, cx, cy)
    }
}

/// Snapshot the capability set of the current display session.
pub fn detect_capabilities(hwnd: HWND) -> UiCapabilities {
    let compositor = compositor_enabled();
    UiCapabilities {
        compositor,
        // Backdrop blur needs DWM composition plus Windows 11 system
        // backdrops. windows-sys 0.52 exposes no direct "backdrop supported"
        // query, so composition is the floor; `apply_backdrop` degrades
        // gracefully when the attribute is unsupported (Windows 10).
        blur: compositor,
        high_contrast: high_contrast_enabled(),
        reduced_motion: reduced_motion_enabled(),
        dpi_scale: dpi_scale_for(hwnd),
        work_area: work_area_for(hwnd),
    }
}

/// Apply DWM window styling for a resolved material.
///
/// Sets the immersive dark-mode flag, rounds the window corners, and — for a
/// blurred material — requests the Windows 11 acrylic backdrop
/// (`DWMSBT_TRANSIENTWINDOW`). Translucent and opaque materials explicitly
/// clear any system backdrop so the renderer paints its own fill.
///
/// Returns `true` when a system backdrop is active (the window background is
/// provided by the compositor and the renderer can draw transparent areas),
/// and `false` when the renderer must paint an opaque/translucent fill itself
/// (Windows 10, where the attribute is unsupported and the call fails).
pub fn apply_backdrop(hwnd: HWND, material: ResolvedMaterial, dark: bool) -> bool {
    let _ = set_dwm_attr(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, i32::from(dark));
    let _ = set_dwm_attr(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND);
    match material {
        ResolvedMaterial::Blurred => {
            set_dwm_attr(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, DWMSBT_TRANSIENTWINDOW).is_ok()
        }
        ResolvedMaterial::Translucent | ResolvedMaterial::Opaque => {
            let _ = set_dwm_attr(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, DWMSBT_NONE);
            false
        }
    }
}

/// Apply the immersive theme to standard common controls.
///
/// Uses `SetWindowTheme` with `DarkMode_Explorer` for dark surfaces and
/// restores the default theme for light surfaces, so buttons/trackbars follow
/// the requested appearance.
pub fn theme_controls(controls: &[HWND], dark: bool) {
    for &hwnd in controls {
        unsafe {
            if dark {
                let _ = SetWindowTheme(
                    hwnd,
                    windows_sys::core::w!("DarkMode_Explorer"),
                    std::ptr::null(),
                );
            } else {
                // Reset to the standard theme.
                let _ = SetWindowTheme(hwnd, std::ptr::null(), std::ptr::null());
            }
        }
    }
}

/// Best-effort `DwmSetWindowAttribute` wrapper for 4-byte attributes.
fn set_dwm_attr(hwnd: HWND, attribute: i32, value: i32) -> std::result::Result<(), ()> {
    unsafe {
        let hr = DwmSetWindowAttribute(hwnd, attribute as u32, &value as *const i32 as *const _, 4);
        if hr == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colorref_uses_bbggrr_byte_order() {
        assert_eq!(colorref(Rgba::from_rgb(0x12, 0x34, 0x56)), 0x0056_3412);
        assert_eq!(colorref(Rgba::from_rgb(0xFF, 0x00, 0x00)), 0x0000_00FF);
        assert_eq!(colorref(Rgba::from_rgb(0x00, 0xFF, 0x00)), 0x0000_FF00);
        assert_eq!(colorref(Rgba::from_rgb(0x00, 0x00, 0xFF)), 0x00FF_0000);
    }

    #[test]
    fn colorref_ignores_alpha_channel() {
        let translucent = Rgba::from_rgb(0x11, 0x22, 0x33).with_alpha(0x80);
        assert_eq!(colorref(translucent), 0x0033_2211);
        assert_eq!(colorref(Rgba::from_rgb(0x11, 0x22, 0x33)), 0x0033_2211);
    }

    #[test]
    fn scale_px_is_identity_at_scale_one() {
        assert_eq!(scale_px(1.0, 0), 0);
        assert_eq!(scale_px(1.0, 178), 178);
    }

    #[test]
    fn scale_px_rounds_half_away_from_zero() {
        // f32::round rounds half away from zero: 12.5 -> 13, 2.5 -> 3.
        assert_eq!(scale_px(1.25, 10), 13);
        assert_eq!(scale_px(1.25, 2), 3);
    }

    #[test]
    fn scale_px_rounds_to_nearest_pixel() {
        assert_eq!(scale_px(1.25, 320), 400);
        assert_eq!(scale_px(1.5, 64), 96);
        assert_eq!(scale_px(1.5, 320), 480);
        assert_eq!(scale_px(2.0, 40), 80);
    }

    #[test]
    fn scale_px_rounds_down_when_below_half() {
        assert_eq!(scale_px(1.25, 1), 1); // 1.25 -> 1
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Signal Glass drawing primitives (Task 3): DPI metrics, float geometry,
// pure shape helpers, and the resource-safe GDI/D2D paint canvas.
// ─────────────────────────────────────────────────────────────────────────────

use std::marker::PhantomData;

use windows_sys::Win32::{
    Foundation::{POINT, RECT},
    Graphics::Gdi::{
        BeginPaint, CreatePen, CreateSolidBrush, DeleteObject, DrawTextW, Ellipse, EndPaint,
        FillRect, GetStockObject, InvalidateRect, Polygon, RoundRect, SelectObject, SetBkMode,
        SetTextColor, DT_CENTER, DT_LEFT, DT_NOPREFIX, DT_RIGHT, DT_SINGLELINE, HDC, NULL_BRUSH,
        NULL_PEN, PAINTSTRUCT, PS_SOLID, TRANSPARENT,
    },
};

use crate::ui::theme::FocusTokens;

use super::d2d::{Direct2dContext, HwndRenderTarget};
use super::text::{create_font_selected, TextAlign, TextLayout};

/// Rectangle with float coordinates in LOGICAL pixels.
///
/// Independent of the integer-pixel [`crate::ui::surface::SurfaceRect`] world:
/// layout is authored in design pixels and scaled exactly once by
/// [`DpiMetrics`] inside the canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectF {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl RectF {
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub const fn width(self) -> f32 {
        self.right - self.left
    }

    pub const fn height(self) -> f32 {
        self.bottom - self.top
    }

    /// Expand (or, with a negative amount, shrink) on all four sides.
    pub fn inflated(self, amount: f32) -> Self {
        Self {
            left: self.left - amount,
            top: self.top - amount,
            right: self.right + amount,
            bottom: self.bottom + amount,
        }
    }

    /// Whether `other` lies fully inside `self` (inclusive edges).
    pub fn contains(self, other: Self) -> bool {
        other.left >= self.left
            && other.top >= self.top
            && other.right <= self.right
            && other.bottom <= self.bottom
    }

    pub fn min_dimension(self) -> f32 {
        self.width().min(self.height())
    }
}

/// Point with float coordinates in LOGICAL pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointF {
    pub x: f32,
    pub y: f32,
}

impl PointF {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Float size in LOGICAL pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SizeF {
    pub width: f32,
    pub height: f32,
}

impl SizeF {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

/// DPI scale metrics for one display.
///
/// The scale is clamped to the supported `[0.5, 4.0]` range (48–384 DPI);
/// non-finite input (NaN, infinity) resolves to the identity scale 1.0 so a
/// bad system query can never panic or produce degenerate geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DpiMetrics {
    scale: f32,
}

impl DpiMetrics {
    /// Create metrics from a scale, clamped to `[0.5, 4.0]`.
    pub fn new(scale: f32) -> Self {
        let scale = if scale.is_finite() {
            scale.clamp(0.5, 4.0)
        } else {
            1.0
        };
        Self { scale }
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Convert LOGICAL pixels to PHYSICAL pixels (round-half-away-from-zero,
    /// same semantics as [`scale_px`]).
    pub fn to_physical(&self, logical: i32) -> i32 {
        scale_px(self.scale, logical)
    }

    /// Convert PHYSICAL pixels to LOGICAL pixels (round-half-away-from-zero).
    pub fn to_logical(&self, physical: i32) -> i32 {
        (physical as f32 / self.scale).round() as i32
    }
}

/// A rounded-rectangle geometry: rect plus corner radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundedRectPath {
    pub rect: RectF,
    pub radius_x: f32,
    pub radius_y: f32,
}

/// Build a rounded rectangle path from a rect and a corner radius.
///
/// The radius is clamped to `[0, min(width, height) / 2]` so the corners can
/// never swallow the shape; a negative radius becomes 0 (a plain rectangle).
pub fn rounded_rect_path(rect: RectF, radius: f32) -> RoundedRectPath {
    let radius = radius.max(0.0).min(rect.min_dimension() * 0.5);
    RoundedRectPath {
        rect,
        radius_x: radius,
        radius_y: radius,
    }
}

/// Four corners of a diamond (muted-state rail marker) around `center`.
///
/// Points are top, right, bottom, left. `half_size` is clamped to `>= 0`; a
/// zero-size diamond degenerates to all points at the center.
pub fn diamond_points(center: PointF, half_size: f32) -> [PointF; 4] {
    let half_size = half_size.max(0.0);
    [
        PointF::new(center.x, center.y - half_size),
        PointF::new(center.x + half_size, center.y),
        PointF::new(center.x, center.y + half_size),
        PointF::new(center.x - half_size, center.y),
    ]
}

/// Bounding boxes of the two focus-ring layers around `rect`.
///
/// Returns `(outer, inner)`: each box is `rect` inflated by the layer's
/// `gap + width / 2` so a centered stroke of that width lands at the token
/// gap from the control edge. The theme invariants (`inner_ring_gap +
/// inner_ring_width < ring_gap`) guarantee the inner layer sits entirely
/// inside the outer one with an air gap between the strokes.
pub fn focus_ring_rects(rect: RectF, focus: &FocusTokens) -> (RectF, RectF) {
    let outer = rect.inflated(focus.ring_gap_px + focus.ring_width_px * 0.5);
    let inner = rect.inflated(focus.inner_ring_gap_px + focus.inner_ring_width_px * 0.5);
    (outer, inner)
}

/// Corner radius for focus rings around a control: the 4px control radius,
/// never exceeding half the control's smallest dimension.
fn focus_ring_radius(rect: RectF) -> f32 {
    (rect.min_dimension() * 0.5).min(4.0)
}

/// Resource-safe paint surface for one `WM_PAINT`.
///
/// `begin_paint` calls `BeginPaint` (RAII `EndPaint` on drop) and picks ONE
/// paint path for the whole frame:
///
/// - **D2D mode** — a Direct2D hwnd render target was created for the window
///   (alpha-capable, DWrite text). Every primitive routes through D2D.
/// - **GDI mode** — the always-working baseline. Every primitive draws via
///   GDI with per-call brushes/pen/fonts that are deleted immediately.
///
/// The paths are never mixed within one frame (a D2D `EndDraw` present would
/// overwrite GDI front-buffer writes). If a D2D operation fails mid-paint the
/// frame is abandoned and the window invalidated, so the next paint renders
/// correctly — GDI if D2D is still unavailable — with a debug log.
///
/// All coordinates are LOGICAL pixels; scaling happens once via the window's
/// [`DpiMetrics`].
pub struct PaintCanvas<'a> {
    hwnd: HWND,
    hdc: HDC,
    paint: PAINTSTRUCT,
    dpi: DpiMetrics,
    d2d: Option<HwndRenderTarget>,
    d2d_broken: bool,
    _lifetime: PhantomData<&'a ()>,
}

impl<'a> PaintCanvas<'a> {
    /// Begin painting `hwnd`. `None` when `BeginPaint` itself fails.
    pub fn begin_paint(hwnd: HWND) -> Option<Self> {
        unsafe {
            let mut paint: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut paint);
            if hdc == 0 {
                return None;
            }
            let dpi = DpiMetrics::new(dpi_scale_for(hwnd));
            // D2D mode is all-or-nothing per paint (see module docs).
            // DWM-owned surfaces skip D2D entirely: on layered windows
            // (WS_EX_LAYERED, e.g. the click-through volume overlay) and on
            // system-backdrop windows (Windows 11 acrylic, e.g. the mixer), an
            // ID2D1HwndRenderTarget presents through a redirection path that
            // never lands in the window's own surface (live-verified on
            // Windows 11: both the layered overlay and the backdrop mixer
            // rendered a stale uniform surface under D2D while the same
            // windows painted correctly via GDI, which writes straight into
            // the surface). GDI remains the always-working baseline.
            let d2d = if d2d_present_supported(hwnd) {
                Direct2dContext::get()
                    .and_then(|context| context.render_target(hwnd))
                    .and_then(|mut target| if target.begin() { Some(target) } else { None })
            } else {
                log::debug!(
                    "PaintCanvas: {hwnd:?} has a DWM-owned surface (layered or \
                     system backdrop); hwnd render targets cannot present into \
                     it — painting via GDI"
                );
                None
            };
            if d2d.is_none() {
                log::debug!("PaintCanvas: D2D unavailable for {hwnd:?}; painting via GDI");
            }
            Some(Self {
                hwnd,
                hdc,
                paint,
                dpi,
                d2d,
                d2d_broken: false,
                _lifetime: PhantomData,
            })
        }
    }

    /// The DPI scale this canvas converts logical coordinates with.
    pub fn dpi(&self) -> DpiMetrics {
        self.dpi
    }

    /// Whether this paint is routed through Direct2D.
    pub fn d2d_active(&self) -> bool {
        self.d2d.is_some() && !self.d2d_broken
    }

    pub fn fill_rect(&mut self, rect: RectF, color: crate::ui::theme::Rgba) {
        if let Some(target) = &mut self.d2d {
            if !self.d2d_broken && !target.fill_rect(rect, color) {
                self.d2d_failed("fill_rect");
            }
            return;
        }
        self.gdi_fill_rect(rect, color);
    }

    pub fn fill_rounded_rect(&mut self, rect: RectF, radius: f32, color: crate::ui::theme::Rgba) {
        if let Some(target) = &mut self.d2d {
            if !self.d2d_broken && !target.fill_rounded_rect(rect, radius, color) {
                self.d2d_failed("fill_rounded_rect");
            }
            return;
        }
        self.gdi_fill_rounded_rect(rect, radius, color);
    }

    pub fn stroke_rounded_rect(
        &mut self,
        rect: RectF,
        radius: f32,
        color: crate::ui::theme::Rgba,
        width_px: f32,
    ) {
        if let Some(target) = &mut self.d2d {
            if !self.d2d_broken && !target.stroke_rounded_rect(rect, radius, color, width_px) {
                self.d2d_failed("stroke_rounded_rect");
            }
            return;
        }
        self.gdi_stroke_rounded_rect(rect, radius, color, width_px);
    }

    pub fn fill_circle(&mut self, center: PointF, radius: f32, color: crate::ui::theme::Rgba) {
        if let Some(target) = &mut self.d2d {
            if !self.d2d_broken && !target.fill_circle(center, radius, color) {
                self.d2d_failed("fill_circle");
            }
            return;
        }
        self.gdi_fill_circle(center, radius, color);
    }

    pub fn stroke_circle(
        &mut self,
        center: PointF,
        radius: f32,
        color: crate::ui::theme::Rgba,
        width_px: f32,
    ) {
        if let Some(target) = &mut self.d2d {
            if !self.d2d_broken && !target.stroke_circle(center, radius, color, width_px) {
                self.d2d_failed("stroke_circle");
            }
            return;
        }
        self.gdi_stroke_circle(center, radius, color, width_px);
    }

    pub fn fill_diamond(&mut self, center: PointF, half_size: f32, color: crate::ui::theme::Rgba) {
        if let Some(target) = &mut self.d2d {
            if !self.d2d_broken && !target.fill_diamond(center, half_size, color) {
                self.d2d_failed("fill_diamond");
            }
            return;
        }
        self.gdi_fill_diamond(center, half_size, color);
    }

    pub fn stroke_diamond(
        &mut self,
        center: PointF,
        half_size: f32,
        color: crate::ui::theme::Rgba,
        width_px: f32,
    ) {
        if let Some(target) = &mut self.d2d {
            if !self.d2d_broken && !target.stroke_diamond(center, half_size, color, width_px) {
                self.d2d_failed("stroke_diamond");
            }
            return;
        }
        self.gdi_stroke_diamond(center, half_size, color, width_px);
    }

    /// Draw the focus-visible indicator: BOTH layers of the two-layer
    /// [`FocusTokens`] ring (outer accent ring + inner contrast ring), each
    /// with its own color, width, and gap.
    pub fn draw_focus_ring(&mut self, rect: RectF, focus: &FocusTokens) {
        let (outer, inner) = focus_ring_rects(rect, focus);
        let radius = focus_ring_radius(rect);
        // Inner contrast layer first, then the outer accent layer.
        self.stroke_rounded_rect(inner, radius, focus.inner_ring, focus.inner_ring_width_px);
        self.stroke_rounded_rect(outer, radius, focus.ring, focus.ring_width_px);
    }

    /// Draw text. Returns `false` when the paint failed (D2D mode falls back
    /// on the next paint; GDI mode returns `false` if no font could be made).
    pub fn draw_text(&mut self, layout: &TextLayout) -> bool {
        if let Some(target) = &mut self.d2d {
            if self.d2d_broken {
                return false;
            }
            if !target.draw_text(layout) {
                self.d2d_failed("draw_text");
                return false;
            }
            return true;
        }
        self.gdi_draw_text(layout)
    }

    /// Mark the D2D frame broken: abandon remaining draws and invalidate so
    /// the next paint re-renders (via GDI if D2D is still unavailable).
    fn d2d_failed(&mut self, op: &str) {
        if self.d2d_broken {
            return;
        }
        self.d2d_broken = true;
        log::debug!("PaintCanvas: D2D {op} failed; next paint will fall back");
        unsafe {
            InvalidateRect(self.hwnd, std::ptr::null(), 0);
        }
    }

    // ── GDI implementations (authoritative baseline) ──────────────────────

    fn gdi_fill_rect(&self, rect: RectF, color: crate::ui::theme::Rgba) {
        unsafe {
            let brush = CreateSolidBrush(colorref(color));
            if brush == 0 {
                return;
            }
            let rect = self.physical_rect(rect);
            FillRect(self.hdc, &rect, brush);
            DeleteObject(brush as _);
        }
    }

    fn gdi_fill_rounded_rect(&self, rect: RectF, radius: f32, color: crate::ui::theme::Rgba) {
        let path = rounded_rect_path(rect, radius);
        unsafe {
            let brush = CreateSolidBrush(colorref(color));
            if brush == 0 {
                return;
            }
            let rect = self.physical_rect(path.rect);
            let radii = self.round_rect_radii(path.radius_x);
            let previous_brush = SelectObject(self.hdc, brush as _);
            let previous_pen = SelectObject(self.hdc, GetStockObject(NULL_PEN));
            RoundRect(
                self.hdc,
                rect.left,
                rect.top,
                rect.right,
                rect.bottom,
                radii.0,
                radii.1,
            );
            SelectObject(self.hdc, previous_pen);
            SelectObject(self.hdc, previous_brush);
            DeleteObject(brush as _);
        }
    }

    fn gdi_stroke_rounded_rect(
        &self,
        rect: RectF,
        radius: f32,
        color: crate::ui::theme::Rgba,
        width_px: f32,
    ) {
        let path = rounded_rect_path(rect, radius);
        unsafe {
            let pen = CreatePen(
                PS_SOLID,
                self.physical_stroke_width(width_px),
                colorref(color),
            );
            if pen == 0 {
                return;
            }
            let rect = self.physical_rect(path.rect);
            let radii = self.round_rect_radii(path.radius_x);
            let previous_pen = SelectObject(self.hdc, pen as _);
            let previous_brush = SelectObject(self.hdc, GetStockObject(NULL_BRUSH));
            RoundRect(
                self.hdc,
                rect.left,
                rect.top,
                rect.right,
                rect.bottom,
                radii.0,
                radii.1,
            );
            SelectObject(self.hdc, previous_brush);
            SelectObject(self.hdc, previous_pen);
            DeleteObject(pen as _);
        }
    }

    fn gdi_fill_circle(&self, center: PointF, radius: f32, color: crate::ui::theme::Rgba) {
        unsafe {
            let brush = CreateSolidBrush(colorref(color));
            if brush == 0 {
                return;
            }
            let box_ = self.circle_bounds(center, radius);
            let previous_brush = SelectObject(self.hdc, brush as _);
            let previous_pen = SelectObject(self.hdc, GetStockObject(NULL_PEN));
            Ellipse(self.hdc, box_.left, box_.top, box_.right, box_.bottom);
            SelectObject(self.hdc, previous_pen);
            SelectObject(self.hdc, previous_brush);
            DeleteObject(brush as _);
        }
    }

    fn gdi_stroke_circle(
        &self,
        center: PointF,
        radius: f32,
        color: crate::ui::theme::Rgba,
        width_px: f32,
    ) {
        unsafe {
            let pen = CreatePen(
                PS_SOLID,
                self.physical_stroke_width(width_px),
                colorref(color),
            );
            if pen == 0 {
                return;
            }
            let box_ = self.circle_bounds(center, radius);
            let previous_pen = SelectObject(self.hdc, pen as _);
            let previous_brush = SelectObject(self.hdc, GetStockObject(NULL_BRUSH));
            Ellipse(self.hdc, box_.left, box_.top, box_.right, box_.bottom);
            SelectObject(self.hdc, previous_brush);
            SelectObject(self.hdc, previous_pen);
            DeleteObject(pen as _);
        }
    }

    fn gdi_fill_diamond(&self, center: PointF, half_size: f32, color: crate::ui::theme::Rgba) {
        unsafe {
            let brush = CreateSolidBrush(colorref(color));
            if brush == 0 {
                return;
            }
            let points = self.physical_points(diamond_points(center, half_size));
            let previous_brush = SelectObject(self.hdc, brush as _);
            let previous_pen = SelectObject(self.hdc, GetStockObject(NULL_PEN));
            Polygon(self.hdc, points.as_ptr(), points.len() as i32);
            SelectObject(self.hdc, previous_pen);
            SelectObject(self.hdc, previous_brush);
            DeleteObject(brush as _);
        }
    }

    fn gdi_stroke_diamond(
        &self,
        center: PointF,
        half_size: f32,
        color: crate::ui::theme::Rgba,
        width_px: f32,
    ) {
        unsafe {
            let pen = CreatePen(
                PS_SOLID,
                self.physical_stroke_width(width_px),
                colorref(color),
            );
            if pen == 0 {
                return;
            }
            let points = self.physical_points(diamond_points(center, half_size));
            let previous_pen = SelectObject(self.hdc, pen as _);
            let previous_brush = SelectObject(self.hdc, GetStockObject(NULL_BRUSH));
            Polygon(self.hdc, points.as_ptr(), points.len() as i32);
            SelectObject(self.hdc, previous_brush);
            SelectObject(self.hdc, previous_pen);
            DeleteObject(pen as _);
        }
    }

    fn gdi_draw_text(&mut self, layout: &TextLayout) -> bool {
        if layout.text.is_empty() {
            return true;
        }
        unsafe {
            let wide: Vec<u16> = layout.text.encode_utf16().collect();
            let height_px = self.dpi.to_physical(layout.role.size_px.round() as i32);
            let (font, previous) = create_font_selected(self.hdc, layout.role, height_px);
            if font == 0 {
                return false;
            }
            // `create_font_selected` already selected the font into the DC.
            // Restore the object it displaced BEFORE deleting the font — a
            // selected object cannot be deleted, and deleting it anyway would
            // leak one HFONT on every GDI text draw.
            SetTextColor(self.hdc, colorref(layout.color));
            SetBkMode(self.hdc, TRANSPARENT as i32);
            let mut rect = self.physical_rect(layout.rect);
            let align = match layout.align {
                TextAlign::Left => DT_LEFT,
                TextAlign::Right => DT_RIGHT,
                TextAlign::Center => DT_CENTER,
            };
            let drawn = DrawTextW(
                self.hdc,
                wide.as_ptr(),
                wide.len() as i32,
                &mut rect,
                DT_SINGLELINE | DT_NOPREFIX | align,
            );
            if previous != 0 {
                SelectObject(self.hdc, previous);
            }
            DeleteObject(font as _);
            drawn != 0
        }
    }

    // ── coordinate helpers ────────────────────────────────────────────────

    fn physical_rect(&self, rect: RectF) -> RECT {
        RECT {
            left: self.dpi.to_physical(rect.left.round() as i32),
            top: self.dpi.to_physical(rect.top.round() as i32),
            right: self.dpi.to_physical(rect.right.round() as i32),
            bottom: self.dpi.to_physical(rect.bottom.round() as i32),
        }
    }

    fn physical_point(&self, point: PointF) -> POINT {
        POINT {
            x: self.dpi.to_physical(point.x.round() as i32),
            y: self.dpi.to_physical(point.y.round() as i32),
        }
    }

    fn physical_points(&self, points: [PointF; 4]) -> [POINT; 4] {
        [
            self.physical_point(points[0]),
            self.physical_point(points[1]),
            self.physical_point(points[2]),
            self.physical_point(points[3]),
        ]
    }

    /// Physical stroke width, at least 1 px.
    fn physical_stroke_width(&self, width_px: f32) -> i32 {
        self.dpi.to_physical(width_px.round() as i32).max(1)
    }

    /// `RoundRect` ellipse width/height (diameters) for a logical radius.
    fn round_rect_radii(&self, radius: f32) -> (i32, i32) {
        let physical = self.dpi.to_physical(radius.round() as i32).max(0);
        (physical * 2, physical * 2)
    }

    fn circle_bounds(&self, center: PointF, radius: f32) -> RECT {
        let radius_px = self.dpi.to_physical(radius.round() as i32).max(0);
        let center_px = self.physical_point(center);
        RECT {
            left: center_px.x - radius_px,
            top: center_px.y - radius_px,
            right: center_px.x + radius_px,
            bottom: center_px.y + radius_px,
        }
    }
}

impl Drop for PaintCanvas<'_> {
    fn drop(&mut self) {
        if let Some(mut target) = self.d2d.take() {
            if !self.d2d_broken && !target.end() {
                log::debug!("PaintCanvas: D2D frame failed; invalidating for a repaint");
                unsafe {
                    InvalidateRect(self.hwnd, std::ptr::null(), 0);
                }
            }
        }
        unsafe {
            EndPaint(self.hwnd, &mut self.paint);
        }
    }
}

#[cfg(test)]
mod drawing_tests {
    use super::*;
    use crate::ui::model::{AccentMode, ThemeMode};
    use crate::ui::theme::{tokens_for, TypographyTokens};
    use windows_sys::Win32::Graphics::Gdi::{GetCurrentObject, OBJ_FONT};
    use windows_sys::Win32::System::Com::CoInitializeEx;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, WNDCLASSW,
    };

    // ── DpiMetrics ─────────────────────────────────────────────────────────

    #[test]
    fn dpi_metrics_identity_at_100_percent() {
        let dpi = DpiMetrics::new(1.0);
        assert_eq!(dpi.scale(), 1.0);
        assert_eq!(dpi.to_physical(336), 336);
        assert_eq!(dpi.to_logical(336), 336);
    }

    #[test]
    fn dpi_metrics_maps_125_and_150_percent() {
        let at_125 = DpiMetrics::new(1.25);
        assert_eq!(at_125.to_physical(336), 420);
        assert_eq!(at_125.to_logical(420), 336);
        let at_150 = DpiMetrics::new(1.5);
        assert_eq!(at_150.to_physical(336), 504);
        assert_eq!(at_150.to_logical(504), 336);
    }

    #[test]
    fn dpi_metrics_keeps_32px_minimum_target() {
        for scale in [1.0f32, 1.25, 1.5, 2.0] {
            let dpi = DpiMetrics::new(scale);
            assert!(
                dpi.to_physical(32) >= 32,
                "32px target shrank at {scale}x: {}",
                dpi.to_physical(32)
            );
        }
    }

    #[test]
    fn dpi_metrics_round_trips() {
        // Exact round-trip holds when the intermediate physical value is
        // integral; every scale above 100% keeps these values integral.
        for scale in [1.0f32, 1.25, 1.5, 2.0, 4.0] {
            let dpi = DpiMetrics::new(scale);
            for logical in [0i32, 1, 4, 16, 32, 100, 336, 760] {
                assert_eq!(
                    dpi.to_logical(dpi.to_physical(logical)),
                    logical,
                    "round trip at {scale}x for {logical}"
                );
            }
        }
        // At 50% only even logical sizes land on integral physical pixels.
        let half = DpiMetrics::new(0.5);
        assert_eq!(half.to_logical(half.to_physical(336)), 336);
        assert_eq!(half.to_logical(half.to_physical(4)), 4);
    }

    #[test]
    fn dpi_metrics_clamps_to_supported_range() {
        assert_eq!(DpiMetrics::new(0.25).scale(), 0.5);
        assert_eq!(DpiMetrics::new(-1.0).scale(), 0.5);
        assert_eq!(DpiMetrics::new(8.0).scale(), 4.0);
        assert_eq!(DpiMetrics::new(f32::NAN).scale(), 1.0);
        assert_eq!(DpiMetrics::new(f32::INFINITY).scale(), 1.0);
        assert_eq!(DpiMetrics::new(f32::NEG_INFINITY).scale(), 1.0);
    }

    // ── focus rings ────────────────────────────────────────────────────────

    fn control_rect() -> RectF {
        RectF::new(100.0, 100.0, 132.0, 132.0) // 32x32 close target
    }

    fn focus() -> FocusTokens {
        // Same values tokens_for produces (theme-independent of mode).
        tokens_for(ThemeMode::Dark, false, AccentMode::System, || None).focus
    }

    #[test]
    fn focus_ring_layers_are_distinct_and_nested() {
        let rect = control_rect();
        let (outer, inner) = focus_ring_rects(rect, &focus());
        assert_ne!(outer, inner, "the two layers must be distinct boxes");
        // Inner ring sits inside the outer ring.
        assert!(
            outer.contains(inner),
            "{outer:?} does not contain {inner:?}"
        );
        // Both rings surround the control.
        assert!(inner.contains(rect), "{inner:?} does not contain {rect:?}");
    }

    #[test]
    fn focus_ring_bands_do_not_overlap_given_theme_invariants() {
        // The stroke bands are [gap, gap + width] from the control edge.
        // Theme invariants keep the inner band strictly inside the outer one.
        let f = focus();
        let inner_band_end = f.inner_ring_gap_px + f.inner_ring_width_px;
        let outer_band_start = f.ring_gap_px;
        assert!(
            inner_band_end < outer_band_start,
            "bands would overlap: {} vs {}",
            inner_band_end,
            outer_band_start
        );
        let (outer, inner) = focus_ring_rects(control_rect(), &f);
        assert!(outer.left < inner.left && inner.left < control_rect().left);
        assert!(inner.right < outer.right && control_rect().right < inner.right);
    }

    #[test]
    fn focus_ring_rects_scale_with_control_size() {
        let small = RectF::new(0.0, 0.0, 32.0, 32.0);
        let big = RectF::new(0.0, 0.0, 64.0, 64.0);
        let (outer_small, _) = focus_ring_rects(small, &focus());
        let (outer_big, _) = focus_ring_rects(big, &focus());
        assert!(outer_big.width() > outer_small.width());
    }

    // ── rounded rects ──────────────────────────────────────────────────────

    #[test]
    fn rounded_rect_path_clamps_radius_to_half_min_dimension() {
        let rect = RectF::new(0.0, 0.0, 40.0, 20.0); // min dimension 20 -> max radius 10
        let path = rounded_rect_path(rect, 100.0);
        assert_eq!(path.radius_x, 10.0);
        assert_eq!(path.radius_y, 10.0);
        assert_eq!(path.rect, rect);
    }

    #[test]
    fn rounded_rect_path_normal_radius_is_preserved() {
        let rect = RectF::new(0.0, 0.0, 40.0, 20.0);
        let path = rounded_rect_path(rect, 4.0);
        assert_eq!(path.radius_x, 4.0);
        assert_eq!(path.radius_y, 4.0);
    }

    #[test]
    fn rounded_rect_path_negative_and_zero_radius_become_rect() {
        let rect = RectF::new(0.0, 0.0, 40.0, 20.0);
        assert_eq!(rounded_rect_path(rect, -3.0).radius_x, 0.0);
        assert_eq!(rounded_rect_path(rect, 0.0).radius_x, 0.0);
    }

    // ── diamonds ───────────────────────────────────────────────────────────

    #[test]
    fn diamond_points_are_symmetric_around_center() {
        let center = PointF::new(10.0, 20.0);
        let points = diamond_points(center, 6.0);
        assert_eq!(points[0], PointF::new(10.0, 14.0)); // top
        assert_eq!(points[1], PointF::new(16.0, 20.0)); // right
        assert_eq!(points[2], PointF::new(10.0, 26.0)); // bottom
        assert_eq!(points[3], PointF::new(4.0, 20.0)); // left
    }

    #[test]
    fn diamond_points_degenerate_when_half_size_zero_or_negative() {
        let center = PointF::new(5.0, 5.0);
        assert_eq!(diamond_points(center, 0.0), [center; 4]);
        assert_eq!(diamond_points(center, -4.0), [center; 4]);
    }

    // ── canvas smoke (headless hidden window) ──────────────────────────────

    fn init_com() {
        unsafe {
            CoInitializeEx(std::ptr::null(), 0);
        }
    }

    fn hidden_window() -> HWND {
        unsafe {
            static WINDOW: std::sync::OnceLock<HWND> = std::sync::OnceLock::new();
            *WINDOW.get_or_init(|| {
                let class = WNDCLASSW {
                    lpfnWndProc: Some(DefWindowProcW),
                    hInstance: windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(
                        std::ptr::null(),
                    ),
                    lpszClassName: windows_sys::core::w!("VolCtlCanvasTestWnd"),
                    ..std::mem::zeroed()
                };
                RegisterClassW(&class);
                CreateWindowExW(
                    0,
                    windows_sys::core::w!("VolCtlCanvasTestWnd"),
                    windows_sys::core::w!("canvas test"),
                    windows_sys::Win32::UI::WindowsAndMessaging::WS_OVERLAPPED,
                    0,
                    0,
                    200,
                    100,
                    0,
                    0,
                    class.hInstance,
                    std::ptr::null(),
                )
            })
        }
    }

    #[test]
    fn paint_canvas_draws_every_primitive_without_error() {
        init_com();
        let hwnd = hidden_window();
        let Some(mut canvas) = PaintCanvas::begin_paint(hwnd) else {
            panic!("BeginPaint failed on the hidden test window");
        };
        let surface = Rgba::from_rgb(0x17, 0x1C, 0x24);
        let accent = Rgba::from_rgb(0x3A, 0xA8, 0xFF);
        let text = Rgba::from_rgb(0xF5, 0xF7, 0xFA);
        let ty = TypographyTokens::default();

        canvas.fill_rect(RectF::new(0.0, 0.0, 184.0, 61.0), surface);
        canvas.fill_rounded_rect(RectF::new(16.0, 16.0, 168.0, 45.0), 8.0, surface);
        canvas.stroke_rounded_rect(RectF::new(16.0, 16.0, 168.0, 45.0), 8.0, accent, 1.0);
        canvas.fill_circle(PointF::new(28.0, 30.0), 5.0, accent);
        canvas.stroke_circle(PointF::new(44.0, 30.0), 5.0, accent, 1.0);
        canvas.fill_diamond(PointF::new(60.0, 30.0), 5.0, accent);
        canvas.stroke_diamond(PointF::new(76.0, 30.0), 5.0, accent, 1.0);
        canvas.draw_focus_ring(RectF::new(88.0, 14.0, 120.0, 46.0), &focus());
        let drawn = canvas.draw_text(&TextLayout {
            text: "72%",
            rect: RectF::new(16.0, 16.0, 168.0, 45.0),
            align: TextAlign::Right,
            role: ty.display_value,
            color: text,
        });
        assert!(drawn, "draw_text must succeed on the hidden window");
        // Dropping the canvas runs EndPaint (and D2D EndDraw when active).
        drop(canvas);
        unsafe {
            DestroyWindow(hwnd);
        }
    }

    #[test]
    fn paint_canvas_measures_dpi_and_reports_paint_mode() {
        init_com();
        let hwnd = hidden_window();
        let canvas = PaintCanvas::begin_paint(hwnd).expect("BeginPaint");
        let dpi = canvas.dpi();
        assert!(dpi.scale() >= 1.0, "scale {}", dpi.scale());
        // Either mode is valid; the point is that it reports one coherent path.
        let _ = canvas.d2d_active();
        drop(canvas);
        unsafe {
            DestroyWindow(hwnd);
        }
    }

    #[test]
    fn paint_canvas_uses_gdi_for_layered_windows() {
        // Layered windows (WS_EX_LAYERED — the click-through overlay) cannot
        // be painted by an hwnd render target: D2D presents never land in the
        // layered surface, so the canvas must select the GDI path for them.
        init_com();
        unsafe {
            static WINDOW: std::sync::OnceLock<HWND> = std::sync::OnceLock::new();
            let hwnd = *WINDOW.get_or_init(|| {
                let class = WNDCLASSW {
                    lpfnWndProc: Some(DefWindowProcW),
                    hInstance: windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(
                        std::ptr::null(),
                    ),
                    lpszClassName: windows_sys::core::w!("VolCtlLayeredTestWnd"),
                    ..std::mem::zeroed()
                };
                RegisterClassW(&class);
                CreateWindowExW(
                    windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_LAYERED,
                    windows_sys::core::w!("VolCtlLayeredTestWnd"),
                    windows_sys::core::w!("layered canvas test"),
                    windows_sys::Win32::UI::WindowsAndMessaging::WS_OVERLAPPED,
                    0,
                    0,
                    200,
                    100,
                    0,
                    0,
                    class.hInstance,
                    std::ptr::null(),
                )
            });
            assert!(super::is_layered(hwnd), "test window must be layered");
            let canvas = PaintCanvas::begin_paint(hwnd).expect("BeginPaint on layered window");
            assert!(
                !canvas.d2d_active(),
                "layered windows must paint via GDI, never an hwnd render target"
            );
            drop(canvas);
            DestroyWindow(hwnd);
        }
    }

    #[test]
    fn paint_canvas_uses_gdi_for_backdrop_windows() {
        // Windows 11 system-backdrop windows (DWMWA_SYSTEMBACKDROP_TYPE, e.g.
        // the acrylic mixer) own a DWM-composited surface that hwnd render
        // targets cannot present into (live-verified: the backdrop mixer
        // rendered a uniform blur-tinted surface under D2D and painted
        // correctly via GDI). The canvas must select the GDI path for them.
        init_com();
        unsafe {
            let class = WNDCLASSW {
                lpfnWndProc: Some(DefWindowProcW),
                hInstance: windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(
                    std::ptr::null(),
                ),
                lpszClassName: windows_sys::core::w!("VolCtlBackdropTestWnd"),
                ..std::mem::zeroed()
            };
            RegisterClassW(&class);
            let hwnd = CreateWindowExW(
                0,
                windows_sys::core::w!("VolCtlBackdropTestWnd"),
                windows_sys::core::w!("backdrop canvas test"),
                windows_sys::Win32::UI::WindowsAndMessaging::WS_OVERLAPPED,
                0,
                0,
                200,
                100,
                0,
                0,
                class.hInstance,
                std::ptr::null(),
            );
            assert_ne!(hwnd, 0, "test window creation");
            let backdrop_set = apply_backdrop(hwnd, crate::ui::ResolvedMaterial::Blurred, false);
            if !backdrop_set {
                // Pre-Windows 11 (or a session without DWM backdrops): the
                // attribute could not be applied, so this window legitimately
                // has no DWM-owned surface — nothing to assert.
                DestroyWindow(hwnd);
                return;
            }
            assert!(backdrop_active(hwnd), "the window must report its backdrop");
            let canvas = PaintCanvas::begin_paint(hwnd).expect("BeginPaint");
            assert!(
                !canvas.d2d_active(),
                "backdrop windows must paint via GDI, never an hwnd render target"
            );
            drop(canvas);
            DestroyWindow(hwnd);
        }
    }

    #[test]
    fn paint_canvas_draw_text_preserves_the_dc_font_slot() {
        init_com();
        let hwnd = hidden_window();
        let mut canvas = PaintCanvas::begin_paint(hwnd).expect("BeginPaint");
        let ty = TypographyTokens::default();

        // The object occupying the DC font slot before the draw must still be
        // there afterwards: in GDI mode `gdi_draw_text` creates and selects a
        // per-call font, and restoring the slot is what makes that font
        // deletable (deleting a still-selected font leaks one HFONT per draw).
        // In D2D mode the GDI slot must be untouched entirely.
        let font_slot_before = unsafe { GetCurrentObject(canvas.hdc, OBJ_FONT as u32) };
        assert_ne!(
            font_slot_before, 0,
            "a window DC always has a font selected"
        );

        let drawn = canvas.draw_text(&TextLayout {
            text: "72%",
            rect: RectF::new(16.0, 16.0, 168.0, 45.0),
            align: TextAlign::Right,
            role: ty.display_value,
            color: Rgba::from_rgb(0xF5, 0xF7, 0xFA),
        });
        assert!(drawn, "draw_text must succeed on the hidden window");

        assert_eq!(
            unsafe { GetCurrentObject(canvas.hdc, OBJ_FONT as u32) },
            font_slot_before,
            "draw_text must leave the DC font slot exactly as it found it"
        );
        drop(canvas);
        unsafe {
            DestroyWindow(hwnd);
        }
    }
}
