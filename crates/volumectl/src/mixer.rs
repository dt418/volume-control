//! Volume Mixer — a Win11-style flyout with a native trackbar slider.
//!
//! Captionless, always-on-top tool window (bottom-right), DWM-styled like the
//! system flyout: rounded corners (33), immersive dark mode (20), and the
//! system backdrop (38). Contains:
//!   - a small accent bar along the top
//!   - "System Volume" + live percentage labels
//!   - a trackbar (0–100, no ticks) — dragging changes volume live
//!   - Mute / Unmute and Reset to 50% buttons
//!
//! User interaction (slider drag, buttons) posts [`WM_APP_MIXER_*`] messages
//! to the host window, which owns the audio backend — so the app's single
//! message loop applies volume changes and syncs the overlay/tray.

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Dwm::{
        DwmSetWindowAttribute, DWMSBT_MAINWINDOW, DWMWA_SYSTEMBACKDROP_TYPE,
        DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    },
    Graphics::Gdi::{
        BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, SetBkMode, SetTextColor,
        HBRUSH, HDC, PAINTSTRUCT, TRANSPARENT,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::Controls::{InitCommonControlsEx, SetWindowTheme, ICC_BAR_CLASSES, INITCOMMONCONTROLSEX},
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, GetSystemMetrics, GetWindowLongPtrW,
        PostMessageW, RegisterClassW, SendMessageW, SetWindowLongPtrW, SetWindowPos,
        SetWindowTextW, ShowWindow, BN_CLICKED, CW_USEDEFAULT, GWLP_USERDATA, HWND_TOPMOST,
        SM_CXSCREEN, SM_CYSCREEN, SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE, WM_CLOSE, WM_COMMAND,
        WM_CTLCOLORSTATIC, WM_ERASEBKGND, WM_HSCROLL, WM_PAINT, WM_USER, WNDCLASSW, WS_CHILD,
        WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_VISIBLE,
    },
};

use crate::audio::VolumeState;
use crate::overlay::{OVERLAY_HEIGHT, OVERLAY_MARGIN_X, OVERLAY_MARGIN_Y};

/// Custom messages the mixer posts to the host window (see `app.rs`).
pub const WM_APP_MIXER_CHANGE: u32 = WM_USER + 11; // wparam = new volume %
pub const WM_APP_MIXER_MUTE: u32 = WM_USER + 12;
pub const WM_APP_MIXER_RESET: u32 = WM_USER + 13;

const WIN_W: i32 = 360;
const WIN_H: i32 = 178;
/// Gap between the mixer card and the transient volume overlay.
const OVERLAY_GAP: i32 = 16;

const ID_BTN_MUTE: usize = 1;
const ID_BTN_RESET: usize = 2;
const ID_BTN_CLOSE: usize = 3;

#[derive(Clone, Copy)]
struct MixerTheme {
    dark: bool,
    background: u32,
    text: u32,
    secondary_text: u32,
}

impl MixerTheme {
    fn system() -> Self {
        // Relocated from the local `system_prefers_dark` into the shared
        // Windows primitives; `None` (unreadable) falls back to light.
        let dark = crate::ui::primitives::system_theme().unwrap_or(false);
        Self {
            dark,
            background: if dark {
                rgb(0x20, 0x20, 0x20)
            } else {
                rgb(0xF9, 0xF9, 0xF9)
            },
            text: if dark {
                rgb(0xF5, 0xF5, 0xF5)
            } else {
                rgb(0x1A, 0x1A, 0x1A)
            },
            secondary_text: if dark {
                rgb(0xB8, 0xB8, 0xB8)
            } else {
                rgb(0x5F, 0x5F, 0x5F)
            },
        }
    }
}

fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

// Trackbar messages (TBM_*) — canonical Win32 values (WM_USER-based,
// verified against CommCtrl.h: TBM_GETPOS=WM_USER, TBM_SETPOS=WM_USER+5,
// TBM_SETRANGE=WM_USER+6).
const TBM_FIRST: u32 = 0x0400;
const TBM_SETRANGE: u32 = TBM_FIRST + 6;
const TBM_GETPOS: u32 = TBM_FIRST + 0;
const TBM_SETPOS: u32 = TBM_FIRST + 5;
const TBS_HORZ: u32 = 0;
const TBS_NOTICKS: u32 = 0x10;

/// Per-window state stored in GWLP_USERDATA.
struct MixerData {
    host: HWND,
    slider: HWND,
    percent_label: HWND,
    mute_btn: HWND,
    _reset_btn: HWND,
    _close_btn: HWND,
    accent: HBRUSH,
    background: HBRUSH,
    theme: MixerTheme,
    muted: bool,
    open: bool,
}

