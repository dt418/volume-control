//! Native Windows Settings window — a modeless, always-on-top tool window
//! built from standard controls only (no WinUI, App SDK, or second runtime).
//!
//! The window owns a [`crate::ui::SettingsDraft`] as its edit buffer and never
//! writes the config or re-registers hotkeys itself: every persistence intent
//! is posted to the host window (`WM_APP_SETTINGS_*`), and the host
//! (`app.rs::handle_action`) drives the commit, adopts the saved config,
//! re-registers hotkeys on a modifier change, and reports the result back
//! through [`Settings::on_apply_result`].
//!
//! # Signal Glass layout (spec §7)
//!
//! The 760x620 (logical) surface has a header band (title, subtitle, close
//! hit target), a navigation rail with six sections (General, Hotkeys,
//! Appearance, Blacklist, Feedback, Storage), a content pane that swaps ONE
//! section's controls at a time, and a sticky footer with the status line and
//! the Reset / Cancel / Save changes buttons. Switching sections only toggles
//! child visibility — the draft is window-scoped, so navigation never loses
//! edits.
//!
//! ## Responsive fallback (spec §7.1)
//!
//! When the available width falls below the 760px desktop size (down to the
//! 620px minimum), the vertical rail becomes a horizontal stacked section
//! selector strip under the header; the content pane still shows one section
//! at a time and the footer stays pinned. This is the simplest robust
//! approach: no scrolling exists, so no control can be clipped at any width,
//! and the Tab cycle (rail → active section → footer → close) is identical in
//! both layouts. The layout mode is decided from the actual window width on
//! `WM_SIZE` (the window is sized to the work area on `show`, so a cramped
//! monitor automatically gets the stacked selector).
//!
//! ## DPI
//!
//! The window follows the mixer's `DpiMetrics` path (Task 6): the window is
//! sized at `dpi.to_physical(WIN_W/H)`, every child is positioned at physical
//! coordinates scaled exactly once, static fonts are created at scaled
//! heights, and the paint canvas converts its logical rects the same way — so
//! the surface scales uniformly at 100/125/150% with no double scaling.
//!
//! ## Inline validation
//!
//! On a failed Apply the offending field shows an inline error (error-token
//! text in the row's helper slot) in addition to the bottom status line, and
//! the window switches to the section containing the field. Editing the
//! offending field clears its stale inline error; the draft keeps the edits
//! and the next Apply revalidates.
//!
//! ## Appearance preview
//!
//! The Appearance section has a small custom-painted card (mini Signal Rail)
//! resolved from the CURRENT DRAFT appearance (theme/material/motion/accent) —
//! never the host's confirmed config — so the preview tracks the draft
//! without mutating host state; only Apply persists.

use std::collections::HashMap;

use windows_sys::core::w;
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{
        CreateFontW, CreateSolidBrush, DeleteObject, InvalidateRect, RedrawWindow, SetBkMode,
        SetTextColor, HBRUSH, HDC, HFONT, RDW_ALLCHILDREN, RDW_ERASE, RDW_INVALIDATE,
        RDW_UPDATENOW, TRANSPARENT,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::Controls::{DRAWITEMSTRUCT, WM_MOUSELEAVE},
    UI::Input::KeyboardAndMouse::{
        EnableWindow, GetFocus, GetKeyState, SetFocus, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
        VK_DOWN, VK_ESCAPE, VK_LEFT, VK_RETURN, VK_RIGHT, VK_SHIFT, VK_TAB, VK_UP,
    },
    UI::WindowsAndMessaging::{
        CallWindowProcW, CreateWindowExW, DefWindowProcW, DestroyWindow, GetAncestor,
        GetClientRect, GetDlgCtrlID, GetWindowLongPtrW, KillTimer, PostMessageW, RegisterClassW,
        SendMessageW, SetTimer, SetWindowLongPtrW, SetWindowPos, ShowWindow, BN_CLICKED,
        BS_OWNERDRAW, CW_USEDEFAULT, GA_PARENT, GWLP_USERDATA, GWLP_WNDPROC, HWND_TOPMOST,
        SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_HIDE, SW_SHOW, WM_CLOSE, WM_COMMAND,
        WM_CTLCOLORSTATIC, WM_DRAWITEM, WM_ERASEBKGND, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MOUSEMOVE,
        WM_PAINT, WM_SIZE, WM_SYSKEYDOWN, WM_TIMER, WM_USER, WNDCLASSW, WS_CHILD, WS_EX_TOOLWINDOW,
        WS_EX_TOPMOST, WS_POPUP, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
    },
};

#[cfg(test)]
use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowRect, GWL_STYLE};

use crate::config::{AppearanceConfig, Config, ConfigError, HotkeyModifier};
use crate::ui::platform::windows::primitives::{
    apply_backdrop, colorref, dpi_scale_for, paint_close_button, theme_controls, work_area_for,
    DpiMetrics, PaintCanvas, PointF, RectF,
};
use crate::ui::platform::windows::text::{TextAlign, TextLayout};
use crate::ui::{
    place_centered, rail_geometry, resolve_material, tokens_for, AccentMode, MarkerGeometry,
    MaterialMode, MotionMode, ResolvedMaterial, SettingsDraft, SignalRail, SignalRailGeometry,
    SurfaceSize, ThemeMode, ThemeTokens, TrackRect, UiCapabilities,
};

/// Custom messages the settings window posts to the host window (see `app.rs`).
/// The host owns all config/hotkey mutation; these intents just tell it which
/// draft action the user requested.
pub const WM_APP_SETTINGS_APPLY: u32 = WM_USER + 20;
pub const WM_APP_SETTINGS_CANCEL: u32 = WM_USER + 21;
pub const WM_APP_SETTINGS_RESET: u32 = WM_USER + 22;
pub const WM_APP_SETTINGS_OPEN_CONFIG: u32 = WM_USER + 23;

// ── Surface geometry (logical px; the mixer's DpiMetrics path scales these) ─
/// Desktop logical width (spec §7.1).
const WIN_W: i32 = 760;
/// Desktop logical height (spec §7.1).
const WIN_H: i32 = 620;
/// Minimum logical size (spec §7.1).
const MIN_W: i32 = 620;
const MIN_H: i32 = 520;

/// Header band height (title + subtitle + close).
const HEADER_H: f32 = 64.0;
/// Sticky footer band height (status + buttons).
const FOOTER_H: f32 = 56.0;
/// Desktop navigation rail width.
const RAIL_W: f32 = 200.0;
/// Rail entries: 36px tall, 4px gap, starting below the header.
const RAIL_Y: f32 = 72.0;
const RAIL_ENTRY_H: f32 = 36.0;
const RAIL_GAP: f32 = 4.0;
/// Narrow-mode stacked selector strip: 32px tall under the header.
const STRIP_Y: f32 = 72.0;
const STRIP_H: f32 = 32.0;
const STRIP_GAP: f32 = 8.0;

/// Appearance preview card size (mini Signal Rail).
const PREVIEW_W: f32 = 120.0;
const PREVIEW_H: f32 = 36.0;
/// Fixed preview percent the mini rail shows.
const PREVIEW_PERCENT: u8 = 60;
/// Preview thumb radius (small card scale).
const PREVIEW_THUMB_RADIUS: f32 = 4.0;

// Control-style bitmasks (canonical Win32 values; the windows-sys constants are
// typed i32, so local u32 copies keep the WS_CHILD | ... expressions uniform).
const ES_AUTOHSCROLL: u32 = 0x0080;
const ES_NUMBER: u32 = 0x2000;
const CBS_DROPDOWNLIST: u32 = 0x0003;
const LBS_NOTIFY: u32 = 0x0001;
const LBS_NOINTEGRALHEIGHT: u32 = 0x0100;
const BS_AUTOCHECKBOX: u32 = 0x0003;
/// SS_ENDELLIPSIS — the config path truncates with an ellipsis instead of
/// clipping mid-glyph.
const SS_ENDELLIPSIS: u32 = 0x4000;
/// WS_DISABLED — the Save changes button's disabled state (style bit; used
/// by the window-level tests).
#[cfg(test)]
const WS_DISABLED: u32 = 0x0800_0000;

// Message ids for standard controls (canonical Win32 values).
const WM_GETTEXT: u32 = 0x000D;
const WM_GETTEXTLENGTH: u32 = 0x000E;
const WM_SETTEXT: u32 = 0x000C;
const WM_SETFONT: u32 = 0x0030;
const BM_GETCHECK: u32 = 0x00F0;
const BM_SETCHECK: u32 = 0x00F1;
const CB_ADDSTRING: u32 = 0x0143;
const CB_GETCURSEL: u32 = 0x0147;
const CB_SETCURSEL: u32 = 0x014E;
const LB_ADDSTRING: u32 = 0x0180;
const LB_GETTEXT: u32 = 0x0189;
const LB_GETTEXTLEN: u32 = 0x018A;
const LB_GETCOUNT: u32 = 0x018B;
const LB_GETCURSEL: u32 = 0x0188;
const LB_RESETCONTENT: u32 = 0x0184;
/// EN_CHANGE — sent by EDIT controls on every text change (including the
/// programmatic WM_SETTEXT used to populate controls).
const EN_CHANGE: u32 = 0x0300;
/// CBN_SELCHANGE — sent by dropdown list combos on selection change.
const CBN_SELCHANGE: u32 = 0x0001;

// ── Interactive control ids (read on Apply / clicked) ─────────────────────
const ID_VOL_STEP: isize = 100;
const ID_VOL_STEP_LARGE: isize = 101;
const ID_OVERLAY_MS: isize = 102;
const ID_COMBO_MODIFIER: isize = 103;
const ID_COMBO_THEME: isize = 104;
const ID_COMBO_MATERIAL: isize = 105;
const ID_COMBO_MOTION: isize = 106;
const ID_COMBO_ACCENT: isize = 107;
const ID_LIST_BLACKLIST: isize = 108;
const ID_EDIT_BLACKLIST: isize = 109;
const ID_BTN_BLACKLIST_ADD: isize = 110;
const ID_BTN_BLACKLIST_REMOVE: isize = 111;
const ID_BTN_BLACKLIST_CLEAR: isize = 112;
const ID_BTN_BLACKLIST_RECOMMEND: isize = 113;
const ID_CHK_BEEP: isize = 114;
const ID_BLOCKED_FREQ: isize = 115;
const ID_BLOCKED_DUR: isize = 116;
const ID_LIMIT_FREQ: isize = 117;
const ID_LIMIT_DUR: isize = 118;
const ID_BTN_OPEN_CONFIG: isize = 119;
const ID_BTN_APPLY: isize = 120;
const ID_BTN_RESET: isize = 121;
const ID_BTN_CANCEL: isize = 122;
const ID_BTN_CLOSE: isize = 123;

/// One-shot timer ID for the deferred DWM backdrop re-apply after a show
/// (see the comment in [`Settings::show`]).
const BACKDROP_TIMER_ID: usize = 1;
/// Delay (ms) between showing the window and re-asserting the resolved
/// backdrop: DWM applies its High-Contrast backdrop override asynchronously
/// after the show — measured on Windows 11 24H2, backdrop writes are
/// clobbered back to AUTO for roughly the first second after the show, and
/// stick once the composition settles. 2000ms lands safely past that window.
const BACKDROP_REAPPLY_MS: u32 = 2000;

// ── Static control ids (coloring + font dispatch) ─────────────────────────
const ID_HDR_GENERAL: isize = 200;
const ID_HDR_HOTKEYS: isize = 201;
const ID_HDR_APPEARANCE: isize = 202;
const ID_HDR_BLACKLIST: isize = 203;
const ID_HDR_FEEDBACK: isize = 204;
const ID_HDR_STORAGE: isize = 205;
const ID_SUB_GENERAL: isize = 206;
const ID_SUB_HOTKEYS: isize = 207;
const ID_SUB_APPEARANCE: isize = 208;
const ID_SUB_BLACKLIST: isize = 209;
const ID_SUB_FEEDBACK: isize = 210;
const ID_SUB_STORAGE: isize = 211;
const ID_LBL_VOL_STEP: isize = 300;
const ID_LBL_VOL_STEP_LARGE: isize = 301;
const ID_LBL_OVERLAY: isize = 302;
const ID_LBL_MODIFIER: isize = 303;
const ID_ST_HOTKEY_STATUS: isize = 304;
const ID_LBL_THEME: isize = 305;
const ID_LBL_MATERIAL: isize = 306;
const ID_LBL_MOTION: isize = 307;
const ID_LBL_ACCENT: isize = 308;
const ID_LBL_BLOCKED_FREQ: isize = 309;
const ID_LBL_BLOCKED_DUR: isize = 310;
const ID_LBL_LIMIT_FREQ: isize = 311;
const ID_LBL_LIMIT_DUR: isize = 312;
const ID_LBL_CONFIG: isize = 313;
const ID_ST_STATUS: isize = 314;
const ID_ST_CONFIG_PATH: isize = 315;
const ID_HELP_VOL_STEP: isize = 316;
const ID_HELP_VOL_STEP_LARGE: isize = 317;
const ID_HELP_OVERLAY: isize = 318;
const ID_HELP_BLOCKED_FREQ: isize = 319;
const ID_HELP_BLOCKED_DUR: isize = 320;
const ID_HELP_LIMIT_FREQ: isize = 321;
const ID_HELP_LIMIT_DUR: isize = 322;
const ID_ST_PREVIEW_CAPTION: isize = 323;
const ID_ST_BLACKLIST_EMPTY_1: isize = 324;
const ID_ST_BLACKLIST_EMPTY_2: isize = 325;
const ID_ST_STORAGE_STATUS: isize = 326;
/// Invisible (off-client) UIA mirror of the footer status line: its window
/// text is `Status: …` / `Alert: …` (spec §11.2).
const ID_ST_STATUS_UIA: isize = 327;
/// Inline validation errors, one per validatable field (see
/// [`INLINE_ERROR_FIELDS`] for the field order).
const ID_ERR_VOL_STEP: isize = 400;
const ID_ERR_VOL_STEP_LARGE: isize = 401;
const ID_ERR_OVERLAY: isize = 402;
const ID_ERR_BLOCKED_FREQ: isize = 403;
const ID_ERR_BLOCKED_DUR: isize = 404;
const ID_ERR_LIMIT_FREQ: isize = 405;
const ID_ERR_LIMIT_DUR: isize = 406;

/// Config field names with inline validation, in [`SettingsData::error_statics`]
/// order (matches `crate::config::validate`).
const INLINE_ERROR_FIELDS: [&str; 7] = [
    "volume_step",
    "volume_step_large",
    "overlay_duration_ms",
    "beep.blocked_freq",
    "beep.blocked_duration_ms",
    "beep.limit_freq",
    "beep.limit_duration_ms",
];

/// The navigation sections (spec §7.2), in rail order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    General,
    Hotkeys,
    Appearance,
    Blacklist,
    Feedback,
    Storage,
}

impl Section {
    const ALL: [Section; 6] = [
        Section::General,
        Section::Hotkeys,
        Section::Appearance,
        Section::Blacklist,
        Section::Feedback,
        Section::Storage,
    ];

    fn index(self) -> usize {
        match self {
            Section::General => 0,
            Section::Hotkeys => 1,
            Section::Appearance => 2,
            Section::Blacklist => 3,
            Section::Feedback => 4,
            Section::Storage => 5,
        }
    }

    fn from_index(index: usize) -> Section {
        Section::ALL[index % Section::ALL.len()]
    }

    fn previous(self) -> Section {
        Section::from_index(self.index() + Section::ALL.len() - 1)
    }

    fn next(self) -> Section {
        Section::from_index(self.index() + 1)
    }

    fn title(self) -> &'static str {
        match self {
            Section::General => "General",
            Section::Hotkeys => "Hotkeys",
            Section::Appearance => "Appearance",
            Section::Blacklist => "Blacklist",
            Section::Feedback => "Feedback",
            Section::Storage => "Storage",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Section::General => "Adjust how volume changes feel.",
            Section::Hotkeys => "The modifier for VolumeControl's custom shortcuts.",
            Section::Appearance => "Theme, material, motion, and accent.",
            Section::Blacklist => "Block shortcuts while these apps have focus.",
            Section::Feedback => "Beep feedback for blocked and limit actions.",
            Section::Storage => "Where VolumeControl keeps its configuration.",
        }
    }
}

/// Role of the status line shown in the footer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusKind {
    None,
    Info,
    Error,
}

/// The window-proc signature of subclassed interactive controls.
type ChildWndProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

/// Adaptive appearance resolved by the host and applied by the window.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SettingsAppearance {
    /// Resolved palette tokens (theme + high-contrast + accent).
    pub tokens: ThemeTokens,
    /// Capability-resolved material treatment (blur/translucent/opaque).
    pub material: ResolvedMaterial,
}

