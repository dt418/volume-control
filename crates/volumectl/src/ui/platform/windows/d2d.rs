//! Direct2D / DirectWrite seam for the Windows renderer.
//!
//! `windows-sys` 0.52 ships **no** `Win32_Graphics_Direct2D` or
//! `Win32_Graphics_DirectWrite` modules (verified against the installed crate
//! source: neither feature exists in its manifest and the generated source
//! contains no D2D/DWrite bindings), so this module follows the hand-rolled
//! vtable idiom from `audio_windows.rs` and links the two DLL exports directly.
//!
//! Every vtable slot below was verified three ways before use:
//!   1. The Windows SDK headers `d2d1.h` / `dwrite.h` / `dcommon.h`
//!      (Win11 SDK 10.0.26100.0), method-by-method, in interface order.
//!   2. The win32metadata-generated bindings from `microsoft/windows-rs`
//!      (tag 0.59.0), which spell out the exact `Vtbl` structs.
//!   3. A runtime probe (`vcprobe`) against the real `d2d1.dll` /
//!      `dwrite.dll` on this machine that created a factory, a hidden-window
//!      render target, a path geometry + sink, and text objects, and invoked
//!      each method through its candidate slot.
//!
//! The probe also settled the one place where header reading was ambiguous:
//! `IDWriteTextLayout::GetMetrics` sits at vtable slot 60 (25 methods on
//! `IDWriteTextFormat`, not 28 — `AddFontFeature` and friends belong to
//! `IDWriteTypography`).
//!
//! The seam is best-effort by contract: `Direct2dContext::new()` returns
//! `None` on ANY failure, every fallible call degrades to `None`/`false`, and
//! the GDI canvas in [`super::primitives::PaintCanvas`] remains authoritative
//! and always works.

use std::ffi::c_void;
use std::sync::OnceLock;

use windows_sys::core::{GUID, PCWSTR};
use windows_sys::Win32::Foundation::HWND;

use crate::ui::theme::{Rgba, TextRole};

use super::primitives::{dpi_scale_for, DpiMetrics, PointF, RectF, SizeF};
use super::text::{TextAlign, TextLayout};

#[allow(clippy::upper_case_acronyms)]
type HRESULT = i32;
const S_OK: HRESULT = 0;

// ── DLL exports (the only two functions d2d1.dll / dwrite.dll expose) ──────

#[link(name = "d2d1")]
extern "system" {
    /// `HRESULT WINAPI D2D1CreateFactory(D2D1_FACTORY_TYPE, REFIID,
    /// const D2D1_FACTORY_OPTIONS*, void**)` — the single export of d2d1.dll.
    fn D2D1CreateFactory(
        factory_type: i32,
        riid: *const GUID,
        factory_options: *const c_void,
        factory: *mut *mut c_void,
    ) -> HRESULT;
}

#[link(name = "dwrite")]
extern "system" {
    /// `HRESULT WINAPI DWriteCreateFactory(DWRITE_FACTORY_TYPE, REFIID,
    /// IUnknown**)` — the single export of dwrite.dll.
    fn DWriteCreateFactory(
        factory_type: i32,
        iid: *const GUID,
        factory: *mut *mut c_void,
    ) -> HRESULT;
}

// ── Interface IDs (from the SDK headers' DX_DECLARE_INTERFACE lines) ────────

const IID_ID2D1FACTORY: GUID = GUID::from_u128(0x06152247_6f50_465a_9245_118bfd3b6007);
const IID_IDWRITEFACTORY: GUID = GUID::from_u128(0xb859ee5a_d838_4b5b_a2e8_1adc7d93db48);

// ── D2D/DWrite constants (verified against d2d1.h, dwrite.h, dcommon.h) ─────

const D2D1_FACTORY_TYPE_MULTI_THREADED: i32 = 1;
const DWRITE_FACTORY_TYPE_SHARED: i32 = 0;
const D2D1_RENDER_TARGET_TYPE_DEFAULT: i32 = 0;
const DXGI_FORMAT_B8G8R8A8_UNORM: u32 = 87;
const D2D1_ALPHA_MODE_PREMULTIPLIED: i32 = 1;
const D2D1_RENDER_TARGET_USAGE_NONE: u32 = 0;
const D2D1_FEATURE_LEVEL_DEFAULT: i32 = 0;
const D2D1_PRESENT_OPTIONS_NONE: u32 = 0;
const D2D1_DRAW_TEXT_OPTIONS_NONE: u32 = 0;
const DWRITE_MEASURING_MODE_NATURAL: i32 = 0;
const D2D1_FILL_MODE_ALTERNATE: i32 = 0;
const D2D1_FIGURE_BEGIN_FILLED: i32 = 0;
const D2D1_FIGURE_END_CLOSED: i32 = 1;
const DWRITE_FONT_STYLE_NORMAL: i32 = 0;
const DWRITE_FONT_STRETCH_NORMAL: i32 = 5;
const DWRITE_TEXT_ALIGNMENT_LEADING: i32 = 0;
const DWRITE_TEXT_ALIGNMENT_TRAILING: i32 = 1;
const DWRITE_TEXT_ALIGNMENT_CENTER: i32 = 2;

