//! Linux renderer (GTK4/libadwaita, spec §10.3).
//!
//! This module is the native Linux renderer boundary. It consumes only shared
//! [`crate::ui`] types — the same [`crate::ui::NativeRenderer`] contract the
//! Windows and macOS renderers implement — so a Linux compile never needs a
//! Windows import and shared code never leaks platform specifics.
//!
//! Renderer contract
//! -----------------
//! - consumes confirmed [`crate::ui::AppState`] and [`crate::ui::ThemeTokens`]
//!   (light/dark/high-contrast plus the resolved accent), resolves the
//!   material treatment from [`crate::ui::UiCapabilities`] (Wayland
//!   layer-shell glass when available, translucent, or opaque fallback), and
//!   places surfaces against the shared work-area math;
//! - dispatches [`crate::ui::AppAction`] values to the application host; it
//!   never mutates audio or writes configuration directly;
//! - draws with GTK4/libadwaita (layer-shell overlay/mixer on Wayland,
//!   borderless windows elsewhere) and exposes §11.2 accessibility labels.
//!
//! The GTK surface code is feature-gated (`gtk-renderer`, plus `layer-shell`
//! for the Wayland path) so the plain CLI fallback still builds on Linux
//! systems without GTK development packages. The [`renderer`] module's pure
//! planning is feature-independent and unit-tested; the GTK smoke tests run
//! under a display session (CI uses `xvfb-run`) and skip cleanly headless.

pub mod renderer;
