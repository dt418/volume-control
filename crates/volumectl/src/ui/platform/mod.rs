//! Platform renderer seam.
//!
//! Windows is the verified renderer and exposes its primitives here. macOS
//! and Linux expose compile-safe seams that consume only shared [`crate::ui`]
//! types; their native renderers (AppKit, GTK/libadwaita) are follow-on work
//! and are not implemented in this plan. Shared code never sees a Windows
//! import on any target.

/// Windows renderer primitives (verified implementation).
#[cfg(target_os = "windows")]
pub mod windows;

/// Windows capability/theme/DPI/work-area helpers.
///
/// Only compiled on `target_os = "windows"`; shared modules must not rely on
/// this path.
#[cfg(target_os = "windows")]
pub use windows::primitives;

/// macOS renderer seam (AppKit + CoreAudio follow-on work).
///
/// Compile-safe boundary consuming only shared [`crate::ui`] types; no AppKit
/// or CoreAudio code exists yet.
#[cfg(target_os = "macos")]
pub mod macos;

/// Linux renderer seam (GTK/libadwaita + PulseAudio/PipeWire follow-on work).
///
/// Compile-safe boundary consuming only shared [`crate::ui`] types; no GTK or
/// PulseAudio/PipeWire code exists yet.
#[cfg(target_os = "linux")]
pub mod linux;