// ── ABI structs (repr(C); field order verified against d2d1.h/dcommon.h) ─────

#[repr(C)]
#[derive(Clone, Copy)]
struct D2D1_COLOR_F {
    r: f32,
    g: f32,
    b: f32,
    a: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct D2D1_POINT_2F {
    x: f32,
    y: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct D2D1_SIZE_U {
    width: u32,
    height: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct D2D1_RECT_F {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct D2D1_PIXEL_FORMAT {
    format: u32,
    alpha_mode: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct D2D1_RENDER_TARGET_PROPERTIES {
    r#type: i32,
    pixel_format: D2D1_PIXEL_FORMAT,
    dpi_x: f32,
    dpi_y: f32,
    usage: u32,
    min_level: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct D2D1_HWND_RENDER_TARGET_PROPERTIES {
    hwnd: HWND,
    pixel_size: D2D1_SIZE_U,
    present_options: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct D2D1_ROUNDED_RECT {
    rect: D2D1_RECT_F,
    radius_x: f32,
    radius_y: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct D2D1_ELLIPSE {
    point: D2D1_POINT_2F,
    radius_x: f32,
    radius_y: f32,
}

/// DWRITE_TEXT_METRICS (dwrite.h): 7 FLOATs then 2 UINT32s.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct DWRITE_TEXT_METRICS {
    left: f32,
    top: f32,
    width: f32,
    width_including_trailing_whitespace: f32,
    height: f32,
    layout_width: f32,
    layout_height: f32,
    max_bidi_reordering_depth: u32,
    line_count: u32,
}

// ── Hand-rolled COM vtables (slots verified, see module docs) ───────────────

type FnUnknown = unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT;
type FnRelease = unsafe extern "system" fn(*mut c_void) -> u32;
/// Placeholder for vtable slots we never call: keeps the struct the same
/// layout (one pointer per slot) with correct ordering and count.
type FnUnused = unsafe extern "system" fn(*mut c_void) -> HRESULT;

/// ID2D1Factory: IUnknown(0-2) + 14 methods (slot 14 = CreateHwndRenderTarget).
#[repr(C)]
struct ID2D1FactoryVtbl {
    query_interface: FnUnknown,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: FnRelease,
    reload_system_metrics: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    // 4-9: GetDesktopDpi, CreateRectangleGeometry, CreateRoundedRectangleGeometry,
    // CreateEllipseGeometry, CreateGeometryGroup, CreateTransformedGeometry.
    unused_4_to_9: [FnUnused; 6],
    create_path_geometry: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    // 11-13: CreateStrokeStyle, CreateDrawingStateBlock, CreateWicBitmapRenderTarget.
    unused_11_to_13: [FnUnused; 3],
    create_hwnd_render_target: unsafe extern "system" fn(
        *mut c_void,
        *const D2D1_RENDER_TARGET_PROPERTIES,
        *const D2D1_HWND_RENDER_TARGET_PROPERTIES,
        *mut *mut c_void,
    ) -> HRESULT,
    // 15-16: CreateDxgiSurfaceRenderTarget, CreateDCRenderTarget.
    unused_15_to_16: [FnUnused; 2],
}

/// ID2D1RenderTarget: IUnknown(0-2) + GetFactory(3) + 53 methods (57 slots).
#[repr(C)]
struct ID2D1RenderTargetVtbl {
    query_interface: FnUnknown,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: FnRelease,
    // 3-7: GetFactory, CreateBitmap, CreateBitmapFromWicBitmap, CreateSharedBitmap,
    // CreateBitmapBrush.
    unused_3_to_7: [FnUnused; 5],
    create_solid_color_brush: unsafe extern "system" fn(
        *mut c_void,
        *const D2D1_COLOR_F,
        *const c_void,
        *mut *mut c_void,
    ) -> HRESULT,
    // 9-16: CreateGradientStopCollection, CreateLinearGradientBrush,
    // CreateRadialGradientBrush, CreateCompatibleRenderTarget, CreateLayer,
    // CreateMesh, DrawLine, DrawRectangle.
    unused_9_to_16: [FnUnused; 8],
    fill_rectangle: unsafe extern "system" fn(*mut c_void, *const D2D1_RECT_F, *mut c_void),
    draw_rounded_rectangle: unsafe extern "system" fn(
        *mut c_void,
        *const D2D1_ROUNDED_RECT,
        *mut c_void,
        f32,
        *mut c_void,
    ),
    fill_rounded_rectangle:
        unsafe extern "system" fn(*mut c_void, *const D2D1_ROUNDED_RECT, *mut c_void),
    draw_ellipse:
        unsafe extern "system" fn(*mut c_void, *const D2D1_ELLIPSE, *mut c_void, f32, *mut c_void),
    fill_ellipse: unsafe extern "system" fn(*mut c_void, *const D2D1_ELLIPSE, *mut c_void),
    draw_geometry:
        unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void, f32, *mut c_void),
    fill_geometry: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void),
    // 24-26: FillMesh, FillOpacityMask, DrawBitmap.
    unused_24_to_26: [FnUnused; 3],
    draw_text: unsafe extern "system" fn(
        *mut c_void,
        PCWSTR,
        u32,
        *mut c_void,
        *const D2D1_RECT_F,
        *mut c_void,
        u32,
        i32,
    ),
    // 28-46: DrawTextLayout, DrawGlyphRun, SetTransform, GetTransform,
    // SetAntialiasMode, GetAntialiasMode, SetTextAntialiasMode,
    // GetTextAntialiasMode, SetTextRenderingParams, GetTextRenderingParams,
    // SetTags, GetTags, PushLayer, PopLayer, Flush, SaveDrawingState,
    // RestoreDrawingState, PushAxisAlignedClip, PopAxisAlignedClip.
    unused_28_to_46: [FnUnused; 19],
    clear: unsafe extern "system" fn(*mut c_void, *const D2D1_COLOR_F),
    /// BeginDraw returns void (header `STDMETHOD_(void, BeginDraw)`).
    begin_draw: unsafe extern "system" fn(*mut c_void),
    end_draw: unsafe extern "system" fn(*mut c_void, *mut u64, *mut u64) -> HRESULT,
    // 50-56: GetPixelFormat, SetDpi, GetDpi, GetSize, GetPixelSize,
    // GetMaximumBitmapSize, IsSupported.
    unused_50_to_56: [FnUnused; 7],
}

/// ID2D1PathGeometry: IUnknown(0-2) + GetFactory(3) + 13 geometry methods
/// (4-16) + Open(17).
#[repr(C)]
struct ID2D1PathGeometryVtbl {
    query_interface: FnUnknown,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: FnRelease,
    unused_3_to_16: [FnUnused; 14],
    open: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
}

/// ID2D1GeometrySink: IUnknown(0-2) + 12 methods (3-14).
#[repr(C)]
struct ID2D1GeometrySinkVtbl {
    query_interface: FnUnknown,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: FnRelease,
    set_fill_mode: unsafe extern "system" fn(*mut c_void, i32),
    set_segment_flags: FnUnused,
    begin_figure: unsafe extern "system" fn(*mut c_void, D2D1_POINT_2F, i32),
    add_lines: unsafe extern "system" fn(*mut c_void, *const D2D1_POINT_2F, u32),
    add_beziers: FnUnused,
    end_figure: unsafe extern "system" fn(*mut c_void, i32),
    close: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    // 10-14: AddLine, AddBezier, AddQuadraticBezier, AddQuadraticBeziers, AddArc.
    unused_10_to_14: [FnUnused; 5],
}

/// IDWriteFactory: IUnknown(0-2) + 16 methods (15 = CreateTextFormat,
/// 18 = CreateTextLayout).
#[repr(C)]
struct IDWriteFactoryVtbl {
    query_interface: FnUnknown,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: FnRelease,
    // 3-14: GetSystemFontCollection, CreateCustomFontCollection,
    // RegisterFontCollectionLoader, UnregisterFontCollectionLoader,
    // CreateFontFileReference, CreateCustomFontFileReference, CreateFontFace,
    // CreateRenderingParams, CreateMonitorRenderingParams,
    // CreateCustomRenderingParams, RegisterFontFileLoader,
    // UnregisterFontFileLoader.
    unused_3_to_14: [FnUnused; 12],
    create_text_format: unsafe extern "system" fn(
        *mut c_void,
        PCWSTR,
        *mut c_void,
        i32,
        i32,
        i32,
        f32,
        PCWSTR,
        *mut *mut c_void,
    ) -> HRESULT,
    create_typography: FnUnused,
    get_gdi_interop: FnUnused,
    create_text_layout: unsafe extern "system" fn(
        *mut c_void,
        PCWSTR,
        u32,
        *mut c_void,
        f32,
        f32,
        *mut *mut c_void,
    ) -> HRESULT,
}

/// IDWriteTextFormat: IUnknown(0-2) + SetTextAlignment(3) + 24 more (unused).
#[repr(C)]
struct IDWriteTextFormatVtbl {
    query_interface: FnUnknown,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: FnRelease,
    set_text_alignment: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
    unused_4_to_27: [FnUnused; 24],
}

/// IDWriteTextLayout: IUnknown(0-2) + IDWriteTextFormat(25 methods, 3-27)
/// + 32 layout methods (28-59) + GetMetrics(60).
#[repr(C)]
struct IDWriteTextLayoutVtbl {
    query_interface: FnUnknown,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: FnRelease,
    unused_3_to_59: [FnUnused; 57],
    get_metrics: unsafe extern "system" fn(*mut c_void, *mut DWRITE_TEXT_METRICS) -> HRESULT,
}

/// Access a vtable by type (audio_windows.rs idiom).
unsafe fn vtbl<T>(this: *const c_void) -> &'static T {
    &**(this as *const *const T)
}

/// Release a COM object through its IUnknown slot.
unsafe fn release(this: *mut c_void) {
    if !this.is_null() {
        (vtbl::<ID2D1RenderTargetVtbl>(this).release)(this);
    }
}

// ── Public context and render target ────────────────────────────────────────

/// Shared D2D + DWrite factory pair.
///
/// Created lazily (first `new`/`get`) and cached for the process lifetime.
/// Both factories are created in their thread-safe modes
/// (`D2D1_FACTORY_TYPE_MULTI_THREADED`, `DWRITE_FACTORY_TYPE_SHARED`), so the
/// context is `Send + Sync` and safe to share between paint sessions.
pub struct Direct2dContext {
    d2d: *mut c_void,
    dwrite: *mut c_void,
}

// The wrapped COM factories are thread-safe (multi-threaded/shared modes).
unsafe impl Send for Direct2dContext {}
unsafe impl Sync for Direct2dContext {}

impl Direct2dContext {
    /// Create both factories, or `None` on ANY failure (never panics).
    pub fn new() -> Option<Self> {
        unsafe {
            let mut d2d: *mut c_void = std::ptr::null_mut();
            let hr = D2D1CreateFactory(
                D2D1_FACTORY_TYPE_MULTI_THREADED,
                &IID_ID2D1FACTORY,
                std::ptr::null(),
                &mut d2d,
            );
            if hr != S_OK || d2d.is_null() {
                log::debug!("d2d: D2D1CreateFactory failed (0x{hr:08x})");
                return None;
            }
            let mut dwrite: *mut c_void = std::ptr::null_mut();
            let hr =
                DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED, &IID_IDWRITEFACTORY, &mut dwrite);
            if hr != S_OK || dwrite.is_null() {
                log::debug!("d2d: DWriteCreateFactory failed (0x{hr:08x})");
                release(d2d);
                return None;
            }
            Some(Self { d2d, dwrite })
        }
    }

    /// The process-wide context, created once on first use.
    pub fn get() -> Option<&'static Self> {
        static CONTEXT: OnceLock<Option<Direct2dContext>> = OnceLock::new();
        CONTEXT.get_or_init(Direct2dContext::new).as_ref()
    }

    /// DWrite text metrics for `text` in `role`, in LOGICAL pixels.
    ///
    /// Pure in the sense of requiring no window. The returned width/height
    /// are DWrite DIPs snapped to the physical pixel grid of `dpi` so that
    /// right/center alignment math matches GDI's integer-pixel rendering.
    pub fn text_layout_metrics(
        &self,
        text: &str,
        role: TextRole,
        dpi: DpiMetrics,
    ) -> Option<SizeF> {
        if text.is_empty() {
            return Some(SizeF::new(0.0, 0.0));
        }
        unsafe {
            let wide: Vec<u16> = text.encode_utf16().collect();
            let fmt = self.create_text_format(role, TextAlign::Left)?;
            let mut layout: *mut c_void = std::ptr::null_mut();
            // Large layout box: single-line text is never wrapped or clipped.
            let hr = (vtbl::<IDWriteFactoryVtbl>(self.dwrite).create_text_layout)(
                self.dwrite,
                wide.as_ptr(),
                wide.len() as u32,
                fmt,
                100_000.0,
                100_000.0,
                &mut layout,
            );
            release(fmt);
            if hr != S_OK || layout.is_null() {
                return None;
            }
            let mut metrics: DWRITE_TEXT_METRICS = Default::default();
            let hr = (vtbl::<IDWriteTextLayoutVtbl>(layout).get_metrics)(layout, &mut metrics);
            release(layout);
            if hr != S_OK {
                return None;
            }
            Some(SizeF {
                width: snap(metrics.width, dpi.scale()),
                height: snap(metrics.height, dpi.scale()),
            })
        }
    }

    /// A D2D render target that draws to `hwnd`'s client area.
    ///
    /// The target is created at the window's current DPI and auto-sizes with
    /// the window (pixel size `{0, 0}`). Draw calls take logical coordinates.
    /// `None` on any failure; callers fall back to GDI.
    pub fn render_target(&self, hwnd: HWND) -> Option<HwndRenderTarget> {
        unsafe {
            let dpi = DpiMetrics::new(dpi_scale_for(hwnd));
            let dpi_px = 96.0 * dpi.scale();
            let properties = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixel_format: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alpha_mode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpi_x: dpi_px,
                dpi_y: dpi_px,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                min_level: D2D1_FEATURE_LEVEL_DEFAULT,
            };
            let hwnd_properties = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd,
                pixel_size: D2D1_SIZE_U {
                    width: 0,
                    height: 0,
                },
                present_options: D2D1_PRESENT_OPTIONS_NONE,
            };
            let mut target: *mut c_void = std::ptr::null_mut();
            let hr = (vtbl::<ID2D1FactoryVtbl>(self.d2d).create_hwnd_render_target)(
                self.d2d,
                &properties,
                &hwnd_properties,
                &mut target,
            );
            if hr != S_OK || target.is_null() {
                log::debug!("d2d: CreateHwndRenderTarget failed (0x{hr:08x})");
                return None;
            }
            // Hold the factories with their own references so the target owns
            // everything it needs beyond the context's lifetime.
            (vtbl::<ID2D1RenderTargetVtbl>(self.d2d).add_ref)(self.d2d);
            (vtbl::<ID2D1RenderTargetVtbl>(self.dwrite).add_ref)(self.dwrite);
            Some(HwndRenderTarget {
                target,
                factory: self.d2d,
                dwrite: self.dwrite,
                dpi,
            })
        }
    }

