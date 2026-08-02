//! Platform-neutral adaptive design tokens.
//!
//! Every renderer derives its palette and layout from a single
//! [`ThemeTokens`] value produced by [`tokens_for`]. The module is
//! deliberately pure: no platform imports, no rendering, no system queries.
//! The host decides whether the system theme is dark (passed in as the
//! `system_is_dark` callback) and whether high contrast is forced; everything
//! downstream is deterministic and unit-testable.
//!
//! Material and motion tokens express *intent* (alpha, blur radius, durations
//! in ms, easing policy) — never animation or composition implementation.
//! Capability resolution (Task 4) decides which intent a platform can honor.

use crate::ui::model::{AccentMode, ThemeMode};

/// RGBA color with 8-bit channels, as passed to renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl Rgba {
    pub const WHITE: Self = Self::from_rgb(0xFF, 0xFF, 0xFF);
    pub const BLACK: Self = Self::from_rgb(0x00, 0x00, 0x00);

    /// Fully opaque color from 8-bit channels.
    pub const fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha: 255,
        }
    }

    /// Copy with a new alpha channel.
    pub const fn with_alpha(mut self, alpha: u8) -> Self {
        self.alpha = alpha;
        self
    }

    pub const fn is_opaque(self) -> bool {
        self.alpha == 255
    }

    /// WCAG relative luminance in linear light (sRGB transfer curve).
    pub fn relative_luminance(self) -> f64 {
        fn channel(c: u8) -> f64 {
            let c = f64::from(c) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * channel(self.red) + 0.7152 * channel(self.green) + 0.0722 * channel(self.blue)
    }

    /// WCAG contrast ratio against `other`, in the range 1.0..=21.0.
    pub fn contrast_ratio(self, other: Self) -> f64 {
        let (l1, l2) = (self.relative_luminance(), other.relative_luminance());
        let (hi, lo) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// Linear blend toward `other` by `t` in `[0, 1]`; alpha blends too.
    pub fn mix(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let lerp = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
        Self {
            red: lerp(self.red, other.red),
            green: lerp(self.green, other.green),
            blue: lerp(self.blue, other.blue),
            alpha: lerp(self.alpha, other.alpha),
        }
    }
}

/// Windows default accent blue (Windows 11), used for [`AccentMode::System`]
/// until platform detection (registry query) lands in the Windows helpers.
const ACCENT_SYSTEM_BLUE: Rgba = Rgba::from_rgb(0x00, 0x67, 0xC0);
/// Windows green accent.
const ACCENT_GREEN: Rgba = Rgba::from_rgb(0x10, 0x7C, 0x10);
/// Windows purple accent.
const ACCENT_PURPLE: Rgba = Rgba::from_rgb(0x74, 0x4D, 0xA9);
/// Windows orange accent.
const ACCENT_ORANGE: Rgba = Rgba::from_rgb(0xCA, 0x50, 0x10);

/// Volume legend colors, preserved from the VolumePro palette
/// (`crate::core::volume_color_rgb`): grey when muted/zero, then
/// green / blue / orange bands by percent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolumeThresholdColors {
    pub muted: Rgba,
    pub low: Rgba,
    pub medium: Rgba,
    pub high: Rgba,
}

impl Default for VolumeThresholdColors {
    fn default() -> Self {
        Self {
            muted: Rgba::from_rgb(0x88, 0x88, 0x88),
            low: Rgba::from_rgb(0x27, 0xAE, 0x60),
            medium: Rgba::from_rgb(0x00, 0x78, 0xD4),
            high: Rgba::from_rgb(0xE0, 0x5C, 0x00),
        }
    }
}

/// Focus-visible indicator intent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FocusTokens {
    /// Ring color (the resolved accent).
    pub ring: Rgba,
    /// Ring stroke width in px.
    pub ring_width_px: f32,
    /// Gap between the control edge and the ring in px.
    pub ring_gap_px: f32,
}

