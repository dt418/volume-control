//! Cairo canvas implementation for Linux.
//!
//! Wraps a `gtk4::cairo::Context` and implements the platform-neutral
//! [`Canvas`] trait. All coordinates are in logical pixels; the Cairo
//! context's transformation matrix handles DPI scaling.

use crate::ui::canvas::{Canvas, PointF, RectF, TextAlign};
use crate::ui::theme::Rgba;

/// Linux 2D drawing surface backed by a Cairo context.
///
/// Construct via [`CairoCanvas::new`] with a Cairo context obtained from
/// a GTK4 DrawingArea or similar surface.
pub struct CairoCanvas {
    ctx: gtk4::cairo::Context,
}

impl CairoCanvas {
    /// Create a canvas from an existing Cairo context.
    pub fn new(ctx: gtk4::cairo::Context) -> Self {
        Self { ctx }
    }

    fn set_source(&self, color: Rgba) {
        self.ctx.set_source_rgba(
            color.red as f64 / 255.0,
            color.green as f64 / 255.0,
            color.blue as f64 / 255.0,
            color.alpha as f64 / 255.0,
        );
    }
}

impl Canvas for CairoCanvas {
    fn fill_rect(&mut self, rect: RectF, color: Rgba) {
        self.set_source(color);
        self.ctx.rectangle(
            rect.left as f64,
            rect.top as f64,
            rect.width() as f64,
            rect.height() as f64,
        );
        self.ctx.fill();
    }

    fn stroke_rect(&mut self, rect: RectF, color: Rgba, width_px: f32) {
        self.set_source(color);
        self.ctx.set_line_width(width_px as f64);
        self.ctx.rectangle(
            rect.left as f64,
            rect.top as f64,
            rect.width() as f64,
            rect.height() as f64,
        );
        self.ctx.stroke();
    }

    fn fill_circle(&mut self, center: PointF, radius: f32, color: Rgba) {
        self.set_source(color);
        self.ctx.arc(
            center.x as f64,
            center.y as f64,
            radius as f64,
            0.0,
            std::f64::consts::TAU,
        );
        self.ctx.fill();
    }

    fn stroke_circle(&mut self, center: PointF, radius: f32, color: Rgba, width_px: f32) {
        self.set_source(color);
        self.ctx.set_line_width(width_px as f64);
        self.ctx.arc(
            center.x as f64,
            center.y as f64,
            radius as f64,
            0.0,
            std::f64::consts::TAU,
        );
        self.ctx.stroke();
    }

    fn fill_diamond(&mut self, center: PointF, half_size: f32, color: Rgba) {
        self.set_source(color);
        let cx = center.x as f64;
        let cy = center.y as f64;
        let hs = half_size as f64;
        self.ctx.move_to(cx, cy - hs);
        self.ctx.line_to(cx + hs, cy);
        self.ctx.line_to(cx, cy + hs);
        self.ctx.line_to(cx - hs, cy);
        self.ctx.close_path();
        self.ctx.fill();
    }

    fn stroke_diamond(&mut self, center: PointF, half_size: f32, color: Rgba, width_px: f32) {
        self.set_source(color);
        self.ctx.set_line_width(width_px as f64);
        let cx = center.x as f64;
        let cy = center.y as f64;
        let hs = half_size as f64;
        self.ctx.move_to(cx, cy - hs);
        self.ctx.line_to(cx + hs, cy);
        self.ctx.line_to(cx, cy + hs);
        self.ctx.line_to(cx - hs, cy);
        self.ctx.close_path();
        self.ctx.stroke();
    }

    fn draw_text(
        &mut self,
        rect: RectF,
        text: &str,
        align: TextAlign,
        size_px: f32,
        _weight: u16,
        color: Rgba,
    ) {
        self.set_source(color);
        self.ctx.select_font_face(
            "sans-serif",
            gtk4::cairo::FontSlant::Normal,
            gtk4::cairo::FontWeight::Normal,
        );
        self.ctx.set_font_size(size_px as f64);

        let extents = self.ctx.text_extents(text);
        let text_width = extents.width;
        let text_height = extents.height;

        let x = match align {
            TextAlign::Left => rect.left as f64,
            TextAlign::Center => {
                let rect_center = (rect.left + rect.width() / 2.0) as f64;
                rect_center - text_width / 2.0
            }
            TextAlign::Right => rect.right as f64 - text_width,
        };

        let y = rect.top as f64 + (rect.height() as f64 + text_height) / 2.0 - extents.y_bearing;

        self.ctx.move_to(x, y);
        self.ctx.show_text(text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cairo_canvas_new_does_not_panic() {
        let surface =
            gtk4::cairo::ImageSurface::create(gtk4::cairo::Format::ARgb32, 100, 100).unwrap();
        let ctx = gtk4::cairo::Context::new(&surface);
        let _canvas = CairoCanvas::new(ctx);
    }
}
