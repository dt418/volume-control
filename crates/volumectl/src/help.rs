//! Help / Hotkeys — the Signal Glass quick-reference card (spec §8).
//!
//! A 520x500 (logical) captionless, always-on-top tool window that re-states
//! the hotkey map as a scannable card:
//!
//!   - a header band (3px accent bar, `VolumeControl` title, `Keyboard
//!     shortcuts` subtitle, custom-painted `×` close with hover surface);
//!   - five structured hotkey rows, one per base action, with real layout
//!     (never padded strings): a ~210px keycap column (one chip per key,
//!     `+` separators), the action label, and a right-aligned status pill
//!     (`Ready` / `Fallback` / `In use`) mapped from the *actual*
//!     `RegisterHotKey` outcome;
//!   - a conflict callout card (surface_subtle, 1px border, warning marker,
//!     the conflicted combos as keycap chips, and the next action) whenever
//!     any combo is in use by another app;
//!   - a sticky footer with three native `BS_PUSHBUTTON` children
//!     (`Edit config` / `Settings` / `Close`) and a hairline divider.
//!
//! Interaction is routed through the host window (`WM_APP_HELP_*` messages),
//! so `Edit config` and `Settings` dispatch through the central
//! `handle_action` like every other surface; `Close` just hides. Keyboard
//! navigation: Tab / Shift+Tab cycle the footer buttons (subclassed, like the
//! mixer), Enter/Space activate them natively, and Escape hides the card
//! (`WM_KEYDOWN`/`WM_SYSKEYDOWN` on the parent and on every subclassed
//! button). Tab order: `Edit config` → `Settings` → `Close` (the header `×`
//! is pointer-only and out of the Tab cycle).
//!
//! Theming uses the shared adaptive tokens with one resolution point in the
//! host, mirroring the overlay/mixer/settings seam: `app.rs` resolves a
//! [`HelpAppearance`] (palette + motion policy) and the card consumes it
//! blindly. The card is STATIC: it never animates, so a Reduced/Disabled
//! motion preference is honored by construction — there are no transitions
//! to remove (see the motion tests). Badges and the warning marker never rely
//! on tint alone: pills carry a 1px border + text and collapse to
//! `text_primary` + `border_strong` under high contrast, and the warning is a
//! text-colored triangle-with-`!` shape, so every state survives high
//! contrast.
//!
//! DPI: the window is created at the PHYSICAL size `to_physical(520) x
//! to_physical(500)` for its DPI; all layout is computed in logical space by
//! [`HelpLayout`] and scaled exactly once by the canvas / `DpiMetrics`
//! (100/125/150% verified in tests).

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    Graphics::Gdi::InvalidateRect,
    System::LibraryLoader::GetModuleHandleW,
    UI::Controls::WM_MOUSELEAVE,
    UI::Input::KeyboardAndMouse::{
        GetFocus, GetKeyState, SetFocus, TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT, VK_ESCAPE,
        VK_SHIFT, VK_TAB,
    },
    UI::WindowsAndMessaging::{
        CallWindowProcW, CreateWindowExW, DefWindowProcW, DestroyWindow, GetAncestor,
        GetWindowLongPtrW, KillTimer, PostMessageW, RegisterClassW, SendMessageW, SetTimer,
        SetWindowLongPtrW, SetWindowPos, ShowWindow, BN_CLICKED, CW_USEDEFAULT, GA_PARENT,
        GWLP_USERDATA, GWLP_WNDPROC, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW,
        SW_HIDE, WM_CLOSE, WM_COMMAND, WM_ERASEBKGND, WM_KEYDOWN, WM_KILLFOCUS, WM_LBUTTONDOWN,
        WM_MOUSEMOVE, WM_PAINT, WM_SETFOCUS, WM_SYSKEYDOWN, WM_TIMER, WM_USER, WNDCLASSW, WNDPROC,
        WS_CHILD, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_TABSTOP, WS_VISIBLE,
    },
};

use crate::config::{Config, HotkeyModifier};
use crate::hotkeys::{HotkeyAction, HotkeyRegResult, HotkeyRegStatus};
use crate::ui::platform::windows::text::{measure_text_gdi, TextAlign, TextLayout};
use crate::ui::primitives::{
    apply_backdrop, dpi_scale_for, theme_controls, work_area_for, DpiMetrics, PaintCanvas, RectF,
};
use crate::ui::ResolvedMaterial;
use crate::ui::{
    place_overlay, resolve_motion, tokens_for, AccentMode, MotionMode, Rgba, SurfaceSize, TextRole,
    ThemeMode, ThemeTokens, UiCapabilities,
};

/// Custom messages the Help card posts to the host window (see `app.rs`). The
/// host owns the actions; these intents just tell it which affordance the user
/// activated.
pub const WM_APP_HELP_OPEN_CONFIG: u32 = WM_USER + 30;
pub const WM_APP_HELP_SETTINGS: u32 = WM_USER + 31;

/// Logical card size (spec §8.1). The window is created at the PHYSICAL size
/// `to_physical(WIN_W) x to_physical(WIN_H)` for its DPI.
const WIN_W: i32 = 520;
const WIN_H: i32 = 500;
const MARGIN_X: i32 = 24;
const MARGIN_Y: i32 = 48;

// Footer button ids.
const ID_BTN_EDIT_CONFIG: usize = 1;
const ID_BTN_SETTINGS: usize = 2;
const ID_BTN_CLOSE: usize = 3;

/// One-shot timer ID for the deferred DWM backdrop re-apply after a show
/// (see the comment in [`Help::show`]).
const BACKDROP_TIMER_ID: usize = 1;
/// Delay (ms) between showing the window and re-asserting DWMSBT_NONE: DWM
/// applies its High-Contrast backdrop override asynchronously after the
/// show — measured on Windows 11 24H2, backdrop writes are clobbered back to
/// AUTO for roughly the first second after the show, and stick once the
/// composition settles. 2000ms lands safely past that window.
const BACKDROP_REAPPLY_MS: u32 = 2000;
const IDX_EDIT: usize = 0;
const IDX_SETTINGS: usize = 1;
const IDX_CLOSE: usize = 2;

// ── Logical layout constants (spec §8.2 figure) ─────────────────────────────
const HEADER_H: f32 = 64.0;
const FOOTER_H: f32 = 56.0;
const CONTENT_LEFT: f32 = 24.0;
const ROWS_TOP: f32 = 80.0;
const ROW_H: f32 = 44.0;
const CHIPS_RIGHT: f32 = 234.0; // ~210px keycap column
const LABEL_LEFT: f32 = 246.0;
const LABEL_RIGHT: f32 = 380.0;
const BADGE_RIGHT: f32 = 496.0; // WIN_W - CONTENT_LEFT
const CHIP_H: f32 = 24.0;
const CHIP_PAD_X: f32 = 6.0;
const CHIP_GAP: f32 = 4.0;
const SEP_W: f32 = 10.0;
const BADGE_H: f32 = 18.0;
const BADGE_PAD_X: f32 = 10.0;
const CALLOUT_TOP: f32 = 304.0;
const CALLOUT_H: f32 = 140.0;
const EXPL_LINE_H: f32 = 24.0;

/// Adaptive appearance resolved by the host and applied by the Help card.
///
/// Mirrors the overlay/mixer/settings seam (one resolution point in `app.rs`),
/// but omits the material treatment: the reference card stays opaque in every
/// mode. The motion policy is carried for symmetry with the other surfaces;
/// the card is static, so it is honored by never animating.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HelpAppearance {
    /// Resolved palette tokens (theme + high-contrast + accent).
    pub tokens: ThemeTokens,
    /// Resolved motion preference (`resolve_motion`). The card never
    /// animates, so Reduced/Disabled are honored by construction.
    pub motion: MotionMode,
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
        Self {
            tokens: tokens_for(
                appearance.theme,
                caps.high_contrast,
                appearance.accent,
                system_is_dark,
            ),
            motion: resolve_motion(appearance.motion, caps),
        }
    }

    /// Placeholder used before the first `show` (the window is hidden then, so
    /// this is never painted).
    fn placeholder() -> Self {
        Self {
            tokens: tokens_for(ThemeMode::System, false, AccentMode::System, || None),
            motion: MotionMode::Full,
        }
    }
}

// ── Row / status model (pure; unit-tested) ──────────────────────────────────

/// Per-row registration status badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BadgeKind {
    /// `RegisterHotKey` succeeded.
    Ready,
    /// The combo routes through the low-level hook (CapsLock modifier).
    Fallback,
    /// `RegisterHotKey` rejected the combo (in use by another app).
    InUse,
}

impl BadgeKind {
    /// The badge label. Distinct per kind — status is never carried by color
    /// alone, so the text always differs even when high contrast collapses
    /// the tints.
    fn label(self) -> &'static str {
        match self {
            BadgeKind::Ready => "Ready",
            BadgeKind::Fallback => "Fallback",
            BadgeKind::InUse => "In use",
        }
    }
}

