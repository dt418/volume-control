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
//! Layout is fixed-pixel (matching the mixer/help/overlay pattern); the app is
//! system-DPI virtualized, so Windows scales the whole surface uniformly at
//! 125%/150% without double-scaling. Theming uses the shared adaptive tokens
//! (`tokens_for` + `primitives::{apply_backdrop, theme_controls, colorref}`),
//! so light/dark/high-contrast and the resolved material all follow the
//! confirmed appearance preferences with one resolution point in the host.

use windows_sys::core::w;
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{
        BeginPaint, CreateFontW, CreateSolidBrush, DeleteObject, EndPaint, FillRect,
        InvalidateRect, RedrawWindow, SetBkMode, SetTextColor, HBRUSH, HDC, HFONT, PAINTSTRUCT,
        RDW_ALLCHILDREN, RDW_ERASE, RDW_INVALIDATE, TRANSPARENT,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::Input::KeyboardAndMouse::{GetKeyState, SetFocus, VK_ESCAPE, VK_SHIFT, VK_TAB},
    UI::WindowsAndMessaging::{
        CallWindowProcW, CreateWindowExW, DefWindowProcW, DestroyWindow, GetAncestor,
        GetDlgCtrlID, GetWindowLongPtrW, PostMessageW, RegisterClassW, SendMessageW,
        SetWindowLongPtrW, SetWindowPos, ShowWindow, BN_CLICKED, CW_USEDEFAULT, GA_PARENT,
        GWLP_USERDATA, GWLP_WNDPROC, HWND_TOPMOST, SW_HIDE, SWP_SHOWWINDOW, WM_CLOSE, WM_COMMAND,
        WM_CTLCOLORSTATIC, WM_ERASEBKGND, WM_KEYDOWN, WM_PAINT, WM_SYSKEYDOWN, WM_USER, WNDCLASSW,
        WS_CHILD, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
    },
};

use crate::config::{Config, ConfigError, HotkeyModifier};
use crate::ui::primitives::{apply_backdrop, colorref, theme_controls, work_area_for};
use crate::ui::{
    place_centered, resolve_material, tokens_for, AccentMode, MaterialMode, MotionMode,
    ResolvedMaterial, SettingsDraft, SurfaceSize, ThemeMode, ThemeTokens, UiCapabilities,
};

/// Custom messages the settings window posts to the host window (see `app.rs`).
/// The host owns all config/hotkey mutation; these intents just tell it which
/// draft action the user requested.
pub const WM_APP_SETTINGS_APPLY: u32 = WM_USER + 20;
pub const WM_APP_SETTINGS_CANCEL: u32 = WM_USER + 21;
pub const WM_APP_SETTINGS_RESET: u32 = WM_USER + 22;
pub const WM_APP_SETTINGS_OPEN_CONFIG: u32 = WM_USER + 23;

// ── Layout (design pixels; system-DPI virtualization scales these) ─────────
const WIN_W: i32 = 580;
const WIN_H: i32 = 636;

// Control-style bitmasks (canonical Win32 values; the windows-sys constants are
// typed i32, so local u32 copies keep the WS_CHILD | ... expressions uniform).
const ES_AUTOHSCROLL: u32 = 0x0080;
const ES_NUMBER: u32 = 0x2000;
const CBS_DROPDOWNLIST: u32 = 0x0003;
const LBS_NOTIFY: u32 = 0x0001;
const LBS_NOINTEGRALHEIGHT: u32 = 0x0100;
const BS_AUTOCHECKBOX: u32 = 0x0003;

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

// ── Static control ids (coloring + font dispatch) ─────────────────────────
const ID_HDR_GENERAL: isize = 200;
const ID_HDR_HOTKEYS: isize = 201;
const ID_HDR_APPEARANCE: isize = 202;
const ID_HDR_BLACKLIST: isize = 203;
const ID_HDR_FEEDBACK: isize = 204;
const ID_HDR_STORAGE: isize = 205;
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

/// Role of the status line shown at the bottom of the window.
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

