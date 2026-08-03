//! Platform capability detection and material fallback contracts.
//!
//! The host (Windows primitives, Task 5) measures the display session and
//! publishes a [`UiCapabilities`] snapshot. Renderers consume the resolved
//! material via [`resolve_material`]; this module contains no platform
//! imports and is fully deterministic given the capabilities.

use crate::ui::model::MaterialMode;
use crate::ui::surface::WorkArea;

/// Capabilities of the current display session, as detected by the host.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiCapabilities {
    /// Desktop composition is enabled (for example DWM on Windows).
    pub compositor: bool,
    /// The compositor can blur the backdrop behind translucent surfaces.
    pub blur: bool,
    /// System high-contrast mode is active; forces opaque rendering.
    pub high_contrast: bool,
    /// The system prefers reduced motion (animations should be limited).
    pub reduced_motion: bool,
    /// DPI scale factor in effect (for example 1.0, 1.25, 1.5).
    pub dpi_scale: f32,
    /// Monitor work area the surfaces are placed against.
    pub work_area: WorkArea,
}

/// The material treatment a renderer must actually apply.
///
/// Degrades gracefully as platform support decreases: [`Self::Blurred`] is
/// translucent plus backdrop blur, [`Self::Translucent`] is translucent
/// without blur, and [`Self::Opaque`] is the always-available fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedMaterial {
    /// Translucent surface with blurred backdrop (best available).
    Blurred,
    /// Translucent surface without backdrop blur.
    Translucent,
    /// Fully opaque surface (no translucency, no blur).
    Opaque,
}

impl ResolvedMaterial {
    pub const fn is_opaque(self) -> bool {
        matches!(self, Self::Opaque)
    }

    pub const fn is_translucent(self) -> bool {
        !self.is_opaque()
    }
}

/// Resolve the requested material mode to what the platform can honor.
///
/// Fallback order:
/// 1. High contrast forces [`ResolvedMaterial::Opaque`] regardless of every
///    other input (readability trumps appearance).
/// 2. [`MaterialMode::Opaque`] resolves to [`ResolvedMaterial::Opaque`].
/// 3. Otherwise the best available treatment is chosen from the
///    capabilities:
///    - blurred/translucent when the compositor supports blur,
///    - translucent when a compositor is present but cannot blur,
///    - opaque when no compositor is available.
///
/// An explicit [`MaterialMode::Translucent`] request resolves to translucent
/// (never blurred), still degrading to opaque without a compositor.
pub fn resolve_material(requested: MaterialMode, caps: &UiCapabilities) -> ResolvedMaterial {
    if caps.high_contrast || requested == MaterialMode::Opaque {
        return ResolvedMaterial::Opaque;
    }
    match requested {
        MaterialMode::Auto => {
            if caps.compositor && caps.blur {
                ResolvedMaterial::Blurred
            } else if caps.compositor {
                ResolvedMaterial::Translucent
            } else {
                ResolvedMaterial::Opaque
            }
        }
        MaterialMode::Translucent => {
            if caps.compositor {
                ResolvedMaterial::Translucent
            } else {
                ResolvedMaterial::Opaque
            }
        }
        MaterialMode::Opaque => ResolvedMaterial::Opaque,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A capability snapshot with everything disabled except what is passed.
    fn caps(compositor: bool, blur: bool, high_contrast: bool) -> UiCapabilities {
        UiCapabilities {
            compositor,
            blur,
            high_contrast,
            reduced_motion: false,
            dpi_scale: 1.0,
            work_area: WorkArea::new(0, 0, 2560, 1400),
        }
    }

    #[test]
    fn auto_resolves_blurred_when_compositor_supports_blur() {
        let c = caps(true, true, false);
        assert_eq!(
            resolve_material(MaterialMode::Auto, &c),
            ResolvedMaterial::Blurred
        );
    }

    #[test]
    fn auto_resolves_translucent_when_compositor_cannot_blur() {
        let c = caps(true, false, false);
        assert_eq!(
            resolve_material(MaterialMode::Auto, &c),
            ResolvedMaterial::Translucent
        );
    }

    #[test]
    fn auto_resolves_opaque_without_compositor() {
        let c = caps(false, false, false);
        assert_eq!(
            resolve_material(MaterialMode::Auto, &c),
            ResolvedMaterial::Opaque
        );
    }

    #[test]
    fn explicit_opaque_forces_opaque_even_with_best_capabilities() {
        let c = caps(true, true, false);
        assert_eq!(
            resolve_material(MaterialMode::Opaque, &c),
            ResolvedMaterial::Opaque
        );
    }

    #[test]
    fn explicit_translucent_never_blurs_and_degrades_without_compositor() {
        let blurred_caps = caps(true, true, false);
        assert_eq!(
            resolve_material(MaterialMode::Translucent, &blurred_caps),
            ResolvedMaterial::Translucent
        );

        let plain_caps = caps(true, false, false);
        assert_eq!(
            resolve_material(MaterialMode::Translucent, &plain_caps),
            ResolvedMaterial::Translucent
        );

        let no_compositor = caps(false, false, false);
        assert_eq!(
            resolve_material(MaterialMode::Translucent, &no_compositor),
            ResolvedMaterial::Opaque
        );
    }

    #[test]
    fn high_contrast_forces_opaque_regardless_of_request_and_capabilities() {
        for (compositor, blur) in [(true, true), (true, false), (false, false)] {
            for requested in [
                MaterialMode::Auto,
                MaterialMode::Translucent,
                MaterialMode::Opaque,
            ] {
                let c = caps(compositor, blur, true);
                assert_eq!(
                    resolve_material(requested, &c),
                    ResolvedMaterial::Opaque,
                    "high contrast must force opaque for {requested:?} with \
                     compositor={compositor} blur={blur}"
                );
            }
        }
    }

    #[test]
    fn opaque_is_the_is_opaque_flag_single_truth() {
        assert!(ResolvedMaterial::Opaque.is_opaque());
        assert!(!ResolvedMaterial::Opaque.is_translucent());
        assert!(ResolvedMaterial::Blurred.is_translucent());
        assert!(ResolvedMaterial::Translucent.is_translucent());
    }
}
