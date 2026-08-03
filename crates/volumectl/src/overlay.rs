//! Volume overlay — native Win32 popup shown briefly on volume changes.
//!
//! Bottom-right, always-on-top, click-through (WS_EX_TRANSPARENT | WS_EX_LAYERED),
//! tool window (no taskbar entry). Paints an adaptive bar — background/text
//! colours from the shared [`crate::ui::ThemeTokens`], threshold-coloured fill
//! from the VolumePro palette — with the percentage via plain GDI. Auto-hides
//! after `config.overlay_duration_ms` (default 1800 ms) via a one-shot timer.
//!
//! Placement is the bottom-right of the *monitor work area* hosting the window
//! (taskbar excluded), computed through [`crate::ui::surface::place_overlay`].
//! The `OVERLAY_*` geometry constants remain the single source of truth that
//! the mixer placement consumes.
//!
//! The overlay is created once at startup (hidden) and shown on demand by the
//! app when a custom hotkey changes the volume. Media keys keep the native
//! Windows flyout, so the overlay never fights it.

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{
        BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, InvalidateRect, SetBkMode,
        SetTextColor, TextOutW, PAINTSTRUCT, TRANSPARENT,
    },
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
use crate::core::volume_color_rgb;
use crate::ui::primitives::{apply_backdrop, colorref, work_area_for};
use crate::ui::{
    place_overlay, resolve_material, tokens_for, AccentMode, ResolvedMaterial, Rgba, SurfaceSize,
    ThemeMode, ThemeTokens, UiCapabilities,
};

pub(crate) const OVERLAY_WIDTH: i32 = 320;
pub(crate) const OVERLAY_HEIGHT: i32 = 64;
pub(crate) const OVERLAY_MARGIN_X: i32 = 20;
pub(crate) const OVERLAY_MARGIN_Y: i32 = 40;
const TIMER_ID: usize = 1;

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
        Self { tokens, material }
    }

    /// Placeholder used before the first `show` (the window is hidden then, so
    /// this is never painted).
    fn placeholder() -> Self {
        Self {
            tokens: tokens_for(ThemeMode::System, false, AccentMode::System, || None),
            material: ResolvedMaterial::Opaque,
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

            // Bottom-right of the monitor work area hosting the window.
            let work_area = work_area_for(self.hwnd);
            let rect = place_overlay(
                work_area,
                SurfaceSize::new(OVERLAY_WIDTH, OVERLAY_HEIGHT),
                OVERLAY_MARGIN_X,
                OVERLAY_MARGIN_Y,
            );
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
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            let data = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayData);
            paint(hdc, data, OVERLAY_WIDTH, OVERLAY_HEIGHT);
            EndPaint(hwnd, &ps);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Draw the overlay contents: adaptive background, bar track, coloured fill, %.
/// When `data.toast` is set, only the toast text is drawn.
unsafe fn paint(hdc: isize, data: &OverlayData, w: i32, h: i32) {
    let tokens = &data.appearance.tokens;

    // Background — always painted; this renderer fill is the material
    // fallback, so the surface is opaque regardless of DWM backdrop support.
    let bg = CreateSolidBrush(colorref(tokens.background));
    let bg_rect = RECT {
        left: 0,
        top: 0,
        right: w,
        bottom: h,
    };
    FillRect(hdc, &bg_rect, bg);
    DeleteObject(bg);

    SetBkMode(hdc, TRANSPARENT as i32);

    // Toast mode: just the text, centered-ish.
    if let Some(text) = &data.toast {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        SetTextColor(hdc, colorref(tokens.text_primary));
        TextOutW(hdc, 24, 24, wide.as_ptr(), (wide.len() - 1) as i32);
        return;
    }

    // Bar track.
    let bar_left = 16;
    let bar_right = w - 16;
    let bar_top = h / 2 - 6;
    let bar_bottom = h / 2 + 6;

    let track = CreateSolidBrush(colorref(tokens.border));
    let track_rect = RECT {
        left: bar_left,
        top: bar_top,
        right: bar_right,
        bottom: bar_bottom,
    };
    FillRect(hdc, &track_rect, track);
    DeleteObject(track);

    // Coloured fill by threshold. The fill still honours the user's
    // `config.color_thresholds` band boundaries via `volume_color_rgb`; its
    // output matches the VolumePro palette encoded in `tokens.volume_threshold`
    // exactly, so the overlay stays consistent with the adaptive tokens while
    // keeping the user's band boundaries authoritative.
    let (r, g, b) = volume_color_rgb(data.percent, data.muted, &data.thresholds);
    let fill_w = ((bar_right - bar_left) * data.percent as i32) / 100;
    if fill_w > 0 {
        let fill = CreateSolidBrush(colorref(Rgba::from_rgb(r, g, b)));
        let fill_rect = RECT {
            left: bar_left,
            top: bar_top,
            right: bar_left + fill_w,
            bottom: bar_bottom,
        };
        FillRect(hdc, &fill_rect, fill);
        DeleteObject(fill);
    }

    // Percentage label (or "Muted"). Muted uses the VolumePro muted grey (the
    // exact legacy colour); the live percentage uses the token primary text
    // colour.
    let label: Vec<u16> = if data.muted {
        "Muted".encode_utf16().chain(std::iter::once(0)).collect()
    } else {
        format!("{}%", data.percent)
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect()
    };
    let label_color = if data.muted {
        tokens.volume_threshold.muted
    } else {
        tokens.text_primary
    };
    SetTextColor(hdc, colorref(label_color));
    TextOutW(hdc, 24, 20, label.as_ptr(), (label.len() - 1) as i32);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{MaterialMode, ThemeMode, WorkArea};

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

    #[test]
    fn appearance_carries_the_volumepro_threshold_palette() {
        // The bar fill stays VolumePro-derived (`volume_color_rgb`); the tokens
        // encode the same palette, so the two never disagree.
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
    fn dark_appearance_preserves_the_legacy_overlay_colours() {
        let a = appearance(ThemeMode::Dark, MaterialMode::Auto, false);
        assert_eq!(a.tokens.background, Rgba::from_rgb(0x14, 0x14, 0x18));
        assert_eq!(a.tokens.text_primary, Rgba::from_rgb(0xDD, 0xDD, 0xDD));
        assert!(a.tokens.is_dark);
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
}