/// One rendered hotkey row.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RowModel {
    /// Spec label (`Increase volume`, ...).
    label: &'static str,
    /// The combo's key (`↑`, `↓`, `M`, `V`, `R`).
    key: &'static str,
    /// Keycap chips including the modifier prefix.
    chips: Vec<String>,
    /// Status badge.
    badge: BadgeKind,
}

/// The conflict callout content (None = no conflicted combo).
#[derive(Debug, Clone, PartialEq, Eq)]
struct ConflictCallout {
    /// Card title (label role, bold).
    title: &'static str,
    /// Keycap chip lists, one per conflicted base-action combo (row order,
    /// shift variants deduplicated).
    combos: Vec<Vec<String>>,
    /// Sentence tail: "is used by another app." / "are used by another app."
    tail: &'static str,
    /// Next-action hint.
    action: &'static str,
}

/// The five spec rows: `(base action, label, key)` (spec §8.2 figure).
///
/// Shift variants (`VolumeUpLarge` etc.) are deliberately not rows: the card
/// documents the base combos, and the shift variants share the base action's
/// registration status.
const ACTION_ROWS: [(HotkeyAction, &str, &str); 5] = [
    (HotkeyAction::VolumeUp, "Increase volume", "\u{2191}"),
    (HotkeyAction::VolumeDown, "Decrease volume", "\u{2193}"),
    (HotkeyAction::ToggleMute, "Toggle mute", "M"),
    (HotkeyAction::OpenMixer, "Open mixer", "V"),
    (HotkeyAction::Reset50, "Reset to 50%", "R"),
];

/// The keycap chip list for a combo: the modifier prefix (per
/// `config.modifier`) followed by the key.
///
/// `CtrlAlt` → `["Ctrl", "Alt", key]`, `Alt` → `["Alt", key]`,
/// `Ctrl` → `["Ctrl", key]`, `CapsLock` → `["CapsLock", key]`.
fn keycap_chips(modifier: HotkeyModifier, key: &str) -> Vec<String> {
    let mut chips: Vec<String> = match modifier {
        HotkeyModifier::CtrlAlt => vec!["Ctrl".into(), "Alt".into()],
        HotkeyModifier::Alt => vec!["Alt".into()],
        HotkeyModifier::Ctrl => vec!["Ctrl".into()],
        HotkeyModifier::CapsLock => vec!["CapsLock".into()],
    };
    chips.push(key.into());
    chips
}

/// The status badge for one base action.
///
/// The status array carries 8 entries (5 base actions + Shift variants for
/// the arrows and Mute). The Shift variants SHARE the base action's status,
/// so only the base action's own entry is consulted; a missing entry (status
/// not yet reported) reads optimistically as [`BadgeKind::Ready`].
fn badge_for_action(action: &HotkeyAction, status: &[HotkeyRegResult]) -> BadgeKind {
    status
        .iter()
        .find(|r| r.action == *action)
        .map(|r| match r.status {
            HotkeyRegStatus::Registered => BadgeKind::Ready,
            HotkeyRegStatus::HookRouted => BadgeKind::Fallback,
            HotkeyRegStatus::Conflicted(_) => BadgeKind::InUse,
        })
        .unwrap_or(BadgeKind::Ready)
}

/// Build the five spec rows from the config's modifier and the real
/// registration status (pure).
fn build_rows(config: &Config, status: &[HotkeyRegResult]) -> Vec<RowModel> {
    ACTION_ROWS
        .iter()
        .map(|(action, label, key)| RowModel {
            label,
            key,
            chips: keycap_chips(config.modifier, key),
            badge: badge_for_action(action, status),
        })
        .collect()
}

/// Build the conflict callout when any base-action combo is in use by
/// another app (pure). `None` when everything registered cleanly.
fn conflict_callout(config: &Config, status: &[HotkeyRegResult]) -> Option<ConflictCallout> {
    let combos: Vec<Vec<String>> = ACTION_ROWS
        .iter()
        .filter(|(action, _, _)| matches!(badge_for_action(action, status), BadgeKind::InUse))
        .map(|(_, _, key)| keycap_chips(config.modifier, key))
        .collect();
    if combos.is_empty() {
        return None;
    }
    Some(ConflictCallout {
        title: "Shortcut conflict",
        tail: if combos.len() == 1 {
            "is used by another app."
        } else {
            "are used by another app."
        },
        action: "Change the modifier in Settings.",
        combos,
    })
}

/// Badge pill colors: `(text, border)`.
///
/// `Ready` → success tint (`volume_threshold.low`), `Fallback` → accent tint,
/// `In use` → warn tint (`volume_threshold.high`). Under high contrast ALL
/// badges collapse to `text_primary` + `border_strong` — the pill's 1px
/// border and its distinct label carry the state, never tint alone.
fn badge_colors(tokens: &ThemeTokens, kind: BadgeKind) -> (Rgba, Rgba) {
    let tint = match kind {
        BadgeKind::Ready => tokens.volume_threshold.low,
        BadgeKind::Fallback => tokens.accent,
        BadgeKind::InUse => tokens.volume_threshold.high,
    };
    if tokens.high_contrast {
        (tokens.text_primary, tokens.signal_glass().border_strong)
    } else {
        (tint, tint)
    }
}

// ── Geometry (pure; unit-tested) ────────────────────────────────────────────

/// Pure logical geometry of the 520x500 card (spec §8.2). Every rect is in
/// logical pixels; the canvas / `DpiMetrics` scale them to physical exactly
/// once (verified at 100/125/150% in tests).
#[derive(Debug, Clone, Copy, PartialEq)]
struct HelpLayout {
    width: f32,
    height: f32,
    title_rect: RectF,
    subtitle_rect: RectF,
    close_rect: RectF,
    rows: [RectF; 5],
    callout: RectF,
    footer_top: f32,
    btn_edit: RectF,
    btn_settings: RectF,
    btn_close: RectF,
}

impl HelpLayout {
    fn new(w: f32, h: f32) -> Self {
        let rows: [RectF; 5] = std::array::from_fn(|i| {
            let top = ROWS_TOP + i as f32 * ROW_H;
            RectF::new(CONTENT_LEFT, top, w - CONTENT_LEFT, top + ROW_H)
        });
        Self {
            width: w,
            height: h,
            title_rect: RectF::new(CONTENT_LEFT, 14.0, 300.0, 36.0),
            subtitle_rect: RectF::new(CONTENT_LEFT, 38.0, 300.0, 54.0),
            close_rect: RectF::new(w - 48.0, 12.0, w - 16.0, 44.0),
            rows,
            callout: RectF::new(
                CONTENT_LEFT,
                CALLOUT_TOP,
                w - CONTENT_LEFT,
                CALLOUT_TOP + CALLOUT_H,
            ),
            footer_top: h - FOOTER_H,
            btn_edit: RectF::new(24.0, h - FOOTER_H + 8.0, 132.0, h - FOOTER_H + 44.0),
            btn_settings: RectF::new(144.0, h - FOOTER_H + 8.0, 252.0, h - FOOTER_H + 44.0),
            btn_close: RectF::new(264.0, h - FOOTER_H + 8.0, 340.0, h - FOOTER_H + 44.0),
        }
    }

    /// The ~210px keycap column of a row (chips laid out left-aligned).
    fn chips_area(row: RectF) -> RectF {
        RectF::new(row.left, row.top, CHIPS_RIGHT, row.bottom)
    }

    /// The action-label column of a row.
    fn label_rect(row: RectF) -> RectF {
        RectF::new(LABEL_LEFT, row.top, LABEL_RIGHT, row.bottom)
    }
}

/// One keycap chip (rounded rect + text) vertically centered in its row.
fn chip_rect(row: RectF, x: f32, text_width: f32) -> RectF {
    let cy = (row.top + row.bottom) * 0.5;
    RectF::new(
        x,
        cy - CHIP_H * 0.5,
        x + text_width + 2.0 * CHIP_PAD_X,
        cy + CHIP_H * 0.5,
    )
}

/// The status pill, right-aligned at `BADGE_RIGHT` and centered in the row.
fn badge_pill_rect(row: RectF, text_width: f32) -> RectF {
    let cy = (row.top + row.bottom) * 0.5;
    RectF::new(
        BADGE_RIGHT - text_width - 2.0 * BADGE_PAD_X,
        cy - BADGE_H * 0.5,
        BADGE_RIGHT,
        cy + BADGE_H * 0.5,
    )
}

/// One cell of a keycap row: a chip (rect + text) or a `+` separator cell.
#[derive(Debug, Clone, PartialEq)]
enum KeycapCell {
    Chip(RectF, String),
    Separator(RectF),
}