impl SettingsAppearance {
    /// Resolve the adaptive appearance from `config.appearance` against `caps`.
    /// Mirrors the mixer's single-resolution-point seam in `app.rs`.
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

/// Parsed working values from the window's controls (see [`read_controls`]).
///
/// Kept separate from the HWND reading so the mapping into a [`Config`] is pure
/// and unit-testable without a live window.
#[derive(Debug, Clone, PartialEq)]
struct ControlValues {
    volume_step: u32,
    volume_step_large: u32,
    overlay_duration_ms: u64,
    modifier: HotkeyModifier,
    theme: ThemeMode,
    material: MaterialMode,
    motion: MotionMode,
    accent: AccentMode,
    beep_enabled: bool,
    blocked_freq: u32,
    blocked_duration_ms: u32,
    limit_freq: u32,
    limit_duration_ms: u32,
    blacklist: Vec<String>,
}

// ── Pure layout (logical px) ───────────────────────────────────────────────

/// One label/field/helper row shared by the General, Appearance, and Feedback
/// sections.
#[derive(Debug, Clone, Copy, PartialEq)]
struct RowLayout {
    label: RectF,
    field: RectF,
    helper: RectF,
}

impl RowLayout {
    /// A row at `r` inside a `cw`-wide content pane starting at `cx`; the
    /// field is `field_w` wide (120 for edits, 200 for combos).
    fn new(cx: f32, r: f32, cw: f32, field_w: f32) -> Self {
        let helper_x = cx + 116.0 + field_w + 12.0;
        Self {
            label: RectF::new(cx, r + 2.0, cx + 104.0, r + 20.0),
            field: RectF::new(cx + 108.0, r + 1.0, cx + 108.0 + field_w, r + 25.0),
            helper: RectF::new(helper_x, r + 6.0, cx + cw, r + 22.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct GeneralLayout {
    rows: [RowLayout; 3],
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct HotkeysLayout {
    label: RectF,
    combo: RectF,
    status: RectF,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct AppearanceLayout {
    rows: [RowLayout; 4],
    preview_caption: RectF,
    preview: RectF,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct BlacklistLayout {
    add_edit: RectF,
    add_btn: RectF,
    list: RectF,
    empty_title: RectF,
    empty_caption: RectF,
    remove: RectF,
    clear: RectF,
    recommend: RectF,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct FeedbackLayout {
    chk: RectF,
    rows: [RowLayout; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct StorageLayout {
    label: RectF,
    path: RectF,
    open: RectF,
    status: RectF,
}

/// Logical layout of one settings frame (all values in logical px, spec §7).
///
/// The layout is the single pure decision point for the responsive behavior:
/// `narrow` is derived from the width, every child control gets a rect, and
/// the navigation is either a vertical rail (desktop) or a horizontal stacked
/// selector strip (narrow). Tests assert that every child rect lands inside
/// the window at both the desktop and the minimum size.
#[derive(Debug, Clone, Copy, PartialEq)]
struct SettingsLayout {
    width: f32,
    height: f32,
    /// True when the surface is too narrow for the desktop rail
    /// (`width < WIN_W`); the rail becomes a stacked selector strip.
    narrow: bool,
    /// Painted header title/subtitle boxes + native close hit target.
    title_rect: RectF,
    subtitle_rect: RectF,
    close_rect: RectF,
    /// Top of the sticky footer band.
    footer_top: f32,
    /// Status line inside the footer (left).
    status_rect: RectF,
    /// Footer buttons, left→right (also the tab order): Reset, Cancel, Save.
    btn_reset: RectF,
    btn_cancel: RectF,
    btn_apply: RectF,
    /// Navigation hit targets (rail entries or strip entries).
    nav_entries: [RectF; 6],
    /// Shared section title/subtitle slots (each section's statics use the
    /// same rects; only one section is visible at a time).
    section_title: RectF,
    section_subtitle: RectF,
    general: GeneralLayout,
    hotkeys: HotkeysLayout,
    appearance: AppearanceLayout,
    blacklist: BlacklistLayout,
    feedback: FeedbackLayout,
    storage: StorageLayout,
}

impl SettingsLayout {
    fn new(width: f32, height: f32) -> Self {
        let narrow = width < WIN_W as f32;
        let footer_top = height - FOOTER_H;
        let content_x = if narrow { 16.0 } else { RAIL_W + 24.0 };
        let content_y = if narrow { 170.0 } else { 128.0 };
        let cw = width - content_x - 24.0;

        let mut nav_entries = [RectF::new(0.0, 0.0, 0.0, 0.0); 6];
        if narrow {
            let strip_w = (width - 32.0 - 5.0 * STRIP_GAP) / 6.0;
            for (i, entry) in nav_entries.iter_mut().enumerate() {
                let x = 16.0 + i as f32 * (strip_w + STRIP_GAP);
                *entry = RectF::new(x, STRIP_Y, x + strip_w, STRIP_Y + STRIP_H);
            }
        } else {
            for (i, entry) in nav_entries.iter_mut().enumerate() {
                let y = RAIL_Y + i as f32 * (RAIL_ENTRY_H + RAIL_GAP);
                *entry = RectF::new(8.0, y, 192.0, y + RAIL_ENTRY_H);
            }
        }

        let section_title = RectF::new(
            content_x,
            content_y - 52.0,
            content_x + cw,
            content_y - 26.0,
        );
        let section_subtitle = RectF::new(
            content_x,
            content_y - 26.0,
            content_x + cw,
            content_y - 10.0,
        );

        let general = GeneralLayout {
            rows: [
                RowLayout::new(content_x, content_y, cw, 120.0),
                RowLayout::new(content_x, content_y + 44.0, cw, 120.0),
                RowLayout::new(content_x, content_y + 88.0, cw, 120.0),
            ],
        };
        let hotkeys = HotkeysLayout {
            label: RectF::new(
                content_x,
                content_y + 2.0,
                content_x + 104.0,
                content_y + 20.0,
            ),
            combo: RectF::new(
                content_x + 108.0,
                content_y + 1.0,
                content_x + 308.0,
                content_y + 25.0,
            ),
            status: RectF::new(
                content_x + 108.0,
                content_y + 30.0,
                content_x + cw,
                content_y + 48.0,
            ),
        };
        let appearance = AppearanceLayout {
            rows: [
                RowLayout::new(content_x, content_y, cw, 200.0),
                RowLayout::new(content_x, content_y + 44.0, cw, 200.0),
                RowLayout::new(content_x, content_y + 88.0, cw, 200.0),
                RowLayout::new(content_x, content_y + 132.0, cw, 200.0),
            ],
            preview_caption: RectF::new(
                content_x,
                content_y + 144.0,
                content_x + 160.0,
                content_y + 160.0,
            ),
            preview: RectF::new(
                content_x,
                content_y + 162.0,
                content_x + PREVIEW_W,
                content_y + 162.0 + PREVIEW_H,
            ),
        };
        let blacklist = BlacklistLayout {
            add_edit: RectF::new(
                content_x,
                content_y,
                content_x + cw - 116.0,
                content_y + 24.0,
            ),
            add_btn: RectF::new(
                content_x + cw - 104.0,
                content_y - 1.0,
                content_x + cw,
                content_y + 27.0,
            ),
            list: RectF::new(
                content_x,
                content_y + 36.0,
                content_x + cw,
                content_y + 176.0,
            ),
            empty_title: RectF::new(
                content_x + 8.0,
                content_y + 50.0,
                content_x + cw - 16.0,
                content_y + 70.0,
            ),
            empty_caption: RectF::new(
                content_x + 8.0,
                content_y + 72.0,
                content_x + cw - 16.0,
                content_y + 88.0,
            ),
            remove: RectF::new(
                content_x,
                content_y + 188.0,
                content_x + 100.0,
                content_y + 216.0,
            ),
            clear: RectF::new(
                content_x + 108.0,
                content_y + 188.0,
                content_x + 208.0,
                content_y + 216.0,
            ),
            recommend: RectF::new(
                content_x + 216.0,
                content_y + 188.0,
                content_x + 386.0,
                content_y + 216.0,
            ),
        };
        let feedback = FeedbackLayout {
            chk: RectF::new(content_x, content_y, content_x + 220.0, content_y + 26.0),
            rows: [
                RowLayout::new(content_x, content_y + 38.0, cw, 120.0),
                RowLayout::new(content_x, content_y + 90.0, cw, 120.0),
                RowLayout::new(content_x, content_y + 142.0, cw, 120.0),
                RowLayout::new(content_x, content_y + 194.0, cw, 120.0),
            ],
        };
        let storage = StorageLayout {
            label: RectF::new(
                content_x,
                content_y + 1.0,
                content_x + 100.0,
                content_y + 19.0,
            ),
            path: RectF::new(
                content_x + 104.0,
                content_y,
                content_x + cw,
                content_y + 26.0,
            ),
            open: RectF::new(
                content_x,
                content_y + 38.0,
                content_x + 140.0,
                content_y + 66.0,
            ),
            status: RectF::new(
                content_x,
                content_y + 78.0,
                content_x + cw,
                content_y + 94.0,
            ),
        };

        let btn_y = footer_top + 12.0;
        let btn_h = 32.0;
        let btn_apply = RectF::new(width - 132.0, btn_y, width - 24.0, btn_y + btn_h);
        let btn_cancel = RectF::new(width - 232.0, btn_y, width - 144.0, btn_y + btn_h);
        let btn_reset = RectF::new(width - 332.0, btn_y, width - 244.0, btn_y + btn_h);

        Self {
            width,
            height,
            narrow,
            title_rect: RectF::new(24.0, 10.0, 520.0, 34.0),
            subtitle_rect: RectF::new(24.0, 36.0, 560.0, 52.0),
            close_rect: RectF::new(width - 56.0, 16.0, width - 24.0, 48.0),
            footer_top,
            status_rect: RectF::new(24.0, footer_top + 13.0, width - 344.0, footer_top + 33.0),
            btn_reset,
            btn_cancel,
            btn_apply,
            nav_entries,
            section_title,
            section_subtitle,
            general,
            hotkeys,
            appearance,
            blacklist,
            feedback,
            storage,
        }
    }
}

/// The layout rect for every positioned child control, keyed by control id.
///
/// This table is the pure geometry contract: tests assert that every entry
/// lands inside the window at both the desktop and the minimum size, which
/// proves nothing can be clipped at any width in either layout mode.
fn child_rects(layout: &SettingsLayout) -> Vec<(isize, RectF)> {
    let mut out = Vec::with_capacity(60);
    // Section titles and subtitles share the single content-pane slots.
    for id in ID_HDR_GENERAL..=ID_HDR_STORAGE {
        out.push((id, layout.section_title));
    }
    for id in ID_SUB_GENERAL..=ID_SUB_STORAGE {
        out.push((id, layout.section_subtitle));
    }
    // General labels / helpers / inline errors.
    out.push((ID_LBL_VOL_STEP, layout.general.rows[0].label));
    out.push((ID_LBL_VOL_STEP_LARGE, layout.general.rows[1].label));
    out.push((ID_LBL_OVERLAY, layout.general.rows[2].label));
    out.push((ID_HELP_VOL_STEP, layout.general.rows[0].helper));
    out.push((ID_HELP_VOL_STEP_LARGE, layout.general.rows[1].helper));
    out.push((ID_HELP_OVERLAY, layout.general.rows[2].helper));
    out.push((ID_ERR_VOL_STEP, layout.general.rows[0].helper));
    out.push((ID_ERR_VOL_STEP_LARGE, layout.general.rows[1].helper));
    out.push((ID_ERR_OVERLAY, layout.general.rows[2].helper));
    // Hotkeys.
    out.push((ID_LBL_MODIFIER, layout.hotkeys.label));
    out.push((ID_ST_HOTKEY_STATUS, layout.hotkeys.status));
    // Appearance labels + preview caption.
    out.push((ID_LBL_THEME, layout.appearance.rows[0].label));
    out.push((ID_LBL_MATERIAL, layout.appearance.rows[1].label));
    out.push((ID_LBL_MOTION, layout.appearance.rows[2].label));
    out.push((ID_LBL_ACCENT, layout.appearance.rows[3].label));
    out.push((ID_ST_PREVIEW_CAPTION, layout.appearance.preview_caption));
    // Blacklist empty state.
    out.push((ID_ST_BLACKLIST_EMPTY_1, layout.blacklist.empty_title));
    out.push((ID_ST_BLACKLIST_EMPTY_2, layout.blacklist.empty_caption));
    // Feedback labels / helpers / inline errors.
    out.push((ID_LBL_BLOCKED_FREQ, layout.feedback.rows[0].label));
    out.push((ID_LBL_BLOCKED_DUR, layout.feedback.rows[1].label));
    out.push((ID_LBL_LIMIT_FREQ, layout.feedback.rows[2].label));
    out.push((ID_LBL_LIMIT_DUR, layout.feedback.rows[3].label));
    out.push((ID_HELP_BLOCKED_FREQ, layout.feedback.rows[0].helper));
    out.push((ID_HELP_BLOCKED_DUR, layout.feedback.rows[1].helper));
    out.push((ID_HELP_LIMIT_FREQ, layout.feedback.rows[2].helper));
    out.push((ID_HELP_LIMIT_DUR, layout.feedback.rows[3].helper));
    out.push((ID_ERR_BLOCKED_FREQ, layout.feedback.rows[0].helper));
    out.push((ID_ERR_BLOCKED_DUR, layout.feedback.rows[1].helper));
    out.push((ID_ERR_LIMIT_FREQ, layout.feedback.rows[2].helper));
    out.push((ID_ERR_LIMIT_DUR, layout.feedback.rows[3].helper));
    // Storage.
    out.push((ID_LBL_CONFIG, layout.storage.label));
    out.push((ID_ST_CONFIG_PATH, layout.storage.path));
    out.push((ID_ST_STORAGE_STATUS, layout.storage.status));
    // Footer status line.
    out.push((ID_ST_STATUS, layout.status_rect));
    // General controls.
    out.push((ID_VOL_STEP, layout.general.rows[0].field));
    out.push((ID_VOL_STEP_LARGE, layout.general.rows[1].field));
    out.push((ID_OVERLAY_MS, layout.general.rows[2].field));
    // Hotkeys control.
    out.push((ID_COMBO_MODIFIER, layout.hotkeys.combo));
    // Appearance controls.
    out.push((ID_COMBO_THEME, layout.appearance.rows[0].field));
    out.push((ID_COMBO_MATERIAL, layout.appearance.rows[1].field));
    out.push((ID_COMBO_MOTION, layout.appearance.rows[2].field));
    out.push((ID_COMBO_ACCENT, layout.appearance.rows[3].field));
    // Blacklist controls.
    out.push((ID_EDIT_BLACKLIST, layout.blacklist.add_edit));
    out.push((ID_BTN_BLACKLIST_ADD, layout.blacklist.add_btn));
    out.push((ID_LIST_BLACKLIST, layout.blacklist.list));
    out.push((ID_BTN_BLACKLIST_REMOVE, layout.blacklist.remove));
    out.push((ID_BTN_BLACKLIST_CLEAR, layout.blacklist.clear));
    out.push((ID_BTN_BLACKLIST_RECOMMEND, layout.blacklist.recommend));
    // Feedback controls.
    out.push((ID_CHK_BEEP, layout.feedback.chk));
    out.push((ID_BLOCKED_FREQ, layout.feedback.rows[0].field));
    out.push((ID_BLOCKED_DUR, layout.feedback.rows[1].field));
    out.push((ID_LIMIT_FREQ, layout.feedback.rows[2].field));
    out.push((ID_LIMIT_DUR, layout.feedback.rows[3].field));
    // Storage + footer actions.
    out.push((ID_BTN_OPEN_CONFIG, layout.storage.open));
    out.push((ID_BTN_APPLY, layout.btn_apply));
    out.push((ID_BTN_RESET, layout.btn_reset));
    out.push((ID_BTN_CANCEL, layout.btn_cancel));
    out.push((ID_BTN_CLOSE, layout.close_rect));
    out
}

/// Per-window state stored in GWLP_USERDATA.
struct SettingsData {
    hwnd: HWND,
    host: HWND,
    draft: SettingsDraft,
    // General
    edit_volume_step: HWND,
    edit_volume_step_large: HWND,
    edit_overlay_ms: HWND,
    // Hotkeys
    combo_modifier: HWND,
    static_hotkey_status: HWND,
    // Appearance
    combo_theme: HWND,
    combo_material: HWND,
    combo_motion: HWND,
    combo_accent: HWND,
    /// Custom-painted mini Signal Rail preview (draft-driven).
    preview: HWND,
    // Blacklist
    list_blacklist: HWND,
    edit_blacklist_new: HWND,
    btn_blacklist_add: HWND,
    btn_blacklist_remove: HWND,
    btn_blacklist_clear: HWND,
    btn_blacklist_recommend: HWND,
    /// "No blocked applications" empty-state statics (over the empty list).
    static_blacklist_empty: [HWND; 2],
    // Feedback
    chk_beep: HWND,
    edit_blocked_freq: HWND,
    edit_blocked_dur: HWND,
    edit_limit_freq: HWND,
    edit_limit_dur: HWND,
    // Storage / actions
    btn_open_config: HWND,
    static_path: HWND,
    static_status: HWND,
    /// Screen-reader mirror of the status line (spec §11.2): a native STATIC
    /// whose window text is `Status: …` / `Alert: …`, parked OUTSIDE the
    /// client area so it is invisible but stays in the UIA tree. The visible
    /// status line (`static_status`) keeps painting the plain text.
    static_status_uia: HWND,
    btn_apply: HWND,
    btn_reset: HWND,
    btn_cancel: HWND,
    btn_close: HWND,
    /// Pointer hover over the owner-drawn close button (see
    /// [`paint_close_button`]).
    close_hover: bool,
    /// Every static (id, hwnd), used for layout positioning and painting.
    static_handles: Vec<(isize, HWND)>,
    /// Inline error statics in [`INLINE_ERROR_FIELDS`] order.
    error_statics: [HWND; 7],
    /// All children per section (statics + interactive), for the
    /// one-section-at-a-time visibility swap.
    section_all: Vec<Vec<HWND>>,
    /// Interactive controls per section, in tab order within the section.
    section_tabs: Vec<Vec<HWND>>,
    // Styling
    appearance: SettingsAppearance,
    bg: HBRUSH,
    accent_brush: HBRUSH,
    hfont_title: HFONT,
    hfont_section: HFONT,
    hfont_label: HFONT,
    hfont_body: HFONT,
    hfont_caption: HFONT,
    status_kind: StatusKind,
    /// Currently selected navigation section.
    section: Section,
    /// DPI scale the window and children were last laid out at.
    dpi: f32,
    /// Logical client size (physical / dpi), last set by WM_SIZE.
    logical_w: i32,
    logical_h: i32,
    open: bool,
    /// Every interactive control in visual (tab) order (parallel to
    /// `orig_procs`), used for subclass dispatch + dark theming.
    tab_order: Vec<HWND>,
    /// Original window procs of the subclassed controls, so the subclass can
    /// forward everything it does not handle.
    orig_procs: Vec<Option<ChildWndProc>>,
}

impl SettingsData {
    fn placeholder(host: HWND) -> Self {
        Self {
            hwnd: 0,
            host,
            draft: SettingsDraft::new(Config::default()),
            edit_volume_step: 0,
            edit_volume_step_large: 0,
            edit_overlay_ms: 0,
            combo_modifier: 0,
            static_hotkey_status: 0,
            combo_theme: 0,
            combo_material: 0,
            combo_motion: 0,
            combo_accent: 0,
            preview: 0,
            list_blacklist: 0,
            edit_blacklist_new: 0,
            btn_blacklist_add: 0,
            btn_blacklist_remove: 0,
            btn_blacklist_clear: 0,
            btn_blacklist_recommend: 0,
            static_blacklist_empty: [0; 2],
            chk_beep: 0,
            edit_blocked_freq: 0,
            edit_blocked_dur: 0,
            edit_limit_freq: 0,
            edit_limit_dur: 0,
            btn_open_config: 0,
            static_path: 0,
            static_status: 0,
            static_status_uia: 0,
            btn_apply: 0,
            btn_reset: 0,
            btn_cancel: 0,
            btn_close: 0,
            close_hover: false,
            static_handles: Vec::new(),
            error_statics: [0; 7],
            section_all: Vec::new(),
            section_tabs: Vec::new(),
            appearance: SettingsAppearance::placeholder(),
            bg: 0,
            accent_brush: 0,
            hfont_title: 0,
            hfont_section: 0,
            hfont_label: 0,
            hfont_body: 0,
            hfont_caption: 0,
            status_kind: StatusKind::None,
            section: Section::General,
            dpi: 1.0,
            logical_w: WIN_W,
            logical_h: WIN_H,
            open: false,
            tab_order: Vec::new(),
            orig_procs: Vec::new(),
        }
    }

    /// True when the surface uses the stacked selector strip layout.
    fn narrow(&self) -> bool {
        self.logical_w < WIN_W
    }
}

pub struct Settings {
    hwnd: HWND,
}

/// Create a Segoe UI font at `height` pixels (already DPI-scaled by the
/// caller).
unsafe fn font(height: i32, bold: bool) -> HFONT {
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
        w!("Segoe UI"),
    )
}

impl Settings {
    /// Create the hidden settings window (shown via `show`).
    pub fn new(host: HWND) -> Result<Settings, Box<dyn std::error::Error>> {
        unsafe {
            let hinst = GetModuleHandleW(std::ptr::null());
            let class = w!("VolCtlSettings");
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.lpfnWndProc = Some(settings_wndproc);
            wc.hInstance = hinst;
            wc.lpszClassName = class;
            RegisterClassW(&wc);

            let preview_class = w!("VolCtlSettingsPreview");
            let mut pwc: WNDCLASSW = std::mem::zeroed();
            pwc.lpfnWndProc = Some(preview_wndproc);
            pwc.hInstance = hinst;
            pwc.lpszClassName = preview_class;
            RegisterClassW(&pwc);

            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                class,
                w!("VolumeControl Settings"),
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
                return Err("settings CreateWindowEx failed".into());
            }

            let data = Box::into_raw(Box::new(SettingsData::placeholder(host)));
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, data as isize);
            let d = &mut *data;
            d.hwnd = hwnd;

            // Typography per role (spec §4.3), scaled by the system DPI exactly
            // once like the mixer's canvas text.
            let scale = dpi_scale_for(hwnd);
            let scaled = |logical: i32| (logical as f32 * scale).round() as i32;
            d.hfont_title = font(scaled(17), true);
            d.hfont_section = font(scaled(15), true);
            d.hfont_label = font(scaled(12), true);
            d.hfont_body = font(scaled(13), false);
            d.hfont_caption = font(scaled(11), false);

            // Creation coordinates come from the desktop layout; the first
            // `show` repositions every child for the actual size/DPI.
            let layout = SettingsLayout::new(WIN_W as f32, WIN_H as f32);
            let rint = |rect: RectF| -> (i32, i32, i32, i32) {
                (
                    rect.left.round() as i32,
                    rect.top.round() as i32,
                    rect.width().round() as i32,
                    rect.height().round() as i32,
                )
            };
            let mut children: Vec<HWND> = Vec::new();
            macro_rules! reg_static {
                ($text:expr, $rect:expr, $id:expr, $font:expr, $visible:expr $(,)?) => {{
                    let (x, y, w, h) = rint($rect);
                    let ctl = make_static(hwnd, hinst, $text, x, y, w, h, $id, $font, $visible);
                    children.push(ctl);
                    d.static_handles.push(($id, ctl));
                    ctl
                }};
            }
            macro_rules! reg_ctl {
                ($class:expr, $text:expr, $style:expr, $rect:expr, $id:expr $(,)?) => {{
                    let (x, y, w, h) = rint($rect);
                    let ctl = make_ctl(hwnd, hinst, $class, $text, $style, x, y, w, h, $id);
                    children.push(ctl);
                    ctl
                }};
            }

            // ── Section titles + subtitles (hidden until their section is
            //    selected; the paint draws the rail, not these). ──────────
            reg_static!(
                w!("General"),
                layout.section_title,
                ID_HDR_GENERAL,
                d.hfont_section,
                false
            );
            reg_static!(
                w!("Hotkeys"),
                layout.section_title,
                ID_HDR_HOTKEYS,
                d.hfont_section,
                false
            );
            reg_static!(
                w!("Appearance"),
                layout.section_title,
                ID_HDR_APPEARANCE,
                d.hfont_section,
                false
            );
            reg_static!(
                w!("Blacklist"),
                layout.section_title,
                ID_HDR_BLACKLIST,
                d.hfont_section,
                false
            );
            reg_static!(
                w!("Feedback"),
                layout.section_title,
                ID_HDR_FEEDBACK,
                d.hfont_section,
                false
            );
            reg_static!(
                w!("Storage"),
                layout.section_title,
                ID_HDR_STORAGE,
                d.hfont_section,
                false
            );
            // Subtitles come from the Section table (single source of truth
            // for the section copy; CreateWindowExW copies the text during the
            // call, so the temporary wide buffer is safe).
            for (section, id) in [
                (Section::General, ID_SUB_GENERAL),
                (Section::Hotkeys, ID_SUB_HOTKEYS),
                (Section::Appearance, ID_SUB_APPEARANCE),
                (Section::Blacklist, ID_SUB_BLACKLIST),
                (Section::Feedback, ID_SUB_FEEDBACK),
                (Section::Storage, ID_SUB_STORAGE),
            ] {
                let wide: Vec<u16> = section
                    .subtitle()
                    .encode_utf16()
                    .chain(std::iter::once(0))
                    .collect();
                reg_static!(
                    wide.as_ptr(),
                    layout.section_subtitle,
                    id,
                    d.hfont_caption,
                    false
                );
            }

            // ── General ───────────────────────────────────────────────────
            reg_static!(
                w!("Volume step"),
                layout.general.rows[0].label,
                ID_LBL_VOL_STEP,
                d.hfont_label,
                false
            );
            d.edit_volume_step = reg_ctl!(
                w!("EDIT"),
                w!(""),
                WS_CHILD | WS_TABSTOP | ES_AUTOHSCROLL | ES_NUMBER,
                layout.general.rows[0].field,
                ID_VOL_STEP,
            );
            reg_static!(
                w!("Small change applied by ↑ / ↓."),
                layout.general.rows[0].helper,
                ID_HELP_VOL_STEP,
                d.hfont_caption,
                false
            );
            reg_static!(
                w!("Large step"),
                layout.general.rows[1].label,
                ID_LBL_VOL_STEP_LARGE,
                d.hfont_label,
                false
            );
            d.edit_volume_step_large = reg_ctl!(
                w!("EDIT"),
                w!(""),
                WS_CHILD | WS_TABSTOP | ES_AUTOHSCROLL | ES_NUMBER,
                layout.general.rows[1].field,
                ID_VOL_STEP_LARGE,
            );
            reg_static!(
                w!("Shift + ↑ / ↓ applies the large change."),
                layout.general.rows[1].helper,
                ID_HELP_VOL_STEP_LARGE,
                d.hfont_caption,
                false
            );
            reg_static!(
                w!("Overlay duration"),
                layout.general.rows[2].label,
                ID_LBL_OVERLAY,
                d.hfont_label,
                false
            );
            d.edit_overlay_ms = reg_ctl!(
                w!("EDIT"),
                w!(""),
                WS_CHILD | WS_TABSTOP | ES_AUTOHSCROLL | ES_NUMBER,
                layout.general.rows[2].field,
                ID_OVERLAY_MS,
            );
            reg_static!(
                w!("How long the volume overlay stays visible."),
                layout.general.rows[2].helper,
                ID_HELP_OVERLAY,
                d.hfont_caption,
                false
            );

            // ── Hotkeys ──────────────────────────────────────────────────
            reg_static!(
                w!("Modifier"),
                layout.hotkeys.label,
                ID_LBL_MODIFIER,
                d.hfont_label,
                false
            );
            d.combo_modifier = reg_ctl!(
                w!("COMBOBOX"),
                w!(""),
                WS_CHILD | WS_TABSTOP | CBS_DROPDOWNLIST,
                layout.hotkeys.combo,
                ID_COMBO_MODIFIER,
            );
            d.static_hotkey_status = reg_static!(
                w!(""),
                layout.hotkeys.status,
                ID_ST_HOTKEY_STATUS,
                d.hfont_body,
                false
            );

            // ── Appearance ───────────────────────────────────────────────
            reg_static!(
                w!("Theme"),
                layout.appearance.rows[0].label,
                ID_LBL_THEME,
                d.hfont_label,
                false
            );
            d.combo_theme = reg_ctl!(
                w!("COMBOBOX"),
                w!(""),
                WS_CHILD | WS_TABSTOP | CBS_DROPDOWNLIST,
                layout.appearance.rows[0].field,
                ID_COMBO_THEME,
            );
            reg_static!(
                w!("Material"),
                layout.appearance.rows[1].label,
                ID_LBL_MATERIAL,
                d.hfont_label,
                false
            );
            d.combo_material = reg_ctl!(
                w!("COMBOBOX"),
                w!(""),
                WS_CHILD | WS_TABSTOP | CBS_DROPDOWNLIST,
                layout.appearance.rows[1].field,
                ID_COMBO_MATERIAL,
            );
            reg_static!(
                w!("Motion"),
                layout.appearance.rows[2].label,
                ID_LBL_MOTION,
                d.hfont_label,
                false
            );
            d.combo_motion = reg_ctl!(
                w!("COMBOBOX"),
                w!(""),
                WS_CHILD | WS_TABSTOP | CBS_DROPDOWNLIST,
                layout.appearance.rows[2].field,
                ID_COMBO_MOTION,
            );
            reg_static!(
                w!("Accent"),
                layout.appearance.rows[3].label,
                ID_LBL_ACCENT,
                d.hfont_label,
                false
            );
            d.combo_accent = reg_ctl!(
                w!("COMBOBOX"),
                w!(""),
                WS_CHILD | WS_TABSTOP | CBS_DROPDOWNLIST,
                layout.appearance.rows[3].field,
                ID_COMBO_ACCENT,
            );
            reg_static!(
                w!("Preview"),
                layout.appearance.preview_caption,
                ID_ST_PREVIEW_CAPTION,
                d.hfont_caption,
                false
            );
            d.preview = CreateWindowExW(
                0,
                preview_class,
                w!(""),
                WS_CHILD,
                0,
                0,
                PREVIEW_W.round() as i32,
                PREVIEW_H.round() as i32,
                hwnd,
                0,
                hinst,
                std::ptr::null(),
            );
            children.push(d.preview);

            // ── Blacklist ────────────────────────────────────────────────
            d.edit_blacklist_new = reg_ctl!(
                w!("EDIT"),
                w!(""),
                WS_CHILD | WS_TABSTOP | ES_AUTOHSCROLL,
                layout.blacklist.add_edit,
                ID_EDIT_BLACKLIST,
            );
            d.btn_blacklist_add = reg_ctl!(
                w!("BUTTON"),
                w!("Add"),
                WS_CHILD | WS_TABSTOP,
                layout.blacklist.add_btn,
                ID_BTN_BLACKLIST_ADD,
            );
            d.list_blacklist = reg_ctl!(
                w!("LISTBOX"),
                w!(""),
                WS_CHILD | WS_TABSTOP | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT,
                layout.blacklist.list,
                ID_LIST_BLACKLIST,
            );
            // Empty-state copy overlays the (empty) list; hidden once entries
            // exist. Created after the list so it paints on top.
            d.static_blacklist_empty[0] = reg_static!(
                w!("No blocked applications"),
                layout.blacklist.empty_title,
                ID_ST_BLACKLIST_EMPTY_1,
                d.hfont_body,
                false
            );
            d.static_blacklist_empty[1] = reg_static!(
                w!("VolumeControl will respond to shortcuts everywhere."),
                layout.blacklist.empty_caption,
                ID_ST_BLACKLIST_EMPTY_2,
                d.hfont_caption,
                false
            );
            d.btn_blacklist_remove = reg_ctl!(
                w!("BUTTON"),
                w!("Remove"),
                WS_CHILD | WS_TABSTOP,
                layout.blacklist.remove,
                ID_BTN_BLACKLIST_REMOVE,
            );
            d.btn_blacklist_clear = reg_ctl!(
                w!("BUTTON"),
                w!("Clear"),
                WS_CHILD | WS_TABSTOP,
                layout.blacklist.clear,
                ID_BTN_BLACKLIST_CLEAR,
            );
            d.btn_blacklist_recommend = reg_ctl!(
                w!("BUTTON"),
                w!("Apply Recommended"),
                WS_CHILD | WS_TABSTOP,
                layout.blacklist.recommend,
                ID_BTN_BLACKLIST_RECOMMEND,
            );

            // ── Feedback ─────────────────────────────────────────────────
            d.chk_beep = reg_ctl!(
                w!("BUTTON"),
                w!("Enable beep feedback"),
                WS_CHILD | WS_TABSTOP | BS_AUTOCHECKBOX,
                layout.feedback.chk,
                ID_CHK_BEEP,
            );
            reg_static!(
                w!("Blocked beep frequency"),
                layout.feedback.rows[0].label,
                ID_LBL_BLOCKED_FREQ,
                d.hfont_label,
                false
            );
            d.edit_blocked_freq = reg_ctl!(
                w!("EDIT"),
                w!(""),
                WS_CHILD | WS_TABSTOP | ES_AUTOHSCROLL | ES_NUMBER,
                layout.feedback.rows[0].field,
                ID_BLOCKED_FREQ,
            );
            reg_static!(
                w!("Beep when a shortcut is blocked."),
                layout.feedback.rows[0].helper,
                ID_HELP_BLOCKED_FREQ,
                d.hfont_caption,
                false
            );
            reg_static!(
                w!("Blocked beep duration"),
                layout.feedback.rows[1].label,
                ID_LBL_BLOCKED_DUR,
                d.hfont_label,
                false
            );
            d.edit_blocked_dur = reg_ctl!(
                w!("EDIT"),
                w!(""),
                WS_CHILD | WS_TABSTOP | ES_AUTOHSCROLL | ES_NUMBER,
                layout.feedback.rows[1].field,
                ID_BLOCKED_DUR,
            );
            reg_static!(
                w!("How long the blocked beep sounds."),
                layout.feedback.rows[1].helper,
                ID_HELP_BLOCKED_DUR,
                d.hfont_caption,
                false
            );
            reg_static!(
                w!("Limit beep frequency"),
                layout.feedback.rows[2].label,
                ID_LBL_LIMIT_FREQ,
                d.hfont_label,
                false
            );
            d.edit_limit_freq = reg_ctl!(
                w!("EDIT"),
                w!(""),
                WS_CHILD | WS_TABSTOP | ES_AUTOHSCROLL | ES_NUMBER,
                layout.feedback.rows[2].field,
                ID_LIMIT_FREQ,
            );
            reg_static!(
                w!("Beep when volume is already at the limit."),
                layout.feedback.rows[2].helper,
                ID_HELP_LIMIT_FREQ,
                d.hfont_caption,
                false
            );
            reg_static!(
                w!("Limit beep duration"),
                layout.feedback.rows[3].label,
                ID_LBL_LIMIT_DUR,
                d.hfont_label,
                false
            );
            d.edit_limit_dur = reg_ctl!(
                w!("EDIT"),
                w!(""),
                WS_CHILD | WS_TABSTOP | ES_AUTOHSCROLL | ES_NUMBER,
                layout.feedback.rows[3].field,
                ID_LIMIT_DUR,
            );
            reg_static!(
                w!("How long the limit beep sounds."),
                layout.feedback.rows[3].helper,
                ID_HELP_LIMIT_DUR,
                d.hfont_caption,
                false
            );

            // ── Storage ──────────────────────────────────────────────────
            reg_static!(
                w!("Config file"),
                layout.storage.label,
                ID_LBL_CONFIG,
                d.hfont_label,
                false
            );
            // The path truncates with an ellipsis (SS_ENDELLIPSIS) instead of
            // clipping mid-glyph on long user names.
            {
                let (x, y, w, h) = rint(layout.storage.path);
                let ctl = CreateWindowExW(
                    0,
                    w!("STATIC"),
                    w!(""),
                    WS_CHILD | WS_VISIBLE | SS_ENDELLIPSIS,
                    x,
                    y,
                    w,
                    h,
                    hwnd,
                    ID_ST_CONFIG_PATH,
                    hinst,
                    std::ptr::null(),
                );
                if ctl != 0 {
                    SendMessageW(ctl, WM_SETFONT, d.hfont_body as usize, 1);
                }
                children.push(ctl);
                d.static_handles.push((ID_ST_CONFIG_PATH, ctl));
                d.static_path = ctl;
            }
            d.btn_open_config = reg_ctl!(
                w!("BUTTON"),
                w!("Open config file"),
                WS_CHILD | WS_TABSTOP,
                layout.storage.open,
                ID_BTN_OPEN_CONFIG,
            );
            reg_static!(
                w!("Config changes are reloaded automatically."),
                layout.storage.status,
                ID_ST_STORAGE_STATUS,
                d.hfont_caption,
                false
            );

            // ── Inline validation errors (hidden until a failing Apply) ──
            let err_font = d.hfont_body;
            d.error_statics[0] = reg_static!(
                w!(""),
                layout.general.rows[0].helper,
                ID_ERR_VOL_STEP,
                err_font,
                false
            );
            d.error_statics[1] = reg_static!(
                w!(""),
                layout.general.rows[1].helper,
                ID_ERR_VOL_STEP_LARGE,
                err_font,
                false
            );
            d.error_statics[2] = reg_static!(
                w!(""),
                layout.general.rows[2].helper,
                ID_ERR_OVERLAY,
                err_font,
                false
            );
            d.error_statics[3] = reg_static!(
                w!(""),
                layout.feedback.rows[0].helper,
                ID_ERR_BLOCKED_FREQ,
                err_font,
                false
            );
            d.error_statics[4] = reg_static!(
                w!(""),
                layout.feedback.rows[1].helper,
                ID_ERR_BLOCKED_DUR,
                err_font,
                false
            );
            d.error_statics[5] = reg_static!(
                w!(""),
                layout.feedback.rows[2].helper,
                ID_ERR_LIMIT_FREQ,
                err_font,
                false
            );
            d.error_statics[6] = reg_static!(
                w!(""),
                layout.feedback.rows[3].helper,
                ID_ERR_LIMIT_DUR,
                err_font,
                false
            );

            // ── Footer (sticky in both layouts) ──────────────────────────
            d.static_status =
                reg_static!(w!(""), layout.status_rect, ID_ST_STATUS, d.hfont_body, true);
            // Screen-reader mirror of the status line (spec §11.2): parked
            // OUTSIDE the client area (1x1 at -200,-200) so it is never
            // visible while staying WS_VISIBLE in the UIA tree; `set_status`
            // writes `Status: …` / `Alert: …` into it. It is deliberately not
            // in `static_handles`/`child_rects`, so relayout and the parent
            // paint never move or repaint it.
            d.static_status_uia = make_static(
                hwnd,
                hinst,
                w!(""),
                -200,
                -200,
                1,
                1,
                ID_ST_STATUS_UIA,
                d.hfont_body,
                true,
            );
            children.push(d.static_status_uia);
            // Footer + close are global (not owned by any section), so they
            // are created visible; section controls start hidden and are
            // shown by `apply_section_visibility`.
            d.btn_reset = reg_ctl!(
                w!("BUTTON"),
                w!("Reset"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                layout.btn_reset,
                ID_BTN_RESET,
            );
            d.btn_cancel = reg_ctl!(
                w!("BUTTON"),
                w!("Cancel"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                layout.btn_cancel,
                ID_BTN_CANCEL,
            );
            d.btn_apply = reg_ctl!(
                w!("BUTTON"),
                w!("Save changes"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                layout.btn_apply,
                ID_BTN_APPLY,
            );
            // Owner-drawn close (spec §11.2): the window text is the UIA name
            // `Close settings` while `settings_wndproc`'s WM_DRAWITEM paints
            // the approved `×` visual via `paint_close_button`.
            d.btn_close = reg_ctl!(
                w!("BUTTON"),
                w!("Close settings"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW as u32,
                layout.close_rect,
                ID_BTN_CLOSE,
            );

            // A failed child leaves the window unusable; tear down cleanly.
            if children.contains(&0) {
                DestroyWindow(hwnd);
                drop(Box::from_raw(data));
                return Err("settings child control failed".into());
            }

            // Per-section visibility + tab buckets (one section at a time).
            build_sections(d);

            // Subclass every interactive control in visual (tab) order; the
            // subclass adds Escape + deterministic Tab/Shift+Tab.
            for (_, ctl) in interactive_handles(d) {
                subclass(d, ctl);
            }

            // Populate the combo list items (selection is set by
            // `populate_controls`).
            combo_add(d.combo_modifier, &["Ctrl+Alt", "Alt", "Ctrl", "CapsLock"]);
            combo_add(d.combo_theme, &["System", "Light", "Dark"]);
            combo_add(d.combo_material, &["Auto", "Translucent", "Opaque"]);
            combo_add(d.combo_motion, &["Full", "Reduced", "Disabled"]);
            combo_add(
                d.combo_accent,
                &["System", "Blue", "Green", "Purple", "Orange"],
            );

            // Seed from defaults so the hidden window is self-consistent before
            // the first `show(config)` re-seeds from the loaded config.
            d.draft = SettingsDraft::new(Config::default());
            populate_controls(d);
            set_status(d, StatusKind::None, "");
            set_control_text(
                d.static_path,
                &crate::config::config_path().display().to_string(),
            );

            // Initial adaptive styling from the placeholder appearance.
            let appearance = SettingsAppearance::placeholder();
            d.appearance = appearance;
            d.bg = CreateSolidBrush(colorref(appearance.tokens.background));
            d.accent_brush = CreateSolidBrush(colorref(appearance.tokens.accent));
            apply_backdrop(hwnd, appearance.material, appearance.tokens.is_dark);
            theme_controls(&d.tab_order, appearance.tokens.is_dark);

            apply_section_visibility(d);
            sync_apply_enabled(d);
            show_inline_errors(d);

            Ok(Settings { hwnd })
        }
    }

    /// Seed a fresh draft from `config` and show the window.
    ///
    /// Opening always re-seeds from the authoritative config (edits from a
    /// previous session are not carried across a close/reopen — `Close` does
    /// not save). The host-resolved appearance themes the window. The window
    /// is sized to the work area (DPI-scaled, clamped to the minimums), so a
    /// cramped monitor automatically lands in the stacked selector layout.
    pub fn show(&mut self, config: &Config, appearance: &SettingsAppearance) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut SettingsData);
            apply_appearance(self.hwnd, d, *appearance);
            d.draft = SettingsDraft::new(config.clone());
            d.section = Section::General;
            populate_controls(d);
            set_status(d, StatusKind::None, "");
            sync_apply_enabled(d);
            show_inline_errors(d);

            let dpi = DpiMetrics::new(dpi_scale_for(self.hwnd));
            d.dpi = dpi.scale();
            let work_area = work_area_for(self.hwnd);
            let mut phys_w = dpi.to_physical(WIN_W);
            let mut phys_h = dpi.to_physical(WIN_H);
            if work_area.width < phys_w {
                phys_w = work_area.width.max(dpi.to_physical(MIN_W));
            }
            if work_area.height < phys_h {
                phys_h = work_area.height.max(dpi.to_physical(MIN_H));
            }
            let rect = place_centered(work_area, SurfaceSize::new(phys_w, phys_h));
            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                rect.left,
                rect.top,
                phys_w,
                phys_h,
                SWP_SHOWWINDOW,
            );
            // DWM re-asserts DWMSBT_AUTO when a window is shown while High
            // Contrast is active — asynchronously, AFTER the show (observed:
            // the reset lands after any immediate re-apply). Re-apply the
            // resolved backdrop on a one-shot timer once the composition has
            // settled, so the opaque painted surface stays visible on screen.
            SetTimer(self.hwnd, BACKDROP_TIMER_ID, BACKDROP_REAPPLY_MS, None);
            // While High Contrast is active, DWM fills the freshly shown
            // surface with the forced AUTO backdrop, which marks the client
            // clean — the system then skips the first WM_PAINT and the
            // painted content never lands (live-verified: under HC only the
            // native child controls self-painted; a probe-side invalidation
            // did not register either). Force the first paint so the opaque
            // fill always draws, over whatever backdrop DWM keeps.
            RedrawWindow(
                self.hwnd,
                std::ptr::null(),
                0,
                RDW_INVALIDATE | RDW_UPDATENOW,
            );
            d.open = true;
            // The window is created at its final size, so SetWindowPos does
            // not resend WM_SIZE (and WM_SIZE already ran before `show` set
            // the DPI); run the relayout explicitly so every child — the
            // preview card included — lands at its real position.
            relayout(d);
            // First focusable control of the active (General) section.
            if let Some(&first) = d.section_tabs[Section::General.index()].first() {
                SetFocus(first);
            }
        }
    }

    /// Hide the window without touching the draft (Close semantics).
    pub fn hide(&mut self) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut SettingsData);
            d.open = false;
            ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    pub fn is_open(&self) -> bool {
        unsafe {
            let d = &*(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *const SettingsData);
            d.open
        }
    }

    /// Re-theme the window from a newly resolved appearance.
    pub fn set_appearance(&mut self, appearance: &SettingsAppearance) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut SettingsData);
            apply_appearance(self.hwnd, d, *appearance);
        }
    }

    /// Adopt a new authoritative baseline from a live file reload while the
    /// window is open. In-progress edits are preserved (the baseline moves but
    /// the controls are only repopulated when the draft is clean, so an
    /// external change never clobbers what the user is typing).
    pub fn reload(&mut self, config: &Config) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut SettingsData);
            d.draft.replace(config.clone());
            if !d.draft.is_dirty() {
                populate_controls(d);
                set_status(d, StatusKind::None, "");
            }
            sync_apply_enabled(d);
            show_inline_errors(d);
            InvalidateRect(self.hwnd, std::ptr::null(), 0);
        }
    }

