//! Platform-neutral drawing canvas and overlay content renderer.
//!
//! The [`Canvas`] trait abstracts 2D drawing primitives that platform backends
//! (CoreGraphics on macOS, Cairo on Linux, GDI/D2D on Windows) implement.
//! [`OverlayContentRenderer`] draws the volume overlay's content — title,
//! percent value, device label, and Signal Rail — using only the [`Canvas`]
//! trait, so the rendering logic is identical on every platform.
//!
//! All coordinates are logical pixels; the platform backend scales once via
//! its DPI metrics.

use crate::ui::signal_rail::{rail_geometry, MarkerGeometry, SignalRail, TrackRect};
use crate::ui::theme::{Rgba, TypographyTokens};

/// Rectangle with float coordinates in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectF {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl RectF {
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub const fn width(self) -> f32 {
        self.right - self.left
    }

    pub const fn height(self) -> f32 {
        self.bottom - self.top
    }
}

/// Point with float coordinates in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointF {
    pub x: f32,
    pub y: f32,
}

impl PointF {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Text alignment for [`Canvas::draw_text`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

/// Platform-neutral 2D drawing surface.
///
/// Every platform backend (CoreGraphics, Cairo, GDI/D2D) implements this trait.
/// Methods accept logical pixel coordinates and RGBA colors; the backend
/// handles DPI scaling internally.
pub trait Canvas {
    /// Fill a rectangle with `color`.
    fn fill_rect(&mut self, rect: RectF, color: Rgba);

    /// Stroke a rectangle with `color` and `width_px` stroke width.
    fn stroke_rect(&mut self, rect: RectF, color: Rgba, width_px: f32);

    /// Fill a circle centered at `center` with `radius`.
    fn fill_circle(&mut self, center: PointF, radius: f32, color: Rgba);

    /// Stroke a circle centered at `center` with `radius` and `width_px` stroke.
    fn stroke_circle(&mut self, center: PointF, radius: f32, color: Rgba, width_px: f32);

    /// Fill a diamond (rotated square) centered at `center` with `half_size`.
    fn fill_diamond(&mut self, center: PointF, half_size: f32, color: Rgba);

    /// Stroke a diamond centered at `center` with `half_size` and `width_px` stroke.
    fn stroke_diamond(&mut self, center: PointF, half_size: f32, color: Rgba, width_px: f32);

    /// Draw `text` inside `rect` with `align` alignment, `size_px` font size,
    /// `weight` font weight, and `color`.
    fn draw_text(
        &mut self,
        rect: RectF,
        text: &str,
        align: TextAlign,
        size_px: f32,
        weight: u16,
        color: Rgba,
    );
}

/// The 336×88 overlay surface layout.
///
/// All rects are in logical pixels, matching the Windows `VolumeLayout`.
const OVERLAY_WIDTH: f32 = 336.0;
const OVERLAY_HEIGHT: f32 = 88.0;
const PADDING: f32 = 16.0;

/// Draws the overlay content on any platform via the [`Canvas`] trait.
///
/// The overlay shows:
/// - Row 1: "Volume" title (left, 11px) and percent/"Muted" (right, 28px)
/// - Row 2: "System output" device label (left, 11px)
/// - Row 3: Signal Rail track (8px tall, centered in 55..88 band)
pub struct OverlayContentRenderer;

impl OverlayContentRenderer {
    /// Render the overlay content for `rail` onto `canvas`.
    ///
    /// `tokens` provides typography and threshold colors. `device_name` is
    /// the output device label (e.g. "System output").
    pub fn render(
        canvas: &mut impl Canvas,
        rail: &SignalRail,
        tokens: &TypographyTokens,
        device_name: &str,
    ) {
        let title_rect = RectF::new(PADDING, PADDING, OVERLAY_WIDTH - PADDING, 32.0);
        let value_rect = RectF::new(PADDING, PADDING, OVERLAY_WIDTH - PADDING, 55.0);
        let output_rect = RectF::new(PADDING, 44.0, OVERLAY_WIDTH - PADDING, 55.0);

        // Background fill.
        canvas.fill_rect(
            RectF::new(0.0, 0.0, OVERLAY_WIDTH, OVERLAY_HEIGHT),
            Rgba::from_rgb(0x17, 0x1C, 0x24),
        );

        // Title: "Volume" (left-aligned, caption size).
        canvas.draw_text(
            title_rect,
            "Volume",
            TextAlign::Left,
            tokens.caption.size_px,
            tokens.caption.weight,
            Rgba::from_rgb(0xAA, 0xB4, 0xC3),
        );

        // Value: "72%" or "Muted" (right-aligned, display_value size).
        let rail = rail.clamped();
        let (value_text, value_color) = if rail.muted {
            ("Muted".to_string(), rail.fill_color())
        } else {
            (
                format!("{}%", rail.percent),
                Rgba::from_rgb(0xF5, 0xF7, 0xFA),
            )
        };
        canvas.draw_text(
            value_rect,
            &value_text,
            TextAlign::Right,
            tokens.display_value.size_px,
            tokens.display_value.weight,
            value_color,
        );

        // Device label (left-aligned, caption size).
        canvas.draw_text(
            output_rect,
            device_name,
            TextAlign::Left,
            tokens.caption.size_px,
            tokens.caption.weight,
            Rgba::from_rgb(0x52, 0x60, 0x71),
        );

        // Signal Rail track: centered vertically in 55..88 band, 8px tall.
        let rail_top = 55.0 + ((88.0 - 55.0 - 8.0) * 0.5);
        let track = TrackRect {
            left: PADDING,
            right: OVERLAY_WIDTH - PADDING,
            top: rail_top,
            bottom: rail_top + 8.0,
        };

        let geometry = rail_geometry(&rail, track, 6.0, 6.0);

        // Track background.
        canvas.fill_rect(
            RectF::new(track.left, track.top, track.right, track.bottom),
            Rgba::from_rgb(0x34, 0x40, 0x52),
        );

        // Threshold fill (up to fill_right).
        canvas.fill_rect(
            RectF::new(track.left, track.top, geometry.fill_right, track.bottom),
            rail.fill_color(),
        );

        // Marker.
        match geometry.marker {
            MarkerGeometry::Thumb {
                center_x,
                center_y,
                radius,
            } => {
                canvas.fill_circle(PointF::new(center_x, center_y), radius, rail.fill_color());
            }
            MarkerGeometry::MutedDiamond {
                center_x,
                center_y,
                half_size,
            } => {
                canvas.stroke_diamond(
                    PointF::new(center_x, center_y),
                    half_size,
                    rail.fill_color(),
                    2.0,
                );
            }
        }
    }
}

/// Mixer surface layout providing component rects.
///
/// All rects are in logical pixels. The mixer uses a wider surface than
/// the overlay and includes a trackbar, mute/reset/close buttons, and
/// a title/subtitle area.
pub struct MixerLayout;

impl MixerLayout {
    /// Title rect in the mixer surface.
    pub fn title_rect() -> RectF {
        RectF::new(16.0, 16.0, 320.0, 32.0)
    }

    /// Subtitle rect (device name) in the mixer surface.
    pub fn subtitle_rect() -> RectF {
        RectF::new(16.0, 36.0, 320.0, 52.0)
    }

    /// Volume value rect (right-aligned percent) in the mixer surface.
    pub fn value_rect() -> RectF {
        RectF::new(16.0, 16.0, 320.0, 52.0)
    }

    /// Slider trackbar rect in the mixer surface.
    pub fn slider_rect() -> RectF {
        RectF::new(16.0, 60.0, 320.0, 68.0)
    }

    /// Mute button rect in the mixer surface.
    pub fn mute_button_rect() -> RectF {
        RectF::new(16.0, 76.0, 80.0, 108.0)
    }

    /// Reset button rect in the mixer surface.
    pub fn reset_button_rect() -> RectF {
        RectF::new(88.0, 76.0, 152.0, 108.0)
    }

    /// Close button rect in the mixer surface.
    pub fn close_button_rect() -> RectF {
        RectF::new(272.0, 76.0, 336.0, 108.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::VolumeThresholdColors;

    /// Mock canvas that records drawing calls for verification.
    #[derive(Debug, Default)]
    struct MockCanvas {
        fill_rects: Vec<(RectF, Rgba)>,
        stroke_rects: Vec<(RectF, Rgba, f32)>,
        fill_circles: Vec<(PointF, f32, Rgba)>,
        stroke_circles: Vec<(PointF, f32, Rgba, f32)>,
        fill_diamonds: Vec<(PointF, f32, Rgba)>,
        stroke_diamonds: Vec<(PointF, f32, Rgba, f32)>,
        texts: Vec<(RectF, String, TextAlign, f32, u16, Rgba)>,
    }

    impl Canvas for MockCanvas {
        fn fill_rect(&mut self, rect: RectF, color: Rgba) {
            self.fill_rects.push((rect, color));
        }

        fn stroke_rect(&mut self, rect: RectF, color: Rgba, width_px: f32) {
            self.stroke_rects.push((rect, color, width_px));
        }

        fn fill_circle(&mut self, center: PointF, radius: f32, color: Rgba) {
            self.fill_circles.push((center, radius, color));
        }

        fn stroke_circle(&mut self, center: PointF, radius: f32, color: Rgba, width_px: f32) {
            self.stroke_circles.push((center, radius, color, width_px));
        }

        fn fill_diamond(&mut self, center: PointF, half_size: f32, color: Rgba) {
            self.fill_diamonds.push((center, half_size, color));
        }

        fn stroke_diamond(&mut self, center: PointF, half_size: f32, color: Rgba, width_px: f32) {
            self.stroke_diamonds
                .push((center, half_size, color, width_px));
        }

        fn draw_text(
            &mut self,
            rect: RectF,
            text: &str,
            align: TextAlign,
            size_px: f32,
            weight: u16,
            color: Rgba,
        ) {
            self.texts
                .push((rect, text.to_string(), align, size_px, weight, color));
        }
    }

    fn rail(percent: u8, muted: bool) -> SignalRail {
        SignalRail::new(percent, muted, VolumeThresholdColors::default(), 40, 75)
    }

    // ── RectF / PointF tests ───────────────────────────────────────────

    #[test]
    fn rectf_new_and_dimensions() {
        let r = RectF::new(10.0, 20.0, 50.0, 80.0);
        assert_eq!(r.width(), 40.0);
        assert_eq!(r.height(), 60.0);
    }

    #[test]
    fn pointf_new() {
        let p = PointF::new(3.0, 7.0);
        assert_eq!(p.x, 3.0);
        assert_eq!(p.y, 7.0);
    }

    // ── OverlayContentRenderer tests ───────────────────────────────────

    #[test]
    fn overlay_draws_background_and_text() {
        let mut canvas = MockCanvas::default();
        let r = rail(72, false);
        let tokens = TypographyTokens::default();

        OverlayContentRenderer::render(&mut canvas, &r, &tokens, "Speakers");

        // Background fill.
        assert_eq!(canvas.fill_rects.len(), 3); // bg + track bg + threshold fill
        assert_eq!(
            canvas.fill_rects[0].0,
            RectF::new(0.0, 0.0, OVERLAY_WIDTH, OVERLAY_HEIGHT)
        );
        assert_eq!(canvas.fill_rects[0].1, Rgba::from_rgb(0x17, 0x1C, 0x24));

        // Text calls: title, value, device.
        assert_eq!(canvas.texts.len(), 3);
        assert_eq!(canvas.texts[0].1, "Volume");
        assert_eq!(canvas.texts[0].2, TextAlign::Left);
        assert_eq!(canvas.texts[1].1, "72%");
        assert_eq!(canvas.texts[1].2, TextAlign::Right);
        assert_eq!(canvas.texts[2].1, "Speakers");
        assert_eq!(canvas.texts[2].2, TextAlign::Left);
    }

    #[test]
    fn overlay_muted_shows_muted_label() {
        let mut canvas = MockCanvas::default();
        let r = rail(50, true);
        let tokens = TypographyTokens::default();

        OverlayContentRenderer::render(&mut canvas, &r, &tokens, "Headphones");

        assert_eq!(canvas.texts[1].1, "Muted");
        assert_eq!(canvas.texts[1].5, VolumeThresholdColors::default().muted);
    }

    #[test]
    fn overlay_normal_uses_thumb_marker() {
        let mut canvas = MockCanvas::default();
        let r = rail(50, false);
        let tokens = TypographyTokens::default();

        OverlayContentRenderer::render(&mut canvas, &r, &tokens, "Device");

        assert_eq!(canvas.fill_circles.len(), 1);
        assert_eq!(canvas.stroke_diamonds.len(), 0);
    }

    #[test]
    fn overlay_muted_uses_diamond_marker() {
        let mut canvas = MockCanvas::default();
        let r = rail(50, true);
        let tokens = TypographyTokens::default();

        OverlayContentRenderer::render(&mut canvas, &r, &tokens, "Device");

        assert_eq!(canvas.fill_circles.len(), 0);
        assert_eq!(canvas.stroke_diamonds.len(), 1);
    }

    #[test]
    fn overlay_draws_track_background() {
        let mut canvas = MockCanvas::default();
        let r = rail(50, false);
        let tokens = TypographyTokens::default();

        OverlayContentRenderer::render(&mut canvas, &r, &tokens, "Device");

        // fill_rects: [0] = bg, [1] = track bg, [2] = threshold fill
        let track_bg = canvas.fill_rects[1];
        assert_eq!(track_bg.0.left, PADDING);
        assert_eq!(track_bg.0.right, OVERLAY_WIDTH - PADDING);
        assert_eq!(track_bg.0.height(), 8.0);
        assert_eq!(track_bg.1, Rgba::from_rgb(0x34, 0x40, 0x52));
    }

    // ── MixerLayout tests ──────────────────────────────────────────────

    #[test]
    fn mixer_layout_rects_have_positive_dimensions() {
        let rects = [
            ("title", MixerLayout::title_rect()),
            ("subtitle", MixerLayout::subtitle_rect()),
            ("value", MixerLayout::value_rect()),
            ("slider", MixerLayout::slider_rect()),
            ("mute", MixerLayout::mute_button_rect()),
            ("reset", MixerLayout::reset_button_rect()),
            ("close", MixerLayout::close_button_rect()),
        ];
        for (name, r) in &rects {
            assert!(r.width() > 0.0, "{name} width {}", r.width());
            assert!(r.height() > 0.0, "{name} height {}", r.height());
        }
    }

    #[test]
    fn mixer_layout_slider_has_expected_height() {
        let slider = MixerLayout::slider_rect();
        assert_eq!(slider.height(), 8.0);
    }

    #[test]
    fn mixer_layout_buttons_are_same_height() {
        let mute = MixerLayout::mute_button_rect();
        let reset = MixerLayout::reset_button_rect();
        let close = MixerLayout::close_button_rect();
        assert_eq!(mute.height(), reset.height());
        assert_eq!(reset.height(), close.height());
        assert_eq!(mute.height(), 32.0);
    }
}
