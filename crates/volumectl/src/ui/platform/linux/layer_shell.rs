//! Wayland layer-shell protocol placement calculations and overlay anchoring.
//!
//! **Crucial Invariant**: Zero Wayland protocol code enters the portable
//! `crates/liquid_glass` renderer crate. All layer-shell placement and stacking
//! decisions live strictly within this host platform adapter.

use crate::ui::surface::{SurfaceRect, WorkArea};

/// Stacking layer in the Wayland compositor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayerShellLayer {
    /// Above standard windows, below lock screen.
    #[default]
    Overlay,
    /// Below overlay, above background windows.
    Top,
}

/// Placement configuration passed to Wayland `zwlr_layer_shell_v1` surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerShellPlacement {
    pub anchor_right: bool,
    pub anchor_bottom: bool,
    pub margin_right: i32,
    pub margin_bottom: i32,
    pub layer: LayerShellLayer,
    pub keyboard_interactive: bool,
}

impl LayerShellPlacement {
    /// Calculate bottom-right placement for the volume overlay HUD against the monitor work area.
    pub fn for_overlay(work_area: WorkArea, rect: SurfaceRect) -> Self {
        Self {
            anchor_right: true,
            anchor_bottom: true,
            margin_right: layer_shell_margin_right(work_area, rect),
            margin_bottom: layer_shell_margin_bottom(work_area, rect),
            layer: LayerShellLayer::Overlay,
            // HUD stays interactive when compositor rejects pointer pass-through
            keyboard_interactive: false,
        }
    }
}

/// Margin in physical/logical pixels from the right edge of the work area to the surface.
pub fn layer_shell_margin_right(work_area: WorkArea, rect: SurfaceRect) -> i32 {
    let right_edge = work_area.x + work_area.width;
    (right_edge - rect.right).max(0)
}

/// Margin in physical/logical pixels from the bottom edge of the work area to the surface.
pub fn layer_shell_margin_bottom(work_area: WorkArea, rect: SurfaceRect) -> i32 {
    let bottom_edge = work_area.y + work_area.height;
    (bottom_edge - rect.bottom).max(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_for_overlay_anchors_bottom_right() {
        let work_area = WorkArea::new(0, 0, 1920, 1080);
        let rect = SurfaceRect::new(1550, 956, 1886, 1044); // 336x88 overlay with 34px margin

        let placement = LayerShellPlacement::for_overlay(work_area, rect);
        assert!(placement.anchor_right);
        assert!(placement.anchor_bottom);
        assert_eq!(placement.margin_right, 34);
        assert_eq!(placement.margin_bottom, 36);
        assert_eq!(placement.layer, LayerShellLayer::Overlay);
        assert!(!placement.keyboard_interactive);
    }

    #[test]
    fn negative_monitor_coordinates_handled_correctly() {
        // Monitor positioned to the left of primary display
        let work_area = WorkArea::new(-1920, -40, 1920, 1040);
        let rect = SurfaceRect::new(-336, 876, 0, 964);

        assert_eq!(layer_shell_margin_right(work_area, rect), 0);
        assert_eq!(layer_shell_margin_bottom(work_area, rect), 36);
    }
}