    /// Apply: read every control into the draft's working copy, validate, and
    /// persist through `SettingsDraft::commit`. Called by the host's
    /// `ApplyConfig` arm — the window never persists on its own.
    ///
    /// Returns the normalized saved config on success; on failure the draft
    /// keeps the edits so the window can show the offending field.
    pub fn apply(&mut self) -> Result<Config, ConfigError> {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut SettingsData);
            let values = read_controls(d);
            let config = control_values_to_config(d.draft.current(), &values);
            d.draft.set_current(config);
            d.draft.commit()
        }
    }

    /// Report the host's apply result back to the window.
    ///
    /// - Success: the draft already adopted the saved config as baseline +
    ///   working copy; repopulate the controls from it (so normalized values
    ///   like lowercased blacklist entries are shown) and show a saved note.
    /// - Validation failure: the draft kept the edits; show the field error
    ///   inline, switch to the offending field's section, and move focus to
    ///   it. The window stays open.
    /// - I/O / serialization failure: validation passed but persistence failed;
    ///   keep the edits and show a "could not save" message.
    pub fn on_apply_result(&mut self, result: &Result<Config, ConfigError>) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut SettingsData);
            match result {
                Ok(_) => {
                    populate_controls(d);
                    sync_apply_enabled(d);
                    show_inline_errors(d);
                    set_status(d, StatusKind::Info, "Changes saved.");
                    InvalidateRect(self.hwnd, std::ptr::null(), 0);
                }
                Err(e) => match e {
                    ConfigError::Validation(ve) => {
                        set_status(d, StatusKind::Error, &ve.to_string());
                        show_inline_errors(d);
                        focus_validation_field(d, ve.field);
                    }
                    ConfigError::Io(ie) => {
                        set_status(
                            d,
                            StatusKind::Error,
                            &format!("Could not save config: {ie}"),
                        );
                    }
                    ConfigError::Serialization(se) => {
                        set_status(
                            d,
                            StatusKind::Error,
                            &format!("Could not save config: {se}"),
                        );
                    }
                },
            }
        }
    }

    /// Discard edits, repopulate from the baseline, and hide (Cancel semantics).
    pub fn cancel(&mut self) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut SettingsData);
            d.draft.cancel();
            populate_controls(d);
            set_status(d, StatusKind::None, "");
            sync_apply_enabled(d);
            show_inline_errors(d);
            d.open = false;
            ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    /// Discard edits and repopulate from the baseline, staying open
    /// (Reset semantics).
    pub fn reset(&mut self) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut SettingsData);
            d.draft.reset();
            populate_controls(d);
            set_status(d, StatusKind::None, "");
            sync_apply_enabled(d);
            show_inline_errors(d);
            InvalidateRect(self.hwnd, std::ptr::null(), 0);
        }
    }

    /// Add an entry to the in-memory blacklist (used by the AppAction arm for
    /// external callers; the window's Add button calls the same internal path).
    pub fn add_blacklist_entry(&mut self, name: &str) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut SettingsData);
            add_entry(d, name);
        }
    }

    /// Remove an entry from the in-memory blacklist by exact name.
    pub fn remove_blacklist_entry(&mut self, name: &str) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut SettingsData);
            remove_entry(d, name);
        }
    }

    /// Clear the in-memory blacklist.
    pub fn clear_blacklist(&mut self) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut SettingsData);
            clear_entries(d);
        }
    }

    /// Free resources + destroy the window.
    fn destroy(&mut self) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut SettingsData);
            // Destroy the window (and its subclassed children) BEFORE freeing
            // `d`: teardown messages route through the child subclass proc,
            // which reads the parent's GWLP_USERDATA. See the mixer for the
            // same ordering.
            DestroyWindow(self.hwnd);
            for brush in [d.bg, d.accent_brush] {
                if brush != 0 {
                    DeleteObject(brush);
                }
            }
            for hfont in [
                d.hfont_title,
                d.hfont_section,
                d.hfont_label,
                d.hfont_body,
                d.hfont_caption,
            ] {
                if hfont != 0 {
                    DeleteObject(hfont);
                }
            }
            drop(Box::from_raw(d));
        }
    }
}

