//! Native Unicode text layout, Vietnamese shaping, and glyph caching.

pub mod fonts;
pub mod layout;
pub mod system;

#[cfg(test)]
pub mod tests;

pub use fonts::{default_font_family, vietnamese_fallback_chain};
pub use layout::{TextLayout, TextProperties};
pub use system::TextSystem;

// Re-export common cosmic_text and glyphon types needed across liquid_glass
pub use glyphon::cosmic_text::{Color, Weight};
pub use glyphon::{TextArea, TextBounds};
