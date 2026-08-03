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
        DwmIsCompositionEnabled, DwmSetWindowAttribute, DWMSBT_NONE, DWMSBT_TRANSIENTWINDOW,
        DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE,
        DWMWCP_ROUND,
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
        GetSystemMetrics, SystemParametersInfoW, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
        SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SPI_GETCLIENTAREAANIMATION, SPI_GETHIGHCONTRAST,
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
