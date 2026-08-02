//! Volume Mixer — a Win11-style flyout with a native trackbar slider.
//!
//! Captionless, always-on-top tool window, DWM-styled like the system flyout:
//! rounded corners (33), immersive dark mode (20), and the system backdrop
//! (38) — all driven by the shared adaptive tokens and the capability-resolved
//! material treatment (Tasks 3–5) instead of local hardcoded colours. Contains:
//!   - a small accent bar along the top
//!   - "System Volume" + live percentage labels
//!   - a trackbar (0–100, no ticks) — dragging changes volume live
//!   - Mute / Unmute and Reset to 50% buttons
//!   - a visible `×` close button that hides (not destroys) the flyout
//!
//! Placement is the bottom-right of the *monitor work area* hosting the window,
//! computed through [`crate::ui::surface::place_mixer_above_overlay`] so the
//! mixer shares the overlay's right edge and sits exactly 16px above its top.
//!
//! Keyboard navigation: the interactive controls (slider + three buttons) are
//! subclassed so that Escape hides the flyout, Tab/Shift+Tab move focus among
//! them, Enter/Space activate the focused button (native `BN_CLICKED`), and
//! focus changes repaint the parent which draws a token-coloured focus ring
//! around the focused control.
//!
//! User interaction (slider drag, buttons) posts [`WM_APP_MIXER_*`] messages
//! to the host window, which owns the audio backend. The host maps each
//! message to a shared [`crate::ui::AppAction`] (`SetVolumePercent` /
//! `ToggleMute` / `ResetVolume`) and dispatches it through its central action
//! handler, which mutates audio and publishes confirmed state.

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{
        BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, FrameRect, InvalidateRect,
        MapWindowPoints, SetBkMode, SetTextColor, TextOutW, HBRUSH, HDC, PAINTSTRUCT, TRANSPARENT,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::Controls::{InitCommonControlsEx, ICC_BAR_CLASSES, INITCOMMONCONTROLSEX},
    UI::Input::KeyboardAndMouse::{GetFocus, GetKeyState, SetFocus, VK_ESCAPE, VK_SHIFT, VK_TAB},
    UI::WindowsAndMessaging::{
        CallWindowProcW, CreateWindowExW, DefWindowProcW, DestroyWindow, GetAncestor,
        GetWindowLongPtrW, GetWindowRect, PostMessageW, RegisterClassW, SendMessageW,
        SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow, BN_CLICKED, CW_USEDEFAULT,
        GA_PARENT, GWLP_USERDATA, GWLP_WNDPROC, HWND_TOPMOST, SWP_NOACTIVATE, SWP_SHOWWINDOW,
        SW_HIDE, WM_CLOSE, WM_COMMAND, WM_CTLCOLORSTATIC, WM_ERASEBKGND, WM_HSCROLL, WM_KEYDOWN,
        WM_KILLFOCUS, WM_PAINT, WM_SETFOCUS, WM_SYSKEYDOWN, WM_USER, WNDCLASSW, WNDPROC, WS_CHILD,
        WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_TABSTOP, WS_VISIBLE,
    },
};

use crate::audio::VolumeState;
use crate::config::Config;
use crate::overlay::{OVERLAY_HEIGHT, OVERLAY_MARGIN_X, OVERLAY_MARGIN_Y, OVERLAY_WIDTH};
use crate::ui::primitives::{apply_backdrop, colorref, theme_controls, work_area_for};
use crate::ui::{
    place_mixer_above_overlay, resolve_material, tokens_for, AccentMode, ResolvedMaterial,
    SurfaceSize, ThemeMode, ThemeTokens, UiCapabilities,
};

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

// Indexes into `MixerData::orig_procs`, kept in sync with the tab order
// (slider -> mute -> reset -> close).
const IDX_SLIDER: usize = 0;
const IDX_MUTE: usize = 1;
const IDX_RESET: usize = 2;
const IDX_CLOSE: usize = 3;

/// The window-proc signature of the subclassed interactive controls.
type ChildWndProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

