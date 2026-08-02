//! Windows renderer primitives.
//!
//! This module is only reachable from [`super::super`] under
//! `#[cfg(target_os = "windows")]`, so it never compiles on other targets.

pub mod primitives;
