//! Material role tokens, appearance resolution, and WGSL SDF pipelines.

pub mod pipeline;
pub mod tokens;

#[cfg(test)]
pub mod tests;

pub use pipeline::{GlobalUniforms, MaterialPipeline, MaterialUniforms, QuadVertex};
pub use tokens::{Appearance, ColorScheme, MaterialTokens};

/// Semantic visual role of a surface or UI component mapping to optics tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MaterialRole {
    /// Window card or main surface background.
    #[default]
    Panel,
    /// Floating temporary head-up display (e.g. Volume Overlay).
    Hud,
    /// Navigation sidebar or panel separator.
    Sidebar,
    /// Interactive button, pill toggle, or slider thumb.
    Control,
    /// Direct accent color fill (e.g. volume gauge fill).
    Accent,
    /// Floating hover tooltip or keyboard shortcut badge.
    Tooltip,
    /// System tray context menu or popup dropdown.
    Menu,
}
