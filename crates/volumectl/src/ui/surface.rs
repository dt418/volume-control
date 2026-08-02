//! Platform-neutral surface geometry and placement contracts.
//!
//! All placement is pure math over a [`WorkArea`]: no screen metrics, no
//! platform queries. The host (Task 5) measures the monitor work area and
//! feeds it in; negative-origin monitors and secondary displays work the
//! same as the primary because coordinates are simply relative to the
//! work-area origin.

/// A monitor work area: the usable region of a screen excluding system
/// chrome such as taskbars and docks.
///
/// Fields are pixel coordinates as reported by the platform. `x`/`y` may be
/// negative when the monitor sits left or above the primary display;
/// `width`/`height` are positive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkArea {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl WorkArea {
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// X coordinate of the right edge (exclusive).
    pub const fn right(self) -> i32 {
        self.x + self.width
    }

    /// Y coordinate of the bottom edge (exclusive).
    pub const fn bottom(self) -> i32 {
        self.y + self.height
    }
}

/// A surface size in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceSize {
    pub width: i32,
    pub height: i32,
}

impl SurfaceSize {
    pub const fn new(width: i32, height: i32) -> Self {
        Self { width, height }
    }
}

/// An axis-aligned surface rectangle in work-area coordinates.
///
/// `left`/`top` are inclusive edges; `right`/`bottom` are exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl SurfaceRect {
    pub const fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub const fn width(self) -> i32 {
        self.right - self.left
    }

    pub const fn height(self) -> i32 {
        self.bottom - self.top
    }
}

/// Place a surface in the bottom-right corner of `work_area`, inset
/// `margin_x` from the right edge and `margin_y` from the bottom edge.
///
/// The returned rectangle always satisfies `right == work_area.right() -
/// margin_x` and `bottom == work_area.bottom() - margin_y`, for any work-area
/// origin (including negative ones). Callers must ensure the surface fits;
/// no clamping is applied.
pub fn place_overlay(
    work_area: WorkArea,
    size: SurfaceSize,
    margin_x: i32,
    margin_y: i32,
) -> SurfaceRect {
    let right = work_area.right() - margin_x;
    let bottom = work_area.bottom() - margin_y;
    SurfaceRect {
        left: right - size.width,
        top: bottom - size.height,
        right,
        bottom,
    }
}

/// Place the mixer in the bottom-right corner, directly above the overlay
/// with exactly `gap` pixels of vertical separation.
///
/// The mixer shares the overlay's right edge (both are inset by `margin_x`)
/// and its bottom edge sits `gap` above the overlay's top edge, so
/// `mixer.bottom + gap == overlay.top` always holds. Coordinates follow the
/// same pure-math rules as [`place_overlay`] and work for negative-origin
/// work areas.
pub fn place_mixer_above_overlay(
    work_area: WorkArea,
    mixer_size: SurfaceSize,
    overlay_size: SurfaceSize,
    margin_x: i32,
    margin_y: i32,
    gap: i32,
) -> SurfaceRect {
    let overlay = place_overlay(work_area, overlay_size, margin_x, margin_y);
    let right = overlay.right;
    let bottom = overlay.top - gap;
    SurfaceRect {
        left: right - mixer_size.width,
        top: bottom - mixer_size.height,
        right,
        bottom,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MARGIN_X: i32 = 20;
    const MARGIN_Y: i32 = 40;
    const GAP: i32 = 16;

    #[test]
    fn overlay_places_bottom_right_on_known_primary_work_area() {
        let work_area = WorkArea::new(0, 0, 2560, 1400);
        let overlay = place_overlay(
            work_area,
            SurfaceSize::new(320, 64),
            MARGIN_X,
            MARGIN_Y,
        );

        assert_eq!(overlay, SurfaceRect::new(2220, 1296, 2540, 1360));
        assert_eq!(overlay.width(), 320);
        assert_eq!(overlay.height(), 64);
    }

    #[test]
    fn mixer_places_above_overlay_with_exact_gap_and_aligned_edges() {
        let work_area = WorkArea::new(0, 0, 2560, 1400);
        let overlay = place_overlay(
            work_area,
            SurfaceSize::new(320, 64),
            MARGIN_X,
            MARGIN_Y,
        );
        let mixer = place_mixer_above_overlay(
            work_area,
            SurfaceSize::new(360, 178),
            SurfaceSize::new(320, 64),
            MARGIN_X,
            MARGIN_Y,
            GAP,
        );

        assert_eq!(mixer, SurfaceRect::new(2180, 1102, 2540, 1280));
        assert_eq!(mixer.width(), 360);
        assert_eq!(mixer.height(), 178);
        assert_eq!(mixer.bottom + GAP, overlay.top);
        assert_eq!(mixer.right, overlay.right);
    }

    #[test]
    fn placement_works_on_negative_origin_work_area() {
        let work_area = WorkArea::new(-1920, 0, 1920, 1040);
        let overlay = place_overlay(
            work_area,
            SurfaceSize::new(320, 64),
            MARGIN_X,
            MARGIN_Y,
        );
        let mixer = place_mixer_above_overlay(
            work_area,
            SurfaceSize::new(360, 178),
            SurfaceSize::new(320, 64),
            MARGIN_X,
            MARGIN_Y,
            GAP,
        );

        // Bottom-right of the work area: right edge at 0 - 20, bottom at
        // 1040 - 40, with negative coordinates for left/top.
        assert_eq!(overlay, SurfaceRect::new(-340, 936, -20, 1000));
        assert_eq!(mixer, SurfaceRect::new(-380, 742, -20, 920));
        assert_eq!(mixer.bottom + GAP, overlay.top);
        assert_eq!(mixer.right, overlay.right);
        assert_eq!(mixer.right, work_area.right() - MARGIN_X);

        // Everything remains within the work area.
        assert!(overlay.left >= work_area.x && mixer.left >= work_area.x);
        assert!(overlay.top >= work_area.y && mixer.top >= work_area.y);
        assert!(overlay.right <= work_area.right());
        assert!(mixer.bottom <= work_area.bottom());
    }

    #[test]
    fn zero_margins_align_surfaces_with_work_area_edges() {
        let work_area = WorkArea::new(0, 0, 800, 600);
        let overlay = place_overlay(work_area, SurfaceSize::new(100, 50), 0, 0);
        let mixer = place_mixer_above_overlay(
            work_area,
            SurfaceSize::new(120, 40),
            SurfaceSize::new(100, 50),
            0,
            0,
            GAP,
        );

        assert_eq!(overlay.right, 800);
        assert_eq!(overlay.bottom, 600);
        assert_eq!(mixer.right, 800);
        assert_eq!(mixer.bottom, 600 - 50 - GAP);
    }
}
