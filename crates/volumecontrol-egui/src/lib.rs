//! VolumeControl egui foundation.
//!
//! The root viewport is the Mixer surface; Settings, Help, and Overlay are
//! immediate child viewports. All user intent flows back through
//! [`volumectl::host_core::AppCore`] actions so `config.ini` stays the single
//! source of truth.

pub mod app;
pub mod fonts;
pub mod i18n;
pub mod placement;
pub mod sink;
pub mod style;
pub mod vibrancy;