/// Error state colors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ErrorTokens {
    pub text: Rgba,
    pub border: Rgba,
    pub surface: Rgba,
}

/// Spacing scale in px (Fluent-inspired).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpacingTokens {
    pub xs_px: f32,
    pub sm_px: f32,
    pub md_px: f32,
    pub lg_px: f32,
    pub xl_px: f32,
}

impl Default for SpacingTokens {
    fn default() -> Self {
        Self {
            xs_px: 4.0,
            sm_px: 8.0,
            md_px: 12.0,
            lg_px: 16.0,
            xl_px: 24.0,
        }
    }
}

/// Corner radius scale in px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadiusTokens {
    pub small_px: f32,
    pub medium_px: f32,
    pub large_px: f32,
}

impl Default for RadiusTokens {
    fn default() -> Self {
        Self {
            small_px: 4.0,
            medium_px: 6.0,
            large_px: 10.0,
        }
    }
}

/// A single typography role: size and weight intent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextRole {
    pub size_px: f32,
    pub weight: u16,
}

/// Typography roles shared by every surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypographyTokens {
    pub display: TextRole,
    pub title: TextRole,
    pub body: TextRole,
    pub caption: TextRole,
}

impl Default for TypographyTokens {
    fn default() -> Self {
        Self {
            display: TextRole {
                size_px: 28.0,
                weight: 600,
            },
            title: TextRole {
                size_px: 18.0,
                weight: 600,
            },
            body: TextRole {
                size_px: 13.0,
                weight: 400,
            },
            caption: TextRole {
                size_px: 11.0,
                weight: 400,
            },
        }
    }
}

/// Elevation levels expressed as shadow opacity intent (0.0 = none).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElevationTokens {
    pub flat: f32,
    pub raised: f32,
    pub overlay: f32,
}

impl Default for ElevationTokens {
    fn default() -> Self {
        Self {
            flat: 0.0,
            raised: 0.12,
            overlay: 0.24,
        }
    }
}

/// Translucent material intent. High contrast always resolves to opaque
/// (`surface_alpha == 1.0`, blur disabled).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialIntent {
    /// Alpha for translucent surface fills (1.0 = fully opaque).
    pub surface_alpha: f32,
    /// Blur the backdrop behind translucent surfaces, when the platform can.
    pub blur_enabled: bool,
    /// Backdrop blur radius intent in px.
    pub blur_radius_px: f32,
}

/// Minimum interactive hit-target sizes in px (Windows guidance: 20 px
/// minimum for mouse, 32 px default, 40 px touch-friendly).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitTargetTokens {
    pub minimum_px: f32,
    pub default_px: f32,
    pub touch_px: f32,
}

impl Default for HitTargetTokens {
    fn default() -> Self {
        Self {
            minimum_px: 20.0,
            default_px: 32.0,
            touch_px: 40.0,
        }
    }
}

/// Easing policy intent. Named qualitatively — renderers map these onto their
/// own curve primitives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EasingPolicy {
    /// Constant velocity.
    Linear,
    /// Fast start, gentle finish — default for entry/exit animations.
    #[default]
    EaseOut,
    /// Gentle start, fast finish — for interruptible/dismissal animations.
    EaseInOut,
}

/// Motion intent: durations in ms plus easing policy. This expresses *what*
/// to animate and at what cadence, never how to animate it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimationTokens {
    pub fast_ms: u32,
    pub normal_ms: u32,
    pub slow_ms: u32,
    pub easing: EasingPolicy,
}

impl Default for AnimationTokens {
    fn default() -> Self {
        Self {
            fast_ms: 120,
            normal_ms: 200,
            slow_ms: 400,
            easing: EasingPolicy::EaseOut,
        }
    }
}