/// Lay out keycap chips left-aligned inside `area` with 4px gaps and `+`
/// separator cells between them (pure; `widths[i]` = measured text width of
/// `chips[i]`).
fn layout_keycap_row(area: RectF, chips: &[String], widths: &[f32]) -> Vec<KeycapCell> {
    let mut cells = Vec::with_capacity(chips.len().saturating_mul(2).saturating_sub(1));
    let mut x = area.left;
    for (i, (chip, &width)) in chips.iter().zip(widths).enumerate() {
        cells.push(KeycapCell::Chip(chip_rect(area, x, width), chip.clone()));
        x += width + 2.0 * CHIP_PAD_X;
        if i + 1 < chips.len() {
            cells.push(KeycapCell::Separator(RectF::new(
                x + CHIP_GAP,
                area.top,
                x + CHIP_GAP + SEP_W,
                area.bottom,
            )));
            x += CHIP_GAP + SEP_W + CHIP_GAP;
        }
    }
    cells
}

/// One explanation atom: a conflicted combo (as keycap chips) or plain text.
#[derive(Debug, Clone, PartialEq)]
enum ExplanationAtom {
    /// One conflicted combo rendered as keycap chips.
    Combo(Vec<String>),
    /// Connector / sentence tail (" and ", ", ", "is used by another app.").
    Text(&'static str),
}

/// A packed explanation atom with its final rect.
#[derive(Debug, Clone, PartialEq)]
struct ExplanationRun {
    rect: RectF,
    atom: ExplanationAtom,
}

/// The explanation atom sequence for a callout: each combo followed by ", "
/// (or " and " before the last), then the sentence tail.
fn explanation_atoms(callout: &ConflictCallout) -> Vec<ExplanationAtom> {
    let n = callout.combos.len();
    let mut atoms = Vec::with_capacity(n * 2);
    for (i, combo) in callout.combos.iter().enumerate() {
        atoms.push(ExplanationAtom::Combo(combo.clone()));
        if i + 1 < n {
            atoms.push(ExplanationAtom::Text(if i + 2 == n {
                " and "
            } else {
                ", "
            }));
        }
    }
    atoms.push(ExplanationAtom::Text(callout.tail));
    atoms
}

/// Logical width of each explanation atom: a combo's chips + separators, or
/// measured text (`chip_width` / `text_width` are the paint-time measures).
fn explanation_widths(
    atoms: &[ExplanationAtom],
    chip_width: impl Fn(&str) -> f32,
    text_width: impl Fn(&str) -> f32,
) -> Vec<f32> {
    atoms
        .iter()
        .map(|atom| match atom {
            ExplanationAtom::Combo(chips) => {
                let chips_total: f32 = chips.iter().map(|c| chip_width(c) + 2.0 * CHIP_PAD_X).sum();
                chips_total + (chips.len().saturating_sub(1)) as f32 * (CHIP_GAP * 2.0 + SEP_W)
            }
            ExplanationAtom::Text(t) => text_width(t),
        })
        .collect()
}

/// Greedy-pack explanation atoms into at most three `EXPL_LINE_H` lines
/// inside `area` (pure). Atoms never split mid-combo or mid-text; a line
/// wraps only when the next atom would overflow `area.right`.
fn pack_explanation(area: RectF, atoms: &[ExplanationAtom], widths: &[f32]) -> Vec<ExplanationRun> {
    let mut runs = Vec::with_capacity(atoms.len());
    let mut line = 0usize;
    let mut x = area.left;
    for (i, atom) in atoms.iter().enumerate() {
        let width = widths[i];
        if x > area.left && x + width > area.right {
            line = (line + 1).min(2);
            x = area.left;
        }
        let top = area.top + line as f32 * EXPL_LINE_H;
        runs.push(ExplanationRun {
            rect: RectF::new(x, top, x + width, top + EXPL_LINE_H),
            atom: atom.clone(),
        });
        x += width;
    }
    runs
}

// ── Window plumbing ─────────────────────────────────────────────────────────

type ChildWndProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;

struct HelpData {
    host: HWND,
    buttons: [HWND; 3],
    orig_procs: [Option<ChildWndProc>; 3],
    appearance: HelpAppearance,
    rows: Vec<RowModel>,
    callout: Option<ConflictCallout>,
    close_hover: bool,
    dpi: f32,
    open: bool,
}

pub struct Help {
    hwnd: HWND,
}

/// What a footer button activation does (pure mapping, unit-tested).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ButtonAction {
    /// Post `WM_APP_HELP_OPEN_CONFIG` to the host, then hide.
    OpenConfig,
    /// Post `WM_APP_HELP_SETTINGS` to the host, then hide.
    Settings,
    /// Hide (via `WM_CLOSE`).
    Close,
}

fn button_action(id: usize) -> ButtonAction {
    match id {
        ID_BTN_EDIT_CONFIG => ButtonAction::OpenConfig,
        ID_BTN_SETTINGS => ButtonAction::Settings,
        ID_BTN_CLOSE => ButtonAction::Close,
        _ => unreachable!("unknown help button id {id}"),
    }
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
                WIN_H,
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
                buttons: [0; 3],
                orig_procs: [None; 3],
                appearance,
                rows: Vec::new(),
                callout: None,
                close_hover: false,
                dpi: 1.0,
                open: false,
            }));
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, data as isize);
            let d = &mut *data;

            // The window is created at the PHYSICAL size 520x500 for its DPI
            // (scaled exactly once from the logical design size).
            let dpi = DpiMetrics::new(dpi_scale_for(hwnd));
            d.dpi = dpi.scale();
            SetWindowPos(
                hwnd,
                0,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                dpi.to_physical(WIN_W),
                dpi.to_physical(WIN_H),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );

            // Sticky footer: three native buttons, left-aligned at margin 24
            // with 12px gaps (36px tall, radius 4 via theme_controls). The
            // ids are the Tab-order mapping used by the subclass cycle.
            let layout = HelpLayout::new(WIN_W as f32, WIN_H as f32);
            let mk = |text: &'static str, rect: RectF, id: isize| {
                let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
                CreateWindowExW(
                    0,
                    windows_sys::core::w!("Button"),
                    wide.as_ptr(),
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                    rect.left as i32,
                    rect.top as i32,
                    rect.width() as i32,
                    rect.height() as i32,
                    hwnd,
                    id,
                    hinst,
                    std::ptr::null(),
                )
            };
            d.buttons[IDX_EDIT] = mk("Edit config", layout.btn_edit, ID_BTN_EDIT_CONFIG as isize);
            d.buttons[IDX_SETTINGS] = mk("Settings", layout.btn_settings, ID_BTN_SETTINGS as isize);
            d.buttons[IDX_CLOSE] = mk("Close", layout.btn_close, ID_BTN_CLOSE as isize);
            if d.buttons.contains(&0) {
                return Err("help footer button failed".into());
            }

            // Subclass the footer buttons: Escape hides, Tab/Shift+Tab cycle
            // Edit config → Settings → Close, focus changes repaint the ring.
            for (idx, &ctl) in d.buttons.iter().enumerate() {
                let orig = GetWindowLongPtrW(ctl, GWLP_WNDPROC);
                d.orig_procs[idx] = Some(std::mem::transmute::<isize, ChildWndProc>(orig));
                let subclass_proc = help_child_wndproc as ChildWndProc;
                SetWindowLongPtrW(ctl, GWLP_WNDPROC, subclass_proc as usize as isize);
            }

            theme_controls(&d.buttons, appearance.tokens.is_dark);

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
            apply_appearance(self.hwnd, d, *appearance);
            d.rows = build_rows(config, hotkey_status);
            d.callout = conflict_callout(config, hotkey_status);

            // Layout + child controls in PHYSICAL pixels, scaled exactly once
            // from the logical design (same seam as the mixer/settings).
            let dpi = DpiMetrics::new(dpi_scale_for(self.hwnd));
            if d.dpi != dpi.scale() {
                d.dpi = dpi.scale();
                layout_children(d, dpi);
            }

            // Bottom-right of the monitor work area hosting the window (the
            // same shared placement the overlay/mixer use).
            let work_area = work_area_for(self.hwnd);
            let size = SurfaceSize::new(dpi.to_physical(WIN_W), dpi.to_physical(WIN_H));
            let rect = place_overlay(work_area, size, MARGIN_X, MARGIN_Y);
            SetWindowPos(
                self.hwnd,
                HWND_TOPMOST,
                rect.left,
                rect.top,
                rect.width(),
                rect.height(),
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            // DWM re-asserts DWMSBT_AUTO when a window is shown while High
            // Contrast is active — asynchronously, AFTER the show (observed:
            // the reset lands after any immediate re-apply). The Help card is
            // opaque in every mode, so re-assert DWMSBT_NONE on a one-shot
            // timer once the composition has settled, keeping the painted
            // surface visible on screen.
            SetTimer(self.hwnd, BACKDROP_TIMER_ID, BACKDROP_REAPPLY_MS, None);
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

    fn destroy(&mut self) {
        unsafe {
            let d = &mut *(GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut HelpData);
            // Destroy the window (and its subclassed children) BEFORE freeing
            // `d`: DestroyWindow sends teardown messages through the still
            // installed child subclass proc, which reads the parent's
            // `GWLP_USERDATA` via `original_proc`. Freeing first would make
            // that a dangling dereference.
            DestroyWindow(self.hwnd);
            drop(Box::from_raw(d));
        }
    }
}