/// Per-window state stored in GWLP_USERDATA.
struct SettingsData {
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
    // Blacklist
    list_blacklist: HWND,
    edit_blacklist_new: HWND,
    btn_blacklist_add: HWND,
    btn_blacklist_remove: HWND,
    btn_blacklist_clear: HWND,
    btn_blacklist_recommend: HWND,
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
    btn_apply: HWND,
    btn_reset: HWND,
    btn_cancel: HWND,
    btn_close: HWND,
    // Styling
    appearance: SettingsAppearance,
    bg: HBRUSH,
    accent_brush: HBRUSH,
    hfont_header: HFONT,
    hfont_body: HFONT,
    status_kind: StatusKind,
    open: bool,
    /// Focus order for Tab/Shift+Tab (parallel to `orig_procs`).
    tab_order: Vec<HWND>,
    /// Original window procs of the subclassed controls, so the subclass can
    /// forward everything it does not handle.
    orig_procs: Vec<Option<ChildWndProc>>,
}

impl SettingsData {
    fn placeholder(host: HWND) -> Self {
        Self {
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
            list_blacklist: 0,
            edit_blacklist_new: 0,
            btn_blacklist_add: 0,
            btn_blacklist_remove: 0,
            btn_blacklist_clear: 0,
            btn_blacklist_recommend: 0,
            chk_beep: 0,
            edit_blocked_freq: 0,
            edit_blocked_dur: 0,
            edit_limit_freq: 0,
            edit_limit_dur: 0,
            btn_open_config: 0,
            static_path: 0,
            static_status: 0,
            btn_apply: 0,
            btn_reset: 0,
            btn_cancel: 0,
            btn_close: 0,
            appearance: SettingsAppearance::placeholder(),
            bg: 0,
            accent_brush: 0,
            hfont_header: 0,
            hfont_body: 0,
            status_kind: StatusKind::None,
            open: false,
            tab_order: Vec::new(),
            orig_procs: Vec::new(),
        }
    }
}

pub struct Settings {
    hwnd: HWND,
}

/// Create a Segoe UI font at `height` pixels.
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
        windows_sys::core::w!("Segoe UI"),
    )
}

