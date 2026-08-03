//! Text layout primitives for the Windows renderer: alignment math, role-based
//! font resolution, and headless GDI text measurement.
//!
//! Everything here is either pure or draws only on an off-screen GDI memory
//! device context, so the module is unit-testable without a visible window.
//! Rendering itself lives in [`super::primitives::PaintCanvas`]; the DWrite
//! text-metrics seam lives in [`super::d2d`].
//!
//! All `windows-sys` FFI signatures below were verified against the installed
//! `windows-sys` 0.52 crate source (cargo registry) before use.

use windows_sys::Win32::{
    Foundation::SIZE,
    Graphics::Gdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, DeleteDC, DeleteObject,
        GetDeviceCaps, GetTextExtentPoint32W, GetTextFaceW, SelectObject, CLEARTYPE_QUALITY,
        CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH, FF_DONTCARE, FF_MODERN, FIXED_PITCH,
        HDC, HFONT, HGDIOBJ, LOGPIXELSX, OUT_DEFAULT_PRECIS,
    },
};

use crate::ui::theme::TextRole;

use super::primitives::{RectF, SizeF};

/// Horizontal text alignment within a layout rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Right,
    Center,
}

/// One text draw request: content, layout box, alignment, role, color.
///
/// Coordinates are LOGICAL pixels; the renderer scales once by its
/// [`super::primitives::DpiMetrics`]. `role` carries the size/weight/face
/// intent from the shared typography tokens.
#[derive(Debug, Clone, Copy)]
pub struct TextLayout<'t> {
    pub text: &'t str,
    pub rect: RectF,
    pub align: TextAlign,
    pub role: TextRole,
    pub color: crate::ui::theme::Rgba,
}

/// X-origin of `text_width`-wide text inside `rect` for `align`.
///
/// Right alignment is real alignment math (`rect.right - text_width`), never
/// space padding. The result is in the same (logical) units as `rect`.
pub fn text_x_origin(rect: RectF, align: TextAlign, text_width: f32) -> f32 {
    match align {
        TextAlign::Left => rect.left,
        TextAlign::Right => rect.right - text_width,
        TextAlign::Center => rect.left + (rect.width() - text_width) * 0.5,
    }
}

/// Font family candidates for a role, most preferred first.
///
/// The final empty entry means "system default" and always resolves, so the
/// chain never runs out. `Segoe UI Variable` is the Windows 11 family; the
/// `Segoe UI` fallback covers Windows 10; the empty entry covers older
/// systems. Monospace roles use the Cascadia/Consolas/Courier chain.
pub fn font_candidates(role: TextRole) -> [&'static str; 3] {
    if role.monospace {
        ["Cascadia Mono", "Consolas", "Courier New"]
    } else {
        ["Segoe UI Variable", "Segoe UI", ""]
    }
}

/// Create a font for `role` at `height_px` (device units) selected into `hdc`.
///
/// Walks [`font_candidates`] and verifies with `GetTextFaceW` that GDI
/// actually honored the requested face (GDI silently substitutes unknown
/// faces, so creation success alone is not a reliable check). On success the
/// created font is left selected in `hdc` and the function returns it
/// together with the object that previously occupied the font slot, so the
/// caller can restore the slot BEFORE deleting the font — GDI refuses to
/// delete an object that is still selected, so deleting first would leak one
/// HFONT per call. Returns `(0, 0)` when every candidate failed.
pub fn create_font_selected(hdc: HDC, role: TextRole, height_px: i32) -> (HFONT, HGDIOBJ) {
    let pitch_and_family = if role.monospace {
        u32::from(FIXED_PITCH | FF_MODERN)
    } else {
        u32::from(DEFAULT_PITCH | FF_DONTCARE)
    };
    for name in font_candidates(role) {
        let name_wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let font = unsafe {
            CreateFontW(
                -height_px,
                0,
                0,
                0,
                i32::from(role.weight),
                0,
                0,
                0,
                u32::from(DEFAULT_CHARSET),
                u32::from(OUT_DEFAULT_PRECIS),
                u32::from(CLIP_DEFAULT_PRECIS),
                u32::from(CLEARTYPE_QUALITY),
                pitch_and_family,
                name_wide.as_ptr(),
            )
        };
        if font == 0 {
            continue;
        }
        if name.is_empty() {
            // Final fallback: the system default face always resolves. Select
            // it and return the displaced slot object like any other success.
            let previous = unsafe { SelectObject(hdc, font as HGDIOBJ) };
            return (font, previous);
        }
        unsafe {
            let previous = SelectObject(hdc, font as HGDIOBJ);
            if previous != 0 && selected_face_matches(hdc, name) {
                return (font, previous);
            }
            if previous != 0 {
                SelectObject(hdc, previous);
            }
        }
        unsafe {
            DeleteObject(font as HGDIOBJ);
        }
    }
    (0, 0)
}

