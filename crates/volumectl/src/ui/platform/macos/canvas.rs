//! CoreGraphics canvas implementation for macOS.
//!
//! Wraps a `CGContextRef` obtained from the current `NSGraphicsContext` and
//! implements the platform-neutral [`Canvas`] trait. All coordinates are
//! converted from logical pixels to physical pixels via the display scale
//! factor stored in [`CoreGraphicsCanvas::scale`].

use crate::ui::canvas::{Canvas, PointF, RectF, TextAlign};
use crate::ui::theme::Rgba;

use std::ffi::c_void;

use objc2_app_kit::{NSGraphicsContext, NSScreen};
use objc2_foundation::MainThreadMarker;

type CGContextRef = *mut c_void;

// ── Core Graphics extern bindings ──────────────────────────────────────

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGContextSetRGBFillColor(ctx: CGContextRef, red: f64, green: f64, blue: f64, alpha: f64);
    fn CGContextSetRGBStrokeColor(ctx: CGContextRef, red: f64, green: f64, blue: f64, alpha: f64);
    fn CGContextSetLineWidth(ctx: CGContextRef, width: f64);
    fn CGContextFillRect(ctx: CGContextRef, rect: CGSysRect);
    fn CGContextStrokeRect(ctx: CGContextRef, rect: CGSysRect);
    fn CGContextFillEllipseInRect(ctx: CGContextRef, rect: CGSysRect);
    fn CGContextStrokeEllipseInRect(ctx: CGContextRef, rect: CGSysRect);
    fn CGContextBeginPath(ctx: CGContextRef);
    fn CGContextMoveToPoint(ctx: CGContextRef, x: f64, y: f64);
    fn CGContextAddLineToPoint(ctx: CGContextRef, x: f64, y: f64);
    fn CGContextClosePath(ctx: CGContextRef);
    fn CGContextFillPath(ctx: CGContextRef);
    fn CGContextStrokePath(ctx: CGContextRef);
}

/// CoreGraphics-compatible rectangle (origin x, y, width, height).
#[repr(C)]
#[derive(Clone, Copy)]
struct CGSysRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl CGSysRect {
    const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// macOS 2D drawing surface backed by a Core Graphics context.
///
/// Construct via [`CoreGraphicsCanvas::current()`], which retrieves the
/// current `NSGraphicsContext`'s `CGContextRef`. All coordinates are in
/// logical pixels; the canvas scales by `scale` (display backing scale)
/// when calling Core Graphics.
pub struct CoreGraphicsCanvas {
    ctx: CGContextRef,
    scale: f64,
}

// SAFETY: Only used within a single draw call on the main thread.
unsafe impl Send for CoreGraphicsCanvas {}
unsafe impl Sync for CoreGraphicsCanvas {}

impl CoreGraphicsCanvas {
    /// Create a canvas from the current `NSGraphicsContext`.
    ///
    /// Returns `None` if there is no current graphics context (e.g. off
    /// the main thread or before the window is configured).
    pub fn current() -> Option<Self> {
        let mtm = MainThreadMarker::new()?;
        let ns_ctx = NSGraphicsContext::currentContext()?;
        let retained_ctx = ns_ctx.CGContext();
        let ctx_ptr = objc2::rc::Retained::as_ptr(&retained_ctx) as *mut c_void;
        if ctx_ptr.is_null() {
            return None;
        }
        let scale = NSScreen::mainScreen(mtm)?.backingScaleFactor();
        Some(Self {
            ctx: ctx_ptr,
            scale,
        })
    }

    fn cg_rect(&self, rect: RectF) -> CGSysRect {
        let s = self.scale;
        CGSysRect::new(
            rect.left as f64 * s,
            rect.top as f64 * s,
            rect.width() as f64 * s,
            rect.height() as f64 * s,
        )
    }

    fn cg_point(&self, pt: PointF) -> (f64, f64) {
        let s = self.scale;
        (pt.x as f64 * s, pt.y as f64 * s)
    }
}

impl Canvas for CoreGraphicsCanvas {
    fn fill_rect(&mut self, rect: RectF, color: Rgba) {
        let c = self.ctx;
        let r = color.red as f64 / 255.0;
        let g = color.green as f64 / 255.0;
        let b = color.blue as f64 / 255.0;
        let a = color.alpha as f64 / 255.0;
        unsafe {
            CGContextSetRGBFillColor(c, r, g, b, a);
            CGContextFillRect(c, self.cg_rect(rect));
        }
    }

    fn stroke_rect(&mut self, rect: RectF, color: Rgba, width_px: f32) {
        let c = self.ctx;
        let r = color.red as f64 / 255.0;
        let g = color.green as f64 / 255.0;
        let b = color.blue as f64 / 255.0;
        let a = color.alpha as f64 / 255.0;
        unsafe {
            CGContextSetRGBStrokeColor(c, r, g, b, a);
            CGContextSetLineWidth(c, width_px as f64 * self.scale);
            CGContextStrokeRect(c, self.cg_rect(rect));
        }
    }