/// Adaptive appearance resolved by the host and applied by the mixer.
///
/// The host resolves this once per `sync`/`toggle` from the confirmed
/// appearance preferences and the display-session capability snapshot, so the
/// mixer stays a dumb consumer with a single resolution point in `app.rs` —
/// the same seam Task 7 established for the overlay.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MixerAppearance {
    /// Resolved palette tokens (theme + high-contrast + accent).
    pub tokens: ThemeTokens,
    /// Capability-resolved material treatment (blur/translucent/opaque).
    pub material: ResolvedMaterial,
}

impl MixerAppearance {
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

    /// Placeholder used before the first `sync`/`toggle` (the window is hidden
    /// then, so this is never painted).
    fn placeholder() -> Self {
        Self {
            tokens: tokens_for(ThemeMode::System, false, AccentMode::System, || None),
            material: ResolvedMaterial::Opaque,
        }
    }
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
    reset_btn: HWND,
    close_btn: HWND,
    /// Original window procs of the subclassed interactive controls, saved so
    /// [`mixer_child_wndproc`] can forward everything it does not handle.
    orig_procs: [Option<ChildWndProc>; 4],
    accent: HBRUSH,
    background: HBRUSH,
    appearance: MixerAppearance,
    muted: bool,
    open: bool,
}

impl MixerData {
    fn placeholder(host: HWND) -> Self {
        Self {
            host,
            slider: 0,
            percent_label: 0,
            mute_btn: 0,
            reset_btn: 0,
            close_btn: 0,
            orig_procs: [None; 4],
            accent: 0,
            background: 0,
            appearance: MixerAppearance::placeholder(),
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
            let data = Box::into_raw(Box::new(MixerData::placeholder(host)));
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, data as isize);

            // Trackbar slider. WS_TABSTOP makes it part of the keyboard focus
            // order (Tab navigation via the child subclass).
            let slider = CreateWindowExW(
                0,
                windows_sys::core::w!("msctls_trackbar32"),
                windows_sys::core::w!("slider"),
                WS_CHILD | WS_VISIBLE | TBS_HORZ | TBS_NOTICKS | WS_TABSTOP,
                18,
                88,
                324,
                28,
                hwnd,
                0, // id (unused)
                hinst,
                std::ptr::null(),
            );
            // Live percentage label (STATIC — not focusable).
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
            // Mute / Reset buttons. WS_TABSTOP keeps them in the focus order.
            let mute_btn = CreateWindowExW(
                0,
                windows_sys::core::w!("Button"),
                windows_sys::core::w!("  Mute"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
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
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
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
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
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
            set_window_text(close_btn, "×");

            // Range 0–100, position 50.
            // TBM_SETRANGE packs MAKELONG(min, max) = (max << 16) | min in the
            // LOWORD/HIWORD of lParam. ((100i64) << 32 | 0) would put 100 above
            // bit 31 where the control can't see it, leaving a degenerate range.
            SendMessageW(slider, TBM_SETRANGE, 1, (100isize) << 16);
            SendMessageW(slider, TBM_SETPOS, 1, 50);

            let d = &mut *data;
            d.slider = slider;
            d.percent_label = percent_label;
            d.mute_btn = mute_btn;
            d.reset_btn = reset_btn;
            d.close_btn = close_btn;

            // Subclass the interactive controls: keyboard navigation
            // (Tab/Shift+Tab/Escape) plus focus-change repaints for the ring.
            subclass(d, slider, IDX_SLIDER);
            subclass(d, mute_btn, IDX_MUTE);
            subclass(d, reset_btn, IDX_RESET);
            subclass(d, close_btn, IDX_CLOSE);

            // Initial adaptive styling from the placeholder appearance. The
            // resolved appearance is applied (and the brushes rebuilt) on the
            // first `sync`/`toggle`, so this only matters while hidden.
            let appearance = MixerAppearance::placeholder();
            d.accent = CreateSolidBrush(colorref(appearance.tokens.accent));
            d.background = CreateSolidBrush(colorref(appearance.tokens.background));
            apply_backdrop(hwnd, appearance.material, appearance.tokens.is_dark);
            theme_controls(
                &[slider, mute_btn, reset_btn, close_btn],
                appearance.tokens.is_dark,
            );

            Ok(Mixer { hwnd })
        }
    }

    /// Show (synced first) or hide. The app calls this from the mixer hotkey.
    ///
    /// `appearance` is the host-resolved adaptive appearance; it is applied
    /// (rebuilding brushes/styling only when it changed) before the window is
    /// positioned and shown.
    pub fn toggle(&mut self, appearance: &MixerAppearance) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut MixerData);
            if d.open {
                ShowWindow(self.hwnd, SW_HIDE);
                d.open = false;
            } else {
                apply_appearance(self.hwnd, d, *appearance);

                // Bottom-right of the monitor work area hosting the mixer,
                // directly above the volume overlay (shared right edge, 16px
                // vertical gap). Task 7 moved the overlay onto the same work
                // area, so the two surfaces stay aligned.
                let work_area = work_area_for(self.hwnd);
                let rect = place_mixer_above_overlay(
                    work_area,
                    SurfaceSize::new(WIN_W, WIN_H),
                    SurfaceSize::new(OVERLAY_WIDTH, OVERLAY_HEIGHT),
                    OVERLAY_MARGIN_X,
                    OVERLAY_MARGIN_Y,
                    OVERLAY_GAP,
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
    ///
    /// Also carries the host-resolved adaptive appearance so the palette,
    /// material, and control theming track the confirmed preferences.
    pub fn sync(&self, state: &VolumeState, appearance: &MixerAppearance) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut MixerData);
            apply_appearance(self.hwnd, d, *appearance);
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
            // Destroy the window (and its subclassed children) BEFORE freeing
            // `d`: `DestroyWindow` sends teardown messages through the still
            // installed child subclass proc, which reads the parent's
            // `GWLP_USERDATA` via `original_proc`. Freeing first would make
            // that a dangling dereference (use-after-free on every drop).
            // GDI brushes are process-owned, so deleting them after the window
            // is gone is safe.
            DestroyWindow(self.hwnd);
            DeleteObject(d.accent);
            DeleteObject(d.background);
            drop(Box::from_raw(d));
        }
    }
}