/// Resolve an accent preference to a concrete color.
///
/// `System` uses the Windows default accent blue; querying the user's actual
/// system accent is a Windows-only host concern that lands later and may
/// override this default.
pub fn accent_color(accent: AccentMode) -> Rgba {
    match accent {
        AccentMode::System | AccentMode::Blue => ACCENT_SYSTEM_BLUE,
        AccentMode::Green => ACCENT_GREEN,
        AccentMode::Purple => ACCENT_PURPLE,
        AccentMode::Orange => ACCENT_ORANGE,
    }
}

/// Compute design tokens for the requested appearance.
///
/// `system_is_dark` resolves [`ThemeMode::System`] on the platform's behalf
/// and may return `None` when detection is unavailable — the unknown case
/// falls back to the light palette. It is only called for `System`.
pub fn tokens_for(
    theme: ThemeMode,
    high_contrast: bool,
    accent: AccentMode,
    system_is_dark: impl Fn() -> Option<bool>,
) -> ThemeTokens {
    let dark = match theme {
        ThemeMode::Dark => true,
        ThemeMode::Light => false,
        ThemeMode::System => system_is_dark().unwrap_or(false),
    };
    ThemeTokens::build(dark, high_contrast, accent)
}

/// A resolved surface/text palette (internal selection table).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Palette {
    background: Rgba,
    surface: Rgba,
    surface_elevated: Rgba,
    border: Rgba,
    text_primary: Rgba,
    text_secondary: Rgba,
    text_disabled: Rgba,
    error_text: Rgba,
    error_border: Rgba,
    error_surface: Rgba,
}

impl Palette {
    /// Fluent-inspired light palette. Background matches the legacy overlay
    /// white; text is near-black for maximum readability.
    const LIGHT: Self = Self {
        background: Rgba::from_rgb(0xFF, 0xFF, 0xFF),
        surface: Rgba::from_rgb(0xF3, 0xF3, 0xF3),
        surface_elevated: Rgba::from_rgb(0xFF, 0xFF, 0xFF),
        border: Rgba::from_rgb(0xE0, 0xE0, 0xE0),
        text_primary: Rgba::from_rgb(0x1B, 0x1B, 0x1B),
        text_secondary: Rgba::from_rgb(0x5C, 0x5C, 0x5C),
        text_disabled: Rgba::from_rgb(0x8A, 0x8A, 0x8A),
        error_text: Rgba::from_rgb(0xC4, 0x2B, 0x1C),
        error_border: Rgba::from_rgb(0xC4, 0x2B, 0x1C),
        error_surface: Rgba::from_rgb(0xFD, 0xE7, 0xE9),
    };

    /// Dark palette. Background and primary text preserve the legacy overlay
    /// colors (`#141418` / `#DDDDDD`).
    const DARK: Self = Self {
        background: Rgba::from_rgb(0x14, 0x14, 0x18),
        surface: Rgba::from_rgb(0x1E, 0x1E, 0x24),
        surface_elevated: Rgba::from_rgb(0x28, 0x28, 0x30),
        border: Rgba::from_rgb(0x38, 0x38, 0x44),
        text_primary: Rgba::from_rgb(0xDD, 0xDD, 0xDD),
        text_secondary: Rgba::from_rgb(0xA8, 0xA8, 0xA8),
        text_disabled: Rgba::from_rgb(0x6E, 0x6E, 0x6E),
        error_text: Rgba::from_rgb(0xFF, 0x99, 0xA4),
        error_border: Rgba::from_rgb(0xFF, 0x99, 0xA4),
        error_surface: Rgba::from_rgb(0x4A, 0x1E, 0x24),
    };

    /// High contrast: opaque black surfaces, pure white text (maximal
    /// contrast), strong borders. Secondary/disabled text collapse to the
    /// primary color so no information is carried by tint alone.
    const HC_DARK: Self = Self {
        background: Rgba::BLACK,
        surface: Rgba::BLACK,
        surface_elevated: Rgba::BLACK,
        border: Rgba::WHITE,
        text_primary: Rgba::WHITE,
        text_secondary: Rgba::WHITE,
        text_disabled: Rgba::WHITE,
        error_text: Rgba::WHITE,
        error_border: Rgba::WHITE,
        error_surface: Rgba::BLACK,
    };