impl Settings {
    /// Create the hidden settings window (shown via `show`).
    pub fn new(host: HWND) -> Result<Settings, Box<dyn std::error::Error>> {
        unsafe {
            let hinst = GetModuleHandleW(std::ptr::null());
            let class = windows_sys::core::w!("VolCtlSettings");
            let mut wc: WNDCLASSW = std::mem::zeroed();
            wc.lpfnWndProc = Some(settings_wndproc);
            wc.hInstance = hinst;
            wc.lpszClassName = class;
            RegisterClassW(&wc);

            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                class,
                windows_sys::core::w!("VolumeControl Settings"),
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

            d.hfont_header = font(12, true);
            d.hfont_body = font(12, false);

            // ── General (left column) ────────────────────────────────────
            make_static(hwnd, hinst, w!("General"), 20, 16, 120, 20, ID_HDR_GENERAL, d.hfont_header);
            make_static(hwnd, hinst, w!("Volume step (1-50):"), 20, 40, 145, 20, ID_LBL_VOL_STEP, d.hfont_body);
            d.edit_volume_step = make_ctl(hwnd, hinst, w!("EDIT"), w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL | ES_NUMBER, 168, 36, 112, 24, ID_VOL_STEP);
            make_static(hwnd, hinst, w!("Large step (step+1..50):"), 20, 72, 145, 20, ID_LBL_VOL_STEP_LARGE, d.hfont_body);
            d.edit_volume_step_large = make_ctl(hwnd, hinst, w!("EDIT"), w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL | ES_NUMBER, 168, 68, 112, 24, ID_VOL_STEP_LARGE);
            make_static(hwnd, hinst, w!("Overlay (ms):"), 20, 104, 145, 20, ID_LBL_OVERLAY, d.hfont_body);
            d.edit_overlay_ms = make_ctl(hwnd, hinst, w!("EDIT"), w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL | ES_NUMBER, 168, 100, 112, 24, ID_OVERLAY_MS);

            // ── Hotkeys / conflicts ──────────────────────────────────────
            make_static(hwnd, hinst, w!("Hotkeys / Conflicts"), 20, 144, 180, 20, ID_HDR_HOTKEYS, d.hfont_header);
            make_static(hwnd, hinst, w!("Modifier:"), 20, 168, 90, 20, ID_LBL_MODIFIER, d.hfont_body);
            d.combo_modifier = make_ctl(hwnd, hinst, w!("COMBOBOX"), w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | CBS_DROPDOWNLIST, 120, 164, 160, 180, ID_COMBO_MODIFIER);
            d.static_hotkey_status = make_static(hwnd, hinst, w!(""), 20, 198, 260, 28, ID_ST_HOTKEY_STATUS, d.hfont_body);

            // ── Appearance ───────────────────────────────────────────────
            make_static(hwnd, hinst, w!("Appearance"), 20, 240, 120, 20, ID_HDR_APPEARANCE, d.hfont_header);
            make_static(hwnd, hinst, w!("Theme:"), 20, 264, 80, 20, ID_LBL_THEME, d.hfont_body);
            d.combo_theme = make_ctl(hwnd, hinst, w!("COMBOBOX"), w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | CBS_DROPDOWNLIST, 110, 260, 170, 170, ID_COMBO_THEME);
            make_static(hwnd, hinst, w!("Material:"), 20, 296, 80, 20, ID_LBL_MATERIAL, d.hfont_body);
            d.combo_material = make_ctl(hwnd, hinst, w!("COMBOBOX"), w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | CBS_DROPDOWNLIST, 110, 292, 170, 170, ID_COMBO_MATERIAL);
            make_static(hwnd, hinst, w!("Motion:"), 20, 328, 80, 20, ID_LBL_MOTION, d.hfont_body);
            d.combo_motion = make_ctl(hwnd, hinst, w!("COMBOBOX"), w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | CBS_DROPDOWNLIST, 110, 324, 170, 170, ID_COMBO_MOTION);
            make_static(hwnd, hinst, w!("Accent:"), 20, 360, 80, 20, ID_LBL_ACCENT, d.hfont_body);
            d.combo_accent = make_ctl(hwnd, hinst, w!("COMBOBOX"), w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | CBS_DROPDOWNLIST, 110, 356, 170, 170, ID_COMBO_ACCENT);

            // ── Blacklist (right column) ─────────────────────────────────
            make_static(hwnd, hinst, w!("Blacklist"), 300, 16, 120, 20, ID_HDR_BLACKLIST, d.hfont_header);
            d.list_blacklist = make_ctl(hwnd, hinst, w!("LISTBOX"), w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_VSCROLL | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT,
                300, 40, 260, 130, ID_LIST_BLACKLIST);
            d.edit_blacklist_new = make_ctl(hwnd, hinst, w!("EDIT"), w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL, 300, 180, 155, 24, ID_EDIT_BLACKLIST);
            d.btn_blacklist_add = make_ctl(hwnd, hinst, w!("BUTTON"), w!("Add"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP, 463, 178, 97, 28, ID_BTN_BLACKLIST_ADD);
            d.btn_blacklist_remove = make_ctl(hwnd, hinst, w!("BUTTON"), w!("Remove"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP, 300, 214, 120, 28, ID_BTN_BLACKLIST_REMOVE);
            d.btn_blacklist_clear = make_ctl(hwnd, hinst, w!("BUTTON"), w!("Clear"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP, 426, 214, 134, 28, ID_BTN_BLACKLIST_CLEAR);
            d.btn_blacklist_recommend = make_ctl(hwnd, hinst, w!("BUTTON"), w!("Apply Recommended"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP, 300, 248, 260, 28, ID_BTN_BLACKLIST_RECOMMEND);

            // ── Feedback ─────────────────────────────────────────────────
            make_static(hwnd, hinst, w!("Feedback"), 300, 292, 120, 20, ID_HDR_FEEDBACK, d.hfont_header);
            d.chk_beep = make_ctl(hwnd, hinst, w!("BUTTON"), w!("Enable beep feedback"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_AUTOCHECKBOX, 300, 316, 220, 24, ID_CHK_BEEP);
            make_static(hwnd, hinst, w!("Blocked freq:"), 300, 348, 120, 20, ID_LBL_BLOCKED_FREQ, d.hfont_body);
            d.edit_blocked_freq = make_ctl(hwnd, hinst, w!("EDIT"), w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL | ES_NUMBER, 430, 344, 130, 24, ID_BLOCKED_FREQ);
            make_static(hwnd, hinst, w!("Blocked ms:"), 300, 380, 120, 20, ID_LBL_BLOCKED_DUR, d.hfont_body);
            d.edit_blocked_dur = make_ctl(hwnd, hinst, w!("EDIT"), w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL | ES_NUMBER, 430, 376, 130, 24, ID_BLOCKED_DUR);
            make_static(hwnd, hinst, w!("Limit freq:"), 300, 412, 120, 20, ID_LBL_LIMIT_FREQ, d.hfont_body);
            d.edit_limit_freq = make_ctl(hwnd, hinst, w!("EDIT"), w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL | ES_NUMBER, 430, 408, 130, 24, ID_LIMIT_FREQ);
            make_static(hwnd, hinst, w!("Limit ms:"), 300, 444, 120, 20, ID_LBL_LIMIT_DUR, d.hfont_body);
            d.edit_limit_dur = make_ctl(hwnd, hinst, w!("EDIT"), w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL | ES_NUMBER, 430, 440, 130, 24, ID_LIMIT_DUR);

            // ── Storage / actions ────────────────────────────────────────
            make_static(hwnd, hinst, w!("Storage"), 20, 488, 120, 20, ID_HDR_STORAGE, d.hfont_header);
            make_static(hwnd, hinst, w!("Config:"), 20, 512, 52, 20, ID_LBL_CONFIG, d.hfont_body);
            d.static_path = make_static(hwnd, hinst, w!(""), 76, 512, 484, 24, ID_ST_CONFIG_PATH, d.hfont_body);
            d.btn_open_config = make_ctl(hwnd, hinst, w!("BUTTON"), w!("Open Config"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP, 20, 544, 160, 28, ID_BTN_OPEN_CONFIG);
            d.static_status = make_static(hwnd, hinst, w!(""), 20, 578, 540, 22, ID_ST_STATUS, d.hfont_body);
            d.btn_apply = make_ctl(hwnd, hinst, w!("BUTTON"), w!("Apply"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP, 20, 608, 88, 28, ID_BTN_APPLY);
            d.btn_reset = make_ctl(hwnd, hinst, w!("BUTTON"), w!("Reset"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP, 114, 608, 88, 28, ID_BTN_RESET);
            d.btn_cancel = make_ctl(hwnd, hinst, w!("BUTTON"), w!("Cancel"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP, 372, 608, 88, 28, ID_BTN_CANCEL);
            d.btn_close = make_ctl(hwnd, hinst, w!("BUTTON"), w!("Close"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP, 466, 608, 88, 28, ID_BTN_CLOSE);

            // A failed child leaves the window unusable; tear down cleanly.
            if [
                d.edit_volume_step, d.edit_volume_step_large, d.edit_overlay_ms,
                d.combo_modifier, d.combo_theme, d.combo_material, d.combo_motion,
                d.combo_accent, d.list_blacklist, d.edit_blacklist_new,
                d.btn_blacklist_add, d.btn_blacklist_remove, d.btn_blacklist_clear,
                d.btn_blacklist_recommend, d.chk_beep, d.edit_blocked_freq,
                d.edit_blocked_dur, d.edit_limit_freq, d.edit_limit_dur,
                d.btn_open_config, d.btn_apply, d.btn_reset, d.btn_cancel, d.btn_close,
                d.static_path, d.static_status,
            ]
            .iter()
            .any(|&c| c == 0)
            {
                DestroyWindow(hwnd);
                drop(Box::from_raw(data));
                return Err("settings child control failed".into());
            }

            // Focus order for Tab/Shift+Tab (visual order).
            for ctl in [
                d.edit_volume_step, d.edit_volume_step_large, d.edit_overlay_ms,
                d.combo_modifier, d.combo_theme, d.combo_material, d.combo_motion,
                d.combo_accent, d.list_blacklist, d.edit_blacklist_new,
                d.btn_blacklist_add, d.btn_blacklist_remove, d.btn_blacklist_clear,
                d.btn_blacklist_recommend, d.chk_beep, d.edit_blocked_freq,
                d.edit_blocked_dur, d.edit_limit_freq, d.edit_limit_dur,
                d.btn_open_config, d.btn_apply, d.btn_reset, d.btn_cancel, d.btn_close,
            ] {
                subclass(d, ctl);
            }

            // Populate the combo list items (selection is set by
            // `populate_controls`).
            combo_add(d.combo_modifier, &["Ctrl+Alt", "Alt", "Ctrl", "CapsLock"]);
            combo_add(d.combo_theme, &["System", "Light", "Dark"]);
            combo_add(d.combo_material, &["Auto", "Translucent", "Opaque"]);
            combo_add(d.combo_motion, &["Full", "Reduced", "Disabled"]);
            combo_add(d.combo_accent, &["System", "Blue", "Green", "Purple", "Orange"]);

            // Seed from defaults so the hidden window is self-consistent before
            // the first `show(config)` re-seeds from the loaded config.
            d.draft = SettingsDraft::new(Config::default());
            populate_controls(d);
            set_status(d, StatusKind::None, "");
            set_control_text(d.static_path, &crate::config::config_path().display().to_string());

            // Initial adaptive styling from the placeholder appearance.
            let appearance = SettingsAppearance::placeholder();
            d.appearance = appearance;
            d.bg = CreateSolidBrush(colorref(appearance.tokens.background));
            d.accent_brush = CreateSolidBrush(colorref(appearance.tokens.accent));
            apply_backdrop(hwnd, appearance.material, appearance.tokens.is_dark);
            theme_controls(&d.tab_order, appearance.tokens.is_dark);

            Ok(Settings { hwnd })
        }
    }

    /// Seed a fresh draft from `config` and show the window.
    ///
    /// Opening always re-seeds from the authoritative config (edits from a
    /// previous session are not carried across a close/reopen — `Close` does
    /// not save). The host-resolved appearance themes the window.
    pub fn show(&mut self, config: &Config, appearance: &SettingsAppearance) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut SettingsData);
            apply_appearance(self.hwnd, d, *appearance);
            d.draft = SettingsDraft::new(config.clone());
            populate_controls(d);
            set_status(d, StatusKind::None, "");

            // Centered on the work area hosting the window.
            let work_area = work_area_for(self.hwnd);
            let rect = place_centered(work_area, SurfaceSize::new(WIN_W, WIN_H));
            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                rect.left,
                rect.top,
                rect.width(),
                rect.height(),
                SWP_SHOWWINDOW,
            );
            d.open = true;
            SetFocus(d.edit_volume_step);
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
    ///   inline and move focus to the offending field. The window stays open.
    /// - I/O / serialization failure: validation passed but persistence failed;
    ///   keep the edits and show a "could not save" message.
    pub fn on_apply_result(&mut self, result: &Result<Config, ConfigError>) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut SettingsData);
            match result {
                Ok(_) => {
                    populate_controls(d);
                    set_status(d, StatusKind::Info, "Settings saved.");
                    InvalidateRect(self.hwnd, std::ptr::null(), 0);
                }
                Err(e) => match e {
                    ConfigError::Validation(ve) => {
                        set_status(d, StatusKind::Error, &ve.to_string());
                        focus_field(d, ve.field);
                    }
                    ConfigError::Io(ie) => {
                        set_status(d, StatusKind::Error, &format!("Could not save config: {ie}"));
                    }
                    ConfigError::Serialization(se) => {
                        set_status(d, StatusKind::Error, &format!("Could not save config: {se}"));
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
            if d.bg != 0 {
                DeleteObject(d.bg);
            }
            if d.accent_brush != 0 {
                DeleteObject(d.accent_brush);
            }
            if d.hfont_header != 0 {
                DeleteObject(d.hfont_header);
            }
            if d.hfont_body != 0 {
                DeleteObject(d.hfont_body);
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
) -> HWND {
    let ctl = CreateWindowExW(
        0,
        windows_sys::core::w!("STATIC"),
        text,
        WS_CHILD | WS_VISIBLE,
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
        SendMessageW(hwnd, WM_GETTEXT, (len + 1) as usize, buf.as_mut_ptr() as isize);
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

/// Read every editable control into a [`ControlValues`] snapshot.
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
    set_control_text(d.edit_blocked_dur, &cfg.beep.blocked_duration_ms.to_string());
    set_control_text(d.edit_limit_freq, &cfg.beep.limit_freq.to_string());
    set_control_text(d.edit_limit_dur, &cfg.beep.limit_duration_ms.to_string());
    populate_blacklist(d);
    set_control_text(
        d.static_hotkey_status,
        &format!("Modifier: {} — hotkeys re-register on Apply.", modifier_label(cfg.modifier)),
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
}

fn set_status(d: &mut SettingsData, kind: StatusKind, text: &str) {
    d.status_kind = kind;
    set_control_text(d.static_status, text);
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
    }
}

fn remove_entry(d: &mut SettingsData, name: &str) {
    let mut cfg = d.draft.current().clone();
    let before = cfg.blacklist.len();
    cfg.blacklist.retain(|entry| entry != name);
    if cfg.blacklist.len() != before {
        d.draft.set_current(cfg);
        populate_blacklist(d);
    }
}

fn clear_entries(d: &mut SettingsData) {
    let mut cfg = d.draft.current().clone();
    if !cfg.blacklist.is_empty() {
        cfg.blacklist.clear();
        d.draft.set_current(cfg);
        populate_blacklist(d);
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

/// Colour a STATIC control by role: section headers in the resolved accent,
/// the status line by its current kind (error / info), everything else in
/// secondary text.
fn static_color(d: &SettingsData, ctl: HWND) -> crate::ui::Rgba {
    let id = unsafe { GetDlgCtrlID(ctl) } as isize;
    if (ID_HDR_GENERAL..=ID_HDR_STORAGE).contains(&id) {
        d.appearance.tokens.accent
    } else if id == ID_ST_STATUS {
        match d.status_kind {
            StatusKind::Error => d.appearance.tokens.error.text,
            _ => d.appearance.tokens.text_secondary,
        }
    } else {
        d.appearance.tokens.text_secondary
    }
}

// ── Subclassed keyboard navigation ─────────────────────────────────────────

/// Save the original window proc of a control and install the shared
/// [`settings_child_wndproc`] subclass.
unsafe fn subclass(d: &mut SettingsData, ctl: HWND) {
    let orig = GetWindowLongPtrW(ctl, GWLP_WNDPROC);
    d.orig_procs.push(Some(std::mem::transmute::<isize, ChildWndProc>(orig)));
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
///   - Tab / Shift+Tab move focus among the controls in the stored tab order.
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
        let n = d.tab_order.len();
        if n > 0 {
            let cur = d.tab_order.iter().position(|&c| c == hwnd).unwrap_or(0);
            let backwards = GetKeyState(VK_SHIFT as i32) < 0;
            let next = if backwards { (cur + n - 1) % n } else { (cur + 1) % n };
            SetFocus(d.tab_order[next]);
        }
        return 0;
    }

    let d = &*(GetWindowLongPtrW(parent, GWLP_USERDATA) as *const SettingsData);
    if let Some(idx) = d.tab_order.iter().position(|&c| c == hwnd) {
        if let Some(proc) = d.orig_procs[idx] {
            return CallWindowProcW(Some(proc), hwnd, msg, wparam, lparam);
        }
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

// ── Window proc ────────────────────────────────────────────────────────────

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
            if code == BN_CLICKED as u32 {
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
                    _ => {}
                }
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
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            let d = &*(userdata as *const SettingsData);
            let rect = RECT {
                left: 0,
                top: 0,
                right: WIN_W,
                bottom: WIN_H,
            };
            FillRect(hdc, &rect, d.bg);
            // Accent bar along the top (mirrors the mixer/help surfaces).
            let bar = RECT {
                left: 0,
                top: 0,
                right: WIN_W,
                bottom: 3,
            };
            FillRect(hdc, &bar, d.accent_brush);
            EndPaint(hwnd, &ps);
            0
        }
        // ── Keyboard navigation when the window itself has focus ─────────
        WM_KEYDOWN | WM_SYSKEYDOWN if (wparam as u32) == (VK_ESCAPE as u32) => {
            let d = &mut *(userdata as *mut SettingsData);
            d.open = false;
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        WM_KEYDOWN if (wparam as u32) == (VK_TAB as u32) => {
            // Tab from the window body moves into the first control.
            let d = &*(userdata as *const SettingsData);
            if !d.tab_order.is_empty() {
                SetFocus(d.tab_order[0]);
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
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert_eq!(cfg.beep.enabled, false);
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

        // The draft round-trips the working copy and stays clean-free.
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
        for m in [MaterialMode::Auto, MaterialMode::Translucent, MaterialMode::Opaque] {
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
}
