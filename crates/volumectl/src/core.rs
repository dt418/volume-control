//! Shared application logic: volume arithmetic, threshold colours, clamping.
//! This is deliberately platform-independent so it can be unit tested.

use crate::config::ColorThresholds;

/// Current volume represented for UI purposes.
#[derive(Debug, Clone, Copy)]
pub struct UiState {
    pub percent: u8,
    pub muted: bool,
}

impl UiState {
    pub fn new(volume: f32, muted: bool) -> Self {
        Self {
            percent: (volume.clamp(0.0, 1.0) * 100.0).round() as u8,
            muted,
        }
    }
}

/// Map a volume % to the legend colour (VolumePro palette).
pub fn volume_color(percent: u8, muted: bool, thresholds: &ColorThresholds) -> &'static str {
    if muted || percent == 0 {
        "#888888" // grey
    } else if percent <= thresholds.green_up_to {
        "#27AE60" // green
    } else if percent <= thresholds.blue_up_to {
        "#0078D4" // blue
    } else {
        "#E05C00" // orange-red
    }
}

/// Clamp a target volume into [0,1].
pub fn clamp_volume(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

/// Apply a signed percentage step to a volume.
/// `step` is in percent (e.g. +2.0 or -10.0). Returns the new volume.
pub fn step_volume(current: f32, step_pct: f32) -> f32 {
    let v = current + step_pct / 100.0;
    clamp_volume(v)
}