impl Drop for Settings {
    fn drop(&mut self) {
        self.destroy();
    }
}

// ── Control creation helpers ───────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
unsafe fn make_static(
    parent: HWND,
    hinst: isize,
    text: *const u16,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: isize,
    font: HFONT,
    visible: bool,
) -> HWND {
    let ctl = CreateWindowExW(
        0,
        w!("STATIC"),
        text,
        WS_CHILD | if visible { WS_VISIBLE } else { 0 },
        x,
        y,
        w,
        h,
        parent,
        id,
        hinst,
        std::ptr::null(),
    );
    if ctl != 0 {
        SendMessageW(ctl, WM_SETFONT, font as usize, 1);
    }
    ctl
}

#[allow(clippy::too_many_arguments)]
unsafe fn make_ctl(
    parent: HWND,
    hinst: isize,
    class: *const u16,
    text: *const u16,
    style: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: isize,
) -> HWND {
    CreateWindowExW(
        0,
        class,
        text,
        style,
        x,
        y,
        w,
        h,
        parent,
        id,
        hinst,
        std::ptr::null(),
    )
}

// ── Control value helpers ──────────────────────────────────────────────────

fn set_control_text(hwnd: HWND, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        SendMessageW(hwnd, WM_SETTEXT, 0, wide.as_ptr() as isize);
    }
}

fn get_control_text(hwnd: HWND) -> String {
    unsafe {
        let len = SendMessageW(hwnd, WM_GETTEXTLENGTH, 0, 0) as usize;
        let mut buf = vec![0u16; len + 1];
        SendMessageW(
            hwnd,
            WM_GETTEXT,
            (len + 1) as usize,
            buf.as_mut_ptr() as isize,
        );
        String::from_utf16_lossy(&buf[..len])
    }
}

fn edit_number(hwnd: HWND) -> u32 {
    get_control_text(hwnd).trim().parse::<u32>().unwrap_or(0)
}

fn edit_number_u64(hwnd: HWND) -> u64 {
    get_control_text(hwnd).trim().parse::<u64>().unwrap_or(0)
}

fn checkbox_checked(hwnd: HWND) -> bool {
    unsafe { SendMessageW(hwnd, BM_GETCHECK, 0, 0) != 0 }
}

fn set_checkbox(hwnd: HWND, checked: bool) {
    unsafe {
        SendMessageW(hwnd, BM_SETCHECK, usize::from(checked), 0);
    }
}

fn combo_add(hwnd: HWND, items: &[&str]) {
    for item in items {
        let wide: Vec<u16> = item.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            SendMessageW(hwnd, CB_ADDSTRING, 0, wide.as_ptr() as isize);
        }
    }
}

fn combo_set_index(hwnd: HWND, index: usize) {
    unsafe {
        SendMessageW(hwnd, CB_SETCURSEL, index, 0);
    }
}

fn combo_get_index(hwnd: HWND) -> i32 {
    unsafe { SendMessageW(hwnd, CB_GETCURSEL, 0, 0) as i32 }
}

fn listbox_reset(hwnd: HWND) {
    unsafe {
        SendMessageW(hwnd, LB_RESETCONTENT, 0, 0);
    }
}

fn listbox_add(hwnd: HWND, item: &str) {
    let wide: Vec<u16> = item.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        SendMessageW(hwnd, LB_ADDSTRING, 0, wide.as_ptr() as isize);
    }
}

fn listbox_items(hwnd: HWND) -> Vec<String> {
    unsafe {
        let count = SendMessageW(hwnd, LB_GETCOUNT, 0, 0) as i32;
        let mut items = Vec::with_capacity(count.max(0) as usize);
        for i in 0..count.max(0) {
            let len = SendMessageW(hwnd, LB_GETTEXTLEN, i as usize, 0) as usize;
            let mut buf = vec![0u16; len + 1];
            SendMessageW(hwnd, LB_GETTEXT, i as usize, buf.as_mut_ptr() as isize);
            items.push(String::from_utf16_lossy(&buf[..len]));
        }
        items
    }
}

fn listbox_selected_text(hwnd: HWND) -> Option<String> {
    unsafe {
        let idx = SendMessageW(hwnd, LB_GETCURSEL, 0, 0) as i32;
        if idx < 0 {
            return None;
        }
        let len = SendMessageW(hwnd, LB_GETTEXTLEN, idx as usize, 0) as usize;
        let mut buf = vec![0u16; len + 1];
        SendMessageW(hwnd, LB_GETTEXT, idx as usize, buf.as_mut_ptr() as isize);
        Some(String::from_utf16_lossy(&buf[..len]))
    }
}

/// Normalize a blacklist entry typed in the window: trim, lowercase, and
/// ensure a `.exe` suffix so the entry survives `crate::config::normalize`
/// (which drops entries without `.exe`). Empty input normalizes to empty.
fn normalize_blacklist_entry(raw: &str) -> String {
    let trimmed = raw.trim().to_lowercase();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.ends_with(".exe") {
        trimmed
    } else {
        format!("{trimmed}.exe")
    }
}

// ── Pure enum <-> combo index mappings ─────────────────────────────────────

fn modifier_label(m: HotkeyModifier) -> &'static str {
    match m {
        HotkeyModifier::CtrlAlt => "Ctrl+Alt",
        HotkeyModifier::Alt => "Alt",
        HotkeyModifier::Ctrl => "Ctrl",
        HotkeyModifier::CapsLock => "CapsLock",
    }
}

fn modifier_index(m: HotkeyModifier) -> usize {
    match m {
        HotkeyModifier::CtrlAlt => 0,
        HotkeyModifier::Alt => 1,
        HotkeyModifier::Ctrl => 2,
        HotkeyModifier::CapsLock => 3,
    }
}

fn modifier_from_index(idx: i32) -> HotkeyModifier {
    match idx {
        1 => HotkeyModifier::Alt,
        2 => HotkeyModifier::Ctrl,
        3 => HotkeyModifier::CapsLock,
        _ => HotkeyModifier::CtrlAlt,
    }
}

