//! Linux/macOS host: there are no native surfaces beyond the core.
//!
//! The [`AppCore`] SSOT (global hotkeys + audio + config) is the entire
//! headless host on these platforms; the three interactive surfaces are
//! webview windows opened by hotkeys (`window-mixer` etc.). The tray menu
//! and the HUD overlay do not exist here — the sink's `show_tray_menu` and
//! `overlay` notifications are no-ops, and the `OpenMenu` hotkey keeps the
//! pre-Tauri behavior (no tray to open).
//!
//! This module intentionally has no runtime state: the host wires AppCore
//! directly and the platform split is expressed by `#[cfg]` in `lib.rs`.
