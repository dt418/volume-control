//! Platform renderer seam.
//!
//! Windows is the verified renderer and exposes its primitives here. macOS
//! and Linux implement the same [`crate::ui::NativeRenderer`] contract and
//! consume only shared [`crate::ui`] types: AppKit surface construction on
//! macOS, GTK4/libadwaita on Linux (behind the `gtk-renderer` feature).
//! Shared code never sees a Windows import on any target.

/// Windows renderer primitives (verified implementation).
#[cfg(target_os = "windows")]
pub mod windows;

/// Windows capability/theme/DPI/work-area helpers.
///
/// Only compiled on `target_os = "windows"`; shared modules must not rely on
/// this path.
#[cfg(target_os = "windows")]
pub use windows::primitives;

/// macOS renderer (AppKit surface construction + CoreAudio backend).
///
/// Compile-safe boundary consuming only shared [`crate::ui`] types; the AppKit
/// surface code lives in this module's `renderer` submodule and the CoreAudio
/// backend in `crate::audio_macos`.
#[cfg(target_os = "macos")]
pub mod macos;

/// Linux renderer (GTK4/libadwaita surface construction + PulseAudio backend).
///
/// Compile-safe boundary consuming only shared [`crate::ui`] types; the GTK
/// surface code lives in this module's `canvas` submodule behind the
/// `gtk-renderer` feature and the PulseAudio backend in `crate::audio_linux`.
#[cfg(target_os = "linux")]
pub mod linux;