fn theme_index(t: ThemeMode) -> usize {
    match t {
        ThemeMode::System => 0,
        ThemeMode::Light => 1,
        ThemeMode::Dark => 2,
    }
}

fn theme_from_index(idx: i32) -> ThemeMode {
    match idx {
        1 => ThemeMode::Light,
        2 => ThemeMode::Dark,
        _ => ThemeMode::System,
    }
}

fn material_index(m: MaterialMode) -> usize {
    match m {
        MaterialMode::Auto => 0,
        MaterialMode::Translucent => 1,
        MaterialMode::Opaque => 2,
    }
}

fn material_from_index(idx: i32) -> MaterialMode {
    match idx {
        1 => MaterialMode::Translucent,
        2 => MaterialMode::Opaque,
        _ => MaterialMode::Auto,
    }
}

fn motion_index(m: MotionMode) -> usize {
    match m {
        MotionMode::Full => 0,
        MotionMode::Reduced => 1,
        MotionMode::Disabled => 2,
    }
}

fn motion_from_index(idx: i32) -> MotionMode {
    match idx {
        1 => MotionMode::Reduced,
        2 => MotionMode::Disabled,
        _ => MotionMode::Full,
    }
}

fn accent_index(a: AccentMode) -> usize {
    match a {
        AccentMode::System => 0,
        AccentMode::Blue => 1,
        AccentMode::Green => 2,
        AccentMode::Purple => 3,
        AccentMode::Orange => 4,
    }
}

fn accent_from_index(idx: i32) -> AccentMode {
    match idx {
        1 => AccentMode::Blue,
        2 => AccentMode::Green,
        3 => AccentMode::Purple,
        4 => AccentMode::Orange,
        _ => AccentMode::System,
    }
}

// ── Reading / populating controls ──────────────────────────────────────────

/// Read every editable control into a [`ControlValues`] snapshot. Hidden
/// sections' controls keep their last values, so Apply sees the full draft.
fn read_controls(d: &SettingsData) -> ControlValues {
    ControlValues {
        volume_step: edit_number(d.edit_volume_step),
        volume_step_large: edit_number(d.edit_volume_step_large),
        overlay_duration_ms: edit_number_u64(d.edit_overlay_ms),
        modifier: modifier_from_index(combo_get_index(d.combo_modifier)),
        theme: theme_from_index(combo_get_index(d.combo_theme)),
        material: material_from_index(combo_get_index(d.combo_material)),
        motion: motion_from_index(combo_get_index(d.combo_motion)),
        accent: accent_from_index(combo_get_index(d.combo_accent)),
        beep_enabled: checkbox_checked(d.chk_beep),
        blocked_freq: edit_number(d.edit_blocked_freq),
        blocked_duration_ms: edit_number(d.edit_blocked_dur),
        limit_freq: edit_number(d.edit_limit_freq),
        limit_duration_ms: edit_number(d.edit_limit_dur),
        blacklist: listbox_items(d.list_blacklist),
    }
}

/// Overlay parsed control values onto `base`, preserving every field the
/// window does not expose (notably `color_thresholds`).
fn control_values_to_config(base: &Config, values: &ControlValues) -> Config {
    let mut cfg = base.clone();
    cfg.volume_step = values.volume_step;
    cfg.volume_step_large = values.volume_step_large;
    cfg.overlay_duration_ms = values.overlay_duration_ms;
    cfg.modifier = values.modifier;
    cfg.appearance.theme = values.theme;
    cfg.appearance.material = values.material;
    cfg.appearance.motion = values.motion;
    cfg.appearance.accent = values.accent;
    cfg.beep.enabled = values.beep_enabled;
    cfg.beep.blocked_freq = values.blocked_freq;
    cfg.beep.blocked_duration_ms = values.blocked_duration_ms;
    cfg.beep.limit_freq = values.limit_freq;
    cfg.beep.limit_duration_ms = values.limit_duration_ms;
    cfg.blacklist = values.blacklist.clone();
    cfg
}

/// Repopulate every control from the draft's working copy.
fn populate_controls(d: &SettingsData) {
    let cfg = d.draft.current();
    set_control_text(d.edit_volume_step, &cfg.volume_step.to_string());
    set_control_text(d.edit_volume_step_large, &cfg.volume_step_large.to_string());
    set_control_text(d.edit_overlay_ms, &cfg.overlay_duration_ms.to_string());
    combo_set_index(d.combo_modifier, modifier_index(cfg.modifier));
    combo_set_index(d.combo_theme, theme_index(cfg.appearance.theme));
    combo_set_index(d.combo_material, material_index(cfg.appearance.material));
    combo_set_index(d.combo_motion, motion_index(cfg.appearance.motion));
    combo_set_index(d.combo_accent, accent_index(cfg.appearance.accent));
    set_checkbox(d.chk_beep, cfg.beep.enabled);
    set_control_text(d.edit_blocked_freq, &cfg.beep.blocked_freq.to_string());
    set_control_text(
        d.edit_blocked_dur,
        &cfg.beep.blocked_duration_ms.to_string(),
    );
    set_control_text(d.edit_limit_freq, &cfg.beep.limit_freq.to_string());
    set_control_text(d.edit_limit_dur, &cfg.beep.limit_duration_ms.to_string());
    populate_blacklist(d);
    set_control_text(
        d.static_hotkey_status,
        &format!(
            "Modifier: {} — hotkeys re-register on Apply.",
            modifier_label(cfg.modifier)
        ),
    );
}

/// Repopulate only the blacklist listbox from the draft's working copy.
///
/// Separate from [`populate_controls`] so Add/Remove/Clear don't clobber
/// in-progress edits in the other fields.
fn populate_blacklist(d: &SettingsData) {
    listbox_reset(d.list_blacklist);
    for entry in &d.draft.current().blacklist {
        listbox_add(d.list_blacklist, entry);
    }
    unsafe {
        update_blacklist_empty_state(d);
    }
}

fn set_status(d: &mut SettingsData, kind: StatusKind, text: &str) {
    d.status_kind = kind;
    set_control_text(d.static_status, text);
    // Spec §11.2: the screen-reader mirror carries the `Status: …` /
    // `Alert: …` phrasing (the visible line stays the plain text).
    let prefix = match kind {
        StatusKind::Error => "Alert: ",
        StatusKind::Info => "Status: ",
        StatusKind::None => "",
    };
    set_control_text(d.static_status_uia, &format!("{prefix}{text}"));
}

/// Every interactive control as (id, hwnd) pairs, in visual (tab) order.
fn interactive_handles(d: &SettingsData) -> Vec<(isize, HWND)> {
    vec![
        (ID_VOL_STEP, d.edit_volume_step),
        (ID_VOL_STEP_LARGE, d.edit_volume_step_large),
        (ID_OVERLAY_MS, d.edit_overlay_ms),
        (ID_COMBO_MODIFIER, d.combo_modifier),
        (ID_COMBO_THEME, d.combo_theme),
        (ID_COMBO_MATERIAL, d.combo_material),
        (ID_COMBO_MOTION, d.combo_motion),
        (ID_COMBO_ACCENT, d.combo_accent),
        (ID_EDIT_BLACKLIST, d.edit_blacklist_new),
        (ID_BTN_BLACKLIST_ADD, d.btn_blacklist_add),
        (ID_LIST_BLACKLIST, d.list_blacklist),
        (ID_BTN_BLACKLIST_REMOVE, d.btn_blacklist_remove),
        (ID_BTN_BLACKLIST_CLEAR, d.btn_blacklist_clear),
        (ID_BTN_BLACKLIST_RECOMMEND, d.btn_blacklist_recommend),
        (ID_CHK_BEEP, d.chk_beep),
        (ID_BLOCKED_FREQ, d.edit_blocked_freq),
        (ID_BLOCKED_DUR, d.edit_blocked_dur),
        (ID_LIMIT_FREQ, d.edit_limit_freq),
        (ID_LIMIT_DUR, d.edit_limit_dur),
        (ID_BTN_OPEN_CONFIG, d.btn_open_config),
        (ID_BTN_APPLY, d.btn_apply),
        (ID_BTN_RESET, d.btn_reset),
        (ID_BTN_CANCEL, d.btn_cancel),
        (ID_BTN_CLOSE, d.btn_close),
    ]
}

/// The section an interactive control belongs to (footer/close are global).
fn interactive_section(id: isize) -> Option<Section> {
    match id {
        ID_VOL_STEP..=ID_OVERLAY_MS => Some(Section::General),
        ID_COMBO_MODIFIER => Some(Section::Hotkeys),
        ID_COMBO_THEME..=ID_COMBO_ACCENT => Some(Section::Appearance),
        ID_LIST_BLACKLIST..=ID_BTN_BLACKLIST_RECOMMEND => Some(Section::Blacklist),
        ID_CHK_BEEP..=ID_LIMIT_DUR => Some(Section::Feedback),
        ID_BTN_OPEN_CONFIG => Some(Section::Storage),
        _ => None,
    }
}

/// The section a static belongs to (the status line is global).
fn static_section(id: isize) -> Option<Section> {
    match id {
        ID_HDR_GENERAL
        | ID_SUB_GENERAL
        | ID_LBL_VOL_STEP
        | ID_LBL_VOL_STEP_LARGE
        | ID_LBL_OVERLAY
        | ID_HELP_VOL_STEP
        | ID_HELP_VOL_STEP_LARGE
        | ID_HELP_OVERLAY => Some(Section::General),
        ID_HDR_HOTKEYS | ID_SUB_HOTKEYS | ID_LBL_MODIFIER | ID_ST_HOTKEY_STATUS => {
            Some(Section::Hotkeys)
        }
        ID_HDR_APPEARANCE
        | ID_SUB_APPEARANCE
        | ID_LBL_THEME..=ID_LBL_ACCENT
        | ID_ST_PREVIEW_CAPTION => Some(Section::Appearance),
        ID_HDR_BLACKLIST | ID_SUB_BLACKLIST | ID_ST_BLACKLIST_EMPTY_1 | ID_ST_BLACKLIST_EMPTY_2 => {
            Some(Section::Blacklist)
        }
        ID_HDR_FEEDBACK
        | ID_SUB_FEEDBACK
        | ID_LBL_BLOCKED_FREQ..=ID_LBL_LIMIT_DUR
        | ID_HELP_BLOCKED_FREQ..=ID_HELP_LIMIT_DUR => Some(Section::Feedback),
        ID_HDR_STORAGE | ID_SUB_STORAGE | ID_LBL_CONFIG | ID_ST_CONFIG_PATH
        | ID_ST_STORAGE_STATUS => Some(Section::Storage),
        _ => None,
    }
}

/// Build the per-section child buckets (visibility swap) and per-section tab
/// lists from the registered handles.
fn build_sections(d: &mut SettingsData) {
    let mut all = vec![Vec::new(); Section::ALL.len()];
    let mut tabs = vec![Vec::new(); Section::ALL.len()];
    for &(id, hwnd) in &d.static_handles {
        if let Some(section) = static_section(id) {
            all[section.index()].push(hwnd);
        }
    }
    for (id, hwnd) in interactive_handles(d) {
        if let Some(section) = interactive_section(id) {
            all[section.index()].push(hwnd);
            tabs[section.index()].push(hwnd);
        }
    }
    d.section_all = all;
    d.section_tabs = tabs;
}

/// Deterministic Tab cycle: the rail/strip (the window itself) first, then the
/// active section's controls, then the footer buttons and Close.
fn tab_cycle(d: &SettingsData) -> Vec<HWND> {
    let mut cycle = Vec::with_capacity(16);
    cycle.push(d.hwnd);
    cycle.extend_from_slice(&d.section_tabs[d.section.index()]);
    cycle.push(d.btn_reset);
    cycle.push(d.btn_cancel);
    cycle.push(d.btn_apply);
    cycle.push(d.btn_close);
    cycle
}

/// Show only the selected section's children; the rest stay hidden. The draft
/// is window-scoped, so navigation never loses edits.
unsafe fn apply_section_visibility(d: &SettingsData) {
    for (i, controls) in d.section_all.iter().enumerate() {
        let visible = i == d.section.index();
        for &ctl in controls {
            ShowWindow(ctl, if visible { SW_SHOW } else { SW_HIDE });
        }
    }
    ShowWindow(
        d.preview,
        if d.section == Section::Appearance {
            SW_SHOW
        } else {
            SW_HIDE
        },
    );
    update_blacklist_empty_state(d);
}

/// Toggle the "No blocked applications" copy over the empty list.
unsafe fn update_blacklist_empty_state(d: &SettingsData) {
    if d.section == Section::Blacklist {
        let empty = listbox_items(d.list_blacklist).is_empty();
        for &ctl in &d.static_blacklist_empty {
            ShowWindow(ctl, if empty { SW_SHOW } else { SW_HIDE });
        }
    }
}

/// Switch the active navigation section. Only child visibility changes; the
/// draft is untouched, so in-progress edits survive section switches.
unsafe fn select_section(d: &mut SettingsData, section: Section) {
    if d.section == section {
        return;
    }
    d.section = section;
    apply_section_visibility(d);
    show_inline_errors(d);
    InvalidateRect(d.hwnd, std::ptr::null(), 0);
    // Deterministic focus: if the focused control belonged to the now-hidden
    // section, move focus to the rail/strip so Up/Down keeps working.
    let focused = GetFocus();
    if focused == 0 || !tab_cycle(d).contains(&focused) {
        SetFocus(d.hwnd);
    }
}

/// The section containing a validation field, if the field is exposed.
fn section_of_field(field: &str) -> Option<Section> {
    match field {
        "volume_step" | "volume_step_large" | "overlay_duration_ms" => Some(Section::General),
        "beep.blocked_freq"
        | "beep.blocked_duration_ms"
        | "beep.limit_freq"
        | "beep.limit_duration_ms" => Some(Section::Feedback),
        _ => None,
    }
}

/// Show inline errors for the draft's stored validation failure (if any),
/// limited to the active section so errors never float over other content.
/// Inline errors use the row's helper slot; the helper itself stays visible
/// until a failing Apply replaces it.
fn show_inline_errors(d: &mut SettingsData) {
    let error = d.draft.error().cloned();
    for (i, field) in INLINE_ERROR_FIELDS.iter().enumerate() {
        let visible = section_of_field(field) == Some(d.section)
            && error.as_ref().is_some_and(|ve| ve.field == *field);
        if let Some(ve) = &error {
            if ve.field == *field {
                set_control_text(d.error_statics[i], &ve.message);
            }
        }
        unsafe {
            ShowWindow(d.error_statics[i], if visible { SW_SHOW } else { SW_HIDE });
        }
    }
}

/// The user edited a field: drop a stale validation error for THAT field (the
/// documented `SettingsDraft::set_current` semantic) and hide its inline
/// error; the next Apply revalidates.
fn clear_field_error(d: &mut SettingsData, field: &str) {
    if d.draft.error().is_some_and(|ve| ve.field == field) {
        let current = d.draft.current().clone();
        d.draft.set_current(current);
        show_inline_errors(d);
    }
}

/// Move keyboard focus to the offending field on a validation failure.
fn focus_field(d: &SettingsData, field: &str) {
    let target = match field {
        "volume_step" => d.edit_volume_step,
        "volume_step_large" => d.edit_volume_step_large,
        "overlay_duration_ms" => d.edit_overlay_ms,
        "beep.blocked_freq" => d.edit_blocked_freq,
        "beep.blocked_duration_ms" => d.edit_blocked_dur,
        "beep.limit_freq" => d.edit_limit_freq,
        "beep.limit_duration_ms" => d.edit_limit_dur,
        _ => 0,
    };
    if target != 0 {
        unsafe {
            SetFocus(target);
        }
    }
}

/// Validation failure handling: switch to the offending field's section (so
/// the inline error is visible), then focus the field itself.
unsafe fn focus_validation_field(d: &mut SettingsData, field: &str) {
    if let Some(section) = section_of_field(field) {
        select_section(d, section);
    }
    focus_field(d, field);
}

/// An appearance combo changed: mirror the selection into the draft's working
/// copy (draft-only — nothing is persisted until Apply) so the Appearance
/// preview tracks the user's edits live, and repaint the preview card.
fn apply_appearance_combo(d: &mut SettingsData, id: isize) {
    let mut cfg = d.draft.current().clone();
    match id {
        ID_COMBO_THEME => cfg.appearance.theme = theme_from_index(combo_get_index(d.combo_theme)),
        ID_COMBO_MATERIAL => {
            cfg.appearance.material = material_from_index(combo_get_index(d.combo_material))
        }
        ID_COMBO_MOTION => {
            cfg.appearance.motion = motion_from_index(combo_get_index(d.combo_motion))
        }
        ID_COMBO_ACCENT => {
            cfg.appearance.accent = accent_from_index(combo_get_index(d.combo_accent))
        }
        _ => return,
    }
    d.draft.set_current(cfg);
    unsafe {
        if d.preview != 0 {
            InvalidateRect(d.preview, std::ptr::null(), 0);
        }
    }
}

/// Keep the Save changes button honest: enabled exactly when the draft is
/// dirty or the controls differ from the draft's working copy (spec §7.4:
/// "Save is disabled when the draft is clean").
fn sync_apply_enabled(d: &SettingsData) {
    let values = read_controls(d);
    let from_controls = control_values_to_config(d.draft.current(), &values);
    let enabled = d.draft.is_dirty() || from_controls != *d.draft.current();
    unsafe {
        EnableWindow(d.btn_apply, if enabled { 1 } else { 0 });
    }
}

/// Merge the recommended blacklist for the modifier currently selected in the
/// combo into the working copy (in-memory only; persisted on Apply).
fn merge_recommended(d: &mut SettingsData) {
    let modifier = modifier_from_index(combo_get_index(d.combo_modifier));
    let recommended = crate::config::recommended_blacklist(modifier);
    if recommended.is_empty() {
        return;
    }
    let mut cfg = d.draft.current().clone();
    let mut added = false;
    for app in recommended {
        if !cfg.blacklist.contains(&app) {
            cfg.blacklist.push(app);
            added = true;
        }
    }
    if added {
        d.draft.set_current(cfg);
        populate_blacklist(d);
        sync_apply_enabled(d);
    }
}

fn add_entry(d: &mut SettingsData, name: &str) {
    let entry = normalize_blacklist_entry(name);
    if entry.is_empty() {
        return;
    }
    let mut cfg = d.draft.current().clone();
    if !cfg.blacklist.contains(&entry) {
        cfg.blacklist.push(entry);
        d.draft.set_current(cfg);
        populate_blacklist(d);
        sync_apply_enabled(d);
    }
}