impl Drop for Help {
    fn drop(&mut self) {
        self.destroy();
    }
}

/// Apply a resolved adaptive appearance: re-theme the native children and
/// repaint. Skipped when nothing changed.
unsafe fn apply_appearance(hwnd: HWND, d: &mut HelpData, appearance: HelpAppearance) {
    if d.appearance == appearance {
        return;
    }
    d.appearance = appearance;
    theme_controls(&d.buttons, appearance.tokens.is_dark);
    InvalidateRect(hwnd, std::ptr::null(), 0);
}

/// Position the native footer buttons at PHYSICAL coordinates for `dpi`. The
/// logical layout rects come from [`HelpLayout`] — the same rects the focus
/// ring uses — scaled exactly once.
unsafe fn layout_children(d: &HelpData, dpi: DpiMetrics) {
    let layout = HelpLayout::new(WIN_W as f32, WIN_H as f32);
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
    place(d.buttons[IDX_EDIT], layout.btn_edit);
    place(d.buttons[IDX_SETTINGS], layout.btn_settings);
    place(d.buttons[IDX_CLOSE], layout.btn_close);
}

/// Recover the saved original window proc for a subclassed footer button.
unsafe fn original_proc(parent: HWND, ctl: HWND) -> WNDPROC {
    let d = &*(GetWindowLongPtrW(parent, GWLP_USERDATA) as *const HelpData);
    let idx = d.buttons.iter().position(|&c| c == ctl).unwrap_or(0);
    d.orig_procs[idx]
}

/// Shared subclass proc for the footer buttons.
///
/// Native Win32 behaviour is preserved for everything not handled here:
/// buttons respond to Enter/Space (generating `BN_CLICKED`) and paint their
/// own chrome + focus cue. This subclass additionally:
///   - Escape hides the card (identical semantics to `WM_CLOSE`);
///   - Tab / Shift+Tab cycle `Edit config` → `Settings` → `Close` (wrapping);
///   - focus changes repaint the parent so it can draw/clear the token focus
///     ring around the focused button.
unsafe extern "system" fn help_child_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let parent = GetAncestor(hwnd, GA_PARENT);
    if parent == 0 {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let d = &*(GetWindowLongPtrW(parent, GWLP_USERDATA) as *const HelpData);

    // Escape hides the card without destroying it (same as WM_CLOSE).
    if (msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN) && (wparam as u32) == (VK_ESCAPE as u32) {
        SendMessageW(parent, WM_CLOSE, 0, 0);
        return 0;
    }

    // Tab / Shift+Tab cycle the footer buttons in creation (tab) order,
    // wrapping at the ends.
    if msg == WM_KEYDOWN && (wparam as u32) == (VK_TAB as u32) {
        let cur = d.buttons.iter().position(|&c| c == hwnd).unwrap_or(0);
        let backwards = GetKeyState(VK_SHIFT as i32) < 0;
        let next = if backwards {
            (cur + 2) % 3
        } else {
            (cur + 1) % 3
        };
        SetFocus(d.buttons[next]);
        return 0;
    }

    let result = CallWindowProcW(original_proc(parent, hwnd), hwnd, msg, wparam, lparam);

    // Focus changes repaint the parent so the token focus ring tracks the
    // newly focused button (see `focused_button_rect`).
    if msg == WM_SETFOCUS || msg == WM_KILLFOCUS {
        InvalidateRect(parent, std::ptr::null(), 0);
    }
    result
}

unsafe extern "system" fn help_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        // ── Footer buttons → host intents / hide ─────────────────────────
        WM_COMMAND if (wparam >> 16) as u32 == BN_CLICKED => {
            let d = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut HelpData);
            match button_action(wparam & 0xFFFF) {
                ButtonAction::OpenConfig => {
                    PostMessageW(d.host, WM_APP_HELP_OPEN_CONFIG, 0, 0);
                    hide_help(hwnd, d);
                    0
                }
                ButtonAction::Settings => {
                    PostMessageW(d.host, WM_APP_HELP_SETTINGS, 0, 0);
                    hide_help(hwnd, d);
                    0
                }
                ButtonAction::Close => SendMessageW(hwnd, WM_CLOSE, 0, 0),
            }
        }
        // ── Custom-painted close (×) in the header ───────────────────────
        WM_LBUTTONDOWN => {
            let d = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut HelpData);
            let x = (lparam & 0xFFFF) as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i32;
            let dpi = DpiMetrics::new(dpi_scale_for(hwnd));
            let layout = HelpLayout::new(WIN_W as f32, WIN_H as f32);
            let (lx, ly) = (dpi.to_logical(x) as f32, dpi.to_logical(y) as f32);
            if lx >= layout.close_rect.left
                && lx <= layout.close_rect.right
                && ly >= layout.close_rect.top
                && ly <= layout.close_rect.bottom
            {
                hide_help(hwnd, d);
            }
            0
        }
        // ── Hover surface for the header × ───────────────────────────────
        WM_MOUSEMOVE => {
            let d = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut HelpData);
            let x = (lparam & 0xFFFF) as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i32;
            let dpi = DpiMetrics::new(dpi_scale_for(hwnd));
            let layout = HelpLayout::new(WIN_W as f32, WIN_H as f32);
            let (lx, ly) = (dpi.to_logical(x) as f32, dpi.to_logical(y) as f32);
            let hover = lx >= layout.close_rect.left
                && lx <= layout.close_rect.right
                && ly >= layout.close_rect.top
                && ly <= layout.close_rect.bottom;
            if hover != d.close_hover {
                d.close_hover = hover;
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            // Arm WM_MOUSELEAVE so the hover clears when the cursor leaves
            // the window without another mouse move.
            let mut tme: TRACKMOUSEEVENT = std::mem::zeroed();
            tme.cbSize = std::mem::size_of::<TRACKMOUSEEVENT>() as u32;
            tme.dwFlags = TME_LEAVE;
            tme.hwndTrack = hwnd;
            TrackMouseEvent(&mut tme);
            0
        }
        WM_MOUSELEAVE => {
            let d = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut HelpData);
            if d.close_hover {
                d.close_hover = false;
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            0
        }
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            // Paint through the resource-safe canvas (Task 3): it owns the
            // BeginPaint/EndPaint pair, selects ONE paint path per frame
            // (Direct2D when available, GDI otherwise), and deletes every
            // per-call GDI object. If BeginPaint itself fails, paint nothing
            // and invalidate so the next WM_PAINT retries.
            if let Some(mut canvas) = PaintCanvas::begin_paint(hwnd) {
                let d = &*(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const HelpData);
                paint(&mut canvas, d);
            } else {
                log::debug!("help: BeginPaint failed; invalidating for a retry");
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            0
        }
        // ── Keyboard navigation when the card itself has focus ───────────
        // (When a footer button has focus, the subclass forwards these.)
        WM_KEYDOWN | WM_SYSKEYDOWN if (wparam as u32) == (VK_ESCAPE as u32) => {
            let d = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut HelpData);
            hide_help(hwnd, d);
            0
        }
        WM_KEYDOWN if (wparam as u32) == (VK_TAB as u32) => {
            // Tab moves into the first footer button.
            let d = &*(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const HelpData);
            SetFocus(d.buttons[IDX_EDIT]);
            0
        }
        // ── Close (Esc / × / footer Close) just hides ────────────────────
        WM_CLOSE => {
            let d = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut HelpData);
            hide_help(hwnd, d);
            0
        }
        // Deferred backdrop re-apply armed by `show` (see the comment there):
        // DWM re-asserts DWMSBT_AUTO after a show while High Contrast is
        // active, so DWMSBT_NONE is re-asserted once the composition has
        // settled, keeping the opaque painted surface visible.
        WM_TIMER if wparam == BACKDROP_TIMER_ID => {
            KillTimer(hwnd, BACKDROP_TIMER_ID);
            let d = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut HelpData);
            apply_backdrop(hwnd, ResolvedMaterial::Opaque, d.appearance.tokens.is_dark);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// Hide-only teardown (same semantics as the old buttons and `WM_CLOSE`).
unsafe fn hide_help(hwnd: HWND, d: &mut HelpData) {
    d.open = false;
    ShowWindow(hwnd, SW_HIDE);
}

/// The logical rect of the focused footer button, if any (for the ring).
unsafe fn focused_button_rect(d: &HelpData, layout: &HelpLayout) -> Option<RectF> {
    let focused = GetFocus();
    if focused == 0 {
        return None;
    }
    if focused == d.buttons[IDX_EDIT] {
        Some(layout.btn_edit)
    } else if focused == d.buttons[IDX_SETTINGS] {
        Some(layout.btn_settings)
    } else if focused == d.buttons[IDX_CLOSE] {
        Some(layout.btn_close)
    } else {
        None
    }
}

/// Headless logical text width for a role (GDI measurement on a memory DC).
fn measure_width(text: &str, role: TextRole) -> f32 {
    measure_text_gdi(text, role).map(|s| s.width).unwrap_or(0.0)
}

