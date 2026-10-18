//! Linux/macOS host: the platform split is `#[cfg]`-only, no separate host.
//!
//! The [`AppCore`] SSOT (global hotkeys + audio + config) owns the state, and
//! the native surfaces are Tauri-managed on these platforms: the tray is built
//! by `crate::tauri_tray` (the sink's `show_tray_menu` routes to it, and the
//! `OpenMenu` hotkey opens that menu), and the HUD overlay is a webview window
//! the sink opens for `overlay` notifications, auto-hidden after
//! `overlay_duration_ms`. The four surfaces are webview windows opened by
//! hotkeys (`window-mixer`, `window-overlay`, etc.).
//!
//! This module intentionally has no runtime state: the host wires AppCore
//! directly and the platform split is expressed by `#[cfg]` in `lib.rs`.
