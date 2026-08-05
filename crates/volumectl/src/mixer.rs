//! Volume Mixer — the Signal Glass precision control card (spec §6).
//!
//! A 400x224 (logical) captionless, always-on-top tool window, DWM-styled
//! like the system flyout: rounded corners (33), immersive dark mode (20),
//! and the system backdrop (38) — all driven by the shared adaptive tokens
//! and the capability-resolved material treatment (Tasks 3–5). The card
//! shows `VOLUME MIXER`, `System output`, a right-aligned 28px live value,
//! and the shared Signal Rail paired with a native trackbar:
//!
//!   - the **trackbar remains the interactive control** — drag, arrows,
//!     Home/End (all `WM_HSCROLL` → `WM_APP_MIXER_CHANGE`), and UIA
//!     semantics — while its chrome is suppressed, so the custom-drawn rail
//!     (threshold fill + thumb/diamond marker) is the single visible volume
//!     visualization and a painted mirror of the confirmed slider position;
//!   - Mute / Unmute and `Reset volume to 50 percent` native buttons;
//!   - a visible `×` close button (UIA name `Close mixer`) that hides (not
//!     destroys) the flyout;
//!   - a two-layer focus ring (outer accent + inner contrast) around the
//!     focused control.
//!
//! Placement is the bottom-right of the *monitor work area* hosting the
//! window, computed through [`crate::ui::surface::place_mixer_above_overlay`]
//! with PHYSICAL (DPI-scaled) sizes for both surfaces, so the mixer shares
//! the overlay's right edge and sits exactly 16px above its top at every
//! scale (100/125/150%).
//!
//! Keyboard navigation: the interactive controls (slider + three buttons) are
//! subclassed so that Escape hides the flyout, Tab/Shift+Tab move focus among
//! them, Enter/Space activate the focused button (native `BN_CLICKED`), and
//! focus changes repaint the parent which draws the token focus ring around
//! the focused control.
//!
//! User interaction (slider drag, buttons) posts [`WM_APP_MIXER_*`] messages
//! to the host window, which owns the audio backend. The host maps each
//! message to a shared [`crate::ui::AppAction`] (`SetVolumePercent` /
//! `ToggleMute` / `ResetVolume`) and dispatches it through its central action
//! handler, which mutates audio and publishes confirmed state.

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::Gdi::{
        CreateSolidBrush, DeleteObject, InvalidateRect, UpdateWindow, ValidateRect, HBRUSH,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::Controls::{
        InitCommonControlsEx, DRAWITEMSTRUCT, ICC_BAR_CLASSES, INITCOMMONCONTROLSEX, WM_MOUSELEAVE,
    },
    UI::Input::KeyboardAndMouse::{
        GetFocus, GetKeyState, SetFocus, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT, VK_ESCAPE,
        VK_SHIFT, VK_TAB,
    },
    UI::WindowsAndMessaging::{
        CallWindowProcW, CreateWindowExW, DefWindowProcW, DestroyWindow, GetAncestor, GetCursorPos,
        GetWindowLongPtrW, GetWindowRect, KillTimer, PostMessageW, RegisterClassW, SendMessageW,
        SetTimer, SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow, BN_CLICKED,
        BS_OWNERDRAW, CW_USEDEFAULT, GA_PARENT, GWLP_USERDATA, GWLP_WNDPROC, HWND_TOPMOST,
        SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_HIDE, WM_CLOSE, WM_COMMAND, WM_DRAWITEM,
        WM_ERASEBKGND, WM_HSCROLL, WM_KEYDOWN, WM_KILLFOCUS, WM_MOUSEMOVE, WM_PAINT, WM_SETFOCUS,
        WM_SYSKEYDOWN, WM_TIMER, WM_USER, WNDCLASSW, WNDPROC, WS_CHILD, WS_EX_TOOLWINDOW,
        WS_EX_TOPMOST, WS_POPUP, WS_TABSTOP, WS_VISIBLE,
    },
};

use crate::audio::VolumeState;
use crate::config::{ColorThresholds, Config};
use crate::overlay::{OVERLAY_HEIGHT, OVERLAY_MARGIN_X, OVERLAY_MARGIN_Y, OVERLAY_WIDTH};
use crate::ui::platform::windows::text::{TextAlign, TextLayout};
use crate::ui::primitives::{
    apply_backdrop, colorref, dpi_scale_for, paint_close_button, theme_controls, work_area_for,
    DpiMetrics, PaintCanvas, PointF, RectF,
};
use crate::ui::{
    place_mixer_above_overlay, rail_geometry, resolve_material, tokens_for, AccentMode,
    MarkerGeometry, ResolvedMaterial, SignalRail, SurfaceSize, ThemeMode, ThemeTokens, TrackRect,
    UiCapabilities,
};

/// Custom messages the mixer posts to the host window (see `app.rs`).
pub const WM_APP_MIXER_CHANGE: u32 = WM_USER + 11; // wparam = new volume %
pub const WM_APP_MIXER_MUTE: u32 = WM_USER + 12;
pub const WM_APP_MIXER_RESET: u32 = WM_USER + 13;

/// Logical width of the precision control card (spec §6.1).
const WIN_W: i32 = 400;
/// Logical height of the precision control card (spec §6.1).
const WIN_H: i32 = 224;
/// Gap between the mixer card and the transient volume overlay.
const OVERLAY_GAP: i32 = 16;

/// Rail thumb diameter 12px → 6px radius (spec §6.2, same as the overlay).
const THUMB_RADIUS: f32 = 6.0;
/// Muted diamond half-size 6px — same extent as the thumb (spec §6.2).
const MUTED_DIAMOND_HALF_SIZE: f32 = 6.0;

const ID_BTN_MUTE: usize = 1;
const ID_BTN_RESET: usize = 2;
const ID_BTN_CLOSE: usize = 3;

/// One-shot timer ID for the deferred DWM backdrop re-apply after a show
/// (see the comment in [`Mixer::toggle`]).
const BACKDROP_TIMER_ID: usize = 1;
/// Delay (ms) between showing the window and re-asserting the resolved
/// backdrop: DWM applies its High-Contrast backdrop override asynchronously
/// after the show — measured on Windows 11 24H2, backdrop writes are
/// clobbered back to AUTO for roughly the first second after the show, and
/// stick once the composition settles. 2000ms lands safely past that window.
const BACKDROP_REAPPLY_MS: u32 = 2000;