    /// High contrast light: inverted.
    const HC_LIGHT: Self = Self {
        background: Rgba::WHITE,
        surface: Rgba::WHITE,
        surface_elevated: Rgba::WHITE,
        border: Rgba::BLACK,
        text_primary: Rgba::BLACK,
        text_secondary: Rgba::BLACK,
        text_disabled: Rgba::BLACK,
        error_text: Rgba::BLACK,
        error_border: Rgba::BLACK,
        error_surface: Rgba::WHITE,
    };
}

/// All resolved design tokens for one surface appearance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeTokens {
    pub is_dark: bool,
    pub high_contrast: bool,

    /// Surfaces.
    pub background: Rgba,
    pub surface: Rgba,
    pub surface_elevated: Rgba,
    pub border: Rgba,

    /// Text.
    pub text_primary: Rgba,
    pub text_secondary: Rgba,
    pub text_disabled: Rgba,
    pub text_on_accent: Rgba,

    /// Accent (resolved) and its hover variant.
    pub accent: Rgba,
    pub accent_hover: Rgba,

    /// Volume legend colors (VolumePro palette, mode-independent).
    pub volume_threshold: VolumeThresholdColors,

    /// Focus-visible indicator.
    pub focus: FocusTokens,

    /// Error states.
    pub error: ErrorTokens,

    /// Layout.
    pub spacing: SpacingTokens,
    pub radii: RadiusTokens,
    pub typography: TypographyTokens,
    pub elevation: ElevationTokens,
    pub hit_target: HitTargetTokens,

    /// Translucent material intent (opaque when high contrast).
    pub material: MaterialIntent,

    /// Motion intent (durations + easing policy).
    pub animation: AnimationTokens,
}

