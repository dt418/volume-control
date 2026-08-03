//! Linux renderer seam (compile-safe).
//!
//! This module is the renderer boundary for a future native GTK/libadwaita
//! build. It is deliberately a *seam*, not a renderer: no GTK, libadwaita,
//! PulseAudio, or PipeWire code exists yet. It consumes only shared
//! [`crate::ui`] types, so a Linux compile never needs a Windows import and
//! shared code never leaks platform specifics.
//!
//! Renderer contract
//! -----------------
//! A Linux renderer:
//!
//! - consumes confirmed [`crate::ui::AppState`] and [`crate::ui::ThemeTokens`],
//!   resolves the material treatment from [`crate::ui::UiCapabilities`]
//!   (Wayland/X11 composition, high contrast, reduced motion, DPI, work
//!   area), and places surfaces against the shared work-area math;
//! - dispatches [`crate::ui::AppAction`] values to the application host; it
//!   never mutates audio or writes configuration directly;
//! - draws with GTK/libadwaita and reads volume through PulseAudio/PipeWire
//!   in a follow-on plan.
//!
//! Follow-on native work: GTK/libadwaita overlay/mixer/settings surfaces, a
//! PulseAudio/PipeWire backend, global hotkeys, and a tray implementation.

use crate::ui::{AppAction, AppState, ThemeTokens, UiCapabilities};

/// The shared UI contract a Linux renderer must consume.
///
/// Compile-time placeholder for the `render` entry point the GTK renderer
/// will implement. It type-checks today — every type is platform-neutral —
/// and will be replaced by real drawing and event wiring.
#[allow(dead_code)]
pub fn render_boundary(
    state: &AppState,
    tokens: &ThemeTokens,
    capabilities: &UiCapabilities,
) -> Vec<AppAction> {
    // Nothing is drawn yet: the seam keeps the renderer contract compiling on
    // `target_os = "linux"` without importing any platform API.
    let _ = (state, tokens, capabilities);
    Vec::new()
}