// Indexes into `MixerData::orig_procs`, kept in sync with the tab order
// (slider -> mute -> reset -> close).
const IDX_SLIDER: usize = 0;
const IDX_MUTE: usize = 1;
const IDX_RESET: usize = 2;
const IDX_CLOSE: usize = 3;

/// The window-proc signature of the subclassed interactive controls.
type ChildWndProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

/// Logical layout of the mixer card (all values in logical px, spec §6.2).
///
/// Vertical rhythm on the 4px grid: 16 top padding; row 1 eyebrow
/// `VOLUME MIXER` (label role) at y 16; row 2 `System output` (caption) at
/// y 40; row 3 air (y 52..66); row 4 the 44px display value (y 66..110,
/// right-aligned); row 5 the Signal Rail band (8px track, center y 132); row
/// 6 air (y 136..172); row 7 the 36px button row (y 172..208) with 16px
/// bottom padding. The native trackbar occupies the same x range as the rail
/// with a 28px-tall hit area centered on the rail band (y 118..146).
///
/// The value row sits 8px above the slider hit area and has 16px of extra
/// vertical room for the 28px display glyph; the slider's outer
/// focus ring (3px gap + 1.5px stroke → 3.75px outset) would cut through
/// the value's ink at y 114.25..115.75, so the row ends at 96px and leaves
/// visible air before the ring — verified by
/// `value_row_clears_the_slider_focus_ring`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct MixerLayout {
    /// `VOLUME MIXER` eyebrow box (label role, secondary text), left at 16.
    eyebrow_rect: RectF,
    /// `System output` caption box at y 40.
    output_rect: RectF,
    /// Live value box ending at `width - 16`; the value is right-aligned
    /// inside it via [`TextAlign::Right`] origin math (`rect.right -
    /// text_width`, never space padding).
    value_rect: RectF,
    /// Signal Rail track: spans x 16..=width-16, 8px tall, centered on the
    /// rail band (center y 128).
    track: TrackRect,
    /// Native trackbar hit area: full rail width, 28px tall, centered on the
    /// rail band so the (chrome-suppressed) thumb x positions agree with the
    /// painted marker.
    slider_rect: RectF,
    mute_rect: RectF,
    reset_rect: RectF,
    /// Close hit target: 32x32 at the top-right, right edge at `width - 16`.
    close_rect: RectF,
}

const CONTENT_MARGIN: f32 = 16.0;
const BUTTON_GAP: f32 = 16.0;
const MUTE_BUTTON_WIDTH: f32 = 132.0;
const VALUE_ROW_HEIGHT: f32 = 44.0;
const VALUE_SLIDER_AIR: f32 = 8.0;

impl MixerLayout {
    fn new(w: f32, h: f32) -> Self {
        let content_left = CONTENT_MARGIN;
        let content_right = w - CONTENT_MARGIN;
        let track_center_y = 132.0;
        let track_half = 4.0;
        let slider_top = track_center_y - 14.0;
        let value_bottom = slider_top - VALUE_SLIDER_AIR;
        let value_top = value_bottom - VALUE_ROW_HEIGHT;
        let mute_right = content_left + MUTE_BUTTON_WIDTH;
        let reset_left = mute_right + BUTTON_GAP;
        // Bottom-anchored button row: 36px tall with 16px bottom padding
        // (y 172..208 for the 224px card).
        let buttons_bottom = h - 16.0;
        let buttons_top = buttons_bottom - 36.0;
        Self {
            eyebrow_rect: RectF::new(content_left, 16.0, content_right, 32.0),
            output_rect: RectF::new(content_left, 40.0, content_right, 52.0),
            value_rect: RectF::new(content_left, value_top, content_right, value_bottom),
            track: TrackRect {
                left: content_left,
                right: content_right,
                top: track_center_y - track_half,
                bottom: track_center_y + track_half,
            },
            slider_rect: RectF::new(
                content_left,
                slider_top,
                content_right,
                track_center_y + 14.0,
            ),
            // Mute gets a fixed comfortable width; Reset fills the remaining
            // column so its full native label remains visible.
            mute_rect: RectF::new(content_left, buttons_top, mute_right, buttons_bottom),
            reset_rect: RectF::new(reset_left, buttons_top, content_right, buttons_bottom),
            close_rect: RectF::new(content_right - 32.0, 12.0, content_right, 44.0),
        }
    }
}

/// Pure paint plan for one mixer frame.
///
/// The plan is the single testable decision point: the logical layout, the
/// rail state (with the user's `config.color_thresholds` band boundaries),
/// and the resolved rail geometry are computed here without a window.
/// [`paint`] only executes the plan through the canvas.
struct MixerPlan {
    width: f32,
    height: f32,
    layout: MixerLayout,
    rail: SignalRail,
    geometry: crate::ui::SignalRailGeometry,
}

/// Resolve the frame plan for `data` in a `w x h` logical surface.
///
/// The rail carries the user's `config.color_thresholds` band boundaries
/// (`green_up_to`/`blue_up_to`) and the token palette, so
/// [`SignalRail::fill_color`] mirrors the authoritative
/// `core::volume_color_rgb` semantics for any config.
fn paint_plan(d: &MixerData, w: f32, h: f32) -> MixerPlan {
    let layout = MixerLayout::new(w, h);
    let rail = SignalRail::new(
        d.percent,
        d.muted,
        d.appearance.tokens.volume_threshold,
        d.thresholds.green_up_to,
        d.thresholds.blue_up_to,
    );
    let geometry = rail_geometry(&rail, layout.track, THUMB_RADIUS, MUTED_DIAMOND_HALF_SIZE);
    MixerPlan {
        width: w,
        height: h,
        layout,
        rail,
        geometry,
    }
}

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
    mute_btn: HWND,
    reset_btn: HWND,
    close_btn: HWND,
    /// Original window procs of the subclassed interactive controls, saved so
    /// [`mixer_child_wndproc`] can forward everything it does not handle.
    orig_procs: [Option<ChildWndProc>; 4],
    accent: HBRUSH,
    background: HBRUSH,
    appearance: MixerAppearance,
    /// Confirmed volume percent the painted rail mirrors (set by `sync`).
    percent: u8,
    muted: bool,
    /// User `config.color_thresholds` band boundaries for the rail fill
    /// (carried by the host via [`Mixer::set_thresholds`]).
    thresholds: ColorThresholds,
    /// DPI scale the window and children were last laid out at (1.0 before
    /// the first `toggle`).
    dpi: f32,
    open: bool,
    /// Pointer hover over the owner-drawn close button (the owner paints the
    /// hover face in `WM_DRAWITEM`; tracked via `WM_MOUSEMOVE`/`WM_MOUSELEAVE`
    /// in the subclass, like the native hover the button had before).
    close_hover: bool,
}