    fn create_text_format(&self, role: TextRole, align: TextAlign) -> Option<*mut c_void> {
        unsafe {
            let family = if role.monospace {
                windows_sys::core::w!("Cascadia Mono")
            } else {
                windows_sys::core::w!("Segoe UI Variable")
            };
            let locale = windows_sys::core::w!("");
            let mut fmt: *mut c_void = std::ptr::null_mut();
            let hr = (vtbl::<IDWriteFactoryVtbl>(self.dwrite).create_text_format)(
                self.dwrite,
                family, // `w!` already yields the null-terminated wide pointer.
                std::ptr::null_mut(),
                i32::from(role.weight),
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                role.size_px,
                locale,
                &mut fmt,
            );
            if hr != S_OK || fmt.is_null() {
                return None;
            }
            let alignment = match align {
                TextAlign::Left => DWRITE_TEXT_ALIGNMENT_LEADING,
                TextAlign::Right => DWRITE_TEXT_ALIGNMENT_TRAILING,
                TextAlign::Center => DWRITE_TEXT_ALIGNMENT_CENTER,
            };
            let hr = (vtbl::<IDWriteTextFormatVtbl>(fmt).set_text_alignment)(fmt, alignment);
            if hr != S_OK {
                release(fmt);
                return None;
            }
            Some(fmt)
        }
    }
}