/// Whether the face currently selected in `hdc` is `requested`
/// (case-insensitive), per `GetTextFaceW`.
fn selected_face_matches(hdc: HDC, requested: &str) -> bool {
    unsafe {
        let mut name = [0u16; 64];
        let written = GetTextFaceW(hdc, name.len() as i32, name.as_mut_ptr());
        if written == 0 {
            return false;
        }
        // `written` includes the terminating null.
        let actual = String::from_utf16_lossy(&name[..(written as usize).saturating_sub(1)]);
        actual.eq_ignore_ascii_case(requested)
    }
}

/// Measure `text` in `role` typography with GDI on a memory DC.
///
/// Returns the extent in LOGICAL pixels (device units of the memory DC
/// divided by that DC's scale), so the result is comparable with DWrite
/// metrics from [`super::d2d::text_layout_metrics`]. Requires no window; the
/// DC, bitmap, and font are created and destroyed inside the call.
pub fn measure_text_gdi(text: &str, role: TextRole) -> Option<SizeF> {
    unsafe {
        let hdc = CreateCompatibleDC(0);
        if hdc == 0 {
            return None;
        }
        let bitmap = CreateCompatibleBitmap(hdc, 1, 1);
        if bitmap == 0 {
            DeleteDC(hdc);
            return None;
        }
        let previous_bitmap = SelectObject(hdc, bitmap as HGDIOBJ);

        // The memory DC inherits the system DPI; express the font size in its
        // device units and divide the measured extent back to logical px.
        let dc_dpi = GetDeviceCaps(hdc, LOGPIXELSX as i32);
        let dc_scale = if dc_dpi > 0 {
            dc_dpi as f32 / 96.0
        } else {
            1.0
        };
        let height_px = (role.size_px * dc_scale).round() as i32;
        let (font, previous_font) = create_font_selected(hdc, role, height_px);

        let mut extent = SIZE { cx: 0, cy: 0 };
        let wide: Vec<u16> = text.encode_utf16().collect();
        let measured = if font == 0 {
            false
        } else {
            GetTextExtentPoint32W(hdc, wide.as_ptr(), wide.len() as i32, &mut extent) != 0
        };

        // Restore the font slot before deleting the font (GDI refuses to
        // delete a selected object); the bitmap slot is restored separately.
        if font != 0 {
            if previous_font != 0 {
                let _ = SelectObject(hdc, previous_font);
            }
            DeleteObject(font as HGDIOBJ);
        }
        if previous_bitmap != 0 {
            let _ = SelectObject(hdc, previous_bitmap as HGDIOBJ);
        }
        DeleteObject(bitmap as HGDIOBJ);
        DeleteDC(hdc);

        if measured {
            Some(SizeF {
                width: extent.cx as f32 / dc_scale,
                height: extent.cy as f32 / dc_scale,
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::platform::windows::primitives::RectF;
    use crate::ui::theme::{Rgba, TypographyTokens};
    use windows_sys::Win32::Graphics::Gdi::{GetCurrentObject, OBJ_FONT};

    fn rect() -> RectF {
        RectF::new(10.0, 20.0, 110.0, 40.0)
    }

    /// A sentinel HFONT selected into the DC's font slot, to prove callers
    /// restore the slot (a slot that still holds a created font cannot be
    /// deleted, which is exactly the leak this guards against).
    unsafe fn sentinel_font() -> HFONT {
        let font = CreateFontW(
            -13,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            u32::from(DEFAULT_CHARSET),
            u32::from(OUT_DEFAULT_PRECIS),
            u32::from(CLIP_DEFAULT_PRECIS),
            u32::from(CLEARTYPE_QUALITY),
            u32::from(DEFAULT_PITCH | FF_DONTCARE),
            windows_sys::core::w!("Segoe UI"),
        );
        assert_ne!(font, 0, "sentinel font creation");
        font
    }

    #[test]
    fn measure_text_gdi_restores_the_dc_font_slot() {
        unsafe {
            let hdc = CreateCompatibleDC(0);
            assert_ne!(hdc, 0, "memory DC");
            let sentinel = sentinel_font();
            let original = SelectObject(hdc, sentinel as HGDIOBJ);
            assert_eq!(
                GetCurrentObject(hdc, OBJ_FONT as u32),
                sentinel as HGDIOBJ,
                "sentinel must occupy the font slot before measuring"
            );

            let measured = measure_text_gdi("72%", TypographyTokens::default().body);

            assert!(
                measured.is_some(),
                "measurement must succeed with the sentinel in the slot"
            );
            assert_eq!(
                GetCurrentObject(hdc, OBJ_FONT as u32),
                sentinel as HGDIOBJ,
                "measure_text_gdi must restore the font slot it displaced"
            );
            // Restoring proves the measured font was deletable; deleting the
            // sentinel proves it, too, is no longer selected.
            SelectObject(hdc, original);
            assert_eq!(
                DeleteObject(sentinel as HGDIOBJ),
                1,
                "sentinel must be deletable after the slot is restored"
            );
            DeleteDC(hdc);
        }
    }

    #[test]
    fn create_font_selected_returns_the_replaced_font_slot_object() {
        unsafe {
            let hdc = CreateCompatibleDC(0);
            assert_ne!(hdc, 0, "memory DC");
            let sentinel = sentinel_font();
            let original = SelectObject(hdc, sentinel as HGDIOBJ);

            let (font, previous) = create_font_selected(hdc, TypographyTokens::default().body, 13);
            assert_ne!(font, 0, "font creation must succeed");
            assert_eq!(
                previous, sentinel as HGDIOBJ,
                "the helper must return the object it replaced in the font slot"
            );
            assert_eq!(GetCurrentObject(hdc, OBJ_FONT as u32), font as HGDIOBJ);

            // Restore the slot, then both created fonts must be deletable.
            SelectObject(hdc, previous);
            assert_eq!(DeleteObject(font as HGDIOBJ), 1, "created font deletable");
            SelectObject(hdc, original);
            assert_eq!(
                DeleteObject(sentinel as HGDIOBJ),
                1,
                "sentinel deletable after restore"
            );
            DeleteDC(hdc);
        }
    }

    #[test]
    fn text_x_origin_left_is_rect_left() {
        assert_eq!(text_x_origin(rect(), TextAlign::Left, 30.0), 10.0);
    }

    #[test]
    fn text_x_origin_right_is_rect_right_minus_width() {
        assert_eq!(text_x_origin(rect(), TextAlign::Right, 30.0), 80.0);
    }

    #[test]
    fn text_x_origin_center_splits_remaining_space() {
        // (110 - 10 - 30) / 2 = 35 off the left edge.
        assert_eq!(text_x_origin(rect(), TextAlign::Center, 30.0), 45.0);
    }

    #[test]
    fn text_x_origin_right_never_pads_with_spaces() {
        // A zero-width measurement must land exactly on the right edge.
        assert_eq!(text_x_origin(rect(), TextAlign::Right, 0.0), 110.0);
    }

    #[test]
    fn font_candidates_cover_both_faces() {
        let ty = TypographyTokens::default();
        assert_eq!(
            font_candidates(ty.body),
            ["Segoe UI Variable", "Segoe UI", ""]
        );
        assert_eq!(
            font_candidates(ty.keycap),
            ["Cascadia Mono", "Consolas", "Courier New"]
        );
    }

    #[test]
    fn measure_text_gdi_returns_positive_extent_headlessly() {
        let ty = TypographyTokens::default();
        let size = measure_text_gdi("72%", ty.display_value).expect("GDI measure");
        assert!(size.width > 0.0, "width {}", size.width);
        assert!(size.height > 0.0, "height {}", size.height);
        // 28 px display value is taller than 11 px caption.
        let small = measure_text_gdi("72%", ty.caption).unwrap();
        assert!(
            size.height > small.height,
            "{} vs {}",
            size.height,
            small.height
        );
    }

    #[test]
    fn measure_text_gdi_handles_empty_and_unicode() {
        let ty = TypographyTokens::default();
        let empty = measure_text_gdi("", ty.body);
        assert!(empty.is_some());
        let unicode = measure_text_gdi("音量", ty.body);
        assert!(unicode.is_some_and(|s| s.width > 0.0));
    }

    #[test]
    fn text_layout_carries_all_fields() {
        let ty = TypographyTokens::default();
        let layout = TextLayout {
            text: "72%",
            rect: rect(),
            align: TextAlign::Right,
            role: ty.display_value,
            color: Rgba::WHITE,
        };
        assert_eq!(layout.text, "72%");
        assert_eq!(layout.align, TextAlign::Right);
        assert_eq!(layout.role, ty.display_value);
        assert_eq!(layout.rect, rect());
    }
}