    fn fill_circle(&mut self, center: PointF, radius: f32, color: Rgba) {
        let c = self.ctx;
        let r = color.red as f64 / 255.0;
        let g = color.green as f64 / 255.0;
        let b = color.blue as f64 / 255.0;
        let a = color.alpha as f64 / 255.0;
        let (cx, cy) = self.cg_point(center);
        let r_scaled = radius as f64 * self.scale;
        let cg_rect = CGSysRect::new(cx - r_scaled, cy - r_scaled, r_scaled * 2.0, r_scaled * 2.0);
        unsafe {
            CGContextSetRGBFillColor(c, r, g, b, a);
            CGContextFillEllipseInRect(c, cg_rect);
        }
    }

    fn stroke_circle(&mut self, center: PointF, radius: f32, color: Rgba, width_px: f32) {
        let c = self.ctx;
        let r = color.red as f64 / 255.0;
        let g = color.green as f64 / 255.0;
        let b = color.blue as f64 / 255.0;
        let a = color.alpha as f64 / 255.0;
        let (cx, cy) = self.cg_point(center);
        let r_scaled = radius as f64 * self.scale;
        let cg_rect = CGSysRect::new(cx - r_scaled, cy - r_scaled, r_scaled * 2.0, r_scaled * 2.0);
        unsafe {
            CGContextSetRGBStrokeColor(c, r, g, b, a);
            CGContextSetLineWidth(c, width_px as f64 * self.scale);
            CGContextStrokeEllipseInRect(c, cg_rect);
        }
    }

    fn fill_diamond(&mut self, center: PointF, half_size: f32, color: Rgba) {
        let c = self.ctx;
        let r = color.red as f64 / 255.0;
        let g = color.green as f64 / 255.0;
        let b = color.blue as f64 / 255.0;
        let a = color.alpha as f64 / 255.0;
        let (cx, cy) = self.cg_point(center);
        let hs = half_size as f64 * self.scale;
        unsafe {
            CGContextSetRGBFillColor(c, r, g, b, a);
            CGContextBeginPath(c);
            CGContextMoveToPoint(c, cx, cy - hs);
            CGContextAddLineToPoint(c, cx + hs, cy);
            CGContextAddLineToPoint(c, cx, cy + hs);
            CGContextAddLineToPoint(c, cx - hs, cy);
            CGContextClosePath(c);
            CGContextFillPath(c);
        }
    }

    fn stroke_diamond(&mut self, center: PointF, half_size: f32, color: Rgba, width_px: f32) {
        let c = self.ctx;
        let r = color.red as f64 / 255.0;
        let g = color.green as f64 / 255.0;
        let b = color.blue as f64 / 255.0;
        let a = color.alpha as f64 / 255.0;
        let (cx, cy) = self.cg_point(center);
        let hs = half_size as f64 * self.scale;
        unsafe {
            CGContextSetRGBStrokeColor(c, r, g, b, a);
            CGContextSetLineWidth(c, width_px as f64 * self.scale);
            CGContextBeginPath(c);
            CGContextMoveToPoint(c, cx, cy - hs);
            CGContextAddLineToPoint(c, cx + hs, cy);
            CGContextAddLineToPoint(c, cx, cy + hs);
            CGContextAddLineToPoint(c, cx - hs, cy);
            CGContextClosePath(c);
            CGContextStrokePath(c);
        }
    }

    fn draw_text(
        &mut self,
        _rect: RectF,
        _text: &str,
        _align: TextAlign,
        _size_px: f32,
        _weight: u16,
        _color: Rgba,
    ) {
        // Stub: text rendering via NSString drawInRect:attributes: will be
        // added in a follow-up task.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cg_rect_converts_logical_to_physical() {
        let canvas = CoreGraphicsCanvas {
            ctx: std::ptr::null_mut(),
            scale: 2.0,
        };
        let r = canvas.cg_rect(RectF::new(10.0, 20.0, 50.0, 80.0));
        assert_eq!(r.x, 20.0);
        assert_eq!(r.y, 40.0);
        assert_eq!(r.width, 80.0);
        assert_eq!(r.height, 120.0);
    }

    #[test]
    fn cg_point_converts_logical_to_physical() {
        let canvas = CoreGraphicsCanvas {
            ctx: std::ptr::null_mut(),
            scale: 2.0,
        };
        let (x, y) = canvas.cg_point(PointF::new(5.0, 15.0));
        assert_eq!(x, 10.0);
        assert_eq!(y, 30.0);
    }
}