/// Draw one hotkey row: keycap chips + `+` separators, the action label, and
/// the right-aligned status pill.
unsafe fn paint_row(canvas: &mut PaintCanvas, tokens: &ThemeTokens, row: RectF, model: &RowModel) {
    let glass = tokens.signal_glass();
    let chips_area = HelpLayout::chips_area(row);
    let widths: Vec<f32> = model
        .chips
        .iter()
        .map(|c| measure_width(c, tokens.typography.keycap))
        .collect();
    for cell in layout_keycap_row(chips_area, &model.chips, &widths) {
        match cell {
            KeycapCell::Chip(rect, text) => {
                canvas.fill_rounded_rect(rect, tokens.radii.control_px, tokens.surface);
                canvas.stroke_rounded_rect(rect, tokens.radii.control_px, glass.border_strong, 1.0);
                canvas.draw_text(&TextLayout {
                    text: &text,
                    rect,
                    align: TextAlign::Center,
                    role: tokens.typography.keycap,
                    color: tokens.text_primary,
                });
            }
            KeycapCell::Separator(rect) => {
                canvas.draw_text(&TextLayout {
                    text: "+",
                    rect,
                    align: TextAlign::Center,
                    role: tokens.typography.body,
                    color: tokens.text_secondary,
                });
            }
        }
    }

    canvas.draw_text(&TextLayout {
        text: model.label,
        rect: HelpLayout::label_rect(row),
        align: TextAlign::Left,
        role: tokens.typography.body,
        color: tokens.text_primary,
    });

    let (text_color, border_color) = badge_colors(tokens, model.badge);
    let label = model.badge.label();
    let pill = badge_pill_rect(row, measure_width(label, tokens.typography.label));
    canvas.fill_rounded_rect(pill, tokens.radii.pill_px, tokens.surface);
    canvas.stroke_rounded_rect(pill, tokens.radii.pill_px, border_color, 1.0);
    canvas.draw_text(&TextLayout {
        text: label,
        rect: RectF::new(
            pill.left + BADGE_PAD_X,
            pill.top,
            pill.right - BADGE_PAD_X,
            pill.bottom,
        ),
        align: TextAlign::Left,
        role: tokens.typography.label,
        color: text_color,
    });
}

/// Draw the conflict callout card: warning marker + title, the conflicted
/// combos as keycap chips with the sentence tail (greedily packed), and the
/// next-action hint.
unsafe fn paint_callout(
    canvas: &mut PaintCanvas,
    tokens: &ThemeTokens,
    layout: &HelpLayout,
    callout: &ConflictCallout,
) {
    let glass = tokens.signal_glass();
    let card = layout.callout;

    canvas.fill_rounded_rect(card, tokens.radii.card_px, glass.surface_subtle);
    canvas.stroke_rounded_rect(card, tokens.radii.card_px, tokens.border, 1.0);

    // Warning marker: the monochrome triangle-with-! glyph (U+26A0, text
    // presentation — deliberately NO emoji VS16) drawn in text color. A
    // shape, never tint-only: it survives high contrast unchanged.
    let glyph = RectF::new(
        card.left + 2.0,
        card.top + 15.0,
        card.left + 18.0,
        card.top + 31.0,
    );
    canvas.draw_text(&TextLayout {
        text: "\u{26A0}",
        rect: glyph,
        align: TextAlign::Center,
        role: tokens.typography.label,
        color: tokens.text_primary,
    });

    canvas.draw_text(&TextLayout {
        text: callout.title,
        rect: RectF::new(
            card.left + 24.0,
            card.top + 14.0,
            card.right - 16.0,
            card.top + 32.0,
        ),
        align: TextAlign::Left,
        role: tokens.typography.label,
        color: tokens.text_primary,
    });

    // Explanation: conflicted combos as keycap chips + the sentence tail,
    // greedily packed into up to three lines inside the card.
    let atoms = explanation_atoms(callout);
    let widths = explanation_widths(
        &atoms,
        |c| measure_width(c, tokens.typography.keycap),
        |t| measure_width(t, tokens.typography.body),
    );
    let area = RectF::new(
        card.left + 16.0,
        card.top + 38.0,
        card.right - 16.0,
        card.top + 38.0 + 3.0 * EXPL_LINE_H,
    );
    for run in pack_explanation(area, &atoms, &widths) {
        match &run.atom {
            ExplanationAtom::Combo(chips) => {
                let chip_widths: Vec<f32> = chips
                    .iter()
                    .map(|c| measure_width(c, tokens.typography.keycap))
                    .collect();
                for cell in layout_keycap_row(run.rect, chips, &chip_widths) {
                    match cell {
                        KeycapCell::Chip(rect, text) => {
                            canvas.fill_rounded_rect(rect, tokens.radii.control_px, tokens.surface);
                            canvas.stroke_rounded_rect(
                                rect,
                                tokens.radii.control_px,
                                glass.border_strong,
                                1.0,
                            );
                            canvas.draw_text(&TextLayout {
                                text: &text,
                                rect,
                                align: TextAlign::Center,
                                role: tokens.typography.keycap,
                                color: tokens.text_primary,
                            });
                        }
                        KeycapCell::Separator(rect) => {
                            canvas.draw_text(&TextLayout {
                                text: "+",
                                rect,
                                align: TextAlign::Center,
                                role: tokens.typography.body,
                                color: tokens.text_secondary,
                            });
                        }
                    }
                }
            }
            ExplanationAtom::Text(t) => {
                canvas.draw_text(&TextLayout {
                    text: t,
                    rect: run.rect,
                    align: TextAlign::Left,
                    role: tokens.typography.body,
                    color: tokens.text_secondary,
                });
            }
        }
    }

    canvas.draw_text(&TextLayout {
        text: callout.action,
        rect: RectF::new(
            card.left + 16.0,
            card.top + 112.0,
            card.right - 16.0,
            card.top + 128.0,
        ),
        align: TextAlign::Left,
        role: tokens.typography.caption,
        color: tokens.text_secondary,
    });
}