impl ThemeTokens {
    fn build(dark: bool, high_contrast: bool, accent: AccentMode) -> Self {
        let accent = accent_color(accent);
        let palette = if high_contrast {
            if dark {
                Palette::HC_DARK
            } else {
                Palette::HC_LIGHT
            }
        } else if dark {
            Palette::DARK
        } else {
            Palette::LIGHT
        };
        // Hover reads toward the background: lighter in dark mode, darker in
        // light mode; high contrast keeps the accent unmodified.
        let accent_hover = if high_contrast {
            accent
        } else if dark {
            accent.mix(Rgba::WHITE, 0.12)
        } else {
            accent.mix(Rgba::BLACK, 0.12)
        };
        let text_on_accent = if accent.relative_luminance() < 0.45 {
            Rgba::WHITE
        } else {
            Rgba::BLACK
        };

        Self {
            is_dark: dark,
            high_contrast,
            background: palette.background,
            surface: palette.surface,
            surface_elevated: palette.surface_elevated,
            border: palette.border,
            text_primary: palette.text_primary,
            text_secondary: palette.text_secondary,
            text_disabled: palette.text_disabled,
            text_on_accent,
            accent,
            accent_hover,
            volume_threshold: VolumeThresholdColors::default(),
            focus: FocusTokens {
                ring: accent,
                ring_width_px: if high_contrast { 2.0 } else { 1.5 },
                ring_gap_px: 1.0,
            },
            error: ErrorTokens {
                text: palette.error_text,
                border: palette.error_border,
                surface: palette.error_surface,
            },
            spacing: SpacingTokens::default(),
            radii: RadiusTokens::default(),
            typography: TypographyTokens::default(),
            elevation: ElevationTokens::default(),
            hit_target: HitTargetTokens::default(),
            material: MaterialIntent {
                surface_alpha: if high_contrast { 1.0 } else { 0.90 },
                blur_enabled: !high_contrast,
                blur_radius_px: 16.0,
            },
            animation: AnimationTokens::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(theme: ThemeMode, high_contrast: bool, system_dark: Option<bool>) -> ThemeTokens {
        tokens_for(theme, high_contrast, AccentMode::System, || system_dark)
    }

    fn light_tokens() -> ThemeTokens {
        tokens(ThemeMode::Light, false, None)
    }

    fn dark_tokens() -> ThemeTokens {
        tokens(ThemeMode::Dark, false, None)
    }

    fn rgb_distance(a: Rgba, b: Rgba) -> f64 {
        let dr = f64::from(a.red) - f64::from(b.red);
        let dg = f64::from(a.green) - f64::from(b.green);
        let db = f64::from(a.blue) - f64::from(b.blue);
        (dr * dr + dg * dg + db * db).sqrt()
    }

    // --- system theme resolution -------------------------------------------------

    #[test]
    fn system_theme_resolves_through_callback() {
        let dark = tokens(ThemeMode::System, false, Some(true));
        assert!(dark.is_dark);
        assert_eq!(dark.background, Palette::DARK.background);

        let light = tokens(ThemeMode::System, false, Some(false));
        assert!(!light.is_dark);
        assert_eq!(light.background, Palette::LIGHT.background);
    }

    #[test]
    fn unknown_system_darkness_defaults_to_light() {
        let unknown = tokens(ThemeMode::System, false, None);
        assert!(!unknown.is_dark);
        assert_eq!(unknown.background, Palette::LIGHT.background);
    }

    #[test]
    fn explicit_theme_overrides_system_detection() {
        let dark = tokens(ThemeMode::Dark, false, Some(false));
        assert!(dark.is_dark);
        let light = tokens(ThemeMode::Light, false, Some(true));
        assert!(!light.is_dark);
    }

    // --- palette invariants -------------------------------------------------------

    #[test]
    fn light_palette_text_meets_contrast_guidelines() {
        assert_text_contrast(&light_tokens());
    }

    #[test]
    fn dark_palette_text_meets_contrast_guidelines() {
        assert_text_contrast(&dark_tokens());
    }

    /// WCAG AA: 7:1 for primary text, 4.5:1 for secondary, 3:1 for disabled
    /// and for text on the accent fill.
    fn assert_text_contrast(tokens: &ThemeTokens) {
        let bg = tokens.background;
        let pair = |a: Rgba, b: Rgba| a.contrast_ratio(b);
        assert!(
            pair(tokens.text_primary, bg) >= 7.0,
            "primary {:.2}:1",
            pair(tokens.text_primary, bg)
        );
        assert!(
            pair(tokens.text_secondary, bg) >= 4.5,
            "secondary {:.2}:1",
            pair(tokens.text_secondary, bg)
        );
        assert!(
            pair(tokens.text_disabled, bg) >= 3.0,
            "disabled {:.2}:1",
            pair(tokens.text_disabled, bg)
        );
        assert!(
            pair(tokens.text_on_accent, tokens.accent) >= 3.0,
            "on-accent {:.2}:1",
            pair(tokens.text_on_accent, tokens.accent)
        );
    }

    // --- high contrast -------------------------------------------------------------

    #[test]
    fn high_contrast_forces_opaque_surfaces_and_disables_blur() {
        for system_dark in [Some(true), Some(false)] {
            let t = tokens(ThemeMode::System, true, system_dark);
            assert!(t.background.is_opaque(), "background {:?}", t.background);
            assert!(t.surface.is_opaque(), "surface {:?}", t.surface);
            assert!(
                t.surface_elevated.is_opaque(),
                "elevated {:?}",
                t.surface_elevated
            );
            assert_eq!(t.material.surface_alpha, 1.0);
            assert!(!t.material.blur_enabled);
            assert_eq!(t.focus.ring_width_px, 2.0);
        }
    }

    #[test]
    fn high_contrast_collapses_text_to_maximal_contrast() {
        let dark = tokens(ThemeMode::Dark, true, None);
        assert_eq!(dark.background, Rgba::BLACK);
        assert_eq!(dark.text_primary, Rgba::WHITE);
        assert_eq!(dark.text_secondary, dark.text_primary);
        assert_eq!(dark.text_disabled, dark.text_primary);
        assert!(dark.text_primary.contrast_ratio(dark.background) >= 21.0 - 1e-6);

        let light = tokens(ThemeMode::Light, true, None);
        assert_eq!(light.background, Rgba::WHITE);
        assert_eq!(light.text_primary, Rgba::BLACK);
        assert_eq!(light.text_secondary, light.text_primary);
        assert!(light.text_primary.contrast_ratio(light.background) >= 21.0 - 1e-6);
    }

    // --- accent mapping -------------------------------------------------------------

    #[test]
    fn accent_modes_map_to_expected_windows_palette() {
        assert_eq!(accent_color(AccentMode::System), ACCENT_SYSTEM_BLUE);
        assert_eq!(accent_color(AccentMode::Blue), ACCENT_SYSTEM_BLUE);
        assert_eq!(accent_color(AccentMode::Green), ACCENT_GREEN);
        assert_eq!(accent_color(AccentMode::Purple), ACCENT_PURPLE);
        assert_eq!(accent_color(AccentMode::Orange), ACCENT_ORANGE);
    }

    #[test]
    fn every_accent_is_distinguishable_from_every_threshold_color() {
        let thresholds = VolumeThresholdColors::default();
        let all = [
            thresholds.muted,
            thresholds.low,
            thresholds.medium,
            thresholds.high,
        ];
        for accent in [
            AccentMode::System,
            AccentMode::Blue,
            AccentMode::Green,
            AccentMode::Purple,
            AccentMode::Orange,
        ] {
            let color = accent_color(accent);
            for threshold in all {
                let d = rgb_distance(color, threshold);
                assert!(
                    d >= 24.0,
                    "{accent:?} ({color:?}) too close to threshold {threshold:?}: distance {d:.1}"
                );
            }
        }
    }

    #[test]
    fn volume_thresholds_preserve_volumepro_palette() {
        let t = VolumeThresholdColors::default();
        assert_eq!(t.muted, Rgba::from_rgb(0x88, 0x88, 0x88));
        assert_eq!(t.low, Rgba::from_rgb(0x27, 0xAE, 0x60));
        assert_eq!(t.medium, Rgba::from_rgb(0x00, 0x78, 0xD4));
        assert_eq!(t.high, Rgba::from_rgb(0xE0, 0x5C, 0x00));
    }

    #[test]
    fn resolved_tokens_carry_the_requested_accent() {
        for accent in [
            AccentMode::System,
            AccentMode::Blue,
            AccentMode::Green,
            AccentMode::Purple,
            AccentMode::Orange,
        ] {
            let t = tokens_for(ThemeMode::Dark, false, accent, || None);
            assert_eq!(t.accent, accent_color(accent));
            assert_eq!(t.focus.ring, accent_color(accent));
        }
    }

    // --- hit targets ----------------------------------------------------------------

    #[test]
    fn hit_targets_meet_minimum_guidance() {
        let t = dark_tokens();
        assert!(
            t.hit_target.minimum_px >= 20.0,
            "minimum {}",
            t.hit_target.minimum_px
        );
        assert!(t.hit_target.default_px >= t.hit_target.minimum_px);
        assert!(t.hit_target.touch_px >= t.hit_target.default_px);
    }

    // --- layout/animation sanity -----------------------------------------------------

    #[test]
    fn layout_and_animation_tokens_are_ordered_and_positive() {
        let t = dark_tokens();
        assert!(t.spacing.xs_px < t.spacing.sm_px && t.spacing.sm_px < t.spacing.md_px);
        assert!(t.radii.small_px <= t.radii.medium_px && t.radii.medium_px <= t.radii.large_px);
        assert!(
            t.animation.fast_ms < t.animation.normal_ms
                && t.animation.normal_ms < t.animation.slow_ms
        );
        assert!(t.typography.caption.size_px < t.typography.body.size_px);
        assert!(t.typography.title.size_px < t.typography.display.size_px);
    }
}