impl Drop for Direct2dContext {
    fn drop(&mut self) {
        unsafe {
            release(self.d2d);
            release(self.dwrite);
        }
    }
}

/// Round a DWrite DIP measure onto the physical pixel grid of `scale`
/// (e.g. a 24.3 DIP width at 1.25x snaps to the 30.4 physical px boundary),
/// returning the value in logical pixels.
fn snap(dips: f32, scale: f32) -> f32 {
    if scale > 0.0 && (scale - 1.0).abs() > f32::EPSILON {
        (dips * scale).round() / scale
    } else {
        dips
    }
}

/// Convenience: process-wide DWrite text metrics (see
/// [`Direct2dContext::text_layout_metrics`]).
pub fn text_layout_metrics(text: &str, role: TextRole, dpi: DpiMetrics) -> Option<SizeF> {
    Direct2dContext::get()?.text_layout_metrics(text, role, dpi)
}

/// An `ID2D1HwndRenderTarget` with the primitive paint ops the renderer needs.
///
/// All coordinates are LOGICAL pixels; the target scales them by the DPI it
/// was created with. Every op returns `false` on failure so the caller can
/// fall back to GDI; `end()` reports whether the frame presented.
pub struct HwndRenderTarget {
    target: *mut c_void,
    factory: *mut c_void,
    dwrite: *mut c_void,
    dpi: DpiMetrics,
}

