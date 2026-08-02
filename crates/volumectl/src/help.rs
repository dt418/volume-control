//! Help / Hotkeys window — a small reference card (VolumePro parity).
//!
//! GDI-painted, captionless, always-on-top tool window. Shows the current
//! hotkey map (modifier-aware), tray entries, beep guide, blacklist count and
//! a conflict note. Two clickable buttons: "Edit Config" and "Got it!".
//! Hit-testing is done manually on `WM_LBUTTONDOWN` against fixed rects.

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{
        BeginPaint, CreateFontW, CreateSolidBrush, DeleteObject, EndPaint, FillRect,
        InvalidateRect, SelectObject, SetBkMode, SetTextColor, TextOutW, HBRUSH, HFONT,
        PAINTSTRUCT, TRANSPARENT,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, GetSystemMetrics, GetWindowLongPtrW, RegisterClassW,
        SetWindowLongPtrW, SetWindowPos, ShowWindow, CW_USEDEFAULT, GWLP_USERDATA, HWND_TOPMOST,
        SM_CXSCREEN, SM_CYSCREEN, SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE, WM_CLOSE, WM_ERASEBKGND,
        WM_LBUTTONDOWN, WM_PAINT, WNDCLASSW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    },
};

use crate::config::Config;

const WIN_W: i32 = 480;
const WIN_H: i32 = 380;
const MARGIN_X: i32 = 24;
const MARGIN_Y: i32 = 48;

// Button geometry (bottom-left).
const BTN_Y: i32 = WIN_H - 44;
const BTN_H: i32 = 30;
const BTN_EDIT: (i32, i32, i32, i32) = (16, BTN_Y, 150, BTN_H);
const BTN_GOTIT: (i32, i32, i32, i32) = (176, BTN_Y, 130, BTN_H);

/// One painted text row: (text, color).
struct Row {
    text: String,
    color: u32,
    bold: bool,
}

struct HelpData {
    rows: Vec<Row>,
    hfont_body: HFONT,
    hfont_bold: HFONT,
    bg: HBRUSH,
    open: bool,
}

pub struct Help {
    hwnd: HWND,
}

fn font(height: i32, bold: bool) -> HFONT {
    unsafe {
        CreateFontW(
            -height,
            0,
            0,
            0,
            if bold { 600 } else { 400 },
            0,
            0,
            0,
            1, // DEFAULT_CHARSET
            0,
            0,
            5, // CLEARTYPE_QUALITY
            0,
            windows_sys::core::w!("Segoe UI"),
        )
    }
}

fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

impl Help {
    pub fn new() -> Result<Help, Box<dyn std::error::Error>> {
        unsafe {
            let hinst = GetModuleHandleW(std::ptr::null());
            let class = windows_sys::core::w!("VolCtlHelp");
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.lpfnWndProc = Some(help_wndproc);
            wc.hInstance = hinst;
            wc.lpszClassName = class;
            RegisterClassW(&wc);

            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                class,
                windows_sys::core::w!("VolumeControl Help"),
                WS_POPUP,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                WIN_W,
                WIN_H + 40,
                0,
                0,
                hinst,
                std::ptr::null(),
            );
            if hwnd == 0 {
                return Err("help CreateWindowEx failed".into());
            }

            let data = Box::into_raw(Box::new(HelpData {
                rows: Vec::new(),
                hfont_body: font(12, false),
                hfont_bold: font(12, true),
                bg: CreateSolidBrush(rgb(0x1E, 0x1E, 0x1E)),
                open: false,
            }));
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, data as isize);

