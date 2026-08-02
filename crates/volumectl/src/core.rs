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
    match volume_color_rgb(percent, muted, thresholds) {
        (0x88, 0x88, 0x88) => "#888888",
        (0x27, 0xAE, 0x60) => "#27AE60",
        (0x00, 0x78, 0xD4) => "#0078D4",
        _ => "#E05C00",
    }
}

/// Map a volume % to an (r, g, b) tuple — used by the GDI overlay directly.
pub fn volume_color_rgb(percent: u8, muted: bool, thresholds: &ColorThresholds) -> (u8, u8, u8) {
    if muted || percent == 0 {
        (0x88, 0x88, 0x88) // grey
    } else if percent <= thresholds.green_up_to {
        (0x27, 0xAE, 0x60) // green
    } else if percent <= thresholds.blue_up_to {
        (0x00, 0x78, 0xD4) // blue
    } else {
        (0xE0, 0x5C, 0x00) // orange-red
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ColorThresholds;

    fn default_thresholds() -> ColorThresholds {
        ColorThresholds {
            green_up_to: 40,
            blue_up_to: 75,
            orange_up_to: 100,
        }
    }

    #[test]
    fn step_volume_clamps_at_limits() {
        assert_eq!(step_volume(0.98, 2.0), 1.0); // at max
        assert_eq!(step_volume(0.02, -2.0), 0.0); // at min
        assert_eq!(step_volume(0.50, 2.0), 0.52);
        assert_eq!(step_volume(0.50, -10.0), 0.40);
        assert_eq!(step_volume(0.0, 50.0), 0.50);
    }

    #[test]
    fn color_thresholds_follow_volumepro_palette() {
        let t = default_thresholds();
        assert_eq!(volume_color_rgb(0, false, &t), (0x88, 0x88, 0x88)); // grey
        assert_eq!(volume_color_rgb(10, false, &t), (0x27, 0xAE, 0x60)); // green
        assert_eq!(volume_color_rgb(40, false, &t), (0x27, 0xAE, 0x60)); // green boundary
        assert_eq!(volume_color_rgb(41, false, &t), (0x00, 0x78, 0xD4)); // blue
        assert_eq!(volume_color_rgb(75, false, &t), (0x00, 0x78, 0xD4)); // blue boundary
        assert_eq!(volume_color_rgb(76, false, &t), (0xE0, 0x5C, 0x00)); // orange
        assert_eq!(volume_color_rgb(100, false, &t), (0xE0, 0x5C, 0x00)); // orange max
    }

    #[test]
    fn muted_is_always_grey() {
        let t = default_thresholds();
        assert_eq!(volume_color_rgb(50, true, &t), (0x88, 0x88, 0x88));
        assert_eq!(volume_color_rgb(0, true, &t), (0x88, 0x88, 0x88));
    }
}