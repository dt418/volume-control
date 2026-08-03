//! Help / Hotkeys window — a small reference card (VolumePro parity).
//!
//! GDI-painted, captionless, always-on-top tool window. Shows the current
//! hotkey map (modifier-aware), tray entries, beep guide, blacklist count and
//! a conflict note that reflects the *actual* `RegisterHotKey` outcome. Three
//! clickable buttons: "Settings", "Edit Config" and "Got it!". Hit-testing is
//! done manually on `WM_LBUTTONDOWN` against fixed rects.
//!
//! Theming uses the shared adaptive tokens (`tokens_for` +
//! `primitives::colorref`) with one resolution point in the host, mirroring
//! the overlay/mixer/settings seam: `app.rs` resolves a [`HelpAppearance`] and
//! the card consumes it blindly. The card stays opaque in every mode (no
//! material/backdrop treatment) so the refactor is behavior-preserving apart
//! from the token colors and the added Settings affordance.
//!
//! Button activation is routed through the host window (`WM_APP_HELP_*`
//! messages), so Settings and Edit Config dispatch through the central
//! `handle_action` like every other surface. "Got it!" just hides.

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{
        BeginPaint, CreateFontW, CreateSolidBrush, DeleteObject, EndPaint, FillRect,
        InvalidateRect, SelectObject, SetBkMode, SetTextColor, TextOutW, HBRUSH, HFONT,
        PAINTSTRUCT, TRANSPARENT,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, GetWindowLongPtrW, PostMessageW, RegisterClassW,
        SetWindowLongPtrW, SetWindowPos, ShowWindow, CW_USEDEFAULT, GWLP_USERDATA, HWND_TOPMOST,
        SWP_NOACTIVATE, SWP_SHOWWINDOW, SW_HIDE, WM_CLOSE, WM_ERASEBKGND, WM_LBUTTONDOWN,
        WM_PAINT, WM_USER, WNDCLASSW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
    },
};

use crate::config::Config;
use crate::hotkeys_win32::{HotkeyRegResult, HotkeyRegStatus};
use crate::ui::primitives::{colorref, work_area_for};
use crate::ui::{
    place_overlay, tokens_for, AccentMode, SurfaceSize, ThemeMode, ThemeTokens, UiCapabilities,
};

/// Custom messages the Help card posts to the host window (see `app.rs`). The
/// host owns the actions; these intents just tell it which affordance the user
/// activated.
pub const WM_APP_HELP_OPEN_CONFIG: u32 = WM_USER + 30;
pub const WM_APP_HELP_SETTINGS: u32 = WM_USER + 31;

const WIN_W: i32 = 480;
const WIN_H: i32 = 380;
const MARGIN_X: i32 = 24;
const MARGIN_Y: i32 = 48;

// Button geometry (bottom-left).
const BTN_Y: i32 = WIN_H - 44;
const BTN_H: i32 = 30;
const BTN_EDIT: (i32, i32, i32, i32) = (16, BTN_Y, 150, BTN_H);
const BTN_SETTINGS: (i32, i32, i32, i32) = (176, BTN_Y, 130, BTN_H);
const BTN_GOTIT: (i32, i32, i32, i32) = (316, BTN_Y, 148, BTN_H);

/// Adaptive appearance resolved by the host and applied by the Help card.
///
/// Mirrors the overlay/mixer/settings seam (one resolution point in `app.rs`),
/// but omits the material treatment: the reference card stays opaque in every
/// mode so the refactor is behavior-preserving apart from the token colors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HelpAppearance {
    /// Resolved palette tokens (theme + high-contrast + accent).
    pub tokens: ThemeTokens,
}

impl HelpAppearance {
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
        Self { tokens }
    }

    /// Placeholder used before the first `show` (the window is hidden then, so
    /// this is never painted).
    fn placeholder() -> Self {
        Self {
            tokens: tokens_for(ThemeMode::System, false, AccentMode::System, || None),
        }
    }
}

/// One painted text row: (text, COLORREF).
struct Row {
    text: String,
    color: u32,
    bold: bool,
}

struct HelpData {
    host: HWND,
    rows: Vec<Row>,
    hfont_body: HFONT,
    hfont_bold: HFONT,
    appearance: HelpAppearance,
    bg: HBRUSH,
    accent_brush: HBRUSH,
    btn_brush: HBRUSH,
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

/// Rebuild the token-colored brushes when the resolved appearance changed.
unsafe fn apply_appearance(d: &mut HelpData, appearance: HelpAppearance) {
    if d.appearance == appearance {
        return;
    }
    if d.bg != 0 {
        DeleteObject(d.bg);
    }
    if d.accent_brush != 0 {
        DeleteObject(d.accent_brush);
    }
    if d.btn_brush != 0 {
        DeleteObject(d.btn_brush);
    }
    d.appearance = appearance;
    d.bg = CreateSolidBrush(colorref(appearance.tokens.background));
    d.accent_brush = CreateSolidBrush(colorref(appearance.tokens.accent));
    d.btn_brush = CreateSolidBrush(colorref(appearance.tokens.surface));
}

impl Help {
    pub fn new(host: HWND) -> Result<Help, Box<dyn std::error::Error>> {
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

            let appearance = HelpAppearance::placeholder();
            let data = Box::into_raw(Box::new(HelpData {
                host,
                rows: Vec::new(),
                hfont_body: font(12, false),
                hfont_bold: font(12, true),
                appearance,
                bg: CreateSolidBrush(colorref(appearance.tokens.background)),
                accent_brush: CreateSolidBrush(colorref(appearance.tokens.accent)),
                btn_brush: CreateSolidBrush(colorref(appearance.tokens.surface)),
                open: false,
            }));
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, data as isize);