fn remove_entry(d: &mut SettingsData, name: &str) {
    let mut cfg = d.draft.current().clone();
    let before = cfg.blacklist.len();
    cfg.blacklist.retain(|entry| entry != name);
    if cfg.blacklist.len() != before {
        d.draft.set_current(cfg);
        populate_blacklist(d);
        sync_apply_enabled(d);
    }
}

fn clear_entries(d: &mut SettingsData) {
    let mut cfg = d.draft.current().clone();
    if !cfg.blacklist.is_empty() {
        cfg.blacklist.clear();
        d.draft.set_current(cfg);
        populate_blacklist(d);
        sync_apply_enabled(d);
    }
}

// ── Appearance preview (draft-driven, isolated) ─────────────────────────────

/// Resolve preview tokens from the CURRENT DRAFT appearance, never the host's
/// confirmed config. The window's resolved tokens supply the high-contrast
/// and system-darkness facts; only Apply persists the draft, so the preview
/// never mutates host state.
fn preview_tokens(
    appearance: &AppearanceConfig,
    high_contrast: bool,
    system_is_dark: Option<bool>,
) -> ThemeTokens {
    tokens_for(
        appearance.theme,
        high_contrast,
        appearance.accent,
        move || system_is_dark,
    )
}

/// Pure paint plan for the mini Signal Rail preview card: draft tokens plus
/// the fixed 60% rail geometry.
struct PreviewPlan {
    tokens: ThemeTokens,
    rail: SignalRail,
    track: TrackRect,
    geometry: SignalRailGeometry,
}

fn preview_plan(d: &SettingsData) -> PreviewPlan {
    let draft = d.draft.current();
    let tokens = preview_tokens(
        &draft.appearance,
        d.appearance.tokens.high_contrast,
        Some(d.appearance.tokens.is_dark),
    );
    let rail = SignalRail::new(
        PREVIEW_PERCENT,
        false,
        tokens.volume_threshold,
        draft.color_thresholds.green_up_to,
        draft.color_thresholds.blue_up_to,
    );
    let track = TrackRect {
        left: 8.0,
        right: PREVIEW_W - 8.0,
        top: 14.0,
        bottom: 22.0,
    };
    let geometry = rail_geometry(&rail, track, PREVIEW_THUMB_RADIUS, PREVIEW_THUMB_RADIUS);
    PreviewPlan {
        tokens,
        rail,
        track,
        geometry,
    }
}

/// Paint the preview card: background token surface, 1px border, an 8px
/// Signal Rail at the fixed preview percent, and the thumb in the DRAFT
/// accent (theme and accent are the visible draft signals).
unsafe fn paint_preview(canvas: &mut PaintCanvas, d: &SettingsData) {
    let plan = preview_plan(d);
    let t = &plan.tokens;
    canvas.fill_rounded_rect(
        RectF::new(0.0, 0.0, PREVIEW_W, PREVIEW_H),
        t.radii.card_px,
        t.background,
    );
    canvas.stroke_rounded_rect(
        RectF::new(0.5, 0.5, PREVIEW_W - 0.5, PREVIEW_H - 0.5),
        t.radii.card_px,
        t.border,
        1.0,
    );
    let track = plan.track;
    canvas.fill_rect(
        RectF::new(track.left, track.top, track.right, track.bottom),
        t.border,
    );
    if plan.geometry.fill_right > track.left {
        canvas.fill_rect(
            RectF::new(
                track.left,
                track.top,
                plan.geometry.fill_right,
                track.bottom,
            ),
            plan.rail.fill_color(),
        );
    }
    if let MarkerGeometry::Thumb {
        center_x,
        center_y,
        radius,
    } = plan.geometry.marker
    {
        let center = PointF::new(center_x, center_y);
        canvas.fill_circle(center, radius, t.accent);
        canvas.stroke_circle(center, radius, t.signal_glass().border_strong, 1.0);
    }
}

// ── Theming ────────────────────────────────────────────────────────────────

/// Apply a resolved adaptive appearance: rebuild the token-coloured brushes,
/// re-apply the DWM material treatment, re-theme the child controls, and
/// repaint the window + children. Skipped when nothing changed.
unsafe fn apply_appearance(hwnd: HWND, d: &mut SettingsData, appearance: SettingsAppearance) {
    if d.appearance == appearance {
        return;
    }
    if d.bg != 0 {
        DeleteObject(d.bg);
    }
    if d.accent_brush != 0 {
        DeleteObject(d.accent_brush);
    }
    d.appearance = appearance;
    d.bg = CreateSolidBrush(colorref(appearance.tokens.background));
    d.accent_brush = CreateSolidBrush(colorref(appearance.tokens.accent));
    apply_backdrop(hwnd, appearance.material, appearance.tokens.is_dark);
    theme_controls(&d.tab_order, appearance.tokens.is_dark);
    RedrawWindow(
        hwnd,
        std::ptr::null(),
        0,
        RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN,
    );
}

/// Colour a STATIC control by role: section titles in primary text, inline
/// errors and the error status line in the error token, everything else in
/// secondary text.
fn static_color(d: &SettingsData, ctl: HWND) -> crate::ui::Rgba {
    let id = unsafe { GetDlgCtrlID(ctl) } as isize;
    if id == ID_ST_STATUS {
        match d.status_kind {
            StatusKind::Error => d.appearance.tokens.error.text,
            _ => d.appearance.tokens.text_secondary,
        }
    } else if (ID_ERR_VOL_STEP..=ID_ERR_LIMIT_DUR).contains(&id) {
        d.appearance.tokens.error.text
    } else if (ID_HDR_GENERAL..=ID_HDR_STORAGE).contains(&id) {
        d.appearance.tokens.text_primary
    } else {
        d.appearance.tokens.text_secondary
    }
}

// ── Subclassed keyboard navigation ─────────────────────────────────────────

/// Save the original window proc of a control and install the shared
/// [`settings_child_wndproc`] subclass.
unsafe fn subclass(d: &mut SettingsData, ctl: HWND) {
    let orig = GetWindowLongPtrW(ctl, GWLP_WNDPROC);
    d.orig_procs
        .push(Some(std::mem::transmute::<isize, ChildWndProc>(orig)));
    d.tab_order.push(ctl);
    let subclass_proc = settings_child_wndproc as ChildWndProc;
    SetWindowLongPtrW(ctl, GWLP_WNDPROC, subclass_proc as usize as isize);
}

