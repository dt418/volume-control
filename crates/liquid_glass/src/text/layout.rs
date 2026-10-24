//! Text buffer shaping, geometric measurement, and ellipsis truncation.

use glyphon::cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Weight};

/// Text layout styling and metrics properties.
#[derive(Debug, Clone)]
pub struct TextProperties<'a> {
    pub font_size: f32,
    pub line_height: f32,
    pub family: &'a str,
    pub weight: Weight,
}

impl Default for TextProperties<'static> {
    fn default() -> Self {
        Self {
            font_size: 14.0,
            line_height: 18.0,
            family: super::fonts::default_font_family(),
            weight: Weight::NORMAL,
        }
    }
}

/// Layout utilities for text measurement, shaping, and truncation.
pub struct TextLayout;

impl TextLayout {
    /// Create and shape a `Buffer` with given text, styling properties, and optional wrap width.
    pub fn create_buffer(
        font_system: &mut FontSystem,
        text: &str,
        props: &TextProperties<'_>,
        max_width: Option<f32>,
    ) -> Buffer {
        let metrics = Metrics::new(props.font_size, props.line_height);
        let mut buffer = Buffer::new(font_system, metrics);
        buffer.set_size(max_width, None);

        let attrs = Attrs::new()
            .family(Family::Name(props.family))
            .weight(props.weight);

        buffer.set_text(text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(font_system, false);
        buffer
    }

    /// Measure the bounding box `(width, height)` of all shaped layout runs in a buffer.
    pub fn measure_buffer(buffer: &Buffer) -> (f32, f32) {
        let mut max_width = 0.0f32;
        let mut total_height = 0.0f32;

        for run in buffer.layout_runs() {
            max_width = max_width.max(run.line_w);
            total_height += run.line_height;
        }

        (max_width, total_height)
    }

    /// Truncate text with trailing ellipsis (`...`) if its measured width exceeds `max_width`.
    pub fn truncate_ellipsis(
        font_system: &mut FontSystem,
        text: &str,
        props: &TextProperties<'_>,
        max_width: f32,
    ) -> String {
        let full_buffer = Self::create_buffer(font_system, text, props, None);
        let (w, _) = Self::measure_buffer(&full_buffer);
        if w <= max_width {
            return text.to_string();
        }

        let ellipsis = "...";
        let chars: Vec<char> = text.chars().collect();
        let mut low = 0;
        let mut high = chars.len();
        let mut best = String::new();

        while low <= high {
            let mid = (low + high) / 2;
            let candidate: String = chars[..mid].iter().collect::<String>() + ellipsis;
            let buf = Self::create_buffer(font_system, &candidate, props, None);
            let (cw, _) = Self::measure_buffer(&buf);

            if cw <= max_width {
                best = candidate;
                low = mid + 1;
            } else {
                if mid == 0 {
                    break;
                }
                high = mid - 1;
            }
        }

        if best.is_empty() {
            ellipsis.to_string()
        } else {
            best
        }
    }
}
