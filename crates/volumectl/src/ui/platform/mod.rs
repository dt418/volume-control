//! Platform renderer seam.
//!
//! Windows is the verified renderer and exposes its primitives here. The
//! module is deliberately empty on macOS/Linux so shared code never sees a
//! Windows import; renderer boundaries for other platforms land in later
//! tasks and stay compile-safe.

#[cfg(target_os = "windows")]
pub mod windows;

/// Windows capability/theme/DPI/work-area helpers.
///
/// Only compiled on `target_os = "windows"`; shared modules must not rely on
/// this path.
#[cfg(target_os = "windows")]
pub use windows::primitives;