/// Apply a resolved adaptive appearance: rebuild the token-coloured brushes,
/// re-apply the DWM material treatment, re-theme the child controls, and
/// repaint. Skipped when nothing changed so the 150ms poll sync stays cheap.
unsafe fn apply_appearance(hwnd: HWND, d: &mut MixerData, appearance: MixerAppearance) {
    if d.appearance == appearance {
        return;
    }
    if d.background != 0 {
        DeleteObject(d.background);
    }
    if d.accent != 0 {
        DeleteObject(d.accent);
    }
    d.appearance = appearance;
    d.accent = CreateSolidBrush(colorref(appearance.tokens.accent));
    d.background = CreateSolidBrush(colorref(appearance.tokens.background));

    // Material fallback: request the DWM treatment (blur/translucent/opaque).
    // The mixer always paints its own opaque fill, so a missing system backdrop
    // (Windows 10) simply keeps the painted fill.
    apply_backdrop(hwnd, appearance.material, appearance.tokens.is_dark);

    // Dark-mode child controls follow the resolved palette.
    theme_controls(
        &[d.slider, d.mute_btn, d.reset_btn, d.close_btn],
        appearance.tokens.is_dark,
    );

    InvalidateRect(hwnd, std::ptr::null(), 0);
}

/// Save the original window proc of an interactive control and install the
/// shared [`mixer_child_wndproc`] subclass.
unsafe fn subclass(d: &mut MixerData, ctl: HWND, idx: usize) {
    let orig = GetWindowLongPtrW(ctl, GWLP_WNDPROC);
    d.orig_procs[idx] = Some(std::mem::transmute::<isize, ChildWndProc>(orig));
    let subclass_proc = mixer_child_wndproc as ChildWndProc;
    SetWindowLongPtrW(ctl, GWLP_WNDPROC, subclass_proc as usize as isize);
}

