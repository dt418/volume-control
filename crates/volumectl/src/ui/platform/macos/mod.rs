//! macOS renderer seam (compile-safe).
//!
//! This module is the renderer boundary for a future native AppKit build. It
//! is deliberately a *seam*, not a renderer: no AppKit, CoreAnimation, or
//! CoreAudio code exists yet. It consumes only shared [`crate::ui`] types, so
//! a macOS compile never needs a Windows import and shared code never leaks
//! platform specifics.
//!
//! Renderer contract
//! -----------------
//! A macOS renderer:
//!
//! - consumes confirmed [`crate::ui::AppState`] and [`crate::ui::ThemeTokens`]
//!   (light/dark/high-contrast plus the resolved accent), resolves the
//!   material treatment from [`crate::ui::UiCapabilities`], and places
//!   surfaces against the shared work-area math;
//! - dispatches [`crate::ui::AppAction`] values to the application host; it
//!   never mutates audio or writes configuration directly;
//! - draws with AppKit and reads volume through CoreAudio in a follow-on plan.
//!
//! Follow-on native work: AppKit overlay/mixer/settings surfaces, a CoreAudio
//! backend, global hotkeys, and a tray implementation.

use crate::ui::{AppAction, AppState, ThemeTokens, UiCapabilities};

/// The shared UI contract a macOS renderer must consume.
///
/// Compile-time placeholder for the `render` entry point the AppKit renderer
/// will implement. It type-checks today — every type is platform-neutral —
/// and will be replaced by real drawing and event wiring.
#[allow(dead_code)]
pub fn render_boundary(
    state: &AppState,
    tokens: &ThemeTokens,
    capabilities: &UiCapabilities,
) -> Vec<AppAction> {
    // Nothing is drawn yet: the seam keeps the renderer contract compiling on
    // `target_os = "macos"` without importing any platform API.
    let _ = (state, tokens, capabilities);
    Vec::new()
}