impl MixerData {
    fn placeholder(host: HWND, theme: MixerTheme) -> Self {
        Self {
            host,
            slider: 0,
            percent_label: 0,
            mute_btn: 0,
            _reset_btn: 0,
            _close_btn: 0,
            accent: 0,
            background: 0,
            theme,
            muted: false,
            open: false,
        }
    }
}

pub struct Mixer {
    hwnd: HWND,
}

impl Mixer {
    /// Create the hidden mixer window (shown via `toggle`).
    pub fn new(host: HWND) -> Result<Mixer, Box<dyn std::error::Error>> {
        unsafe {
            let mut icc: INITCOMMONCONTROLSEX = std::mem::zeroed();
            icc.dwSize = std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32;
            icc.dwICC = ICC_BAR_CLASSES;
            InitCommonControlsEx(&icc);
        }

        unsafe {
            let hinst = GetModuleHandleW(std::ptr::null());
            let class = windows_sys::core::w!("VolCtlMixer");
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.lpfnWndProc = Some(mixer_wndproc);
            wc.hInstance = hinst;
            wc.lpszClassName = class;
            RegisterClassW(&wc);

            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                class,
                windows_sys::core::w!("Volume Mixer"),
                WS_POPUP,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                WIN_W,
                WIN_H,
                0,
                0,
                hinst,
                std::ptr::null(),
            );
            if hwnd == 0 {
                return Err("mixer CreateWindowEx failed".into());
            }
            let theme = MixerTheme::system();
            Self::style(hwnd, theme.dark);
            let data = Box::into_raw(Box::new(MixerData::placeholder(host, theme)));
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, data as isize);

            // Trackbar slider.
            let slider = CreateWindowExW(
                0,
                windows_sys::core::w!("msctls_trackbar32"),
                windows_sys::core::w!("slider"),
                WS_CHILD | WS_VISIBLE | TBS_HORZ | TBS_NOTICKS,
                18,
                88,
                324,
                28,
                hwnd,
                0, // id (unused)
                hinst,
                std::ptr::null(),
            );
            // Live percentage label.
            let percent_label = CreateWindowExW(
                0,
                windows_sys::core::w!("STATIC"),
                windows_sys::core::w!("100%"),
                WS_CHILD | WS_VISIBLE,
                18,
                44,
                200,
                30,
                hwnd,
                0,
                hinst,
                std::ptr::null(),
            );
            // Mute / Reset buttons.
            let mute_btn = CreateWindowExW(
                0,
                windows_sys::core::w!("Button"),
                windows_sys::core::w!("  Mute"),
                WS_CHILD | WS_VISIBLE,
                18,
                128,
                150,
                30,
                hwnd,
                ID_BTN_MUTE as isize,
                hinst,
                std::ptr::null(),
            );
            let reset_btn = CreateWindowExW(
                0,
                windows_sys::core::w!("Button"),
                windows_sys::core::w!("  Reset to 50%"),
                WS_CHILD | WS_VISIBLE,
                186,
                128,
                156,
                30,
                hwnd,
                ID_BTN_RESET as isize,
                hinst,
                std::ptr::null(),
            );
            // A visible close affordance hides the flyout without changing
            // the existing hotkey/tray toggle behavior.
            let close_btn = CreateWindowExW(
                0,
                windows_sys::core::w!("Button"),
                windows_sys::core::w!("×"),
                WS_CHILD | WS_VISIBLE,
                WIN_W - 42,
                12,
                28,
                28,
                hwnd,
                ID_BTN_CLOSE as isize,
                hinst,
                std::ptr::null(),
            );
            if slider == 0
                || percent_label == 0
                || mute_btn == 0
                || reset_btn == 0
                || close_btn == 0
            {
                return Err("mixer child control failed".into());
            }

            // Use the dark Explorer theme only for dark mode; the default
            // theme gives light mode the native light button/trackbar styling.
            if theme.dark {
                for ctl in [mute_btn, reset_btn, close_btn, slider] {
                    SetWindowTheme(
                        ctl,
                        windows_sys::core::w!("DarkMode_Explorer"),
                        std::ptr::null(),
                    );
                }
            }
            set_window_text(close_btn, "×");

            // Range 0–100, position 50.
            // TBM_SETRANGE packs MAKELONG(min, max) = (max << 16) | min in the
            // LOWORD/HIWORD of lParam. ((100i64) << 32 | 0) would put 100 above
            // bit 31 where the control can't see it, leaving a degenerate range.
            SendMessageW(slider, TBM_SETRANGE, 1, (100isize) << 16);
            SendMessageW(slider, TBM_SETPOS, 1, 50);

            let accent = CreateSolidBrush(0x00_00_78_D4); // 0x00BBGGRR — blue
            let background = CreateSolidBrush(theme.background);

            let d = &mut *data;
            d.slider = slider;
            d.percent_label = percent_label;
            d.mute_btn = mute_btn;
            d._reset_btn = reset_btn;
            d._close_btn = close_btn;
            d.accent = accent;
            d.background = background;

