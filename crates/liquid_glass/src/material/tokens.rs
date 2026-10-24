//! Material tokens, appearance configuration, and accessibility resolution.

use glam::Vec4;

use crate::material::MaterialRole;

/// Operating system or application color scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorScheme {
    #[default]
    Dark,
    Light,
}

/// Display modifiers and system accessibility preferences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Appearance {
    pub color_scheme: ColorScheme,
    pub reduce_transparency: bool,
    pub increase_contrast: bool,
    pub reduce_motion: bool,
}

/// Concrete optical parameters passed to WGSL material shaders.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialTokens {
    pub blur_radius: f32,
    pub tint_color: Vec4,
    pub saturation: f32,
    pub specular_rim: f32,
    pub edge_width: f32,
    pub opacity: f32,
    pub is_opaque: bool,
}

impl MaterialTokens {
    /// Resolve optical tokens from semantic role and active appearance settings.
    pub fn resolve(role: MaterialRole, appearance: &Appearance) -> Self {
        // Accessibility Priority: High contrast or reduced transparency forces opaque fallback
        if appearance.increase_contrast || appearance.reduce_transparency {
            return Self::opaque_fallback(role, appearance);
        }

        let is_dark = appearance.color_scheme == ColorScheme::Dark;

        match role {
            MaterialRole::Hud => Self {
                blur_radius: 28.0,
                tint_color: if is_dark {
                    Vec4::new(0.08, 0.08, 0.10, 0.15)
                } else {
                    Vec4::new(0.95, 0.95, 0.98, 0.15)
                },
                saturation: 1.8,
                specular_rim: 1.0,
                edge_width: 1.0,
                opacity: 0.85,
                is_opaque: false,
            },
            MaterialRole::Panel => Self {
                blur_radius: 20.0,
                tint_color: if is_dark {
                    Vec4::new(0.05, 0.05, 0.07, 0.30)
                } else {
                    Vec4::new(0.92, 0.92, 0.95, 0.30)
                },
                saturation: 1.5,
                specular_rim: 0.75,
                edge_width: 0.75,
                opacity: 0.90,
                is_opaque: false,
            },
            MaterialRole::Sidebar => Self {
                blur_radius: 18.0,
                tint_color: if is_dark {
                    Vec4::new(0.03, 0.03, 0.05, 0.45)
                } else {
                    Vec4::new(0.88, 0.88, 0.92, 0.45)
                },
                saturation: 1.2,
                specular_rim: 0.5,
                edge_width: 0.5,
                opacity: 0.95,
                is_opaque: false,
            },
            MaterialRole::Control => Self {
                blur_radius: 10.0,
                tint_color: if is_dark {
                    Vec4::new(0.15, 0.15, 0.18, 0.70)
                } else {
                    Vec4::new(0.80, 0.80, 0.85, 0.70)
                },
                saturation: 1.0,
                specular_rim: 1.5,
                edge_width: 1.0,
                opacity: 0.95,
                is_opaque: false,
            },
            MaterialRole::Accent => Self {
                blur_radius: 0.0,
                tint_color: Vec4::new(0.0, 0.47, 1.0, 1.0),
                saturation: 1.0,
                specular_rim: 0.0,
                edge_width: 0.0,
                opacity: 1.0,
                is_opaque: true,
            },
            MaterialRole::Tooltip => Self {
                blur_radius: 8.0,
                tint_color: if is_dark {
                    Vec4::new(0.02, 0.02, 0.04, 0.20)
                } else {
                    Vec4::new(0.96, 0.96, 0.98, 0.20)
                },
                saturation: 1.6,
                specular_rim: 0.5,
                edge_width: 0.5,
                opacity: 0.88,
                is_opaque: false,
            },
            MaterialRole::Menu => Self {
                blur_radius: 16.0,
                tint_color: if is_dark {
                    Vec4::new(0.06, 0.06, 0.08, 0.25)
                } else {
                    Vec4::new(0.94, 0.94, 0.96, 0.25)
                },
                saturation: 1.7,
                specular_rim: 1.0,
                edge_width: 0.75,
                opacity: 0.92,
                is_opaque: false,
            },
        }
    }

    fn opaque_fallback(role: MaterialRole, appearance: &Appearance) -> Self {
        let is_dark = appearance.color_scheme == ColorScheme::Dark;
        let edge_width = if appearance.increase_contrast {
            2.0
        } else {
            1.0
        };

        let tint_color = match role {
            MaterialRole::Accent => Vec4::new(0.0, 0.47, 1.0, 1.0),
            MaterialRole::Sidebar => {
                if is_dark {
                    Vec4::new(0.08, 0.08, 0.09, 1.0)
                } else {
                    Vec4::new(0.90, 0.90, 0.92, 1.0)
                }
            }
            MaterialRole::Control => {
                if is_dark {
                    Vec4::new(0.20, 0.20, 0.22, 1.0)
                } else {
                    Vec4::new(0.82, 0.82, 0.84, 1.0)
                }
            }
            _ => {
                if is_dark {
                    Vec4::new(0.12, 0.12, 0.14, 1.0)
                } else {
                    Vec4::new(0.96, 0.96, 0.98, 1.0)
                }
            }
        };

        Self {
            blur_radius: 0.0,
            tint_color,
            saturation: 1.0,
            specular_rim: 0.0,
            edge_width,
            opacity: 1.0,
            is_opaque: true,
        }
    }
}
