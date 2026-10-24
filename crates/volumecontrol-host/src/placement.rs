//! Window placement math for native Liquid Glass surfaces.
//!
//! Pure functions mapping a surface, the monitor work area, and the DPI
//! scale to physical window geometry. Placement stays host-owned per the
//! runtime adapter contract; the liquid_glass crate never touches it.

use volumectl::ui::SurfaceId;

use liquid_glass::surfaces::{
    HELP_HEIGHT, HELP_WIDTH, MIXER_HEIGHT, MIXER_WIDTH, OVERLAY_HEIGHT, OVERLAY_WIDTH,
    SETTINGS_DEFAULT_HEIGHT, SETTINGS_DEFAULT_WIDTH,
};

/// Margin between a docked surface and the work area edge, logical pixels.
pub const EDGE_MARGIN: f32 = 16.0;

/// Vertical gap between the mixer and the volume overlay, logical pixels.
pub const MIXER_OVERLAY_GAP: f32 = 8.0;

/// Extra clearance above the bottom edge for the overlay so the HUD sits
/// above the Windows taskbar / system tray, logical pixels.
pub const OVERLAY_TRAY_CLEARANCE: f32 = 40.0;

/// Physical window geometry: position and size in device pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Logical (unrotated, unscaled) surface size for a native surface.
///
/// [`SurfaceId::Tray`] is not a rendered glass surface; it maps to a
/// zero-size placeholder so placement math stays total.
#[must_use]
pub fn logical_size(surface: SurfaceId) -> (f32, f32) {
    match surface {
        SurfaceId::Overlay => (OVERLAY_WIDTH, OVERLAY_HEIGHT),
        SurfaceId::Mixer => (MIXER_WIDTH, MIXER_HEIGHT),
        SurfaceId::Settings => (SETTINGS_DEFAULT_WIDTH, SETTINGS_DEFAULT_HEIGHT),
        SurfaceId::Help => (HELP_WIDTH, HELP_HEIGHT),
        SurfaceId::Tray => (0.0, 0.0),
    }
}