            Ok(Mixer { hwnd })
        }
    }

    /// Apply the Win11 DWM styling (rounded corners, theme-aware backdrop).
    fn style(hwnd: HWND, dark: bool) {
        unsafe {
            let v: i32 = DWMWCP_ROUND;
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                &v as *const _ as *const _,
                4,
            );
            let v: i32 = if dark { 1 } else { 0 };
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
                &v as *const _ as *const _,
                4,
            );
            let v: i32 = DWMSBT_MAINWINDOW;
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE as u32,
                &v as *const _ as *const _,
                4,
            );
        }
    }

    /// Show (synced first) or hide. The app calls this from the mixer hotkey.
    pub fn toggle(&mut self) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut MixerData);
            if d.open {
                ShowWindow(self.hwnd, SW_HIDE);
                d.open = false;
            } else {
                let sw = GetSystemMetrics(SM_CXSCREEN);
                let sh = GetSystemMetrics(SM_CYSCREEN);
                SetWindowPos(
                    self.hwnd,
                    HWND_TOPMOST,
                    sw - WIN_W - OVERLAY_MARGIN_X,
                    sh - WIN_H - OVERLAY_HEIGHT - OVERLAY_GAP - OVERLAY_MARGIN_Y,
                    WIN_W,
                    WIN_H,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
                d.open = true;
            }
        }
    }

    pub fn is_open(&self) -> bool {
        unsafe {
            let d = &*(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *const MixerData);
            d.open
        }
    }

    /// Push the current audio state into the controls (poll sync).
    /// `TBM_SETPOS` does not emit `WM_HSCROLL`, so no feedback loop.
    pub fn sync(&self, state: &VolumeState) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut MixerData);
            d.muted = state.muted;
            let pct = state.percent() as isize;
            let cur = SendMessageW(d.slider, TBM_GETPOS, 0, 0);
            if cur != pct {
                log::debug!(
                    "mixer sync: slider {} -> {} (muted={})",
                    cur,
                    pct,
                    state.muted
                );
                SendMessageW(d.slider, TBM_SETPOS, 1, pct);
            }
            set_window_text(d.percent_label, &format!("{}%", state.percent()));
            set_window_text(d.mute_btn, if state.muted { "  Unmute" } else { "  Mute" });
        }
    }

    /// Free resources + destroy the window.
    fn destroy(&mut self) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut MixerData);
            DeleteObject(d.accent);
            DeleteObject(d.background);
            drop(Box::from_raw(d));
            DestroyWindow(self.hwnd);
        }
    }
}

unsafe extern "system" fn mixer_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        // ── Slider moved by the user → tell the host to set volume ───────
        WM_HSCROLL => {
            let d = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MixerData);
            let pos = SendMessageW(d.slider, TBM_GETPOS, 0, 0) as u32;
            log::debug!("mixer hscroll: pos={}", pos);
            PostMessageW(d.host, WM_APP_MIXER_CHANGE, pos as usize, 0);
            0
        }
        // ── Buttons → tell the host ──────────────────────────────────────
        WM_COMMAND if (wparam >> 16) as u32 == BN_CLICKED => {
            let d = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MixerData);
            match (wparam & 0xFFFF) as usize {
                ID_BTN_MUTE => PostMessageW(d.host, WM_APP_MIXER_MUTE, 0, 0),
                ID_BTN_RESET => PostMessageW(d.host, WM_APP_MIXER_RESET, 0, 0),
                ID_BTN_CLOSE => {
                    SendMessageW(hwnd, WM_CLOSE, 0, 0);
                    0
                }
                _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
            };
            0
        }
        WM_CTLCOLORSTATIC => {
            let d = &*(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const MixerData);
            SetBkMode(wparam as HDC, TRANSPARENT as i32);
            SetTextColor(wparam as HDC, d.theme.text);
            d.background as LRESULT
        }
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            let d = &*(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const MixerData);
            let rect = RECT {
                left: 0,
                top: 0,
                right: WIN_W,
                bottom: WIN_H,
            };
            FillRect(hdc, &rect, d.background);
            SetBkMode(hdc, TRANSPARENT as i32);
            SetTextColor(hdc, d.theme.secondary_text);
            let label: Vec<u16> = "System volume"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            windows_sys::Win32::Graphics::Gdi::TextOutW(
                hdc,
                18,
                20,
                label.as_ptr(),
                (label.len() - 1) as i32,
            );
            EndPaint(hwnd, &ps);
            0
        }
        // ── Close (Esc / close) just hides ───────────────────────────────
        WM_CLOSE => {
            let d = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MixerData);
            d.open = false;
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

impl Drop for Mixer {
    fn drop(&mut self) {
        self.destroy();
    }
}
/// Set the text of a child control (UTF-16).
fn set_window_text(hwnd: HWND, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        SetWindowTextW(hwnd, wide.as_ptr());
    }
}