impl MixerData {
    fn placeholder(host: HWND) -> Self {
        Self {
            host,
            slider: 0,
            mute_btn: 0,
            reset_btn: 0,
            close_btn: 0,
            orig_procs: [None; 4],
            accent: 0,
            background: 0,
            appearance: MixerAppearance::placeholder(),
            percent: 0,
            muted: false,
            thresholds: Config::default().color_thresholds,
            dpi: 1.0,
            open: false,
            close_hover: false,
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

            // The native controls are laid out in LOGICAL pixels; `toggle`
            // repositions them to PHYSICAL pixels for the window's DPI before
            // the first show (the same rects the rail and focus ring use).
            let layout = MixerLayout::new(WIN_W as f32, WIN_H as f32);

            // Trackbar slider. WS_TABSTOP makes it part of the keyboard focus
            // order (Tab navigation via the child subclass). Its chrome is
            // suppressed by the subclass so the painted Signal Rail is the
            // visible volume visualization; the control itself stays fully
            // interactive (drag, arrows, Home/End, UIA).
            let slider = CreateWindowExW(
                0,
                windows_sys::core::w!("msctls_trackbar32"),
                // Spec §11.2: the UIA name of the trackbar is its window text;
                // Narrator reads `slider, System output volume, 72 percent`
                // from the name plus the native value pattern.
                windows_sys::core::w!("System output volume"),
                WS_CHILD | WS_VISIBLE | TBS_HORZ | TBS_NOTICKS | WS_TABSTOP,
                layout.slider_rect.left as i32,
                layout.slider_rect.top as i32,
                layout.slider_rect.width() as i32,
                layout.slider_rect.height() as i32,
                hwnd,
                0, // id (unused)
                hinst,
                std::ptr::null(),
            );
            // Mute / Reset buttons. WS_TABSTOP keeps them in the focus order.
            let mute_btn = CreateWindowExW(
                0,
                windows_sys::core::w!("Button"),
                windows_sys::core::w!("Mute"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                layout.mute_rect.left as i32,
                layout.mute_rect.top as i32,
                layout.mute_rect.width() as i32,
                layout.mute_rect.height() as i32,
                hwnd,
                ID_BTN_MUTE as isize,
                hinst,
                std::ptr::null(),
            );
            let reset_btn = CreateWindowExW(
                0,
                windows_sys::core::w!("Button"),
                // Spec §11.2: the reset button's UIA name (its window text).
                windows_sys::core::w!("Reset volume to 50 percent"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                layout.reset_rect.left as i32,
                layout.reset_rect.top as i32,
                layout.reset_rect.width() as i32,
                layout.reset_rect.height() as i32,
                hwnd,
                ID_BTN_RESET as isize,
                hinst,
                std::ptr::null(),
            );
            // A visible close affordance hides the flyout without changing
            // the existing hotkey/tray toggle behavior. Spec §11.2: the
            // button is OWNER-DRAWN so its window text (the UIA name) can be
            // `Close mixer` while the owner paints the approved `×` visual
            // (see `mixer_wndproc`'s WM_DRAWITEM and
            // [`crate::ui::primitives::paint_close_button`]).
            let close_btn = CreateWindowExW(
                0,
                windows_sys::core::w!("Button"),
                windows_sys::core::w!("Close mixer"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW as u32,
                layout.close_rect.left as i32,
                layout.close_rect.top as i32,
                layout.close_rect.width() as i32,
                layout.close_rect.height() as i32,
                hwnd,
                ID_BTN_CLOSE as isize,
                hinst,
                std::ptr::null(),
            );
            if slider == 0 || mute_btn == 0 || reset_btn == 0 || close_btn == 0 {
                return Err("mixer child control failed".into());
            }

            // Range 0–100, position 50.
            // TBM_SETRANGE packs MAKELONG(min, max) = (max << 16) | min in the
            // LOWORD/HIWORD of lParam. ((100i64) << 32 | 0) would put 100 above
            // bit 31 where the control can't see it, leaving a degenerate range.
            SendMessageW(slider, TBM_SETRANGE, 1, (100isize) << 16);
            SendMessageW(slider, TBM_SETPOS, 1, 50);

            let d = &mut *data;
            d.slider = slider;
            d.mute_btn = mute_btn;
            d.reset_btn = reset_btn;
            d.close_btn = close_btn;

            // Subclass the interactive controls: keyboard navigation
            // (Tab/Shift+Tab/Escape), slider chrome suppression, plus
            // focus-change repaints for the ring.
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
    ///
    /// DPI: the window's PHYSICAL size and every child control are scaled
    /// exactly once from the logical design size (like the overlay, Task 5).
    /// The overlay's physical size is scaled the same way, so the 16px
    /// placement gap holds in physical space at any scale.
    pub fn toggle(&mut self, appearance: &MixerAppearance) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut MixerData);
            if d.open {
                ShowWindow(self.hwnd, SW_HIDE);
                d.open = false;
            } else {
                apply_appearance(self.hwnd, d, *appearance);

                let dpi = DpiMetrics::new(dpi_scale_for(self.hwnd));
                if d.dpi != dpi.scale() {
                    d.dpi = dpi.scale();
                    layout_children(d, dpi);
                }

                // Bottom-right of the monitor work area hosting the mixer,
                // directly above the volume overlay (shared right edge, 16px
                // vertical gap) in physical pixels for both surfaces.
                let work_area = work_area_for(self.hwnd);
                let mixer_size = SurfaceSize::new(dpi.to_physical(WIN_W), dpi.to_physical(WIN_H));
                let overlay_size = SurfaceSize::new(
                    dpi.to_physical(OVERLAY_WIDTH),
                    dpi.to_physical(OVERLAY_HEIGHT),
                );
                let rect = place_mixer_above_overlay(
                    work_area,
                    mixer_size,
                    overlay_size,
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
                // DWM re-asserts DWMSBT_AUTO when a window is shown while
                // High Contrast is active — asynchronously, AFTER the show
                // (observed: the reset lands after any immediate re-apply).
                // Re-apply the resolved backdrop on a one-shot timer once the
                // composition has settled, so the opaque painted surface
                // stays visible on screen.
                SetTimer(self.hwnd, BACKDROP_TIMER_ID, BACKDROP_REAPPLY_MS, None);
                // While High Contrast is active, DWM fills the freshly shown
                // surface with the forced AUTO backdrop, which marks the
                // client clean — the system then skips the first WM_PAINT and
                // the painted content never lands (live-verified: under HC
                // the mixer showed only its native child controls; a
                // probe-side invalidation did not register either). Force the
                // first paint so the opaque fill always draws, over whatever
                // backdrop DWM keeps.
                log::debug!("mixer toggle: invalidating + updating");
                InvalidateRect(self.hwnd, std::ptr::null(), 0);
                UpdateWindow(self.hwnd);
                d.open = true;
                // Sync the owner-drawn close button's hover state with the
                // pointer (the cursor may already be over the button when the
                // flyout reappears; no WM_MOUSEMOVE fires until it moves).
                Self::sync_close_hover(d);
            }
        }
    }

    /// Whether the pointer currently sits inside the close button's rect —
    /// the owner-drawn hover state for the just-shown window.
    fn sync_close_hover(d: &mut MixerData) {
        unsafe {
            let mut cursor: POINT = std::mem::zeroed();
            if GetCursorPos(&mut cursor) == 0 {
                d.close_hover = false;
                return;
            }
            let mut rc: RECT = std::mem::zeroed();
            if GetWindowRect(d.close_btn, &mut rc) == 0 {
                d.close_hover = false;
                return;
            }
            d.close_hover = cursor.x >= rc.left
                && cursor.x < rc.right
                && cursor.y >= rc.top
                && cursor.y < rc.bottom;
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
    /// material, and control theming track the confirmed preferences, and
    /// repaints the parent so the painted Signal Rail mirrors the confirmed
    /// trackbar position — the rail and the trackbar never disagree.
    ///
    /// Focus stability: `sync` never steals focus or resets the focused
    /// control (it only moves the slider position, sets button text, and
    /// invalidates paint regions — none of which change focus; covered by
    /// tests).
    pub fn sync(&self, state: &VolumeState, appearance: &MixerAppearance) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut MixerData);
            apply_appearance(self.hwnd, d, *appearance);
            d.percent = state.percent();
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
            set_window_text(d.mute_btn, if state.muted { "Unmute" } else { "Mute" });
            // The rail repaints from confirmed state (the paint reads
            // `d.percent`/`d.muted`); the slider's own chrome never paints,
            // so the rail is the single visible visualization.
            InvalidateRect(self.hwnd, std::ptr::null(), 0);
        }
    }

    /// Carry the user's `config.color_thresholds` band boundaries into the
    /// rail fill — the same values the overlay receives per-show. The
    /// mixer's `sync`/`toggle` signatures are preserved, so the thresholds
    /// travel on their own seam; the host calls this on every confirmed-state
    /// publication (before `sync`).
    pub fn set_thresholds(&mut self, thresholds: ColorThresholds) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut MixerData);
            if d.thresholds != thresholds {
                d.thresholds = thresholds;
                InvalidateRect(self.hwnd, std::ptr::null(), 0);
            }
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

/// Position the native controls at PHYSICAL coordinates for `dpi`. The
/// controls' logical layout rects come from [`MixerLayout`] — the same rects
/// the rail and the focus ring use — scaled exactly once.
unsafe fn layout_children(d: &MixerData, dpi: DpiMetrics) {
    let layout = MixerLayout::new(WIN_W as f32, WIN_H as f32);
    let place = |ctl: HWND, rect: RectF| {
        SetWindowPos(
            ctl,
            0,
            dpi.to_physical(rect.left.round() as i32),
            dpi.to_physical(rect.top.round() as i32),
            dpi.to_physical(rect.width().round() as i32),
            dpi.to_physical(rect.height().round() as i32),
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    };
    place(d.slider, layout.slider_rect);
    place(d.mute_btn, layout.mute_rect);
    place(d.reset_btn, layout.reset_rect);
    place(d.close_btn, layout.close_rect);
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
/// responds to the arrow keys / Home / End / drag (`WM_HSCROLL`). This
/// subclass additionally:
///   - suppresses the native trackbar's chrome (the mixer paints the Signal
///     Rail over the slider's band, so the rail is the single visible volume
///     visualization while the control stays fully interactive: mouse drag,
///     arrows/Home/End via `WM_HSCROLL`, and UIA semantics);
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
    let d = &*(GetWindowLongPtrW(parent, GWLP_USERDATA) as *const MixerData);

    // Owner-drawn close button hover tracking: the owner paints the hover
    // face, so the button must repaint when the pointer enters/leaves (the
    // native hover the button had before owner-drawing). The window proc runs
    // on the owning thread, so mutating the per-window state here is safe.
    if hwnd == d.close_btn {
        match msg {
            WM_MOUSEMOVE => {
                if !d.close_hover {
                    (&mut *(GetWindowLongPtrW(parent, GWLP_USERDATA) as *mut MixerData))
                        .close_hover = true;
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                    let mut tme: TRACKMOUSEEVENT = std::mem::zeroed();
                    tme.cbSize = std::mem::size_of::<TRACKMOUSEEVENT>() as u32;
                    tme.dwFlags = TME_LEAVE;
                    tme.hwndTrack = hwnd;
                    TrackMouseEvent(&mut tme);
                }
            }
            WM_MOUSELEAVE => {
                if d.close_hover {
                    (&mut *(GetWindowLongPtrW(parent, GWLP_USERDATA) as *mut MixerData))
                        .close_hover = false;
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                }
            }
            _ => {}
        }
    }

    // Native trackbar chrome suppression: swallow its paint (validating the
    // update region so no repaint storm) so the parent's painted rail stays
    // visible over the slider's band.
    if hwnd == d.slider {
        match msg {
            WM_ERASEBKGND => return 1,
            WM_PAINT => {
                ValidateRect(hwnd, std::ptr::null());
                return 0;
            }
            _ => {}
        }
    }

    // Escape closes the flyout without destroying it (same as WM_CLOSE).
    if (msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN) && (wparam as u32) == (VK_ESCAPE as u32) {
        SendMessageW(parent, WM_CLOSE, 0, 0);
        return 0;
    }

    // Tab / Shift+Tab move focus among the interactive controls in creation
    // (tab) order, wrapping at the ends.
    if msg == WM_KEYDOWN && (wparam as u32) == (VK_TAB as u32) {
        let order = [d.slider, d.mute_btn, d.reset_btn, d.close_btn];
        let cur = order.iter().position(|&c| c == hwnd).unwrap_or(0);
        let backwards = GetKeyState(VK_SHIFT as i32) < 0;
        let next = if backwards {
            (cur + 3) % 4
        } else {
            (cur + 1) % 4
        };
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
        WM_ERASEBKGND => 1,
        WM_DRAWITEM => {
            // The owner-drawn close button (spec §11.2): its window text
            // carries the UIA name `Close mixer` while this paints the `×`
            // visual (system button face + glyph + hover/pressed/focus states
            // via `paint_close_button`).
            let d = &*(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const MixerData);
            if paint_close_button(lparam as *const DRAWITEMSTRUCT, d.close_hover) {
                0
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
        WM_PAINT => {
            log::debug!("mixer WM_PAINT received hwnd={hwnd:?}");
            // Paint through the resource-safe canvas (Task 3): it owns the
            // BeginPaint/EndPaint pair, selects ONE paint path per frame
            // (Direct2D when available, GDI otherwise), and deletes every
            // per-call GDI object. If BeginPaint itself fails, paint nothing
            // and invalidate so the next WM_PAINT retries.
            if let Some(mut canvas) = PaintCanvas::begin_paint(hwnd) {
                let d = &*(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const MixerData);
                paint(&mut canvas, d);
            } else {
                log::debug!("mixer: BeginPaint failed; invalidating for a retry");
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
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
        // Deferred backdrop re-apply armed by `toggle` (see the comment
        // there): DWM re-asserts DWMSBT_AUTO after a show while High Contrast
        // is active, so the resolved backdrop is re-asserted once the
        // composition has settled, keeping the opaque painted surface visible.
        WM_TIMER if (wparam as usize) == BACKDROP_TIMER_ID => {
            KillTimer(hwnd, BACKDROP_TIMER_ID);
            let d = &*(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const MixerData);
            apply_backdrop(hwnd, d.appearance.material, d.appearance.tokens.is_dark);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Draw the mixer contents from the resolved [`MixerPlan`]: adaptive
/// background, optional opaque-mode border, the eyebrow/caption/value rows,
/// the Signal Rail (threshold fill + thumb/diamond marker — the painted
/// mirror of the native trackbar, whose chrome is suppressed), and the
/// two-layer focus ring. All coordinates are logical; the canvas scales them
/// to physical pixels exactly once via its DPI metrics.
///
/// The parent's full-client fill covers the native controls; the buttons are
/// invalidated at the end so they repaint their chrome on top. The slider is
/// chrome-suppressed, so the painted rail stays the visible visualization.
unsafe fn paint(canvas: &mut PaintCanvas, d: &MixerData) {
    let tokens = &d.appearance.tokens;
    let plan = paint_plan(d, WIN_W as f32, WIN_H as f32);
    let layout = &plan.layout;

    // Background: always-painted opaque token fill. This renderer fill is the
    // material fallback — the surface stays fully readable even when no DWM
    // backdrop is available (Windows 10 / unsupported backdrop attribute).
    // The card corners are rounded by the DWM corner preference requested in
    // `apply_backdrop`.
    canvas.fill_rect(
        RectF::new(0.0, 0.0, plan.width, plan.height),
        tokens.background,
    );

    // 1px border in opaque mode (same policy as the overlay, spec §5.2).
    // Blurred/translucent modes draw no border: the DWM backdrop provides the
    // surface edge. High contrast always resolves to opaque, so it always
    // gets the border.
    if d.appearance.material.is_opaque() {
        canvas.stroke_rounded_rect(
            RectF::new(0.5, 0.5, plan.width - 0.5, plan.height - 0.5),
            tokens.radii.surface_px,
            tokens.border,
            1.0,
        );
    }

    // Row 1: eyebrow (label role, secondary text).
    canvas.draw_text(&TextLayout {
        text: "VOLUME MIXER",
        rect: layout.eyebrow_rect,
        align: TextAlign::Left,
        role: tokens.typography.label,
        color: tokens.text_secondary,
    });

    // Row 2: output identity.
    canvas.draw_text(&TextLayout {
        text: "System output",
        rect: layout.output_rect,
        align: TextAlign::Left,
        role: tokens.typography.caption,
        color: tokens.text_secondary,
    });

    // Row 4: live value, right-aligned by real alignment math
    // (`TextAlign::Right` origin = rect.right - text_width, never space
    // padding). Muted renders as the `Muted` status cue in the muted legend
    // colour (the text status cue from spec §6.3); the shape cue below
    // (MutedDiamond) pairs with the text so the state never relies on colour
    // alone.
    let value = if plan.rail.muted {
        "Muted".to_string()
    } else {
        format!("{}%", plan.rail.percent)
    };
    canvas.draw_text(&TextLayout {
        text: &value,
        rect: layout.value_rect,
        align: TextAlign::Right,
        role: if plan.rail.muted {
            tokens.typography.label
        } else {
            tokens.typography.display_value
        },
        color: if plan.rail.muted {
            tokens.volume_threshold.muted
        } else {
            tokens.text_primary
        },
    });

    // Row 5: Signal Rail — track, threshold fill, marker. The fill honours
    // the user's `config.color_thresholds` band boundaries through the rail
    // (Task 4, proven identical to `core::volume_color_rgb` for any config).
    let t = plan.geometry.track;
    canvas.fill_rect(RectF::new(t.left, t.top, t.right, t.bottom), tokens.border);
    if plan.geometry.fill_right > t.left {
        canvas.fill_rect(
            RectF::new(t.left, t.top, plan.geometry.fill_right, t.bottom),
            plan.rail.fill_color(),
        );
    }
    match plan.geometry.marker {
        MarkerGeometry::Thumb {
            center_x,
            center_y,
            radius,
        } => {
            let center = PointF::new(center_x, center_y);
            // Filled surface circle with a strong outline: the thumb stays
            // visible against both the fill and the track.
            canvas.fill_circle(center, radius, tokens.surface);
            canvas.stroke_circle(center, radius, tokens.signal_glass().border_strong, 1.0);
        }
        MarkerGeometry::MutedDiamond {
            center_x,
            center_y,
            half_size,
        } => {
            // Outline diamond (◇), never a filled grey copy of the thumb: the
            // shape carries the muted state in high contrast.
            canvas.stroke_diamond(
                PointF::new(center_x, center_y),
                half_size,
                tokens.text_primary,
                1.0,
            );
        }
    }

    // Two-layer focus ring around the focused interactive control.
    paint_focus_ring(canvas, d, layout);

    // The parent's full-client fill covers the native buttons; repaint them
    // on top so their chrome is never wiped by a sync repaint. (InvalidateRect
    // never changes focus, so the focused control stays stable.) The slider
    // is chrome-suppressed, so the painted rail remains the visible volume
    // visualization.
    for ctl in [d.mute_btn, d.reset_btn, d.close_btn] {
        InvalidateRect(ctl, std::ptr::null(), 0);
    }
}

/// Draw the two-layer focus ring around the currently focused interactive
/// control (Task 1 cross-task follow-up: BOTH layers — the outer accent ring
/// and the inner contrast ring — via [`PaintCanvas::draw_focus_ring`]).
///
/// The control rects come from the logical [`MixerLayout`] — the same rects
/// the native controls are positioned at (scaled by the same DPI) — so the
/// ring always lands exactly around the focused control without window-rect
/// mapping.
unsafe fn paint_focus_ring(canvas: &mut PaintCanvas, d: &MixerData, layout: &MixerLayout) {
    let Some(rect) = focused_control_rect(d, layout) else {
        return;
    };
    canvas.draw_focus_ring(rect, &d.appearance.tokens.focus);
}

/// The logical rect of the focused interactive control, if any.
unsafe fn focused_control_rect(d: &MixerData, layout: &MixerLayout) -> Option<RectF> {
    let focused = GetFocus();
    if focused == 0 {
        return None;
    }
    if focused == d.slider {
        Some(layout.slider_rect)
    } else if focused == d.mute_btn {
        Some(layout.mute_rect)
    } else if focused == d.reset_btn {
        Some(layout.reset_rect)
    } else if focused == d.close_btn {
        Some(layout.close_rect)
    } else {
        None
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::ui::platform::windows::text::text_x_origin;
    use crate::ui::primitives::focus_ring_rects;
    use crate::ui::{MaterialMode, ThemeMode, WorkArea};
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowTextW, GWL_STYLE};

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

    /// A mixer frame plan for the given state and thresholds (pure; no window).
    fn plan_with(percent: u8, muted: bool, thresholds: ColorThresholds) -> MixerPlan {
        let data = MixerData {
            host: 0,
            slider: 0,
            mute_btn: 0,
            reset_btn: 0,
            close_btn: 0,
            orig_procs: [None; 4],
            accent: 0,
            background: 0,
            appearance: appearance(ThemeMode::Dark, MaterialMode::Opaque, false),
            percent,
            muted,
            thresholds,
            dpi: 1.0,
            open: false,
            close_hover: false,
        };
        paint_plan(&data, WIN_W as f32, WIN_H as f32)
    }

    /// A mixer frame plan with the default VolumePro band boundaries.
    fn plan(percent: u8, muted: bool) -> MixerPlan {
        plan_with(percent, muted, Config::default().color_thresholds)
    }

    /// Read a child control's text (UTF-16 → UTF-8).
    fn window_text(hwnd: HWND) -> String {
        unsafe {
            let mut buf = [0u16; 128];
            let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
            String::from_utf16_lossy(&buf[..n.max(0) as usize])
        }
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
        // The `System output` caption is painted with secondary text and the
        // live percentage with primary text, so they must differ.
        let a = appearance(ThemeMode::Light, MaterialMode::Opaque, false);
        assert_ne!(a.tokens.text_primary, a.tokens.text_secondary);
    }

    // ── DPI scaling (pure math) ─────────────────────────────────────────────

    #[test]
    fn dpi_scales_the_400x224_mixer_to_physical_pixels() {
        // The window logical size is the spec card; physical size comes from a
        // single DpiMetrics conversion (the same path `toggle` and the canvas
        // use).
        assert_eq!((WIN_W, WIN_H), (400, 224));
        let at_100 = DpiMetrics::new(1.0);
        assert_eq!(
            (at_100.to_physical(WIN_W), at_100.to_physical(WIN_H)),
            (400, 224)
        );
        let at_125 = DpiMetrics::new(1.25);
        assert_eq!(
            (at_125.to_physical(WIN_W), at_125.to_physical(WIN_H)),
            (500, 280)
        );
        let at_150 = DpiMetrics::new(1.5);
        assert_eq!(
            (at_150.to_physical(WIN_W), at_150.to_physical(WIN_H)),
            (600, 336)
        );
    }

    #[test]
    fn physical_16px_gap_holds_at_125_and_150_percent() {
        // The mixer placement consumes PHYSICAL sizes for both surfaces (the
        // overlay scales its own 336x88 the same way), so the 16px gap holds
        // in physical space at every scale (Task 5 cross-task follow-up).
        let work_area = WorkArea::new(0, 0, 2560, 1400);
        for scale in [1.25f32, 1.5] {
            let dpi = DpiMetrics::new(scale);
            let mixer_size = SurfaceSize::new(dpi.to_physical(WIN_W), dpi.to_physical(WIN_H));
            let overlay_size = SurfaceSize::new(
                dpi.to_physical(OVERLAY_WIDTH),
                dpi.to_physical(OVERLAY_HEIGHT),
            );
            let mixer = place_mixer_above_overlay(
                work_area,
                mixer_size,
                overlay_size,
                OVERLAY_MARGIN_X,
                OVERLAY_MARGIN_Y,
                OVERLAY_GAP,
            );
            let overlay = crate::ui::place_overlay(
                work_area,
                overlay_size,
                OVERLAY_MARGIN_X,
                OVERLAY_MARGIN_Y,
            );
            assert_eq!(
                mixer.bottom + OVERLAY_GAP,
                overlay.top,
                "16px physical gap at {scale}x"
            );
            assert_eq!(mixer.right, overlay.right, "shared right edge at {scale}x");
            assert_eq!(mixer.width(), dpi.to_physical(WIN_W), "width at {scale}x");
            assert_eq!(mixer.height(), dpi.to_physical(WIN_H), "height at {scale}x");
            assert_eq!(
                overlay.width(),
                dpi.to_physical(OVERLAY_WIDTH),
                "overlay width at {scale}x"
            );
        }
    }

    // ── layout ──────────────────────────────────────────────────────────────

    #[test]
    fn mixer_layout_matches_the_spec_rows() {
        let l = MixerLayout::new(WIN_W as f32, WIN_H as f32);
        // Eyebrow top-left at the 16px padding.
        assert_eq!(l.eyebrow_rect.left, 16.0);
        assert_eq!(l.eyebrow_rect.top, 16.0);
        // Close: >=32x32 hit target at the top-right.
        assert_eq!(l.close_rect.width(), 32.0);
        assert_eq!(l.close_rect.height(), 32.0);
        assert_eq!(l.close_rect.right, WIN_W as f32 - 16.0);
        // Value right-anchored at width - 16, aligned by math, never padded.
        assert_eq!(l.value_rect.right, WIN_W as f32 - 16.0);
        assert_eq!(
            text_x_origin(l.value_rect, TextAlign::Right, 0.0),
            l.value_rect.right
        );
        assert_eq!(
            text_x_origin(l.value_rect, TextAlign::Right, 42.0),
            l.value_rect.right - 42.0
        );
        // Rail: full content width, 8px tall, centered in the rail band.
        assert_eq!(l.track.left, 16.0);
        assert_eq!(l.track.right, WIN_W as f32 - 16.0);
        assert_eq!(l.track.height(), 8.0);
        // The slider's hit area is centered on the rail band.
        assert_eq!(
            (l.slider_rect.top + l.slider_rect.bottom) * 0.5,
            l.track.center_y()
        );
        assert_eq!(l.slider_rect.height(), 28.0);
        // Buttons: 36px tall, at least 8px apart, fully inside the surface.
        assert_eq!(l.mute_rect.height(), 36.0);
        assert_eq!(l.reset_rect.height(), 36.0);
        assert!(
            l.reset_rect.left - l.mute_rect.right >= 8.0,
            "button gap {}",
            l.reset_rect.left - l.mute_rect.right
        );
        assert!(l.mute_rect.left >= 16.0 && l.reset_rect.right <= WIN_W as f32);
        assert!(l.mute_rect.bottom <= WIN_H as f32 && l.mute_rect.top >= 0.0);
    }

    #[test]
    fn buttons_fit_inside_card_without_overlap() {
        let layout = MixerLayout::new(WIN_W as f32, WIN_H as f32);
        let content_right = WIN_W as f32 - 16.0;

        assert!(layout.mute_rect.right <= content_right);
        assert!(layout.reset_rect.right <= content_right);
        assert!(layout.mute_rect.right <= layout.reset_rect.left);
        assert!(layout.reset_rect.width() >= 200.0);
        assert!(
            layout.value_rect.bottom + VALUE_SLIDER_AIR <= layout.slider_rect.top,
            "volume value must leave the configured air before the slider"
        );
    }

    #[test]
    fn volume_value_moves_one_spacing_unit_above_the_slider() {
        let layout = MixerLayout::new(WIN_W as f32, WIN_H as f32);

        assert_eq!(layout.value_rect.top, 66.0);
        assert_eq!(layout.value_rect.bottom, 110.0);
        assert_eq!(layout.value_rect.height(), 44.0);
        assert!(
            layout.output_rect.bottom <= layout.value_rect.top,
            "caption and volume value boxes must not overlap"
        );
        assert!(
            layout.value_rect.bottom + 8.0 <= layout.slider_rect.top,
            "volume value must keep the expanded air above the slider"
        );
    }

    // ── rail integration (pure paint plan) ───────────────────────────────────

    #[test]
    fn mixer_plan_rail_matches_0_50_100_and_muted() {
        let t = plan(50, false).layout.track;
        for (percent, expected) in [
            (0u8, t.left),
            (50, t.left + t.width() * 0.5),
            (100, t.right),
        ] {
            let p = plan(percent, false);
            assert_eq!(p.geometry.fill_right, expected, "percent {percent}");
            let MarkerGeometry::Thumb {
                center_x, radius, ..
            } = p.geometry.marker
            else {
                panic!("percent {percent} must be a thumb");
            };
            assert_eq!(radius, THUMB_RADIUS);
            assert_eq!(center_x, expected.clamp(t.left + radius, t.right - radius));
        }
        // Threshold fill colours for 0/50/100 (default VolumePro bands).
        let (p0, p50, p100) = (plan(0, false), plan(50, false), plan(100, false));
        assert_eq!(p0.rail.fill_color(), p0.rail.thresholds.muted);
        assert_eq!(p50.rail.fill_color(), p50.rail.thresholds.medium);
        assert_eq!(p100.rail.fill_color(), p100.rail.thresholds.high);
        // Muted: the marker must be a diamond (never a thumb) at the same
        // center, with the muted grey fill.
        let normal = plan(50, false);
        let muted = plan(50, true);
        let MarkerGeometry::Thumb { center_x, .. } = normal.geometry.marker else {
            panic!("normal marker must be a thumb");
        };
        let MarkerGeometry::MutedDiamond {
            center_x: dx,
            half_size,
            ..
        } = muted.geometry.marker
        else {
            panic!("muted marker must be a diamond");
        };
        assert_eq!(dx, center_x, "same marker center as the thumb");
        assert_eq!(half_size, MUTED_DIAMOND_HALF_SIZE);
        assert_ne!(
            normal.geometry.marker, muted.geometry.marker,
            "muted marker must differ from the thumb"
        );
        assert_eq!(muted.rail.fill_color(), muted.rail.thresholds.muted);
    }

    #[test]
    fn rail_carries_user_threshold_boundaries_into_the_mixer_fill() {
        // A custom band config (green 25 / blue 60) must drive the rail fill
        // through the paint plan, matching core::volume_color_rgb semantics.
        let mut cfg = Config::default();
        cfg.color_thresholds.green_up_to = 25;
        cfg.color_thresholds.blue_up_to = 60;
        // 26% is the medium band only under the custom config.
        let p = plan_with(26, false, cfg.color_thresholds);
        assert_eq!(p.rail.green_up_to, 25);
        assert_eq!(p.rail.blue_up_to, 60);
        assert_eq!(p.rail.fill_color(), p.rail.thresholds.medium);
    }

    // ── two-layer focus ring ─────────────────────────────────────────────────

    #[test]
    fn focus_ring_has_two_distinct_layers_for_every_mixer_control() {
        // Task 1 cross-task follow-up: the mixer draws BOTH layers of the
        // two-layer FocusTokens ring. Each control rect must yield distinct
        // nested layers with distinct colours and widths.
        let a = appearance(ThemeMode::Dark, MaterialMode::Opaque, false);
        let focus = a.tokens.focus;
        assert_ne!(focus.ring, focus.inner_ring, "layer colours must differ");
        assert_ne!(
            focus.ring_width_px, focus.inner_ring_width_px,
            "layer widths must differ"
        );
        let l = MixerLayout::new(WIN_W as f32, WIN_H as f32);
        for rect in [l.slider_rect, l.mute_rect, l.reset_rect, l.close_rect] {
            let (outer, inner) = focus_ring_rects(rect, &focus);
            assert_ne!(outer, inner, "{rect:?}: layers must be distinct boxes");
            assert!(outer.contains(inner), "{rect:?}: outer must contain inner");
            assert!(
                inner.contains(rect),
                "{rect:?}: inner must surround the control"
            );
            assert!(
                outer.left >= 0.0 && outer.right <= WIN_W as f32,
                "{rect:?} ring in bounds"
            );
        }
    }

    #[test]
    fn value_row_clears_the_slider_focus_ring() {
        // Regression (user report 2026-08-04): with the slider focused, the
        // outer focus ring (3px gap + 1.5px stroke → 3.75px outset) used to
        // cut through the right-aligned value text — the old value row ended
        // at y 112, only 2px above the slider hit area (y 114), while the
        // ring's top stroke landed at y 110.25..111.75, right across the
        // value ink (live-verified: ring stroke pixels interleaved with the
        // "51%" text on a real mixer). The row now sits at y 66..110 with
        // 44px of vertical padding, leaving visible air before the ring for
        // both standard and high-contrast focus tokens.
        for (high_contrast, label) in [(false, "standard"), (true, "high-contrast")] {
            let focus = appearance(ThemeMode::Dark, MaterialMode::Opaque, high_contrast)
                .tokens
                .focus;
            let l = MixerLayout::new(WIN_W as f32, WIN_H as f32);
            assert_eq!(l.value_rect.height(), 44.0, "value row needs glyph padding");
            let (outer, inner) = focus_ring_rects(l.slider_rect, &focus);
            for (layer, rect) in [("outer", outer), ("inner", inner)] {
                assert!(
                    rect.top >= l.value_rect.bottom,
                    "{label} {layer} ring top ({}) must clear the value row bottom ({})",
                    rect.top,
                    l.value_rect.bottom
                );
                assert!(
                    rect.top - l.value_rect.bottom >= 2.0,
                    "{label} {layer} ring: at least 2px air below the value row"
                );
            }
        }
    }

    // ── sync / focus stability (real hidden window) ──────────────────────────

    #[test]
    fn sync_pushes_confirmed_state_into_the_trackbar_and_keeps_focus() {
        // Real-window test (hidden): sync() must move the trackbar to the
        // confirmed state, flip the mute button label, and never steal focus
        // from the focused control.
        let mixer = Mixer::new(0).expect("mixer window creates");
        unsafe {
            let d = &*(GetWindowLongPtrW(mixer.hwnd, GWLP_USERDATA) as *const MixerData);
            let a = appearance(ThemeMode::Dark, MaterialMode::Opaque, false);

            let normal = VolumeState {
                volume: 0.42,
                muted: false,
            };
            mixer.sync(&normal, &a);
            assert_eq!(SendMessageW(d.slider, TBM_GETPOS, 0, 0), 42);
            assert_eq!(window_text(d.mute_btn), "Mute");
            assert_eq!(window_text(d.reset_btn), "Reset volume to 50 percent");

            let muted = VolumeState {
                volume: 0.42,
                muted: true,
            };
            mixer.sync(&muted, &a);
            assert_eq!(
                SendMessageW(d.slider, TBM_GETPOS, 0, 0),
                42,
                "mute never changes the slider position"
            );
            assert_eq!(window_text(d.mute_btn), "Unmute");

            // Focus stability invariant: sync() must not steal focus or reset
            // the focused control (it only moves the slider, sets button
            // text, and invalidates paint regions). SetFocus works on the
            // hidden window (this thread owns it), so the invariant is
            // actually exercised.
            let _ = SetFocus(d.slider);
            assert_eq!(
                GetFocus(),
                d.slider,
                "SetFocus must work on the hidden mixer window"
            );
            mixer.sync(&muted, &a);
            assert_eq!(
                GetFocus(),
                d.slider,
                "sync must not move focus off the focused control"
            );
            mixer.sync(&normal, &a);
            assert_eq!(
                GetFocus(),
                d.slider,
                "sync must not move focus after a state change"
            );
        }
        drop(mixer);
    }

    #[test]
    fn mixer_controls_expose_spec_section_11_2_accessibility_names() {
        // The UIA name of a native control IS its window text, so the window
        // text asserts the names Narrator reads: the trackbar's name plus the
        // native value pattern yield `slider, System output volume, 72
        // percent`; the buttons carry their spec names. The close button is
        // OWNER-DRAWN (the × visual is painted by the owner) so its window
        // text can be the required `Close mixer`.
        let mixer = Mixer::new(0).expect("mixer window creates");
        unsafe {
            let d = &*(GetWindowLongPtrW(mixer.hwnd, GWLP_USERDATA) as *const MixerData);
            assert_eq!(
                window_text(d.slider),
                "System output volume",
                "trackbar name (spec §11.2)"
            );
            assert_eq!(
                window_text(d.reset_btn),
                "Reset volume to 50 percent",
                "reset button name (spec §11.2)"
            );
            assert_eq!(
                window_text(d.close_btn),
                "Close mixer",
                "close button name (spec §11.2)"
            );
            assert_eq!(window_text(d.mute_btn), "Mute");
            // The close button must be owner-drawn: the window text carries
            // the name while the owner paints the × visual.
            let style = GetWindowLongPtrW(d.close_btn, GWL_STYLE);
            assert_ne!(
                style & BS_OWNERDRAW as isize,
                0,
                "close button must be BS_OWNERDRAW"
            );
            let slider_style = GetWindowLongPtrW(d.slider, GWL_STYLE);
            assert_eq!(
                slider_style & BS_OWNERDRAW as isize,
                0,
                "the slider is not a button and must stay native"
            );
        }
        drop(mixer);
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