// The factories are thread-safe and the target is confined to one paint.
unsafe impl Send for HwndRenderTarget {}
unsafe impl Sync for HwndRenderTarget {}

impl HwndRenderTarget {
    fn rt(&self) -> &'static ID2D1RenderTargetVtbl {
        unsafe { vtbl::<ID2D1RenderTargetVtbl>(self.target) }
    }

    fn factory(&self) -> &'static ID2D1FactoryVtbl {
        unsafe { vtbl::<ID2D1FactoryVtbl>(self.factory) }
    }

    /// The DPI scale the target converts logical coordinates with.
    pub fn dpi(&self) -> DpiMetrics {
        self.dpi
    }

    /// Begin a draw frame. Cannot fail itself (BeginDraw is void); errors
    /// surface at [`Self::end`].
    pub fn begin(&mut self) -> bool {
        unsafe {
            (self.rt().begin_draw)(self.target);
        }
        true
    }

    /// Present the frame. `true` on success; on failure the caller should
    /// invalidate and repaint (a fresh target is recreated per paint).
    pub fn end(&mut self) -> bool {
        unsafe {
            let hr = (self.rt().end_draw)(self.target, std::ptr::null_mut(), std::ptr::null_mut());
            if hr != S_OK {
                log::debug!("d2d: EndDraw failed (0x{hr:08x})");
            }
            hr == S_OK
        }
    }

    pub fn clear(&mut self, color: Rgba) -> bool {
        let c = premultiplied(color);
        unsafe {
            (self.rt().clear)(self.target, &c);
        }
        true
    }

    pub fn fill_rect(&mut self, rect: RectF, color: Rgba) -> bool {
        self.with_brush(color, |brush| {
            let r = D2D1_RECT_F::from(rect);
            unsafe {
                (self.rt().fill_rectangle)(self.target, &r, brush);
            }
        })
    }

    pub fn fill_rounded_rect(&mut self, rect: RectF, radius: f32, color: Rgba) -> bool {
        self.with_brush(color, |brush| {
            let rr = D2D1_ROUNDED_RECT {
                rect: D2D1_RECT_F::from(rect),
                radius_x: radius,
                radius_y: radius,
            };
            unsafe {
                (self.rt().fill_rounded_rectangle)(self.target, &rr, brush);
            }
        })
    }

    pub fn stroke_rounded_rect(
        &mut self,
        rect: RectF,
        radius: f32,
        color: Rgba,
        width_px: f32,
    ) -> bool {
        self.with_brush(color, |brush| {
            let rr = D2D1_ROUNDED_RECT {
                rect: D2D1_RECT_F::from(rect),
                radius_x: radius,
                radius_y: radius,
            };
            unsafe {
                (self.rt().draw_rounded_rectangle)(
                    self.target,
                    &rr,
                    brush,
                    width_px,
                    std::ptr::null_mut(),
                );
            }
        })
    }

    pub fn fill_circle(&mut self, center: PointF, radius: f32, color: Rgba) -> bool {
        self.with_brush(color, |brush| {
            let e = D2D1_ELLIPSE {
                point: D2D1_POINT_2F {
                    x: center.x,
                    y: center.y,
                },
                radius_x: radius,
                radius_y: radius,
            };
            unsafe {
                (self.rt().fill_ellipse)(self.target, &e, brush);
            }
        })
    }

    pub fn stroke_circle(
        &mut self,
        center: PointF,
        radius: f32,
        color: Rgba,
        width_px: f32,
    ) -> bool {
        self.with_brush(color, |brush| {
            let e = D2D1_ELLIPSE {
                point: D2D1_POINT_2F {
                    x: center.x,
                    y: center.y,
                },
                radius_x: radius,
                radius_y: radius,
            };
            unsafe {
                (self.rt().draw_ellipse)(self.target, &e, brush, width_px, std::ptr::null_mut());
            }
        })
    }

    pub fn fill_diamond(&mut self, center: PointF, half_size: f32, color: Rgba) -> bool {
        let Some(geometry) = self.diamond_geometry(center, half_size) else {
            return false;
        };
        let ok = self.with_brush(color, |brush| unsafe {
            (self.rt().fill_geometry)(self.target, geometry, brush, std::ptr::null_mut());
        });
        unsafe {
            release(geometry);
        }
        ok
    }

    pub fn stroke_diamond(
        &mut self,
        center: PointF,
        half_size: f32,
        color: Rgba,
        width_px: f32,
    ) -> bool {
        let Some(geometry) = self.diamond_geometry(center, half_size) else {
            return false;
        };
        let ok = self.with_brush(color, |brush| unsafe {
            (self.rt().draw_geometry)(self.target, geometry, brush, width_px, std::ptr::null_mut());
        });
        unsafe {
            release(geometry);
        }
        ok
    }

    /// Draw text via DWrite; the text format is created and destroyed inside
    /// the call (mirroring the GDI per-call font).
    pub fn draw_text(&mut self, layout: &TextLayout) -> bool {
        unsafe {
            let Some(fmt) = Direct2dContext::get()
                .and_then(|ctx| ctx.create_text_format(layout.role, layout.align))
            else {
                return false;
            };
            let wide: Vec<u16> = layout.text.encode_utf16().collect();
            let rect = D2D1_RECT_F::from(layout.rect);
            let color = premultiplied(layout.color);
            let mut brush: *mut c_void = std::ptr::null_mut();
            let hr = (self.rt().create_solid_color_brush)(
                self.target,
                &color,
                std::ptr::null(),
                &mut brush,
            );
            let ok = hr == S_OK && !brush.is_null();
            if ok {
                (self.rt().draw_text)(
                    self.target,
                    wide.as_ptr(),
                    wide.len() as u32,
                    fmt,
                    &rect,
                    brush,
                    D2D1_DRAW_TEXT_OPTIONS_NONE,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
                release(brush);
            }
            release(fmt);
            ok
        }
    }

    /// Create a solid brush, run `draw`, destroy the brush. `false` when the
    /// brush could not be created (caller falls back to GDI).
    fn with_brush(&self, color: Rgba, draw: impl FnOnce(*mut c_void)) -> bool {
        unsafe {
            let c = premultiplied(color);
            let mut brush: *mut c_void = std::ptr::null_mut();
            let hr =
                (self.rt().create_solid_color_brush)(self.target, &c, std::ptr::null(), &mut brush);
            if hr != S_OK || brush.is_null() {
                return false;
            }
            draw(brush);
            release(brush);
            true
        }
    }

    /// A diamond (muted-volume marker) as an `ID2D1PathGeometry`, or `None`.
    fn diamond_geometry(&self, center: PointF, half_size: f32) -> Option<*mut c_void> {
        let half_size = half_size.max(0.0);
        unsafe {
            let mut geometry: *mut c_void = std::ptr::null_mut();
            let hr = (self.factory().create_path_geometry)(self.factory, &mut geometry);
            if hr != S_OK || geometry.is_null() {
                return None;
            }
            let mut sink: *mut c_void = std::ptr::null_mut();
            let hr = (vtbl::<ID2D1PathGeometryVtbl>(geometry).open)(geometry, &mut sink);
            if hr != S_OK || sink.is_null() {
                release(geometry);
                return None;
            }
            let sink_vtbl = vtbl::<ID2D1GeometrySinkVtbl>(sink);
            (sink_vtbl.set_fill_mode)(sink, D2D1_FILL_MODE_ALTERNATE);
            let top = D2D1_POINT_2F {
                x: center.x,
                y: center.y - half_size,
            };
            let right = D2D1_POINT_2F {
                x: center.x + half_size,
                y: center.y,
            };
            let bottom = D2D1_POINT_2F {
                x: center.x,
                y: center.y + half_size,
            };
            let left = D2D1_POINT_2F {
                x: center.x - half_size,
                y: center.y,
            };
            (sink_vtbl.begin_figure)(sink, top, D2D1_FIGURE_BEGIN_FILLED);
            (sink_vtbl.add_lines)(sink, [right, bottom, left].as_ptr(), 3);
            (sink_vtbl.end_figure)(sink, D2D1_FIGURE_END_CLOSED);
            let hr = (sink_vtbl.close)(sink);
            release(sink);
            if hr != S_OK {
                release(geometry);
                return None;
            }
            Some(geometry)
        }
    }
}