/// Shared subclass proc for every interactive settings control.
///
/// Native behaviour is preserved for everything not handled here (buttons
/// respond to Enter/Space, edits to typing, combos to arrows). This subclass
/// adds:
///   - Escape hides the window (identical semantics to `WM_CLOSE`);
///   - Tab / Shift+Tab move focus through the deterministic cycle
///     (rail → active section → footer → Close), skipping hidden sections.
unsafe extern "system" fn settings_child_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let parent = GetAncestor(hwnd, GA_PARENT);
    if parent == 0 {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    if (msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN) && (wparam as u32) == (VK_ESCAPE as u32) {
        SendMessageW(parent, WM_CLOSE, 0, 0);
        return 0;
    }

    if msg == WM_KEYDOWN && (wparam as u32) == (VK_TAB as u32) {
        let d = &*(GetWindowLongPtrW(parent, GWLP_USERDATA) as *const SettingsData);
        let cycle = tab_cycle(d);
        let n = cycle.len();
        if n > 0 {
            let cur = cycle.iter().position(|&c| c == hwnd).unwrap_or(0);
            let backwards = GetKeyState(VK_SHIFT as i32) < 0;
            let next = if backwards {
                (cur + n - 1) % n
            } else {
                (cur + 1) % n
            };
            SetFocus(cycle[next]);
        }
        return 0;
    }

    let d = &*(GetWindowLongPtrW(parent, GWLP_USERDATA) as *const SettingsData);
    // Owner-drawn close button hover tracking: the owner paints the hover
    // face, so the button must repaint when the pointer enters/leaves (the
    // native hover the button had before owner-drawing). The window proc runs
    // on the owning thread, so mutating the per-window state here is safe.
    if hwnd == d.btn_close {
        match msg {
            WM_MOUSEMOVE => {
                if !d.close_hover {
                    (&mut *(GetWindowLongPtrW(parent, GWLP_USERDATA) as *mut SettingsData))
                        .close_hover = true;
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                    let mut tme: TRACKMOUSEEVENT = std::mem::zeroed();
                    tme.cbSize = std::mem::size_of::<TRACKMOUSEEVENT>() as u32;
                    tme.dwFlags = TME_LEAVE;
                    tme.hwndTrack = hwnd;
                    TrackMouseEvent(&mut tme);
                }
            }
            WM_MOUSELEAVE if d.close_hover => {
                (&mut *(GetWindowLongPtrW(parent, GWLP_USERDATA) as *mut SettingsData))
                    .close_hover = false;
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            _ => {}
        }
    }
    if let Some(idx) = d.tab_order.iter().position(|&c| c == hwnd) {
        if let Some(proc) = d.orig_procs[idx] {
            return CallWindowProcW(Some(proc), hwnd, msg, wparam, lparam);
        }
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

// ── Window proc ────────────────────────────────────────────────────────────

/// Position every child at PHYSICAL coordinates for `dpi` from the logical
/// layout (the mixer's DpiMetrics path, Task 6).
unsafe fn layout_children(d: &SettingsData, layout: &SettingsLayout, dpi: DpiMetrics) {
    let place = |ctl: HWND, rect: RectF| {
        if ctl != 0 {
            SetWindowPos(
                ctl,
                0,
                dpi.to_physical(rect.left.round() as i32),
                dpi.to_physical(rect.top.round() as i32),
                dpi.to_physical(rect.width().round() as i32),
                dpi.to_physical(rect.height().round() as i32),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    };
    let mut by_id: HashMap<isize, HWND> = HashMap::new();
    for &(id, hwnd) in &d.static_handles {
        by_id.insert(id, hwnd);
    }
    for (id, hwnd) in interactive_handles(d) {
        by_id.insert(id, hwnd);
    }
    for (id, rect) in child_rects(layout) {
        if let Some(ctl) = by_id.get(&id) {
            place(*ctl, rect);
        }
    }
    place(d.preview, layout.appearance.preview);
}

/// Whether a point (logical px) lies inside a rect (inclusive left/top,
/// exclusive right/bottom).
fn rect_contains(rect: RectF, x: f32, y: f32) -> bool {
    x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
}

/// Recompute the layout from the actual client size: derive the logical size
/// from the physical client rect, reposition every child (including the
/// preview card), re-apply section visibility, and repaint.
///
/// Called from `show` (the window is created at its final size, so
/// `SetWindowPos` does not resend `WM_SIZE`) and from `WM_SIZE` (so a
/// work-area-clamped or resized surface falls back to the stacked selector
/// layout automatically).
unsafe fn relayout(d: &mut SettingsData) {
    let mut rc: RECT = std::mem::zeroed();
    GetClientRect(d.hwnd, &mut rc);
    let w = rc.right - rc.left;
    let h = rc.bottom - rc.top;
    if w <= 0 || h <= 0 {
        return;
    }
    let dpi = DpiMetrics::new(d.dpi);
    d.logical_w = dpi.to_logical(w);
    d.logical_h = dpi.to_logical(h);
    let layout = SettingsLayout::new(d.logical_w.max(MIN_W) as f32, d.logical_h.max(MIN_H) as f32);
    layout_children(d, &layout, dpi);
    apply_section_visibility(d);
    show_inline_errors(d);
    InvalidateRect(d.hwnd, std::ptr::null(), 0);
}

/// Paint one settings frame: header band, body, sticky footer, and the
/// navigation (vertical rail on the desktop, stacked selector strip when
/// narrow). All coordinates are logical; the canvas scales them to physical
/// pixels exactly once via its DPI metrics.
///
/// The parent's full-client fill covers the native children; they are
/// invalidated at the end so their chrome repaints on top (mixer pattern).
unsafe fn paint(canvas: &mut PaintCanvas, d: &SettingsData) {
    let w = d.logical_w.max(MIN_W) as f32;
    let h = d.logical_h.max(MIN_H) as f32;
    let layout = SettingsLayout::new(w, h);
    let tokens = &d.appearance.tokens;
    let glass = tokens.signal_glass();

    // Header band: elevated surface, 3px accent rail, title + subtitle,
    // divider line under the band.
    canvas.fill_rect(RectF::new(0.0, 0.0, w, HEADER_H), tokens.surface_elevated);
    canvas.fill_rect(RectF::new(0.0, 0.0, w, 3.0), tokens.accent);
    canvas.fill_rect(RectF::new(0.0, HEADER_H, w, HEADER_H + 1.0), tokens.border);
    canvas.draw_text(&TextLayout {
        text: "Settings",
        rect: layout.title_rect,
        align: TextAlign::Left,
        role: tokens.typography.surface_title,
        color: tokens.text_primary,
    });
    canvas.draw_text(&TextLayout {
        text: "Configure how VolumeControl behaves",
        rect: layout.subtitle_rect,
        align: TextAlign::Left,
        role: tokens.typography.caption,
        color: tokens.text_secondary,
    });

    // Body + sticky footer bands.
    canvas.fill_rect(
        RectF::new(0.0, HEADER_H, w, layout.footer_top),
        tokens.background,
    );
    canvas.fill_rect(
        RectF::new(0.0, layout.footer_top, w, h),
        tokens.surface_elevated,
    );
    canvas.fill_rect(
        RectF::new(0.0, layout.footer_top - 1.0, w, layout.footer_top),
        tokens.border,
    );

    // Navigation: selected entry = accent/elevated surface + primary text +
    // a 3px accent rail (desktop) — shape + text, never tint-only, so the
    // selection stays visible in high contrast (which collapses surfaces and
    // text but preserves the accent rail and the filled block).
    for (i, entry) in layout.nav_entries.iter().enumerate() {
        let selected = i == d.section.index();
        if selected {
            canvas.fill_rounded_rect(*entry, tokens.radii.control_px, glass.selected_surface);
            if !layout.narrow {
                canvas.fill_rect(
                    RectF::new(
                        entry.left,
                        entry.top + 4.0,
                        entry.left + 3.0,
                        entry.bottom - 4.0,
                    ),
                    tokens.accent,
                );
            }
        }
        let label_rect = if layout.narrow {
            RectF::new(
                entry.left + 8.0,
                entry.top + 7.0,
                entry.right - 8.0,
                entry.bottom - 7.0,
            )
        } else {
            RectF::new(
                entry.left + 14.0,
                entry.top + 10.0,
                entry.right - 8.0,
                entry.bottom - 10.0,
            )
        };
        canvas.draw_text(&TextLayout {
            text: Section::from_index(i).title(),
            rect: label_rect,
            align: TextAlign::Left,
            role: tokens.typography.body,
            color: if selected {
                glass.selected_text
            } else {
                tokens.text_secondary
            },
        });
    }
    // Two-layer focus ring on the selected entry while the rail has focus.
    if GetFocus() == d.hwnd {
        canvas.draw_focus_ring(layout.nav_entries[d.section.index()], &tokens.focus);
    }

    // Desktop rail divider.
    if !layout.narrow {
        canvas.fill_rect(
            RectF::new(RAIL_W - 1.0, HEADER_H, RAIL_W, layout.footer_top),
            tokens.border,
        );
    }

    // Repaint the children the parent fill just covered.
    for &(_, ctl) in &d.static_handles {
        InvalidateRect(ctl, std::ptr::null(), 0);
    }
    for (_, ctl) in interactive_handles(d) {
        InvalidateRect(ctl, std::ptr::null(), 0);
    }
    InvalidateRect(d.preview, std::ptr::null(), 0);
}

unsafe extern "system" fn settings_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let userdata = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if userdata == 0 {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    match msg {
        WM_COMMAND => {
            let d = &mut *(userdata as *mut SettingsData);
            let id = (wparam & 0xFFFF) as isize;
            let code = (wparam >> 16) as u32;
            if code == BN_CLICKED {
                match id {
                    // Persistence intents go to the host; the host owns all
                    // config/hotkey mutation and reports the result back.
                    ID_BTN_APPLY => {
                        PostMessageW(d.host, WM_APP_SETTINGS_APPLY, 0, 0);
                        return 0;
                    }
                    ID_BTN_RESET => {
                        PostMessageW(d.host, WM_APP_SETTINGS_RESET, 0, 0);
                        return 0;
                    }
                    ID_BTN_CANCEL => {
                        PostMessageW(d.host, WM_APP_SETTINGS_CANCEL, 0, 0);
                        return 0;
                    }
                    ID_BTN_CLOSE => {
                        SendMessageW(hwnd, WM_CLOSE, 0, 0);
                        return 0;
                    }
                    ID_BTN_OPEN_CONFIG => {
                        // Opening the config is a host-owned action (the host's
                        // `OpenConfigLocation` also shows the overlay toast), so
                        // it is routed through the host like every other intent
                        // instead of being invoked from the window directly.
                        PostMessageW(d.host, WM_APP_SETTINGS_OPEN_CONFIG, 0, 0);
                        return 0;
                    }
                    // Blacklist edits update the in-memory draft (the window's
                    // edit buffer); nothing touches disk until Apply.
                    ID_BTN_BLACKLIST_ADD => {
                        let name = get_control_text(d.edit_blacklist_new);
                        let entry = normalize_blacklist_entry(&name);
                        if !entry.is_empty() {
                            add_entry(d, &entry);
                        }
                        set_control_text(d.edit_blacklist_new, "");
                        SetFocus(d.edit_blacklist_new);
                        return 0;
                    }
                    ID_BTN_BLACKLIST_REMOVE => {
                        if let Some(name) = listbox_selected_text(d.list_blacklist) {
                            remove_entry(d, &name);
                        }
                        return 0;
                    }
                    ID_BTN_BLACKLIST_CLEAR => {
                        clear_entries(d);
                        return 0;
                    }
                    ID_BTN_BLACKLIST_RECOMMEND => {
                        merge_recommended(d);
                        return 0;
                    }
                    ID_CHK_BEEP => {
                        // The checkbox is a draft edit: keep Apply's enabled
                        // state honest.
                        sync_apply_enabled(d);
                        return 0;
                    }
                    _ => {}
                }
                return DefWindowProcW(hwnd, msg, wparam, lparam);
            }
            if code == CBN_SELCHANGE {
                // Appearance combos mirror into the draft immediately so the
                // live preview tracks them (draft-only; Apply persists).
                // The modifier combo and the Save button state follow the
                // same path — edits land on the next Apply.
                if matches!(
                    id,
                    ID_COMBO_THEME | ID_COMBO_MATERIAL | ID_COMBO_MOTION | ID_COMBO_ACCENT
                ) {
                    apply_appearance_combo(d, id);
                }
                sync_apply_enabled(d);
                return 0;
            }
            if code == EN_CHANGE {
                // Editing a field with a stale validation error clears that
                // field's inline error (the next Apply revalidates).
                if let Some(field) = field_for_control(id) {
                    clear_field_error(d, field);
                }
                sync_apply_enabled(d);
                return 0;
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_CTLCOLORSTATIC => {
            let d = &*(userdata as *const SettingsData);
            let hdc = wparam as HDC;
            let ctl = lparam as HWND;
            SetBkMode(hdc, TRANSPARENT as i32);
            SetTextColor(hdc, colorref(static_color(d, ctl)));
            d.bg as LRESULT
        }
        WM_DRAWITEM => {
            // The owner-drawn close button (spec §11.2): its window text is
            // the UIA name `Close settings`; this paints the approved `×`
            // visual (system button face + glyph + hover/pressed/focus via
            // `paint_close_button`).
            let d = &*(userdata as *const SettingsData);
            if paint_close_button(lparam as *const DRAWITEMSTRUCT, d.close_hover) {
                0
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
        WM_ERASEBKGND => 1,
        WM_SIZE => {
            // The layout mode (desktop rail vs stacked selector strip) and
            // every child position are recomputed from the actual client size
            // — a work-area-clamped or resized surface automatically falls
            // back to the narrow layout.
            let d = &mut *(userdata as *mut SettingsData);
            relayout(d);
            0
        }
        WM_PAINT => {
            // Paint through the resource-safe canvas (Task 3): it owns the
            // BeginPaint/EndPaint pair, selects ONE paint path per frame
            // (Direct2D when available, GDI otherwise), and deletes every
            // per-call GDI object. If BeginPaint itself fails, paint nothing
            // and invalidate so the next WM_PAINT retries.
            if let Some(mut canvas) = PaintCanvas::begin_paint(hwnd) {
                let d = &*(userdata as *const SettingsData);
                paint(&mut canvas, d);
            } else {
                log::debug!("settings: BeginPaint failed; invalidating for a retry");
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            0
        }
        // ── Rail/strip navigation + keyboard focus on the window itself ──
        WM_LBUTTONDOWN => {
            let d = &mut *(userdata as *mut SettingsData);
            let x = (lparam & 0xFFFF) as i16 as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
            let dpi = DpiMetrics::new(d.dpi);
            let (x, y) = (dpi.to_logical(x), dpi.to_logical(y));
            let layout =
                SettingsLayout::new(d.logical_w.max(MIN_W) as f32, d.logical_h.max(MIN_H) as f32);
            for (i, entry) in layout.nav_entries.iter().enumerate() {
                if rect_contains(*entry, x as f32, y as f32) {
                    select_section(d, Section::from_index(i));
                    SetFocus(d.hwnd);
                    return 0;
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_KEYDOWN => {
            let d = &mut *(userdata as *mut SettingsData);
            match wparam as u16 {
                // ── Escape hides (identical semantics to WM_CLOSE) ──────
                VK_ESCAPE => {
                    d.open = false;
                    ShowWindow(hwnd, SW_HIDE);
                }
                // Tab / Enter move from the rail into the active section.
                VK_TAB | VK_RETURN => {
                    if let Some(&first) = d.section_tabs[d.section.index()].first() {
                        SetFocus(first);
                    }
                }
                // Up/Down navigate the vertical rail; Left/Right navigate the
                // horizontal stacked selector strip.
                VK_UP if !d.narrow() => select_section(d, d.section.previous()),
                VK_DOWN if !d.narrow() => select_section(d, d.section.next()),
                VK_LEFT if d.narrow() => select_section(d, d.section.previous()),
                VK_RIGHT if d.narrow() => select_section(d, d.section.next()),
                _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
            }
            0
        }
        // ── Close (Esc / X / Close button) just hides ────────────────────
        WM_CLOSE => {
            let d = &mut *(userdata as *mut SettingsData);
            d.open = false;
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        // Deferred backdrop re-apply armed by `show` (see the comment there):
        // DWM re-asserts DWMSBT_AUTO after a show while High Contrast is
        // active, so the resolved backdrop is re-asserted once the composition
        // has settled, keeping the opaque painted surface visible.
        WM_TIMER if wparam == BACKDROP_TIMER_ID => {
            KillTimer(hwnd, BACKDROP_TIMER_ID);
            let d = &mut *(userdata as *mut SettingsData);
            apply_backdrop(hwnd, d.appearance.material, d.appearance.tokens.is_dark);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// The config field an interactive control edits, if the field has inline
/// validation (matches `INLINE_ERROR_FIELDS`).
fn field_for_control(id: isize) -> Option<&'static str> {
    match id {
        ID_VOL_STEP => Some("volume_step"),
        ID_VOL_STEP_LARGE => Some("volume_step_large"),
        ID_OVERLAY_MS => Some("overlay_duration_ms"),
        ID_BLOCKED_FREQ => Some("beep.blocked_freq"),
        ID_BLOCKED_DUR => Some("beep.blocked_duration_ms"),
        ID_LIMIT_FREQ => Some("beep.limit_freq"),
        ID_LIMIT_DUR => Some("beep.limit_duration_ms"),
        _ => None,
    }
}

/// Window proc of the custom-painted Appearance preview child. It reads the
/// parent's [`SettingsData`] on paint, so the preview always reflects the
/// CURRENT DRAFT appearance.
unsafe extern "system" fn preview_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            if let Some(mut canvas) = PaintCanvas::begin_paint(hwnd) {
                let parent = GetAncestor(hwnd, GA_PARENT);
                if parent != 0 {
                    let d = &*(GetWindowLongPtrW(parent, GWLP_USERDATA) as *const SettingsData);
                    paint_preview(&mut canvas, d);
                }
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::accent_color_for;
    use crate::ui::WorkArea;

    fn control_values() -> ControlValues {
        ControlValues {
            volume_step: 2,
            volume_step_large: 10,
            overlay_duration_ms: 1800,
            modifier: HotkeyModifier::CtrlAlt,
            theme: ThemeMode::System,
            material: MaterialMode::Auto,
            motion: MotionMode::Full,
            accent: AccentMode::System,
            beep_enabled: true,
            blocked_freq: 400,
            blocked_duration_ms: 80,
            limit_freq: 600,
            limit_duration_ms: 60,
            blacklist: vec!["chrome.exe".to_string()],
        }
    }

    /// Create a fully wired hidden settings window (no `show`, so no window
    /// appears and focus is never stolen during tests).
    fn hidden_window() -> Settings {
        let settings = Settings::new(0).expect("settings window creates");
        unsafe {
            let d = &mut *(GetWindowLongPtrW(settings.hwnd, GWLP_USERDATA) as *mut SettingsData);
            d.draft = SettingsDraft::new(Config::default());
            populate_controls(d);
            set_status(d, StatusKind::None, "");
            sync_apply_enabled(d);
            show_inline_errors(d);
            d.logical_w = WIN_W;
            d.logical_h = WIN_H;
            let dpi = DpiMetrics::new(1.0);
            let layout = SettingsLayout::new(WIN_W as f32, WIN_H as f32);
            layout_children(d, &layout, dpi);
            apply_section_visibility(d);
        }
        settings
    }

    fn data(settings: &Settings) -> &SettingsData {
        unsafe { &*(GetWindowLongPtrW(settings.hwnd, GWLP_USERDATA) as *const SettingsData) }
    }

    fn data_mut(settings: &mut Settings) -> &mut SettingsData {
        unsafe { &mut *(GetWindowLongPtrW(settings.hwnd, GWLP_USERDATA) as *mut SettingsData) }
    }

    /// The child's own WS_VISIBLE bit (IsWindowVisible also requires every
    /// ancestor visible, which never holds for the hidden test window).
    fn child_visible(ctl: HWND) -> bool {
        unsafe { GetWindowLongPtrW(ctl, GWL_STYLE) & WS_VISIBLE as isize != 0 }
    }

    // ── Existing pure tests (kept) ────────────────────────────────────────

    #[test]
    fn control_values_map_into_config_and_preserve_unexposed_fields() {
        let base = Config::default();
        let mut values = control_values();
        values.volume_step = 5;
        values.volume_step_large = 25;
        values.overlay_duration_ms = 3000;
        values.modifier = HotkeyModifier::Ctrl;
        values.theme = ThemeMode::Dark;
        values.material = MaterialMode::Translucent;
        values.motion = MotionMode::Reduced;
        values.accent = AccentMode::Purple;
        values.beep_enabled = false;
        values.blocked_freq = 500;
        values.blocked_duration_ms = 100;
        values.limit_freq = 700;
        values.limit_duration_ms = 90;
        values.blacklist = vec!["msedge.exe".to_string()];

        let cfg = control_values_to_config(&base, &values);

        assert_eq!(cfg.volume_step, 5);
        assert_eq!(cfg.volume_step_large, 25);
        assert_eq!(cfg.overlay_duration_ms, 3000);
        assert_eq!(cfg.modifier, HotkeyModifier::Ctrl);
        assert_eq!(cfg.appearance.theme, ThemeMode::Dark);
        assert_eq!(cfg.appearance.material, MaterialMode::Translucent);
        assert_eq!(cfg.appearance.motion, MotionMode::Reduced);
        assert_eq!(cfg.appearance.accent, AccentMode::Purple);
        assert!(!cfg.beep.enabled);
        assert_eq!(cfg.beep.blocked_freq, 500);
        assert_eq!(cfg.beep.blocked_duration_ms, 100);
        assert_eq!(cfg.beep.limit_freq, 700);
        assert_eq!(cfg.beep.limit_duration_ms, 90);
        assert_eq!(cfg.blacklist, vec!["msedge.exe".to_string()]);
        // Fields the window does not expose are preserved from the base.
        assert_eq!(cfg.color_thresholds, base.color_thresholds);
    }

    #[test]
    fn invalid_control_values_fail_validation_and_keep_the_draft_intact() {
        let mut draft = SettingsDraft::new(Config::default());
        let mut values = control_values();
        values.volume_step = 30;
        values.volume_step_large = 29; // must be strictly larger
        let invalid = control_values_to_config(&Config::default(), &values);

        draft.set_current(invalid.clone());
        let before = draft.current().clone();

        let error = draft.validate().expect_err("invalid draft must fail");
        assert_eq!(error.field, "volume_step_large");

        // commit validates before persisting, so the draft keeps the edits.
        let error = draft.commit().expect_err("invalid draft must not persist");
        assert!(matches!(error, ConfigError::Validation(_)));
        assert_eq!(draft.current(), &before, "edits stay intact");
        assert!(draft.is_dirty());
    }

    #[test]
    fn valid_control_values_pass_validation_and_normalize_for_save() {
        let mut values = control_values();
        values.volume_step = 4;
        values.volume_step_large = 12;
        values.blacklist = vec!["  Chrome.EXE  ".to_string()];

        // The window's mapping produces a config that passes strict validation
        // (the same gate `save_validated` applies before any disk write).
        let cfg = control_values_to_config(&Config::default(), &values);
        crate::config::validate(&cfg).expect("control values are valid");

        // Normalization (lowercased/trimmed blacklist) matches what the
        // persisted config would adopt on a successful commit.
        let normalized = crate::config::normalize(cfg.clone());
        assert_eq!(normalized.volume_step, 4);
        assert_eq!(normalized.blacklist, vec!["chrome.exe".to_string()]);

        // The draft round-trips the working copy and stays dirty-free.
        let mut draft = SettingsDraft::new(Config::default());
        draft.set_current(cfg);
        assert!(draft.validate().is_ok());
        assert!(draft.is_dirty());
    }

    #[test]
    fn enum_index_mappings_round_trip() {
        for m in [
            HotkeyModifier::CtrlAlt,
            HotkeyModifier::Alt,
            HotkeyModifier::Ctrl,
            HotkeyModifier::CapsLock,
        ] {
            assert_eq!(modifier_from_index(modifier_index(m) as i32), m);
        }
        for t in [ThemeMode::System, ThemeMode::Light, ThemeMode::Dark] {
            assert_eq!(theme_from_index(theme_index(t) as i32), t);
        }
        for m in [
            MaterialMode::Auto,
            MaterialMode::Translucent,
            MaterialMode::Opaque,
        ] {
            assert_eq!(material_from_index(material_index(m) as i32), m);
        }
        for m in [MotionMode::Full, MotionMode::Reduced, MotionMode::Disabled] {
            assert_eq!(motion_from_index(motion_index(m) as i32), m);
        }
        for a in [
            AccentMode::System,
            AccentMode::Blue,
            AccentMode::Green,
            AccentMode::Purple,
            AccentMode::Orange,
        ] {
            assert_eq!(accent_from_index(accent_index(a) as i32), a);
        }
    }

    #[test]
    fn combo_defaults_are_first_item() {
        // A combo with no selection (index -1) maps to the default enum.
        assert_eq!(modifier_from_index(-1), HotkeyModifier::CtrlAlt);
        assert_eq!(theme_from_index(-1), ThemeMode::System);
        assert_eq!(material_from_index(-1), MaterialMode::Auto);
        assert_eq!(motion_from_index(-1), MotionMode::Full);
        assert_eq!(accent_from_index(-1), AccentMode::System);
    }

    #[test]
    fn blacklist_entry_normalization_trims_lowercases_and_ensures_exe() {
        assert_eq!(normalize_blacklist_entry("Chrome"), "chrome.exe");
        assert_eq!(normalize_blacklist_entry("  code.EXE  "), "code.exe");
        assert_eq!(normalize_blacklist_entry("notepad++.exe"), "notepad++.exe");
        assert_eq!(normalize_blacklist_entry(""), "");
        assert_eq!(normalize_blacklist_entry("   "), "");
    }

    #[test]
    fn centered_placement_uses_the_work_area() {
        let wa = WorkArea::new(0, 0, 2560, 1400);
        let rect = place_centered(wa, SurfaceSize::new(WIN_W, WIN_H));
        assert_eq!(rect.width(), WIN_W);
        assert_eq!(rect.height(), WIN_H);
        assert!(rect.left >= wa.x && rect.top >= wa.y);
    }

    #[test]
    fn settings_window_constructs_and_drops_without_crashing() {
        // Smoke guard for the Drop path: create a real (hidden) Win32 window
        // and destroy it. `destroy()` destroys the subclassed children BEFORE
        // freeing state (see mixer for the same ordering), which this exercises
        // deterministically.
        let settings = Settings::new(0).expect("settings window creates");
        drop(settings);
    }

    // ── Navigation rail ──────────────────────────────────────────────────

    #[test]
    fn section_index_round_trip_and_wrap_navigation() {
        for (i, section) in Section::ALL.iter().enumerate() {
            assert_eq!(section.index(), i);
            assert_eq!(Section::from_index(i), *section);
        }
        assert_eq!(Section::General.previous(), Section::Storage);
        assert_eq!(Section::Storage.next(), Section::General);
        assert_eq!(Section::Appearance.next(), Section::Blacklist);
        assert_eq!(Section::Appearance.previous(), Section::Hotkeys);
    }

    #[test]
    fn switching_sections_keeps_the_draft_and_toggles_only_active_controls() {
        let mut settings = hidden_window();
        settings.add_blacklist_entry("chrome.exe");
        let d = data_mut(&mut settings);
        assert!(d.draft.is_dirty(), "blacklist edit dirties the draft");

        // Only the General section is visible initially.
        assert!(child_visible(d.edit_volume_step));
        assert!(!child_visible(d.combo_theme));
        assert!(!child_visible(d.chk_beep));

        unsafe {
            select_section(d, Section::Appearance);
        }
        assert_eq!(d.section, Section::Appearance);
        assert!(
            !child_visible(d.edit_volume_step),
            "General controls hide when Appearance is selected"
        );
        assert!(child_visible(d.combo_theme));
        assert!(child_visible(d.combo_material));
        assert!(child_visible(d.combo_accent));
        assert!(!child_visible(d.chk_beep));
        assert!(
            d.draft.is_dirty(),
            "switching sections must never lose draft state"
        );

        // Blacklist section shows its controls; the list already holds
        // chrome.exe, so the empty-state copy stays hidden.
        unsafe {
            select_section(d, Section::Blacklist);
        }
        assert!(child_visible(d.list_blacklist));
        assert!(!child_visible(d.static_blacklist_empty[0]));
        assert!(!child_visible(d.combo_theme));

        // Removing the last entry restores the empty-state copy.
        settings.remove_blacklist_entry("chrome.exe");
        let d = data_mut(&mut settings);
        assert!(child_visible(d.static_blacklist_empty[0]));
        assert!(child_visible(d.static_blacklist_empty[1]));

        // A populated blacklist hides it again.
        settings.add_blacklist_entry("code.exe");
        let d = data_mut(&mut settings);
        assert!(!child_visible(d.static_blacklist_empty[0]));

        // Switching back to General restores its controls.
        unsafe {
            select_section(d, Section::General);
        }
        assert!(child_visible(d.edit_volume_step));
        assert!(!child_visible(d.combo_theme));
        assert_eq!(d.draft.current().blacklist, vec!["code.exe"]);
    }

    #[test]
    fn tab_cycle_is_rail_section_footer_close_and_follows_the_selection() {
        let mut settings = hidden_window();
        let d = data_mut(&mut settings);

        // General: rail, the three General controls, then Reset/Cancel/Save/Close.
        let cycle = tab_cycle(d);
        assert_eq!(
            cycle,
            vec![
                d.hwnd,
                d.edit_volume_step,
                d.edit_volume_step_large,
                d.edit_overlay_ms,
                d.btn_reset,
                d.btn_cancel,
                d.btn_apply,
                d.btn_close,
            ]
        );

        // Hotkeys: only the modifier combo sits between the rail and footer.
        unsafe {
            select_section(d, Section::Hotkeys);
        }
        let cycle = tab_cycle(d);
        assert_eq!(
            cycle,
            vec![
                d.hwnd,
                d.combo_modifier,
                d.btn_reset,
                d.btn_cancel,
                d.btn_apply,
                d.btn_close
            ]
        );

        // The same cycle structure holds in the narrow layout (geometry only).
        d.logical_w = MIN_W;
        d.logical_h = MIN_H;
        assert!(d.narrow());
        let cycle = tab_cycle(d);
        assert_eq!(cycle[0], d.hwnd);
        assert_eq!(cycle[cycle.len() - 1], d.btn_close);
    }

    // ── Inline validation ─────────────────────────────────────────────────

    #[test]
    fn apply_validation_failure_shows_inline_error_and_keeps_the_draft() {
        let mut settings = hidden_window();
        let original = data(&settings).draft.original().clone();
        let error_idx = INLINE_ERROR_FIELDS
            .iter()
            .position(|&f| f == "volume_step_large")
            .unwrap();

        // Enter values that fail strict validation (30 is not < 29).
        {
            let d = data_mut(&mut settings);
            set_control_text(d.edit_volume_step, "30");
            set_control_text(d.edit_volume_step_large, "29");
        }

        // The window posts Apply to the host; the host reports the result
        // back through on_apply_result (the same contract in app.rs).
        let error = settings.apply().expect_err("invalid draft must fail");
        assert!(matches!(error, ConfigError::Validation(_)));
        settings.on_apply_result(&Err(error));
        let d = data_mut(&mut settings);
        // The invalid edits are preserved (not reverted), and the baseline is
        // untouched — the draft stays visible/editable after a failed Apply.
        assert_eq!(d.draft.current().volume_step, 30u32, "edits stay intact");
        assert_eq!(
            d.draft.current().volume_step_large,
            29u32,
            "edits stay intact"
        );
        assert_eq!(d.draft.original(), &original, "baseline untouched");
        assert!(d.draft.is_dirty());

        // The inline error is visible next to the offending field...
        assert!(child_visible(d.error_statics[error_idx]));
        assert_eq!(
            get_control_text(d.error_statics[error_idx]),
            "must be greater than volume_step"
        );
        // ...and the bottom status line reports the same failure globally.
        assert_eq!(d.status_kind, StatusKind::Error);

        // Editing the offending field clears its stale inline error
        // (the EN_CHANGE path), while the draft edit itself stays.
        set_control_text(d.edit_volume_step_large, "31");
        clear_field_error(d, "volume_step_large");
        assert!(!child_visible(d.error_statics[error_idx]));
        assert_eq!(
            d.draft.current().volume_step_large,
            29u32,
            "control edits only land on Apply"
        );
    }

    #[test]
    fn clean_draft_has_no_inline_errors_and_success_keeps_them_hidden() {
        let mut settings = hidden_window();
        for st in data(&settings).error_statics {
            assert!(!child_visible(st), "no inline errors on a clean draft");
        }
        // A successful apply result (the host committed) keeps errors hidden
        // and reports the saved note.
        let saved = data(&settings).draft.current().clone();
        settings.on_apply_result(&Ok(saved));
        let d = data(&settings);
        for st in d.error_statics {
            assert!(!child_visible(st));
        }
        assert_eq!(d.status_kind, StatusKind::Info);
        assert_eq!(get_control_text(d.static_status), "Changes saved.");
    }

    #[test]
    fn status_line_exposes_status_and_alert_semantics_for_screen_readers() {
        // Spec §11.2: the status line must be readable as
        // `Status: Changes saved` / `Alert: Volume step must be lower than
        // large volume step`. The UIA mirror is a native STATIC parked
        // outside the client area: it stays in the UIA tree (WS_VISIBLE)
        // without painting anything, while the visible line keeps the plain
        // text.
        let mut settings = hidden_window();
        let d = data(&settings);
        assert_eq!(
            get_control_text(d.static_status_uia),
            "",
            "no status yet → no announcement"
        );
        // The mirror is a visible (UIA-tree) child parked outside the client
        // area: in the tree, never painted.
        assert!(
            child_visible(d.static_status_uia),
            "mirror must be WS_VISIBLE"
        );
        unsafe {
            let mut mirror_rc: RECT = std::mem::zeroed();
            let mut parent_rc: RECT = std::mem::zeroed();
            assert_ne!(GetWindowRect(d.static_status_uia, &mut mirror_rc), 0);
            assert_ne!(GetWindowRect(settings.hwnd, &mut parent_rc), 0);
            assert!(
                mirror_rc.right <= parent_rc.left
                    || mirror_rc.left >= parent_rc.right
                    || mirror_rc.bottom <= parent_rc.top
                    || mirror_rc.top >= parent_rc.bottom,
                "mirror must sit outside the client area"
            );
        }

        // A failed Apply reports the failure as an Alert...
        {
            let d = data_mut(&mut settings);
            set_control_text(d.edit_volume_step, "30");
            set_control_text(d.edit_volume_step_large, "29");
        }
        let error = settings.apply().expect_err("invalid draft must fail");
        settings.on_apply_result(&Err(error));
        let d = data(&settings);
        assert_eq!(d.status_kind, StatusKind::Error);
        assert_eq!(
            get_control_text(d.static_status_uia),
            "Alert: volume_step_large: must be greater than volume_step",
            "the alert phrasing mirrors the error (spec §11.2)"
        );
        // ...while the visible status line keeps the plain text.
        assert_eq!(
            get_control_text(d.static_status),
            "volume_step_large: must be greater than volume_step"
        );

        // A successful Apply reports the saved note as a Status.
        let saved = data(&settings).draft.current().clone();
        settings.on_apply_result(&Ok(saved));
        let d = data(&settings);
        assert_eq!(d.status_kind, StatusKind::Info);
        assert_eq!(
            get_control_text(d.static_status_uia),
            "Status: Changes saved.",
            "the status phrasing (spec §11.2)"
        );
        assert_eq!(get_control_text(d.static_status), "Changes saved.");
    }

    #[test]
    fn close_button_exposes_its_accessibility_name_and_owner_draw_style() {
        // Spec §11.2: the header close button exposes a real name while the
        // owner paints the `×` visual (BS_OWNERDRAW keeps the window text —
        // the UIA name — free of the glyph).
        let settings = hidden_window();
        let d = data(&settings);
        assert_eq!(
            get_control_text(d.btn_close),
            "Close settings",
            "close button UIA name"
        );
        unsafe {
            let style = GetWindowLongPtrW(d.btn_close, GWL_STYLE);
            assert_ne!(
                style & BS_OWNERDRAW as isize,
                0,
                "close button must be BS_OWNERDRAW"
            );
        }
    }

    #[test]
    fn validation_field_sections_cover_every_inline_field() {
        assert_eq!(section_of_field("volume_step"), Some(Section::General));
        assert_eq!(
            section_of_field("volume_step_large"),
            Some(Section::General)
        );
        assert_eq!(
            section_of_field("overlay_duration_ms"),
            Some(Section::General)
        );
        assert_eq!(
            section_of_field("beep.blocked_freq"),
            Some(Section::Feedback)
        );
        assert_eq!(
            section_of_field("beep.blocked_duration_ms"),
            Some(Section::Feedback)
        );
        assert_eq!(section_of_field("beep.limit_freq"), Some(Section::Feedback));
        assert_eq!(
            section_of_field("beep.limit_duration_ms"),
            Some(Section::Feedback)
        );
        assert_eq!(section_of_field("anything_else"), None);
        // The inline-error table stays in sync with `crate::config::validate`.
        for field in INLINE_ERROR_FIELDS {
            assert!(section_of_field(field).is_some(), "{field}");
        }
    }

    // ── Appearance preview ────────────────────────────────────────────────

    #[test]
    fn preview_tokens_derive_from_the_draft_appearance() {
        let mut cfg = Config::default();
        cfg.appearance.theme = ThemeMode::Dark;
        cfg.appearance.accent = AccentMode::Orange;
        let tokens = preview_tokens(&cfg.appearance, false, None);
        assert!(tokens.is_dark, "draft theme drives the preview darkness");
        assert_eq!(
            tokens.accent,
            accent_color_for(AccentMode::Orange, true),
            "draft accent drives the preview accent"
        );
    }

    #[test]
    fn draft_accent_change_changes_the_preview_without_touching_the_host_config() {
        let host = Config::default();
        let mut draft_cfg = host.clone();
        draft_cfg.appearance.accent = AccentMode::Green;

        let host_tokens = preview_tokens(&host.appearance, false, Some(true));
        let draft_tokens = preview_tokens(&draft_cfg.appearance, false, Some(true));
        assert_eq!(
            host_tokens.accent,
            accent_color_for(AccentMode::System, true)
        );
        assert_eq!(
            draft_tokens.accent,
            accent_color_for(AccentMode::Green, true)
        );
        assert_ne!(
            host_tokens.accent, draft_tokens.accent,
            "the preview follows the draft, not the confirmed config"
        );
        // The host config is untouched by the preview derivation.
        assert_eq!(host.appearance.accent, AccentMode::System);
        assert_eq!(host.appearance.theme, ThemeMode::System);
    }

    #[test]
    fn appearance_combo_change_updates_the_draft_preview_without_touching_the_host_config() {
        let mut settings = hidden_window();
        let original = data(&settings).draft.original().clone();
        let preview_before = preview_plan(data(&settings));

        // A user picking "Orange" in the dropdown list sets the selection and
        // the combo delivers CBN_SELCHANGE to the parent (programmatic
        // CB_SETCURSEL does not). Deliver the same notification message.
        unsafe {
            let combo = data(&settings).combo_accent;
            SendMessageW(combo, CB_SETCURSEL, 4, 0);
            SendMessageW(
                settings.hwnd,
                WM_COMMAND,
                ((CBN_SELCHANGE as usize) << 16) | (ID_COMBO_ACCENT as usize),
                combo,
            );
        }
        let d = data_mut(&mut settings);
        assert_eq!(
            d.draft.current().appearance.accent,
            AccentMode::Orange,
            "the appearance combo edit lands in the draft working copy"
        );
        assert!(
            d.draft.is_dirty(),
            "a draft appearance edit dirties the draft"
        );
        assert_eq!(d.draft.original(), &original, "host baseline untouched");
        assert_eq!(original.appearance.accent, AccentMode::System);

        // The preview plan derives from the DRAFT, so its accent changed.
        let preview_after = preview_plan(d);
        assert_ne!(
            preview_after.tokens.accent, preview_before.tokens.accent,
            "the live preview tracks the draft accent"
        );
    }

    // ── Responsive geometry ───────────────────────────────────────────────

    fn assert_all_children_inside(layout: &SettingsLayout, label: &str) {
        let window = RectF::new(0.0, 0.0, layout.width, layout.height);
        for (id, rect) in child_rects(layout) {
            assert!(
                window.contains(rect),
                "{label}: id {id} at {rect:?} escapes the window"
            );
            if id == ID_ST_STATUS || (ID_BTN_APPLY..=ID_BTN_CANCEL).contains(&id) {
                // The status line and the action buttons live INSIDE the
                // sticky footer band (their own containment is asserted below).
                assert!(
                    rect.top >= layout.footer_top,
                    "{label}: id {id} must sit in the footer"
                );
            } else if id == ID_BTN_CLOSE {
                // The close hit target lives in the header band.
                assert!(
                    rect.bottom <= HEADER_H,
                    "{label}: close target must sit in the header"
                );
            } else {
                assert!(
                    rect.top >= HEADER_H,
                    "{label}: id {id} at {rect:?} overlaps the header band"
                );
                assert!(
                    rect.bottom <= layout.footer_top,
                    "{label}: id {id} at {rect:?} overlaps the sticky footer"
                );
            }
        }
        for (i, entry) in layout.nav_entries.iter().enumerate() {
            assert!(
                window.contains(*entry),
                "{label}: nav {i} at {entry:?} escapes"
            );
            assert!(
                entry.top >= HEADER_H,
                "{label}: nav {i} overlaps the header"
            );
            assert!(
                entry.bottom <= layout.footer_top,
                "{label}: nav {i} overlaps the footer"
            );
        }
        assert!(
            window.contains(layout.appearance.preview),
            "{label}: preview card escapes"
        );
        // The footer buttons stay inside the sticky footer band.
        for rect in [layout.btn_reset, layout.btn_cancel, layout.btn_apply] {
            assert!(rect.top >= layout.footer_top, "{label}: button in footer");
            assert!(
                rect.bottom <= layout.height,
                "{label}: button inside window"
            );
            assert!(
                rect.left >= 0.0 && rect.right <= layout.width,
                "{label}: button width"
            );
        }
    }

    #[test]
    fn desktop_layout_places_every_control_within_the_window() {
        for (w, h) in [(WIN_W as f32, WIN_H as f32), (WIN_W as f32, MIN_H as f32)] {
            let layout = SettingsLayout::new(w, h);
            assert!(!layout.narrow, "desktop width keeps the rail");
            assert_all_children_inside(&layout, &format!("desktop {w}x{h}"));
        }
    }

    #[test]
    fn narrow_layout_places_every_control_within_the_minimum_window() {
        let layout = SettingsLayout::new(MIN_W as f32, MIN_H as f32);
        assert!(
            layout.narrow,
            "minimum width uses the stacked selector strip"
        );
        assert_all_children_inside(&layout, "narrow 620x520");
    }

    #[test]
    fn layout_turns_narrow_below_the_desktop_width() {
        assert!(!SettingsLayout::new(760.0, 620.0).narrow);
        assert!(SettingsLayout::new(759.0, 620.0).narrow);
        assert!(SettingsLayout::new(700.0, 600.0).narrow);
        assert!(SettingsLayout::new(620.0, 520.0).narrow);
    }

    #[test]
    fn narrow_strip_entries_fill_the_width_without_overlapping() {
        let layout = SettingsLayout::new(MIN_W as f32, MIN_H as f32);
        assert!(layout.narrow);
        let mut prev_right = 0.0f32;
        for (i, entry) in layout.nav_entries.iter().enumerate() {
            assert!(
                entry.left >= 16.0 && entry.right <= MIN_W as f32 - 16.0,
                "entry {i}"
            );
            assert!(entry.left >= prev_right, "entries must not overlap");
            prev_right = entry.right;
        }
    }

    // ── Window lifecycle ──────────────────────────────────────────────────

    /// The preview card's rect relative to the settings window's origin.
    fn preview_relative_rect(settings: &Settings) -> RectF {
        unsafe {
            let d = &*(GetWindowLongPtrW(settings.hwnd, GWLP_USERDATA) as *const SettingsData);
            let mut p_rect: RECT = std::mem::zeroed();
            let mut w_rect: RECT = std::mem::zeroed();
            GetWindowRect(d.preview, &mut p_rect);
            GetWindowRect(settings.hwnd, &mut w_rect);
            RectF::new(
                (p_rect.left - w_rect.left) as f32,
                (p_rect.top - w_rect.top) as f32,
                (p_rect.right - w_rect.left) as f32,
                (p_rect.bottom - w_rect.top) as f32,
            )
        }
    }

    #[test]
    fn relayout_positions_the_preview_card_in_both_layout_modes() {
        // Regression guard for the live-verified bug where the preview card
        // stayed at its creation origin (0,0): `show` sizes the window to its
        // final size, so SetWindowPos does NOT resend WM_SIZE and the layout
        // must be applied explicitly.
        let settings = Settings::new(0).expect("settings window creates");
        unsafe {
            let d = &mut *(GetWindowLongPtrW(settings.hwnd, GWLP_USERDATA) as *mut SettingsData);
            d.dpi = 1.0;
            d.logical_w = WIN_W;
            d.logical_h = WIN_H;
            relayout(d);
        }
        let desktop = SettingsLayout::new(WIN_W as f32, WIN_H as f32);
        assert_eq!(
            preview_relative_rect(&settings),
            desktop.appearance.preview,
            "desktop: the preview card must land in the Appearance section slot"
        );

        // Resize the hidden window to the minimum size and re-layout: the
        // narrow stacked-selector layout moves the preview with the content.
        unsafe {
            SetWindowPos(
                settings.hwnd,
                0,
                0,
                0,
                MIN_W,
                MIN_H,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        let narrow = SettingsLayout::new(MIN_W as f32, MIN_H as f32);
        assert!(narrow.narrow);
        assert_eq!(
            preview_relative_rect(&settings),
            narrow.appearance.preview,
            "narrow: the preview card must land in the Appearance section slot"
        );
        drop(settings);
    }

    /// Whether a control is enabled (WS_DISABLED clear).
    fn enabled(ctl: HWND) -> bool {
        unsafe { GetWindowLongPtrW(ctl, GWL_STYLE) & WS_DISABLED as isize == 0 }
    }

    #[test]
    fn save_button_is_disabled_when_the_draft_is_clean_and_tracks_edits() {
        let mut settings = hidden_window();
        {
            let d = data(&settings);
            assert!(child_visible(d.btn_apply), "footer is always visible");
            // Clean draft + controls matching the draft → disabled (spec §7.4).
            assert!(!enabled(d.btn_apply), "Save changes disabled while clean");
        }
        // A draft edit (blacklist op) enables it.
        settings.add_blacklist_entry("chrome.exe");
        let d = data(&settings);
        assert!(enabled(d.btn_apply));
        // Typing in a control enables it too (the EN_CHANGE sync path).
        {
            let d = data_mut(&mut settings);
            set_control_text(d.edit_volume_step, "5");
            sync_apply_enabled(d);
        }
        let d = data(&settings);
        assert!(enabled(d.btn_apply));
        // Cancel re-enables the clean state.
        settings.cancel();
        let d = data(&settings);
        assert!(!enabled(d.btn_apply));
    }

    #[test]
    fn draft_and_window_state_are_consistent_after_blacklist_ops() {
        let mut settings = hidden_window();
        settings.add_blacklist_entry("Chrome");
        let d = data_mut(&mut settings);
        assert_eq!(d.draft.current().blacklist, vec!["chrome.exe"]);
        settings.remove_blacklist_entry("chrome.exe");
        let d = data_mut(&mut settings);
        assert!(d.draft.current().blacklist.is_empty());
        assert!(
            !d.draft.is_dirty(),
            "removing the only edit restores the baseline"
        );
    }
}