            Ok(Help { hwnd })
        }
    }

    /// Rebuild the content from the current config and the per-action hotkey
    /// registration status, then show the window themed by `appearance`.
    pub fn show(
        &mut self,
        config: &Config,
        hotkey_status: &[HotkeyRegResult],
        appearance: &HelpAppearance,
    ) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut HelpData);
            apply_appearance(d, *appearance);

            let mod_label = match config.modifier {
                crate::config::HotkeyModifier::CtrlAlt => "Ctrl+Alt",
                crate::config::HotkeyModifier::CapsLock => "CapsLock",
                crate::config::HotkeyModifier::Alt => "Alt",
                crate::config::HotkeyModifier::Ctrl => "Ctrl",
            };

            let t = &d.appearance.tokens;
            let accent = colorref(t.accent);
            let dim = colorref(t.text_secondary);
            let body = colorref(t.text_primary);
            // Success/warning notes: the VolumePro palette encodes a green and a
            // warm color, but high contrast collapses them to the primary text
            // so no information is carried by tint alone.
            let green = colorref(if t.high_contrast {
                t.text_primary
            } else {
                t.volume_threshold.low
            });
            let gold = colorref(if t.high_contrast {
                t.text_primary
            } else {
                t.volume_threshold.high
            });

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

            // The static note covers modifier-intrinsic conflicts; the dynamic
            // note overrides it when RegisterHotKey actually rejected combos
            // (the "in use by another app" case the status exposure surfaces).
            let static_note = match config.modifier {
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
            let conflicted: Vec<&HotkeyRegResult> = hotkey_status
                .iter()
                .filter(|r| matches!(r.status, HotkeyRegStatus::Conflicted(_)))
                .collect();
            let (note, note_color) = if conflicted.is_empty() {
                static_note
            } else {
                let labels: Vec<&str> = conflicted.iter().map(|r| r.action.label()).collect();
                (
                    format!(
                        "\u{26A0}\u{FE0F} In use by another app: {}",
                        labels.join(", ")
                    ),
                    gold,
                )
            };
            rows.push(Row {
                text: note,
                color: note_color,
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
                text: "Settings / Edit Config / Reload Config / Rec. Blacklist / Exit".into(),
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

            // Bottom-right of the monitor work area hosting the window (the
            // same shared placement the overlay/mixer use), so multi-monitor
            // and taskbar changes are respected.
            let work_area = work_area_for(self.hwnd);
            let rect = place_overlay(
                work_area,
                SurfaceSize::new(WIN_W, WIN_H + 40),
                MARGIN_X,
                MARGIN_Y,
            );
            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                rect.left,
                rect.top,
                rect.width(),
                rect.height(),
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
                // Routed through the host so Edit Config follows the same
                // action path as every other surface (the host's
                // OpenConfigLocation also shows the overlay toast).
                PostMessageW(d.host, WM_APP_HELP_OPEN_CONFIG, 0, 0);
                d.open = false;
                ShowWindow(hwnd, SW_HIDE);
            } else if in_rect(BTN_SETTINGS) {
                PostMessageW(d.host, WM_APP_HELP_SETTINGS, 0, 0);
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
    let t = &d.appearance.tokens;

    // Background.
    let bg_rect = RECT {
        left: 0,
        top: 0,
        right: WIN_W,
        bottom: WIN_H + 40,
    };
    FillRect(hdc, &bg_rect, d.bg);

    // Accent bar on top.
    let acc_rect = RECT {
        left: 0,
        top: 0,
        right: WIN_W,
        bottom: 3,
    };
    FillRect(hdc, &acc_rect, d.accent_brush);

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
        let rect = RECT {
            left: r.0,
            top: r.1,
            right: r.0 + r.2,
            bottom: r.1 + r.3,
        };
        FillRect(hdc, &rect, d.btn_brush);
        SelectObject(hdc, d.hfont_body);
        SetTextColor(hdc, colorref(t.text_primary));
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
    draw_btn(BTN_SETTINGS, "\u{1F527}\u{FE0F} Settings");
    draw_btn(BTN_GOTIT, "\u{2713} Got it!");
}