/// Recover the saved original window proc for a subclassed control.
unsafe fn original_proc(parent: HWND, ctl: HWND) -> WNDPROC {
    let d = &*(GetWindowLongPtrW(parent, GWLP_USERDATA) as *const MixerData);
    let idx = if ctl == d.slider {
        IDX_SLIDER
    } else if ctl == d.mute_btn {
        IDX_MUTE
    } else if ctl == d.reset_btn {
        IDX_RESET
    } else {
        IDX_CLOSE
    };
    d.orig_procs[idx]
}

/// Shared subclass proc for the interactive mixer controls.
///
/// Native Win32 behaviour is preserved for everything not handled here:
/// buttons respond to Enter/Space (generating `BN_CLICKED`) and the trackbar
/// responds to the arrow keys (`WM_HSCROLL`). This subclass adds:
///   - Escape hides the flyout (identical semantics to `WM_CLOSE`);
///   - Tab / Shift+Tab move focus among the interactive controls;
///   - focus changes repaint the parent so it can draw/clear the token focus
///     ring around the focused control.
unsafe extern "system" fn mixer_child_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let parent = GetAncestor(hwnd, GA_PARENT);
    if parent == 0 {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    // Escape closes the flyout without destroying it (same as WM_CLOSE).
    if (msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN) && (wparam as u32) == (VK_ESCAPE as u32) {
        SendMessageW(parent, WM_CLOSE, 0, 0);
        return 0;
    }

    // Tab / Shift+Tab move focus among the interactive controls in creation
    // (tab) order, wrapping at the ends.
    if msg == WM_KEYDOWN && (wparam as u32) == (VK_TAB as u32) {
        let d = &*(GetWindowLongPtrW(parent, GWLP_USERDATA) as *const MixerData);
        let order = [d.slider, d.mute_btn, d.reset_btn, d.close_btn];
        let cur = order.iter().position(|&c| c == hwnd).unwrap_or(0);
        let backwards = GetKeyState(VK_SHIFT as i32) < 0;
        let next = if backwards { (cur + 3) % 4 } else { (cur + 1) % 4 };
        SetFocus(order[next]);
        return 0;
    }

    let result = CallWindowProcW(original_proc(parent, hwnd), hwnd, msg, wparam, lparam);

    // Focus changes repaint the parent so the token focus ring tracks the
    // newly focused control (see `paint_focus_ring`).
    if msg == WM_SETFOCUS || msg == WM_KILLFOCUS {
        InvalidateRect(parent, std::ptr::null(), 0);
    }
    result
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
        // The live percentage label paints with the token primary text colour
        // and the token background brush.
        WM_CTLCOLORSTATIC => {
            let d = &*(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const MixerData);
            SetBkMode(wparam as HDC, TRANSPARENT as i32);
            SetTextColor(wparam as HDC, colorref(d.appearance.tokens.text_primary));
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
            SetTextColor(hdc, colorref(d.appearance.tokens.text_secondary));
            let label: Vec<u16> = "System volume"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            TextOutW(
                hdc,
                18,
                20,
                label.as_ptr(),
                (label.len() - 1) as i32,
            );
            paint_focus_ring(hwnd, hdc, d);
            EndPaint(hwnd, &ps);
            0
        }
        // ── Keyboard navigation when the flyout window itself has focus ──
        // (When a child control has focus, the subclass forwards these.)
        WM_KEYDOWN | WM_SYSKEYDOWN if (wparam as u32) == (VK_ESCAPE as u32) => {
            // Same hide-only semantics as WM_CLOSE below.
            let d = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MixerData);
            d.open = false;
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        WM_KEYDOWN if (wparam as u32) == (VK_TAB as u32) => {
            // Tab moves into the first interactive control.
            let d = &*(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const MixerData);
            SetFocus(d.slider);
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

/// Draw a focus ring around the currently focused interactive control.
///
/// Uses the shared [`crate::ui::theme::FocusTokens`] (`ring` colour + width).
/// The ring is drawn in the mixer's client background, just outside the
/// focused control's window rect, so it stays visible around the child window.
unsafe fn paint_focus_ring(hwnd: HWND, hdc: HDC, d: &MixerData) {
    let focused = GetFocus();
    if focused == 0
        || (focused != d.slider
            && focused != d.mute_btn
            && focused != d.reset_btn
            && focused != d.close_btn)
    {
        return;
    }

    let focus = d.appearance.tokens.focus;
    let mut rc: RECT = std::mem::zeroed();
    if GetWindowRect(focused, &mut rc) == 0 {
        return;
    }
    // Map the control's screen-space window rect into the mixer's client
    // coordinates (RECT doubles as two POINTs for MapWindowPoints).
    MapWindowPoints(0, hwnd, &mut rc as *mut RECT as *mut _, 2);

    let gap = focus.ring_gap_px.round() as i32;
    let width = (focus.ring_width_px.round() as i32).max(1);
    let ring = CreateSolidBrush(colorref(focus.ring));
    for i in 0..width {
        let r = RECT {
            left: rc.left - gap - i,
            top: rc.top - gap - i,
            right: rc.right + gap + i,
            bottom: rc.bottom + gap + i,
        };
        FrameRect(hdc, &r, ring);
    }
    DeleteObject(ring);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
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
    ) -> MixerAppearance {
        let mut cfg = Config::default();
        cfg.appearance.theme = theme;
        cfg.appearance.material = material;
        MixerAppearance::resolve(&cfg, &caps(true, high_contrast), || None)
    }

    #[test]
    fn dark_appearance_resolves_dark_tokens_and_blurred_material() {
        let a = appearance(ThemeMode::Dark, MaterialMode::Auto, false);
        assert!(a.tokens.is_dark);
        assert_eq!(a.material, ResolvedMaterial::Blurred);
    }

    #[test]
    fn system_theme_resolves_through_the_system_is_dark_callback() {
        let mut cfg = Config::default();
        cfg.appearance.theme = ThemeMode::System;

        let dark = MixerAppearance::resolve(&cfg, &caps(true, false), || Some(true));
        assert!(dark.tokens.is_dark);

        let light = MixerAppearance::resolve(&cfg, &caps(true, false), || Some(false));
        assert!(!light.tokens.is_dark);
    }

    #[test]
    fn high_contrast_forces_opaque_material_and_hc_tokens() {
        let a = appearance(ThemeMode::System, MaterialMode::Auto, true);
        assert!(a.material.is_opaque());
        assert!(a.tokens.high_contrast);
        assert!(a.tokens.background.is_opaque());
    }

    #[test]
    fn explicit_opaque_resolves_opaque_even_with_best_capabilities() {
        let a = appearance(ThemeMode::Dark, MaterialMode::Opaque, false);
        assert_eq!(a.material, ResolvedMaterial::Opaque);
    }

    #[test]
    fn text_roles_are_distinguishable_for_label_vs_percent() {
        // The "System volume" label is painted with secondary text and the live
        // percentage with primary text, so they must differ.
        let a = appearance(ThemeMode::Light, MaterialMode::Opaque, false);
        assert_ne!(a.tokens.text_primary, a.tokens.text_secondary);
    }

    #[test]
    fn mixer_constructs_and_drops_without_crashing() {
        // Smoke guard for the Drop path. `destroy()` must destroy the
        // subclassed child controls (whose teardown messages route through the
        // child subclass proc, which dereferences the parent's MixerData via
        // `original_proc`) BEFORE freeing that state — freeing first would be a
        // use-after-free on every `Mixer` drop. Constructing a real (hidden)
        // Win32 window and dropping it exercises that exact path; a hard crash
        // would need an allocator with guard pages or a live app run, but this
        // still verifies construct→drop end to end deterministically.
        let mixer = Mixer::new(0).expect("mixer window creates");
        drop(mixer);
    }
}