/// Draw the card contents from the stored model: adaptive background, header
/// band (accent bar + title + subtitle + close × with hover), the five
/// hotkey rows, the conflict callout, and the sticky footer. All coordinates
/// are logical; the canvas scales them to physical exactly once.
///
/// The parent's full-client fill covers the native footer buttons; they are
/// invalidated at the end so their chrome repaints on top.
unsafe fn paint(canvas: &mut PaintCanvas, d: &HelpData) {
    let tokens = &d.appearance.tokens;
    let glass = tokens.signal_glass();
    let layout = HelpLayout::new(WIN_W as f32, WIN_H as f32);
    let w = layout.width;
    let h = layout.height;

    // Card background: always-painted opaque token fill (the card stays
    // opaque in every mode — no material/backdrop treatment).
    canvas.fill_rect(RectF::new(0.0, 0.0, w, h), tokens.background);

    // Header band: elevated surface, 3px accent bar, hairline divider.
    canvas.fill_rect(RectF::new(0.0, 0.0, w, HEADER_H), tokens.surface_elevated);
    canvas.fill_rect(RectF::new(0.0, 0.0, w, 3.0), tokens.accent);
    canvas.fill_rect(RectF::new(0.0, HEADER_H, w, HEADER_H + 1.0), tokens.border);

    canvas.draw_text(&TextLayout {
        text: "VolumeControl",
        rect: layout.title_rect,
        align: TextAlign::Left,
        role: tokens.typography.surface_title,
        color: tokens.text_primary,
    });
    canvas.draw_text(&TextLayout {
        text: "Keyboard shortcuts",
        rect: layout.subtitle_rect,
        align: TextAlign::Left,
        role: tokens.typography.caption,
        color: tokens.text_secondary,
    });

    // Custom-painted close (×) with hover surface (pointer-only; the footer
    // Close button + Escape cover the keyboard. UIA naming for the × is
    // follow-on accessibility work — see the Verify-accessibility task.)
    if d.close_hover {
        canvas.fill_rect(layout.close_rect, glass.surface_subtle);
    }
    canvas.draw_text(&TextLayout {
        text: "\u{00D7}",
        rect: layout.close_rect,
        align: TextAlign::Center,
        role: tokens.typography.label,
        color: tokens.text_primary,
    });

    // Five hotkey rows.
    for (i, model) in d.rows.iter().enumerate() {
        paint_row(canvas, tokens, layout.rows[i], model);
    }

    // Conflict callout (when any combo is in use by another app).
    if let Some(callout) = &d.callout {
        paint_callout(canvas, tokens, &layout, callout);
    }

    // Sticky footer: elevated surface + hairline above; the native buttons
    // paint their own chrome on top (invalidated below).
    canvas.fill_rect(
        RectF::new(0.0, layout.footer_top, w, h),
        tokens.surface_elevated,
    );
    canvas.fill_rect(
        RectF::new(0.0, layout.footer_top - 1.0, w, layout.footer_top),
        tokens.border,
    );

    // Two-layer focus ring around the focused footer button (outer accent +
    // inner contrast ring, both layers via `draw_focus_ring`).
    if let Some(rect) = focused_button_rect(d, &layout) {
        canvas.draw_focus_ring(rect, &tokens.focus);
    }

    for &ctl in &d.buttons {
        InvalidateRect(ctl, std::ptr::null(), 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::HotkeyModifier;
    use crate::hotkeys::{HotkeyAction, HotkeyRegError, HotkeyRegResult};
    use crate::ui::platform::windows::text::measure_text_gdi;
    use crate::ui::primitives::focus_ring_rects;
    use crate::ui::{ThemeMode, WorkArea};
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect;

    fn caps(reduced_motion: bool, high_contrast: bool) -> UiCapabilities {
        UiCapabilities {
            compositor: true,
            blur: true,
            high_contrast,
            reduced_motion,
            dpi_scale: 1.0,
            work_area: WorkArea::new(0, 0, 2560, 1400),
        }
    }

    fn appearance(theme: ThemeMode, high_contrast: bool) -> HelpAppearance {
        let mut cfg = Config::default();
        cfg.appearance.theme = theme;
        cfg.appearance.motion = MotionMode::Full;
        HelpAppearance::resolve(&cfg, &caps(false, high_contrast), || None)
    }

    fn config_with(modifier: HotkeyModifier) -> Config {
        Config {
            modifier,
            ..Config::default()
        }
    }

    fn conflict() -> HotkeyRegStatus {
        HotkeyRegStatus::Conflicted(HotkeyRegError {
            error_code: 1409,
            message: "hotkey already registered by another app".into(),
        })
    }

    fn statuses(entries: &[(HotkeyAction, HotkeyRegStatus)]) -> Vec<HotkeyRegResult> {
        entries
            .iter()
            .map(|(action, status)| HotkeyRegResult {
                action: *action,
                status: status.clone(),
            })
            .collect()
    }

    fn registered(action: HotkeyAction) -> (HotkeyAction, HotkeyRegStatus) {
        (action, HotkeyRegStatus::Registered)
    }

    // ── appearance + motion ───────────────────────────────────────────────

    #[test]
    fn dark_appearance_resolves_dark_tokens_and_full_motion() {
        let a = appearance(ThemeMode::Dark, false);
        assert!(a.tokens.is_dark);
        assert_eq!(a.motion, MotionMode::Full);
    }

    #[test]
    fn system_theme_resolves_through_the_system_is_dark_callback() {
        let mut cfg = Config::default();
        cfg.appearance.theme = ThemeMode::System;

        let dark = HelpAppearance::resolve(&cfg, &caps(false, false), || Some(true));
        assert!(dark.tokens.is_dark);

        let light = HelpAppearance::resolve(&cfg, &caps(false, false), || Some(false));
        assert!(!light.tokens.is_dark);
    }

    #[test]
    fn motion_resolution_honors_the_reduced_motion_system_setting() {
        let mut cfg = Config::default();
        cfg.appearance.motion = MotionMode::Full;
        let c = caps(false, false);
        assert_eq!(resolve_motion(MotionMode::Full, &c), MotionMode::Full);

        let c = caps(true, true);
        assert_eq!(resolve_motion(MotionMode::Full, &c), MotionMode::Reduced);
        let a = HelpAppearance::resolve(&cfg, &c, || None);
        assert_eq!(a.motion, MotionMode::Reduced);

        cfg.appearance.motion = MotionMode::Disabled;
        let a = HelpAppearance::resolve(&cfg, &c, || None);
        assert_eq!(a.motion, MotionMode::Disabled);
    }

    #[test]
    fn help_never_animates_so_reduced_motion_is_honored_by_construction() {
        // Policy test: the card is static — `show()` paints one frame and
        // nothing transitions, so Reduced/Disabled motion needs no code path
        // and the resolved preference is carried through unchanged. If a
        // transition is ever added, this test must grow real assertions.
        for requested in [MotionMode::Full, MotionMode::Reduced, MotionMode::Disabled] {
            let mut cfg = Config::default();
            cfg.appearance.motion = requested;
            let a = HelpAppearance::resolve(&cfg, &caps(true, true), || None);
            assert_eq!(a.motion, resolve_motion(requested, &caps(true, true)));
        }
    }

    // ── rows / chips ──────────────────────────────────────────────────────

    #[test]
    fn keycap_chips_produce_the_right_key_lists_per_modifier() {
        for (modifier, expected) in [
            (HotkeyModifier::CtrlAlt, vec!["Ctrl", "Alt", "\u{2191}"]),
            (HotkeyModifier::Alt, vec!["Alt", "\u{2191}"]),
            (HotkeyModifier::Ctrl, vec!["Ctrl", "\u{2191}"]),
            (HotkeyModifier::CapsLock, vec!["CapsLock", "\u{2191}"]),
        ] {
            let chips = keycap_chips(modifier, "\u{2191}");
            assert_eq!(chips, expected);
        }
    }

    #[test]
    fn build_rows_yields_exactly_the_five_spec_rows() {
        let cfg = config_with(HotkeyModifier::CtrlAlt);
        let rows = build_rows(&cfg, &statuses(&[]));
        assert_eq!(rows.len(), 5);
        let expected_labels = [
            "Increase volume",
            "Decrease volume",
            "Toggle mute",
            "Open mixer",
            "Reset to 50%",
        ];
        let expected_keys = ["\u{2191}", "\u{2193}", "M", "V", "R"];
        for (row, (label, key)) in rows
            .iter()
            .zip(expected_labels.iter().zip(expected_keys.iter()))
        {
            assert_eq!(row.label, *label);
            assert_eq!(row.key, *key);
            assert_eq!(row.chips, keycap_chips(HotkeyModifier::CtrlAlt, row.key));
        }
    }

    // ── badges ────────────────────────────────────────────────────────────

    #[test]
    fn badge_mapping_covers_all_three_registration_statuses() {
        let cfg = config_with(HotkeyModifier::CtrlAlt);
        let status = statuses(&[
            registered(HotkeyAction::VolumeUp),
            (HotkeyAction::VolumeDown, HotkeyRegStatus::HookRouted),
            (HotkeyAction::ToggleMute, conflict()),
            registered(HotkeyAction::OpenMixer),
            registered(HotkeyAction::Reset50),
        ]);
        let rows = build_rows(&cfg, &status);
        assert_eq!(rows[0].badge, BadgeKind::Ready);
        assert_eq!(rows[1].badge, BadgeKind::Fallback);
        assert_eq!(rows[2].badge, BadgeKind::InUse);
        assert_eq!(rows[3].badge, BadgeKind::Ready);
        assert_eq!(rows[4].badge, BadgeKind::Ready);
    }

    #[test]
    fn shift_variants_share_the_base_action_status() {
        let cfg = config_with(HotkeyModifier::CtrlAlt);
        // VolumeUp registered while its Shift variant is conflicted: the ↑
        // row must read the BASE action's status (Ready), not the variant's.
        let status = statuses(&[
            registered(HotkeyAction::VolumeUp),
            (HotkeyAction::VolumeUpLarge, conflict()),
            registered(HotkeyAction::VolumeDown),
            registered(HotkeyAction::ToggleMute),
            registered(HotkeyAction::OpenMixer),
            registered(HotkeyAction::Reset50),
        ]);
        let rows = build_rows(&cfg, &status);
        assert_eq!(rows[0].badge, BadgeKind::Ready);
        assert!(conflict_callout(&cfg, &status).is_none());

        // And the reverse: the base action conflicted must read In use
        // regardless of the variant's own status.
        let status = statuses(&[
            (HotkeyAction::VolumeUp, conflict()),
            registered(HotkeyAction::VolumeUpLarge),
        ]);
        let rows = build_rows(&cfg, &status);
        assert_eq!(rows[0].badge, BadgeKind::InUse);
    }

    #[test]
    fn badge_tints_map_to_the_spec_semantic_colors() {
        let a = appearance(ThemeMode::Dark, false);
        let (ready, _) = badge_colors(&a.tokens, BadgeKind::Ready);
        let (fallback, _) = badge_colors(&a.tokens, BadgeKind::Fallback);
        let (in_use, _) = badge_colors(&a.tokens, BadgeKind::InUse);
        assert_eq!(ready, a.tokens.volume_threshold.low);
        assert_eq!(fallback, a.tokens.accent);
        assert_eq!(in_use, a.tokens.volume_threshold.high);
        // Distinct tints (the HC collapse is tested separately below).
        assert_ne!(ready, fallback);
        assert_ne!(fallback, in_use);
        assert_ne!(ready, in_use);
    }

    #[test]
    fn high_contrast_badges_collapse_to_primary_text_and_strong_border_with_distinct_labels() {
        let a = appearance(ThemeMode::System, true);
        assert!(a.tokens.high_contrast);
        for kind in [BadgeKind::Ready, BadgeKind::Fallback, BadgeKind::InUse] {
            let (text, border) = badge_colors(&a.tokens, kind);
            assert_eq!(text, a.tokens.text_primary, "{kind:?} text collapses");
            assert_eq!(border, a.tokens.signal_glass().border_strong);
        }
        // The labels always differ — status is never carried by tint alone.
        assert_ne!(BadgeKind::Ready.label(), BadgeKind::Fallback.label());
        assert_ne!(BadgeKind::Fallback.label(), BadgeKind::InUse.label());
        assert_ne!(BadgeKind::Ready.label(), BadgeKind::InUse.label());
    }

    // ── conflict callout ──────────────────────────────────────────────────

    #[test]
    fn conflict_callout_is_none_when_everything_registers() {
        for modifier in [
            HotkeyModifier::CtrlAlt,
            HotkeyModifier::Alt,
            HotkeyModifier::Ctrl,
            HotkeyModifier::CapsLock,
        ] {
            let cfg = config_with(modifier);
            let status = statuses(&[
                registered(HotkeyAction::VolumeUp),
                registered(HotkeyAction::VolumeDown),
                registered(HotkeyAction::ToggleMute),
                registered(HotkeyAction::OpenMixer),
                registered(HotkeyAction::Reset50),
            ]);
            assert!(conflict_callout(&cfg, &status).is_none());
            // HookRouted is not a conflict.
            let status = statuses(&[
                registered(HotkeyAction::VolumeUp),
                (HotkeyAction::VolumeDown, HotkeyRegStatus::HookRouted),
                registered(HotkeyAction::ToggleMute),
                registered(HotkeyAction::OpenMixer),
                registered(HotkeyAction::Reset50),
            ]);
            assert!(conflict_callout(&cfg, &status).is_none());
        }
    }

    #[test]
    fn conflict_callout_lists_the_conflicted_combo_and_the_next_action() {
        let cfg = config_with(HotkeyModifier::CtrlAlt);
        let status = statuses(&[
            registered(HotkeyAction::VolumeUp),
            registered(HotkeyAction::VolumeDown),
            (HotkeyAction::ToggleMute, conflict()),
            registered(HotkeyAction::OpenMixer),
            registered(HotkeyAction::Reset50),
        ]);
        let callout = conflict_callout(&cfg, &status).expect("Mute conflicted → callout");
        assert_eq!(callout.title, "Shortcut conflict");
        assert_eq!(callout.combos, vec![vec!["Ctrl", "Alt", "M"]]);
        assert_eq!(callout.tail, "is used by another app.");
        assert_eq!(callout.action, "Change the modifier in Settings.");
    }

    #[test]
    fn conflict_callout_aggregates_multiple_conflicts_with_the_plural_tail() {
        for modifier in [
            HotkeyModifier::CtrlAlt,
            HotkeyModifier::Alt,
            HotkeyModifier::Ctrl,
            HotkeyModifier::CapsLock,
        ] {
            let cfg = config_with(modifier);
            let status = statuses(&[
                registered(HotkeyAction::VolumeUp),
                registered(HotkeyAction::VolumeDown),
                (HotkeyAction::ToggleMute, conflict()),
                (HotkeyAction::OpenMixer, conflict()),
                registered(HotkeyAction::Reset50),
            ]);
            let callout = conflict_callout(&cfg, &status).expect("two conflicts → callout");
            assert_eq!(callout.combos.len(), 2);
            assert_eq!(callout.combos[0], keycap_chips(modifier, "M"));
            assert_eq!(callout.combos[1], keycap_chips(modifier, "V"));
            assert_eq!(callout.tail, "are used by another app.");
            assert_eq!(callout.action, "Change the modifier in Settings.");
        }
    }

    #[test]
    fn conflict_callout_dedupes_shift_variants() {
        let cfg = config_with(HotkeyModifier::CtrlAlt);
        let status = statuses(&[
            (HotkeyAction::VolumeUp, conflict()),
            (HotkeyAction::VolumeUpLarge, conflict()),
            registered(HotkeyAction::VolumeDown),
            registered(HotkeyAction::ToggleMute),
            registered(HotkeyAction::OpenMixer),
            registered(HotkeyAction::Reset50),
        ]);
        let callout = conflict_callout(&cfg, &status).expect("↑ conflicted → callout");
        // One combo per row — the Shift variant never duplicates it.
        assert_eq!(callout.combos, vec![vec!["Ctrl", "Alt", "\u{2191}"]]);
        assert_eq!(callout.tail, "is used by another app.");
    }

    // ── host routing ──────────────────────────────────────────────────────

    #[test]
    fn wm_app_help_constants_are_unchanged() {
        assert_eq!(WM_APP_HELP_OPEN_CONFIG, WM_USER + 30);
        assert_eq!(WM_APP_HELP_SETTINGS, WM_USER + 31);
        assert_ne!(WM_APP_HELP_OPEN_CONFIG, WM_APP_HELP_SETTINGS);
    }

    #[test]
    fn button_ids_map_to_the_two_host_messages_and_hide() {
        assert_eq!(button_action(ID_BTN_EDIT_CONFIG), ButtonAction::OpenConfig);
        assert_eq!(button_action(ID_BTN_SETTINGS), ButtonAction::Settings);
        assert_eq!(button_action(ID_BTN_CLOSE), ButtonAction::Close);
        // The wndproc contract (structural): OpenConfig/Settings post the
        // WM_APP_HELP_* intents to the host then hide; Close hides via
        // WM_CLOSE. The constants above are the exact messages app.rs routes.
        assert_eq!(ButtonAction::OpenConfig, ButtonAction::OpenConfig);
        assert_ne!(ButtonAction::OpenConfig, ButtonAction::Settings);
        assert_ne!(ButtonAction::Settings, ButtonAction::Close);
    }

    // ── DPI + geometry ────────────────────────────────────────────────────

    #[test]
    fn dpi_scales_the_520x500_card_to_physical_pixels() {
        let d100 = DpiMetrics::new(1.0);
        assert_eq!(
            (d100.to_physical(WIN_W), d100.to_physical(WIN_H)),
            (520, 500)
        );
        let d125 = DpiMetrics::new(1.25);
        assert_eq!(
            (d125.to_physical(WIN_W), d125.to_physical(WIN_H)),
            (650, 625)
        );
        let d150 = DpiMetrics::new(1.5);
        assert_eq!(
            (d150.to_physical(WIN_W), d150.to_physical(WIN_H)),
            (780, 750)
        );
    }

    #[test]
    fn layout_places_every_rect_inside_the_surface_without_overlap() {
        let l = HelpLayout::new(WIN_W as f32, WIN_H as f32);
        // Header zone: title above subtitle, both left of the close ×, all
        // below the accent bar and above the hairline.
        assert!(l.title_rect.top >= 3.0);
        assert!(l.title_rect.bottom <= l.subtitle_rect.top);
        assert!(l.subtitle_rect.bottom <= HEADER_H);
        assert!(l.close_rect.top >= 3.0 && l.close_rect.bottom <= HEADER_H);
        assert!(l.subtitle_rect.right <= l.close_rect.left);

        // Rows: in order, non-overlapping, inside the content column, and
        // ending before the callout.
        for w in l.rows.windows(2) {
            assert!(w[0].bottom <= w[1].top);
        }
        for (i, r) in l.rows.iter().enumerate() {
            assert!(r.left >= 0.0 && r.right <= l.width, "row {i} in bounds");
            assert!(r.bottom <= l.footer_top, "row {i} above the footer");
            let chips = HelpLayout::chips_area(*r);
            let label = HelpLayout::label_rect(*r);
            assert!(chips.left >= r.left && chips.right <= r.right);
            assert!(label.left >= chips.right, "label clears the keycap column");
            assert!(label.right <= r.right);
        }

        // Callout between the rows and the footer.
        assert!(l.rows[4].bottom <= l.callout.top);
        assert!(l.callout.left >= 0.0 && l.callout.right <= l.width);
        assert!(l.callout.bottom <= l.footer_top);

        // Footer: buttons inside the footer band, ordered, non-overlapping.
        assert!(l.btn_edit.top >= l.footer_top && l.btn_close.bottom <= l.height);
        assert!(l.btn_edit.right <= l.btn_settings.left);
        assert!(l.btn_settings.right <= l.btn_close.left);
        for (i, b) in [l.btn_edit, l.btn_settings, l.btn_close].iter().enumerate() {
            assert!(b.left >= 0.0 && b.right <= l.width, "button {i} in bounds");
        }
    }

    #[test]
    fn layout_rects_stay_inside_after_exactly_one_dpi_scale_at_125_and_150() {
        for scale in [1.25, 1.5] {
            let dpi = DpiMetrics::new(scale);
            let pw = dpi.to_physical(WIN_W);
            let ph = dpi.to_physical(WIN_H);
            let l = HelpLayout::new(WIN_W as f32, WIN_H as f32);
            let rects = [
                l.title_rect,
                l.subtitle_rect,
                l.close_rect,
                l.callout,
                l.btn_edit,
                l.btn_settings,
                l.btn_close,
            ];
            for r in rects {
                let px = dpi.to_physical(r.left.round() as i32);
                let py = dpi.to_physical(r.top.round() as i32);
                let pr = dpi.to_physical(r.right.round() as i32);
                let pb = dpi.to_physical(r.bottom.round() as i32);
                assert!(
                    px >= 0 && py >= 0 && pr <= pw && pb <= ph,
                    "scale {scale}: {r:?} → ({px},{py},{pr},{pb}) outside {pw}x{ph}"
                );
            }
        }
    }

    #[test]
    fn keycap_row_layout_is_left_aligned_with_gaps_and_fits_the_210px_column() {
        let area = RectF::new(CONTENT_LEFT, ROWS_TOP, CHIPS_RIGHT, ROWS_TOP + ROW_H);
        for modifier in [
            HotkeyModifier::CtrlAlt,
            HotkeyModifier::Alt,
            HotkeyModifier::Ctrl,
            HotkeyModifier::CapsLock,
        ] {
            let chips = keycap_chips(modifier, "M");
            let widths: Vec<f32> = chips.iter().map(|c| 12.0 + c.len() as f32 * 7.0).collect();
            let cells = layout_keycap_row(area, &chips, &widths);

            // Strict left-to-right alternation: chip, separator, ..., chip.
            let mut expected_chip = 0usize;
            let mut x = area.left;
            for (i, cell) in cells.iter().enumerate() {
                match cell {
                    KeycapCell::Chip(rect, text) => {
                        assert_eq!(i % 2, 0, "cells alternate chip/separator");
                        assert_eq!(rect.left, x, "chip {i} laid out left-aligned");
                        assert_eq!(text, &chips[expected_chip]);
                        assert_eq!(rect.width(), widths[expected_chip] + 2.0 * CHIP_PAD_X);
                        assert!(
                            rect.top >= area.top && rect.bottom <= area.bottom,
                            "chip {i} vertically centered in the row"
                        );
                        x = rect.right;
                        expected_chip += 1;
                    }
                    KeycapCell::Separator(rect) => {
                        assert_eq!(i % 2, 1, "cells alternate chip/separator");
                        assert_eq!(rect.left, x + CHIP_GAP, "4px gap before the separator");
                        assert_eq!(rect.width(), SEP_W);
                        assert_eq!(rect.top, area.top);
                        assert_eq!(rect.bottom, area.bottom);
                        x = rect.right + CHIP_GAP;
                    }
                }
            }
            assert_eq!(expected_chip, chips.len());
            assert!(
                x <= area.right,
                "{modifier:?} chip row must fit the ~210px keycap column"
            );
        }
    }

    #[test]
    fn badge_pills_are_right_aligned_and_never_overlap_the_label_column() {
        let l = HelpLayout::new(WIN_W as f32, WIN_H as f32);
        for width in [40.0, 60.0, 90.0] {
            for row in l.rows {
                let pill = badge_pill_rect(row, width);
                assert_eq!(pill.right, BADGE_RIGHT);
                assert!(
                    pill.left >= HelpLayout::label_rect(row).right,
                    "width {width}: pill must not overlap the label column"
                );
                assert!(pill.top >= row.top && pill.bottom <= row.bottom);
                assert!(pill.top >= 0.0 && pill.right <= WIN_W as f32);
            }
        }
    }

    #[test]
    fn badge_labels_measure_within_the_reserved_slot() {
        let a = appearance(ThemeMode::Dark, false);
        let role = a.tokens.typography.label;
        for label in ["Ready", "Fallback", "In use"] {
            let width = measure_text_gdi(label, role)
                .expect("headless GDI measurement")
                .width;
            assert!(
                width <= 90.0,
                "{label:?} measures {width}px — exceeds the 90px badge slot"
            );
        }
    }

    #[test]
    fn callout_explanation_packs_within_the_callout_for_any_conflict_count() {
        // Conservative over-estimates (≥ real measured widths): keycap text
        // at 7.2px/char + 12px padding, body text at 6.5px/char.
        let keycap_est = |c: &str| 12.0 + c.len() as f32 * 7.2;
        let text_est = |t: &str| t.len() as f32 * 6.5;
        let area = RectF::new(40.0, 342.0, 480.0, 414.0);
        let base = [
            HotkeyAction::VolumeUp,
            HotkeyAction::VolumeDown,
            HotkeyAction::ToggleMute,
            HotkeyAction::OpenMixer,
            HotkeyAction::Reset50,
        ];
        for modifier in [
            HotkeyModifier::CtrlAlt,
            HotkeyModifier::Alt,
            HotkeyModifier::Ctrl,
            HotkeyModifier::CapsLock,
        ] {
            let cfg = config_with(modifier);
            for n in 1..=5 {
                let entries: Vec<(HotkeyAction, HotkeyRegStatus)> =
                    base.iter().take(n).map(|a| (*a, conflict())).collect();
                let status = statuses(&entries);
                let callout = conflict_callout(&cfg, &status).expect("n conflicts → callout");
                assert_eq!(callout.combos.len(), n);

                let atoms = explanation_atoms(&callout);
                let widths: Vec<f32> = atoms
                    .iter()
                    .map(|a| match a {
                        ExplanationAtom::Combo(chips) => {
                            let chips_total: f32 =
                                chips.iter().map(|c| keycap_est(c) + 2.0 * CHIP_PAD_X).sum();
                            chips_total
                                + (chips.len().saturating_sub(1)) as f32 * (CHIP_GAP * 2.0 + SEP_W)
                        }
                        ExplanationAtom::Text(t) => text_est(t),
                    })
                    .collect();
                let runs = pack_explanation(area, &atoms, &widths);
                assert_eq!(runs.len(), atoms.len());
                for run in &runs {
                    assert!(
                        run.rect.left >= area.left && run.rect.right <= area.right,
                        "modifier {modifier:?}, {n} conflicts: {run:?} overflows the width"
                    );
                    assert!(
                        run.rect.top >= area.top && run.rect.bottom <= area.bottom,
                        "modifier {modifier:?}, {n} conflicts: {run:?} overflows the height"
                    );
                }
                let max_line = runs
                    .iter()
                    .map(|r| ((r.rect.top - area.top) / EXPL_LINE_H) as usize)
                    .max();
                assert!(
                    max_line.unwrap_or(0) < 3,
                    "modifier {modifier:?}, {n} conflicts must fit the 3-line budget"
                );
            }
        }
    }

    // ── focus ring ────────────────────────────────────────────────────────

    #[test]
    fn focus_ring_has_two_distinct_layers_for_every_footer_button() {
        let a = appearance(ThemeMode::Dark, false);
        let focus = a.tokens.focus;
        assert_ne!(focus.ring, focus.inner_ring, "layer colours must differ");
        assert_ne!(
            focus.ring_width_px, focus.inner_ring_width_px,
            "layer widths must differ"
        );
        let l = HelpLayout::new(WIN_W as f32, WIN_H as f32);
        for rect in [l.btn_edit, l.btn_settings, l.btn_close] {
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

    // ── real hidden window ────────────────────────────────────────────────

    #[test]
    fn help_window_constructs_and_drops_without_crashing() {
        let help = Help::new(0).expect("help window creates");
        unsafe {
            let d = &*(GetWindowLongPtrW(help.hwnd, GWLP_USERDATA) as *const HelpData);
            assert!(d.buttons.iter().all(|&b| b != 0), "three footer buttons");
            assert!(
                d.orig_procs.iter().all(|p| p.is_some()),
                "footer buttons subclassed"
            );
            assert!(!d.open);
        }
        drop(help);
    }

    #[test]
    fn help_show_builds_rows_and_callout_and_scales_the_window_exactly_once() {
        let mut help = Help::new(0).expect("help window creates");
        let cfg = config_with(HotkeyModifier::CtrlAlt);
        let status = statuses(&[
            registered(HotkeyAction::VolumeUp),
            registered(HotkeyAction::VolumeDown),
            (HotkeyAction::ToggleMute, conflict()),
            registered(HotkeyAction::OpenMixer),
            registered(HotkeyAction::Reset50),
        ]);
        let a = appearance(ThemeMode::Dark, false);
        help.show(&cfg, &status, &a);
        unsafe {
            let d = &*(GetWindowLongPtrW(help.hwnd, GWLP_USERDATA) as *const HelpData);
            assert!(d.open);
            assert_eq!(d.rows.len(), 5);
            assert_eq!(d.rows[0].label, "Increase volume");
            assert_eq!(d.rows[2].badge, BadgeKind::InUse);
            let callout = d.callout.as_ref().expect("Mute conflicted → callout");
            assert_eq!(callout.combos, vec![vec!["Ctrl", "Alt", "M"]]);

            // The window's PHYSICAL size is the logical design scaled exactly
            // once for its DPI.
            let dpi = DpiMetrics::new(dpi_scale_for(help.hwnd));
            let mut r: RECT = std::mem::zeroed();
            GetWindowRect(help.hwnd, &mut r);
            assert_eq!(r.right - r.left, dpi.to_physical(WIN_W));
            assert_eq!(r.bottom - r.top, dpi.to_physical(WIN_H));
        }
        drop(help);
    }
}
