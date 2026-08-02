//! Volume overlay — native Win32 popup shown briefly on volume changes.
//!
//! Bottom-right, always-on-top, click-through (WS_EX_TRANSPARENT | WS_EX_LAYERED),
//! tool window (no taskbar entry). Paints a dark bar with a threshold-coloured
//! fill and the percentage via plain GDI. Auto-hides after
//! `config.overlay_duration_ms` (default 1800 ms) via a one-shot timer.
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
        CreateWindowExW, DefWindowProcW, DestroyWindow, GetSystemMetrics, GetWindowLongPtrW,
        KillTimer, RegisterClassW, SetLayeredWindowAttributes, SetTimer, SetWindowLongPtrW,
        SetWindowPos, ShowWindow, CS_HREDRAW, CS_VREDRAW, GWLP_USERDATA, HWND_TOPMOST, LWA_ALPHA,
        SM_CXSCREEN, SM_CYSCREEN, SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE, WM_ERASEBKGND, WM_PAINT,
        WM_TIMER, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
        WS_EX_TRANSPARENT, WS_POPUP,
    },
};

use crate::audio::VolumeState;
use crate::config::{ColorThresholds, Config};
use crate::core::volume_color_rgb;

pub(crate) const OVERLAY_WIDTH: i32 = 320;
pub(crate) const OVERLAY_HEIGHT: i32 = 64;
pub(crate) const OVERLAY_MARGIN_X: i32 = 20;
pub(crate) const OVERLAY_MARGIN_Y: i32 = 40;
const TIMER_ID: usize = 1;

/// GDI COLORREF is 0x00BBGGRR.
fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

/// State shared with the window proc via GWLP_USERDATA.
struct OverlayData {
    percent: u8,
    muted: bool,
    thresholds: ColorThresholds,
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
            SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, data as isize);

            Ok(Overlay { hwnd, data })
        }
    }

    /// Show (or refresh) the overlay for the given volume state.
    pub fn show(&mut self, state: &VolumeState, config: &Config) {
        unsafe {
            let d = &mut *self.data;
            d.percent = state.percent();
            d.muted = state.muted;
            d.thresholds = config.color_thresholds.clone();
            d.toast = None;

            // Bottom-right of the primary screen.
            let sw = GetSystemMetrics(SM_CXSCREEN);
            let sh = GetSystemMetrics(SM_CYSCREEN);
            let ok = SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                sw - OVERLAY_WIDTH - OVERLAY_MARGIN_X,
                sh - OVERLAY_HEIGHT - OVERLAY_MARGIN_Y,
                OVERLAY_WIDTH,
                OVERLAY_HEIGHT,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            log::debug!(
                "overlay show hwnd={:x} pos=({},{}) ok={}",
                self.hwnd,
                sw - OVERLAY_WIDTH - OVERLAY_MARGIN_X,
                sh - OVERLAY_HEIGHT - OVERLAY_MARGIN_Y,
                ok
            );

            InvalidateRect(self.hwnd, std::ptr::null(), 0);

            // Restart the auto-hide timer.
            KillTimer(self.hwnd, TIMER_ID);
            let ms = config.overlay_duration_ms.min(10_000) as u32;
            SetTimer(self.hwnd, TIMER_ID, ms, None);
        }
    }

    /// Show a text toast (e.g. "Config reloaded", "No blacklist needed").
    pub fn show_text(&mut self, text: &str, config: &Config) {
        unsafe {
            let d = &mut *self.data;
            d.toast = Some(text.to_string());

            let sw = GetSystemMetrics(SM_CXSCREEN);
            let sh = GetSystemMetrics(SM_CYSCREEN);
            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                sw - OVERLAY_WIDTH - OVERLAY_MARGIN_X,
                sh - OVERLAY_HEIGHT - OVERLAY_MARGIN_Y,
                OVERLAY_WIDTH,
                OVERLAY_HEIGHT,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            InvalidateRect(self.hwnd, std::ptr::null(), 0);

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

/// Draw the overlay contents: dark background, bar track, coloured fill, %.
/// When `data.toast` is set, only the toast text is drawn.
unsafe fn paint(hdc: isize, data: &OverlayData, w: i32, h: i32) {
    // Background.
    let bg = CreateSolidBrush(rgb(0x14, 0x14, 0x18));
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
        SetTextColor(hdc, rgb(0xDD, 0xDD, 0xDD));
        TextOutW(hdc, 24, 24, wide.as_ptr(), (wide.len() - 1) as i32);
        return;
    }

    // Bar track.
    let bar_left = 16;
    let bar_right = w - 16;
    let bar_top = h / 2 - 6;
    let bar_bottom = h / 2 + 6;

    let track = CreateSolidBrush(rgb(0x34, 0x34, 0x40));
    let track_rect = RECT {
        left: bar_left,
        top: bar_top,
        right: bar_right,
        bottom: bar_bottom,
    };
    FillRect(hdc, &track_rect, track);
    DeleteObject(track);

    // Coloured fill by threshold.
    let (r, g, b) = volume_color_rgb(data.percent, data.muted, &data.thresholds);
    let fill_w = ((bar_right - bar_left) * data.percent as i32) / 100;
    if fill_w > 0 {
        let fill = CreateSolidBrush(rgb(r, g, b));
        let fill_rect = RECT {
            left: bar_left,
            top: bar_top,
            right: bar_left + fill_w,
            bottom: bar_bottom,
        };
        FillRect(hdc, &fill_rect, fill);
        DeleteObject(fill);
    }

    // Percentage label (or "Muted").
    let label: Vec<u16> = if data.muted {
        "Muted".encode_utf16().chain(std::iter::once(0)).collect()
    } else {
        format!("{}%", data.percent)
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect()
    };
    SetTextColor(
        hdc,
        if data.muted {
            rgb(0x88, 0x88, 0x88)
        } else {
            rgb(0xDD, 0xDD, 0xDD)
        },
    );
    TextOutW(hdc, 24, 20, label.as_ptr(), (label.len() - 1) as i32);
}