impl Drop for HwndRenderTarget {
    fn drop(&mut self) {
        unsafe {
            release(self.target);
            release(self.factory);
            release(self.dwrite);
        }
    }
}

impl From<RectF> for D2D1_RECT_F {
    fn from(rect: RectF) -> Self {
        Self {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        }
    }
}

/// Convert an RGBA color to the premultiplied form a premultiplied-alpha
/// target expects (the GDI path drops alpha; D2D honors it).
fn premultiplied(color: Rgba) -> D2D1_COLOR_F {
    let a = f32::from(color.alpha) / 255.0;
    D2D1_COLOR_F {
        r: f32::from(color.red) / 255.0 * a,
        g: f32::from(color.green) / 255.0 * a,
        b: f32::from(color.blue) / 255.0 * a,
        a,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::System::Com::CoInitializeEx;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, WNDCLASSW,
    };

    fn init_com() {
        unsafe {
            CoInitializeEx(std::ptr::null(), 0);
        }
    }

    /// A hidden test window (headless-friendly: no interaction, no paint
    /// delivery, valid client area for D2D).
    fn hidden_window() -> HWND {
        unsafe {
            static CLASS: OnceLock<HWND> = OnceLock::new();
            *CLASS.get_or_init(|| {
                let class = WNDCLASSW {
                    lpfnWndProc: Some(DefWindowProcW),
                    hInstance: windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(
                        std::ptr::null(),
                    ),
                    lpszClassName: windows_sys::core::w!("VolCtlD2dTestWnd"),
                    ..std::mem::zeroed()
                };
                RegisterClassW(&class);
                CreateWindowExW(
                    0,
                    windows_sys::core::w!("VolCtlD2dTestWnd"),
                    windows_sys::core::w!("d2d test"),
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
    fn context_creation_and_text_metrics_work_when_d2d_available() {
        init_com();
        let Some(context) = Direct2dContext::new() else {
            // Headless-friendly: graceful fallback is a valid outcome.
            return;
        };
        let role = TextRole {
            size_px: 13.0,
            weight: 400,
            monospace: false,
        };
        let size = context
            .text_layout_metrics("72%", role, DpiMetrics::new(1.0))
            .expect("text metrics");
        assert!(size.width > 0.0, "width {}", size.width);
        assert!(size.height > 0.0, "height {}", size.height);
        // Larger role => wider text.
        let display = TextRole {
            size_px: 28.0,
            weight: 600,
            monospace: false,
        };
        let big = context
            .text_layout_metrics("72%", display, DpiMetrics::new(1.0))
            .unwrap();
        assert!(big.width > size.width, "{} vs {}", big.width, size.width);
        // The singleton shares the factories.
        assert!(Direct2dContext::get().is_some());
    }

    #[test]
    fn render_target_paints_on_a_hidden_window_when_d2d_available() {
        init_com();
        let Some(context) = Direct2dContext::new() else {
            return;
        };
        let hwnd = hidden_window();
        let mut target = context.render_target(hwnd).expect("render target");
        assert!(target.begin());
        assert!(target.clear(Rgba::from_rgb(0x10, 0x13, 0x1A)));
        assert!(target.fill_rect(RectF::new(10.0, 10.0, 100.0, 50.0), Rgba::WHITE));
        assert!(target.fill_rounded_rect(RectF::new(20.0, 20.0, 80.0, 60.0), 8.0, Rgba::BLACK));
        assert!(target.stroke_rounded_rect(
            RectF::new(20.0, 20.0, 80.0, 60.0),
            4.0,
            Rgba::WHITE,
            1.5
        ));
        assert!(target.fill_circle(PointF::new(50.0, 40.0), 6.0, Rgba::BLACK));
        assert!(target.stroke_circle(PointF::new(50.0, 40.0), 6.0, Rgba::WHITE, 1.0));
        assert!(target.fill_diamond(PointF::new(60.0, 45.0), 6.0, Rgba::BLACK));
        assert!(target.stroke_diamond(PointF::new(60.0, 45.0), 6.0, Rgba::WHITE, 1.0));
        assert!(target.end(), "EndDraw presents the frame");
        unsafe {
            DestroyWindow(hwnd);
        }
    }

    #[test]
    fn text_layout_metrics_snaps_to_physical_grid() {
        init_com();
        let Some(context) = Direct2dContext::new() else {
            return;
        };
        let role = TextRole {
            size_px: 13.0,
            weight: 400,
            monospace: false,
        };
        let at_125 = context
            .text_layout_metrics("Volume", role, DpiMetrics::new(1.25))
            .expect("metrics at 1.25");
        // (dips * 1.25) must be an integer (within float tolerance).
        let physical = at_125.width * 1.25;
        assert!(
            (physical - physical.round()).abs() < 0.01,
            "width {} not on the 1.25x grid",
            at_125.width
        );
        let at_100 = context
            .text_layout_metrics("Volume", role, DpiMetrics::new(1.0))
            .unwrap();
        // Snapping to the coarser 125% grid cannot grow the measure much.
        assert!((at_125.width - at_100.width).abs() < 1.0);
    }
}
