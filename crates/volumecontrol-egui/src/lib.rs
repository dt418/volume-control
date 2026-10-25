//! VolumeControl egui foundation.
//!
//! The root viewport is the Mixer surface; Settings, Help, and Overlay are
//! immediate child viewports. All user intent flows back through
//! [`volumectl::host_core::AppCore`] actions so `config.ini` stays the single
//! source of truth.

pub mod app;
pub mod cli;
pub mod fonts;
pub mod i18n;
pub mod icon;
pub mod native_win32;
pub mod placement;
pub mod settings;
pub mod single_instance;
pub mod sink;
pub mod style;
pub mod surfaces;
pub mod updates;
pub mod vibrancy;