            Ok(Help { hwnd })
        }
    }

    /// Rebuild the content from the current config and show the window.
    pub fn show(&mut self, config: &Config) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut HelpData);

            let mod_label = match config.modifier {
                crate::config::HotkeyModifier::CtrlAlt => "Ctrl+Alt",
                crate::config::HotkeyModifier::CapsLock => "CapsLock",
                crate::config::HotkeyModifier::Alt => "Alt",
                crate::config::HotkeyModifier::Ctrl => "Ctrl",
            };

            let accent = rgb(0x00, 0x78, 0xD4);
            let dim = rgb(0x88, 0x88, 0x88);
            let body = rgb(0xCC, 0xCC, 0xCC);
            let green = rgb(0x27, 0xAE, 0x60);
            let gold = rgb(0xE0, 0xA8, 0x00);

            let mut rows: Vec<Row> = Vec::new();
            rows.push(Row {
                text: "VolumeControl".into(),
                color: body,
                bold: true,
            });
            rows.push(Row {
                text: "Advanced Volume Controller".into(),
                color: dim,
                bold: false,
            });
            rows.push(Row {
                text: format!("  Hotkeys  (Modifier: {mod_label})"),
                color: accent,
                bold: true,
            });
            rows.push(Row {
                text: format!(
                    "{mod_label} + \u{2191} / \u{2193}          Volume \u{00B1}{}%",
                    config.volume_step
                ),
                color: body,
                bold: false,
            });
            rows.push(Row {
                text: format!(
                    "Shift + {mod_label} + \u{2191} / \u{2193}   Volume \u{00B1}{}%",
                    config.volume_step_large
                ),
                color: body,
                bold: false,
            });
            rows.push(Row {
                text: format!("{mod_label} + Scroll          Volume via mouse wheel"),
                color: body,
                bold: false,
            });
            rows.push(Row {
                text: format!("{mod_label} + M                 Toggle Mute"),
                color: body,
                bold: false,
            });
            rows.push(Row {
                text: format!("{mod_label} + V                 Open / Close Mixer"),
                color: body,
                bold: false,
            });
            rows.push(Row {
                text: format!("{mod_label} + R                 Reset to 50%"),
                color: body,
                bold: false,
            });
            rows.push(Row {
                text: "Volume Up / Down / Mute    Media keys (native flyout)".into(),
                color: body,
                bold: false,
            });

            let conflict = match config.modifier {
                crate::config::HotkeyModifier::CtrlAlt | crate::config::HotkeyModifier::CapsLock => {
                    (String::from("\u{2705} No known conflicts"), green)
                }
                crate::config::HotkeyModifier::Alt => (
                    String::from("\u{26A0}\u{FE0F} Alt+\u{2191}\u{2193} conflicts with move-line in code editors"),
                    gold,
                ),
                crate::config::HotkeyModifier::Ctrl => (
                    String::from("\u{26A0}\u{FE0F} Ctrl+V (paste), Ctrl+Scroll (zoom), Ctrl+R (reload)"),
                    gold,
                ),
            };
            rows.push(Row {
                text: conflict.0,
                color: conflict.1,
                bold: false,
            });

            let bl_count = config.blacklist.len();
            rows.push(Row {
                text: format!(
                    "\u{1F6E1}\u{FE0F} Blacklist: {}",
                    if bl_count == 0 {
                        "empty".to_string()
                    } else {
                        format!("{bl_count} app(s)")
                    }
                ),
                color: body,
                bold: false,
            });

            rows.push(Row {
                text: "  SYSTEM TRAY  (right-click icon)".into(),
                color: accent,
                bold: true,
            });
            rows.push(Row {
                text: "Volume Mixer / Mute \u{2022} Unmute / Help / Hotkeys".into(),
                color: body,
                bold: false,
            });
            rows.push(Row {
                text: "Edit Config / Reload Config / Rec. Blacklist / Exit".into(),
                color: body,
                bold: false,
            });

            rows.push(Row {
                text: "  BEEP GUIDE".into(),
                color: accent,
                bold: true,
            });
            rows.push(Row {
                text: "Low beep (400 Hz)    Hotkey blocked \u{2014} app in blacklist".into(),
                color: body,
                bold: false,
            });
            rows.push(Row {
                text: "High beep (600 Hz)   Volume at limit (0% or 100%)".into(),
                color: body,
                bold: false,
            });

            d.rows = rows;

            let sw = GetSystemMetrics(SM_CXSCREEN);
            let sh = GetSystemMetrics(SM_CYSCREEN);
            let h = WIN_H + 40;
            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                sw - WIN_W - MARGIN_X,
                sh - h - MARGIN_Y,
                WIN_W,
                h,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            d.open = true;
            InvalidateRect(self.hwnd, std::ptr::null(), 0);
        }
    }

    pub fn hide(&mut self) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut HelpData);
            d.open = false;
            ShowWindow(self.hwnd, SW_HIDE);
        }
    }
}

unsafe extern "system" fn help_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            paint(hdc, hwnd);
            EndPaint(hwnd, &ps);
            0
        }
        WM_LBUTTONDOWN => {
            let d = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut HelpData);
            let x = (lparam & 0xFFFF) as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i32;
            let in_rect =
                |r: (i32, i32, i32, i32)| x >= r.0 && x <= r.0 + r.2 && y >= r.1 && y <= r.1 + r.3;
            if in_rect(BTN_EDIT) {
                crate::config::open_in_editor();
                d.open = false;
                ShowWindow(hwnd, SW_HIDE);
            } else if in_rect(BTN_GOTIT) {
                d.open = false;
                ShowWindow(hwnd, SW_HIDE);
            }
            0
        }
        WM_CLOSE => {
            let d = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut HelpData);
            d.open = false;
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn paint(hdc: isize, hwnd: HWND) {
    let d = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut HelpData);

    // Background.
    let bg_rect = RECT {
        left: 0,
        top: 0,
        right: WIN_W,
        bottom: WIN_H + 40,
    };
    FillRect(hdc, &bg_rect, d.bg);

    // Accent bar on top.
    let accent = CreateSolidBrush(rgb(0x00, 0x78, 0xD4));
    let acc_rect = RECT {
        left: 0,
        top: 0,
        right: WIN_W,
        bottom: 3,
    };
    FillRect(hdc, &acc_rect, accent);
    DeleteObject(accent);

    SetBkMode(hdc, TRANSPARENT as i32);

    let mut y = 20;
    for row in &d.rows {
        let hfont = if row.bold { d.hfont_bold } else { d.hfont_body };
        SelectObject(hdc, hfont);
        SetTextColor(hdc, row.color);
        let text: Vec<u16> = row.text.encode_utf16().chain(std::iter::once(0)).collect();
        TextOutW(hdc, 24, y, text.as_ptr(), (text.len() - 1) as i32);
        y += if row.bold { 22 } else { 19 };
    }

    // Buttons.
    let draw_btn = |r: (i32, i32, i32, i32), label: &str| {
        let br = CreateSolidBrush(rgb(0x2D, 0x2D, 0x2D));
        let rect = RECT {
            left: r.0,
            top: r.1,
            right: r.0 + r.2,
            bottom: r.1 + r.3,
        };
        FillRect(hdc, &rect, br);
        DeleteObject(br);
        SelectObject(hdc, d.hfont_body);
        SetTextColor(hdc, rgb(0xDD, 0xDD, 0xDD));
        let text: Vec<u16> = label.encode_utf16().chain(std::iter::once(0)).collect();
        TextOutW(
            hdc,
            r.0 + 10,
            r.1 + 7,
            text.as_ptr(),
            (text.len() - 1) as i32,
        );
    };
    draw_btn(BTN_EDIT, "\u{2699}\u{FE0F} Edit Config");
    draw_btn(BTN_GOTIT, "\u{2713} Got it!");
}