/// Work area the placement math positions into, in physical device pixels.
#[derive(Debug, Clone, Copy)]
pub struct WorkArea {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl WorkArea {
    #[must_use]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// - Overlay: bottom-right dock, lifted above the tray clearance.
/// - Mixer and Help: bottom-right dock directly above the overlay.
/// - Settings: centered in the work area.
pub fn frame_for(surface: SurfaceId, area: &WorkArea, scale: f32) -> Placement {
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let (logical_w, logical_h) = logical_size(surface);
    let width = ((logical_w * scale).round() as u32).max(1);
    let height = ((logical_h * scale).round() as u32).max(1);
    let margin = (EDGE_MARGIN * scale).round() as i32;
    let tray = (OVERLAY_TRAY_CLEARANCE * scale).round() as i32;
    let area_right = ((area.x + area.width) * scale) as i32;
    let area_bottom = ((area.y + area.height) * scale) as i32;
    let area_x = (area.x * scale) as i32;
    let area_y = (area.y * scale) as i32;

    let (x, y) = match surface {
        SurfaceId::Overlay => (
            area_right - width as i32 - margin,
            area_bottom - height as i32 - margin - tray,
        ),
        SurfaceId::Mixer => {
            let overlay_top = area_bottom - (OVERLAY_HEIGHT * scale).round() as i32 - margin - tray;
            (
                area_right - width as i32 - margin,
                overlay_top - (MIXER_OVERLAY_GAP * scale).round() as i32 - height as i32,
            )
        }
        SurfaceId::Settings | SurfaceId::Tray => (
            (area_x as f32 + ((area.width * scale - width as f32) / 2.0)).round() as i32,
            (area_y as f32 + ((area.height * scale - height as f32) / 2.0)).round() as i32,
        ),
        SurfaceId::Help => {
            let overlay_top = area_bottom - (OVERLAY_HEIGHT * scale).round() as i32 - margin - tray;
            (
                area_right - width as i32 - margin,
                overlay_top - (MIXER_OVERLAY_GAP * scale).round() as i32 - height as i32,
            )
        }
    };

    Placement {
        x,
        y,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: WorkArea = WorkArea::new(0.0, 0.0, 1600.0, 900.0);

    #[test]
    fn overlay_docks_bottom_right_above_tray() {
        let frame = frame_for(SurfaceId::Overlay, &AREA, 1.0);
        assert_eq!(frame.width, OVERLAY_WIDTH as u32);
        assert_eq!(frame.height, OVERLAY_HEIGHT as u32);
        assert_eq!(frame.x, 1600 - OVERLAY_WIDTH as i32 - 16);
        assert_eq!(
            frame.y,
            900 - OVERLAY_HEIGHT as i32 - 16 - OVERLAY_TRAY_CLEARANCE as i32
        );
    }

    #[test]
    fn mixer_docks_above_bottom_right_overlay() {
        let frame = frame_for(SurfaceId::Mixer, &AREA, 1.0);
        let overlay = frame_for(SurfaceId::Overlay, &AREA, 1.0);
        assert_eq!(frame.x, overlay.x + (OVERLAY_WIDTH - MIXER_WIDTH) as i32);
        assert_eq!(
            frame.y + frame.height as i32 + MIXER_OVERLAY_GAP as i32,
            overlay.y
        );
    }

    #[test]
    fn settings_centers_and_help_docks_bottom_right() {
        let settings = frame_for(SurfaceId::Settings, &AREA, 1.0);
        assert_eq!(
            settings.x,
            ((1600.0 - SETTINGS_DEFAULT_WIDTH) / 2.0).round() as i32
        );
        assert_eq!(
            settings.y,
            ((900.0 - SETTINGS_DEFAULT_HEIGHT) / 2.0).round() as i32
        );

        let help = frame_for(SurfaceId::Help, &AREA, 1.0);
        let overlay = frame_for(SurfaceId::Overlay, &AREA, 1.0);
        assert_eq!(help.x, 1600 - HELP_WIDTH as i32 - 16);
        assert_eq!(
            help.y + help.height as i32 + MIXER_OVERLAY_GAP as i32,
            overlay.y
        );
    }

    #[test]
    fn scale_multiplies_geometry() {
        let frame = frame_for(SurfaceId::Overlay, &AREA, 2.0);
        assert_eq!(frame.width, (OVERLAY_WIDTH * 2.0) as u32);
        assert_eq!(frame.height, (OVERLAY_HEIGHT * 2.0) as u32);
        assert_eq!(frame.x, 3200 - (OVERLAY_WIDTH * 2.0) as i32 - 32);
    }

    #[test]
    fn physical_dpi_preserves_negative_monitor_origin() {
        let area = WorkArea::new(-1920.0, 100.0, 1920.0, 1080.0);
        let frame = frame_for(SurfaceId::Mixer, &area, 2.0);
        assert_eq!(frame.x, -((MIXER_WIDTH * 2.0) as i32) - 32);
        let expected_bottom = (100.0 + 1080.0) * 2.0;
        let overlay_top = expected_bottom
            - (OVERLAY_HEIGHT * 2.0)
            - (EDGE_MARGIN * 2.0)
            - (OVERLAY_TRAY_CLEARANCE * 2.0);
        let expected_y = overlay_top - (MIXER_OVERLAY_GAP * 2.0) - (MIXER_HEIGHT * 2.0);
        assert_eq!(frame.y, expected_y as i32);
    }

    #[test]
    fn non_positive_scale_falls_back_to_one() {
        let sane = frame_for(SurfaceId::Overlay, &AREA, 1.0);
        for scale in [0.0, -1.0, f32::NAN] {
            assert_eq!(frame_for(SurfaceId::Overlay, &AREA, scale), sane);
        }
    }

    #[test]
    fn tiny_work_area_keeps_window_on_screen_origin() {
        let tiny = WorkArea::new(0.0, 0.0, 10.0, 10.0);
        let frame = frame_for(SurfaceId::Overlay, &tiny, 1.0);
        assert_eq!(frame.x, 10 - OVERLAY_WIDTH as i32 - 16);
        assert_eq!(
            frame.y,
            10 - OVERLAY_HEIGHT as i32 - 16 - OVERLAY_TRAY_CLEARANCE as i32
        );
    }
}
