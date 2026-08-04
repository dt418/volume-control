//! Windows renderer primitives.
//!
//! This module is only reachable from [`super::super`] under
//! `#[cfg(target_os = "windows")]`, so it never compiles on other targets.
//! Shared (platform-neutral) code must not depend on anything here.

pub mod d2d;
pub mod primitives;
pub mod text;

pub use d2d::{text_layout_metrics, Direct2dContext, HwndRenderTarget};
pub use primitives::{
    diamond_points, focus_ring_rects, rounded_rect_path, DpiMetrics, PaintCanvas, PointF, RectF,
    RoundedRectPath, SizeF,
};
pub use text::{measure_text_gdi, text_x_origin, TextAlign, TextLayout};
