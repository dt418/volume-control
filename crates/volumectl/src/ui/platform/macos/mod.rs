//! macOS renderer (AppKit, spec §10.2).
//!
//! This module is the native macOS renderer boundary. It consumes only shared
//! [`crate::ui`] types — the same [`crate::ui::NativeRenderer`] contract the
//! Windows renderer implements — so a macOS compile never needs a Windows
//! import and shared code never leaks platform specifics.
//!
//! Renderer contract
//! -----------------
//! - consumes confirmed [`crate::ui::AppState`] and [`crate::ui::ThemeTokens`]
//!   (light/dark/high-contrast plus the resolved accent), resolves the
//!   material treatment from [`crate::ui::UiCapabilities`] (public AppKit
//!   material APIs when available, translucent, or opaque fallback), and
//!   places surfaces against the shared work-area math;
//! - dispatches [`crate::ui::AppAction`] values to the application host; it
//!   never mutates audio or writes configuration directly;
//! - draws with AppKit (`NSPanel`/`NSWindow` + `NSVisualEffectView` for the
//!   glass treatment) and exposes VoiceOver labels. Private APIs are
//!   prohibited; material APIs are availability-gated at runtime.
//!
//! The [`renderer`] module is pure planning + tests and compiles on any
//! target that includes this file (macOS only, via the `platform` gate); the
//! AppKit surface code lives in [`renderer`]'s `appkit` submodule and is
//! compiled on macOS.

pub mod renderer;
