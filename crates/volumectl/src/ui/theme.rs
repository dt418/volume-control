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

/// Windows default accent blue, light theme (Windows 11), used for
/// [`AccentMode::System`] until platform detection (registry query) lands in
/// the Windows helpers.
const ACCENT_SYSTEM_BLUE: Rgba = Rgba::from_rgb(0x00, 0x67, 0xC0);
/// Windows default accent blue, dark theme (Signal Glass approved value).
const ACCENT_SYSTEM_BLUE_DARK: Rgba = Rgba::from_rgb(0x3A, 0xA8, 0xFF);
/// Approved Signal Glass hover for the light accent blue.
const ACCENT_SYSTEM_BLUE_HOVER: Rgba = Rgba::from_rgb(0x00, 0x5A, 0xAB);
/// Approved Signal Glass hover for the dark accent blue.
const ACCENT_SYSTEM_BLUE_HOVER_DARK: Rgba = Rgba::from_rgb(0x62, 0xB8, 0xFF);
/// Approved Signal Glass pressed for the light accent blue.
const ACCENT_SYSTEM_BLUE_PRESSED: Rgba = Rgba::from_rgb(0x00, 0x4A, 0x8D);
/// Approved Signal Glass pressed for the dark accent blue.
const ACCENT_SYSTEM_BLUE_PRESSED_DARK: Rgba = Rgba::from_rgb(0x19, 0x8F, 0xEA);
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

/// Focus-visible indicator intent: a visible two-layer ring.
///
/// The **outer layer** (`ring`) is the focus color (resolved accent); the
/// **inner layer** (`inner_ring`) is a contrast stroke drawn close to the
/// control edge so both rings stay visible on light, dark, and high-contrast
/// surfaces. The two layers have distinct widths and an air gap between them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FocusTokens {
    /// Outer ring color (the resolved accent).
    pub ring: Rgba,
    /// Outer ring stroke width in px.
    pub ring_width_px: f32,
    /// Gap between the control edge and the outer ring in px.
    pub ring_gap_px: f32,
    /// Inner (contrast) ring color, distinct from the outer ring.
    pub inner_ring: Rgba,
    /// Inner ring stroke width in px (distinct from `ring_width_px`).
    pub inner_ring_width_px: f32,
    /// Gap between the control edge and the inner ring in px.
    pub inner_ring_gap_px: f32,
}

/// Error state colors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ErrorTokens {
    pub text: Rgba,
    pub border: Rgba,
    pub surface: Rgba,
}

/// Spacing scale in px (Signal Glass 4px grid).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpacingTokens {
    pub xs_px: f32,
    pub sm_px: f32,
    pub md_px: f32,
    pub lg_px: f32,
    pub xl_px: f32,
    /// Window-level separation (approved scale).
    pub xxl_px: f32,
}

impl Default for SpacingTokens {
    fn default() -> Self {
        Self {
            xs_px: 4.0,
            sm_px: 8.0,
            md_px: 12.0,
            lg_px: 16.0,
            xl_px: 24.0,
            xxl_px: 32.0,
        }
    }
}

/// Corner radius scale in px.
///
/// `small_px`/`medium_px`/`large_px` are legacy aliases kept for API
/// compatibility; the approved roles are `control`, `card`, `surface`, `pill`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadiusTokens {
    pub small_px: f32,
    pub medium_px: f32,
    pub large_px: f32,
    /// Buttons, fields, combo boxes.
    pub control_px: f32,
    /// Nested groups.
    pub card_px: f32,
    /// Overlay, mixer, settings, Help surfaces.
    pub surface_px: f32,
    /// Status badges and compact indicators.
    pub pill_px: f32,
}

impl Default for RadiusTokens {
    fn default() -> Self {
        Self {
            small_px: 4.0,
            medium_px: 6.0,
            large_px: 10.0,
            control_px: 4.0,
            card_px: 8.0,
            surface_px: 12.0,
            pill_px: 999.0,
        }
    }
}

/// A single typography role: size, weight, and face intent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextRole {
    pub size_px: f32,
    pub weight: u16,
    /// Render with the platform monospace face (keycap role).
    pub monospace: bool,
}

/// Typography roles shared by every surface (approved Signal Glass scale).
///
/// `display` and `title` are legacy aliases kept for API compatibility; they
/// mirror `display_value` and `surface_title` exactly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypographyTokens {
    /// Legacy alias for [`Self::display_value`].
    pub display: TextRole,
    /// Live volume value.
    pub display_value: TextRole,
    /// Legacy alias for [`Self::surface_title`].
    pub title: TextRole,
    /// Surface title.
    pub surface_title: TextRole,
    /// Settings/Help group title.
    pub section_title: TextRole,
    /// Primary content.
    pub body: TextRole,
    /// Field labels, eyebrows.
    pub label: TextRole,
    /// Helper text.
    pub caption: TextRole,
    /// Hotkey combinations (monospace).
    pub keycap: TextRole,
}

impl Default for TypographyTokens {
    fn default() -> Self {
        let display_value = TextRole {
            size_px: 28.0,
            weight: 600,
            monospace: false,
        };
        let surface_title = TextRole {
            size_px: 17.0,
            weight: 600,
            monospace: false,
        };
        Self {
            display: display_value,
            display_value,
            title: surface_title,
            surface_title,
            section_title: TextRole {
                size_px: 15.0,
                weight: 600,
                monospace: false,
            },
            body: TextRole {
                size_px: 13.0,
                weight: 400,
                monospace: false,
            },
            label: TextRole {
                size_px: 12.0,
                weight: 600,
                monospace: false,
            },
            caption: TextRole {
                size_px: 11.0,
                weight: 400,
                monospace: false,
            },
            keycap: TextRole {
                size_px: 12.0,
                weight: 600,
                monospace: true,
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

/// Resolve an accent preference to its **light-theme** color.
///
/// Kept for API compatibility; `System`/`Blue` resolve theme-aware via
/// [`accent_color_for`]. Querying the user's actual system accent is a
/// Windows-only host concern that lands later and may override this default.
pub fn accent_color(accent: AccentMode) -> Rgba {
    accent_color_for(accent, false)
}

/// Resolve an accent preference to a concrete, theme-aware color.
///
/// `System` and `Blue` use the approved Signal Glass blues: `#0067C0` in the
/// light theme and `#3AA8FF` in the dark theme. The remaining accents are
/// theme-independent.
pub fn accent_color_for(accent: AccentMode, dark: bool) -> Rgba {
    match (accent, dark) {
        (AccentMode::System | AccentMode::Blue, true) => ACCENT_SYSTEM_BLUE_DARK,
        (AccentMode::System | AccentMode::Blue, false) => ACCENT_SYSTEM_BLUE,
        (AccentMode::Green, _) => ACCENT_GREEN,
        (AccentMode::Purple, _) => ACCENT_PURPLE,
        (AccentMode::Orange, _) => ACCENT_ORANGE,
    }
}

/// Approved Signal Glass hover color for the resolved accent.
///
/// The blue family uses the exact approved values; the remaining accents
/// derive hover toward the background (no approved values exist for them).
fn accent_hover_for(accent: Rgba, dark: bool) -> Rgba {
    match (accent, dark) {
        (ACCENT_SYSTEM_BLUE, false) => ACCENT_SYSTEM_BLUE_HOVER,
        (ACCENT_SYSTEM_BLUE_DARK, true) => ACCENT_SYSTEM_BLUE_HOVER_DARK,
        (other, true) => other.mix(Rgba::WHITE, 0.12),
        (other, false) => other.mix(Rgba::BLACK, 0.12),
    }
}

/// Approved Signal Glass pressed color for the resolved accent.
///
/// The blue family uses the exact approved values; the remaining accents
/// derive pressed toward the background (no approved values exist for them).
fn accent_pressed_for(accent: Rgba, dark: bool) -> Rgba {
    match (accent, dark) {
        (ACCENT_SYSTEM_BLUE, false) => ACCENT_SYSTEM_BLUE_PRESSED,
        (ACCENT_SYSTEM_BLUE_DARK, true) => ACCENT_SYSTEM_BLUE_PRESSED_DARK,
        (other, true) => other.mix(Rgba::WHITE, 0.25),
        (other, false) => other.mix(Rgba::BLACK, 0.25),
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
    surface_subtle: Rgba,
    border: Rgba,
    border_strong: Rgba,
    text_primary: Rgba,
    text_secondary: Rgba,
    text_disabled: Rgba,
    error_text: Rgba,
    error_border: Rgba,
    error_surface: Rgba,
}

impl Palette {
    /// Approved Signal Glass light palette.
    const LIGHT: Self = Self {
        background: Rgba::from_rgb(0xF7, 0xF9, 0xFC),
        surface: Rgba::from_rgb(0xFF, 0xFF, 0xFF),
        surface_elevated: Rgba::from_rgb(0xFF, 0xFF, 0xFF),
        surface_subtle: Rgba::from_rgb(0xF1, 0xF4, 0xF8),
        border: Rgba::from_rgb(0xD7, 0xDE, 0xE8),
        border_strong: Rgba::from_rgb(0xAE, 0xB9, 0xC8),
        text_primary: Rgba::from_rgb(0x17, 0x20, 0x2B),
        text_secondary: Rgba::from_rgb(0x52, 0x60, 0x71),
        text_disabled: Rgba::from_rgb(0x89, 0x95, 0xA3),
        error_text: Rgba::from_rgb(0xC4, 0x2B, 0x1C),
        error_border: Rgba::from_rgb(0xC4, 0x2B, 0x1C),
        error_surface: Rgba::from_rgb(0xFD, 0xE7, 0xE9),
    };

    /// Approved Signal Glass dark palette.
    const DARK: Self = Self {
        background: Rgba::from_rgb(0x10, 0x13, 0x1A),
        surface: Rgba::from_rgb(0x17, 0x1C, 0x24),
        surface_elevated: Rgba::from_rgb(0x20, 0x27, 0x35),
        surface_subtle: Rgba::from_rgb(0x1C, 0x22, 0x2D),
        border: Rgba::from_rgb(0x34, 0x40, 0x52),
        border_strong: Rgba::from_rgb(0x53, 0x62, 0x76),
        text_primary: Rgba::from_rgb(0xF5, 0xF7, 0xFA),
        text_secondary: Rgba::from_rgb(0xAA, 0xB4, 0xC3),
        text_disabled: Rgba::from_rgb(0x75, 0x81, 0x92),
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
        surface_subtle: Rgba::BLACK,
        border: Rgba::WHITE,
        border_strong: Rgba::WHITE,
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
        surface_subtle: Rgba::WHITE,
        border: Rgba::BLACK,
        border_strong: Rgba::BLACK,
        text_primary: Rgba::BLACK,
        text_secondary: Rgba::BLACK,
        text_disabled: Rgba::BLACK,
        error_text: Rgba::BLACK,
        error_border: Rgba::BLACK,
        error_surface: Rgba::WHITE,
    };

    fn select(dark: bool, high_contrast: bool) -> Self {
        if high_contrast {
            if dark {
                Self::HC_DARK
            } else {
                Self::HC_LIGHT
            }
        } else if dark {
            Self::DARK
        } else {
            Self::LIGHT
        }
    }
}

/// Additive Signal Glass semantic state tokens.
///
/// Deliberately a separate type rather than new [`ThemeTokens`] fields:
/// `ThemeTokens` is public without `#[non_exhaustive]`, so adding fields would
/// break downstream struct literals and patterns. Renderers derive this from a
/// resolved [`ThemeTokens`] via [`ThemeTokens::signal_glass`].
///
/// High contrast collapses every tint-only meaning: all surfaces equal
/// `surface`, strong borders equal `border`, and all state/status text equals
/// `text_primary`. Status colors are semantic fills only — renderers must pair
/// them with labels or shapes, never with color alone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SignalGlassTokens {
    /// Quiet surface for embedded controls and wells.
    pub surface_subtle: Rgba,
    /// Emphasis border for cards, dividers, and active rails.
    pub border_strong: Rgba,
    /// Accent pressed state.
    pub accent_pressed: Rgba,
    /// Selected control/surface fill (the resolved accent).
    pub selected_surface: Rgba,
    /// Text on the selected fill.
    pub selected_text: Rgba,
    /// Disabled control fill.
    pub disabled_surface: Rgba,
    /// Disabled text.
    pub disabled_text: Rgba,
    /// Muted/zero volume (VolumePro legend, seeded from `volume_threshold`).
    pub muted: Rgba,
    /// Success status fill (VolumePro low band).
    pub status_success: Rgba,
    /// Warning status fill (VolumePro high band).
    pub status_warning: Rgba,
    /// Informational status fill (VolumePro medium band).
    pub status_info: Rgba,
}

impl ThemeTokens {
    /// Derive the additive Signal Glass state tokens from this resolved theme.
    pub fn signal_glass(self) -> SignalGlassTokens {
        let palette = Palette::select(self.is_dark, self.high_contrast);
        if self.high_contrast {
            // Opaque surfaces, strong borders, no tint-only meaning.
            SignalGlassTokens {
                surface_subtle: palette.surface,
                border_strong: palette.border,
                accent_pressed: self.accent,
                selected_surface: palette.surface,
                selected_text: palette.text_primary,
                disabled_surface: palette.surface,
                disabled_text: palette.text_primary,
                muted: palette.text_primary,
                status_success: palette.text_primary,
                status_warning: palette.text_primary,
                status_info: palette.text_primary,
            }
        } else {
            SignalGlassTokens {
                surface_subtle: palette.surface_subtle,
                border_strong: palette.border_strong,
                accent_pressed: accent_pressed_for(self.accent, self.is_dark),
                selected_surface: self.accent,
                selected_text: self.text_on_accent,
                disabled_surface: palette.surface_subtle,
                disabled_text: self.text_disabled,
                muted: self.volume_threshold.muted,
                status_success: self.volume_threshold.low,
                status_warning: self.volume_threshold.high,
                status_info: self.volume_threshold.medium,
            }
        }
    }
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
        // Theme-aware accent: `System`/`Blue` resolve to the approved light
        // (`#0067C0`) or dark (`#3AA8FF`) Signal Glass blue.
        let accent = accent_color_for(accent, dark);
        let palette = Palette::select(dark, high_contrast);
        // Hover uses the exact approved values for the blue family and reads
        // toward the background for the remaining accents; high contrast keeps
        // the accent unmodified. Pressed lives on `SignalGlassTokens`.
        let accent_hover = if high_contrast {
            accent
        } else {
            accent_hover_for(accent, dark)
        };
        // Threshold 0.30 luminance keeps `text_on_accent` at or above 3:1 on
        // both sides for every accent in both themes.
        let text_on_accent = if accent.relative_luminance() <= 0.30 {
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
                ring_gap_px: 3.0,
                // Contrast layer: visible against every surface, including
                // high contrast, so the ring never relies on the accent alone.
                inner_ring: palette.text_primary,
                inner_ring_width_px: 1.0,
                inner_ring_gap_px: 1.0,
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
    /// and for text on the accent fill. Disabled text is checked against the
    /// *surface* (where disabled content actually renders); the approved
    /// Signal Glass disabled colors sit just above 3:1 there.
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
            pair(tokens.text_disabled, tokens.surface) >= 3.0,
            "disabled {:.2}:1",
            pair(tokens.text_disabled, tokens.surface)
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
    fn system_and_blue_accents_resolve_theme_aware() {
        // Approved values: light `#0067C0`, dark `#3AA8FF`.
        assert_eq!(
            accent_color_for(AccentMode::System, false),
            ACCENT_SYSTEM_BLUE
        );
        assert_eq!(
            accent_color_for(AccentMode::System, true),
            ACCENT_SYSTEM_BLUE_DARK
        );
        assert_eq!(
            accent_color_for(AccentMode::Blue, false),
            ACCENT_SYSTEM_BLUE
        );
        assert_eq!(
            accent_color_for(AccentMode::Blue, true),
            ACCENT_SYSTEM_BLUE_DARK
        );
        // The remaining accents are theme-independent.
        assert_eq!(accent_color_for(AccentMode::Green, true), ACCENT_GREEN);
        assert_eq!(accent_color_for(AccentMode::Purple, true), ACCENT_PURPLE);
        assert_eq!(accent_color_for(AccentMode::Orange, true), ACCENT_ORANGE);
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
        for dark in [false, true] {
            let theme = if dark {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            };
            for accent in [
                AccentMode::System,
                AccentMode::Blue,
                AccentMode::Green,
                AccentMode::Purple,
                AccentMode::Orange,
            ] {
                let t = tokens_for(theme, false, accent, || None);
                let resolved = accent_color_for(accent, dark);
                assert_eq!(t.accent, resolved);
                assert_eq!(t.focus.ring, resolved);
            }
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

    // --- approved Signal Glass surface palette ----------------------------------------

    #[test]
    fn light_palette_matches_the_approved_signal_glass_hex_values() {
        let t = light_tokens();
        let sg = t.signal_glass();
        assert_eq!(t.background, Rgba::from_rgb(0xF7, 0xF9, 0xFC));
        assert_eq!(t.surface, Rgba::from_rgb(0xFF, 0xFF, 0xFF));
        assert_eq!(t.surface_elevated, Rgba::from_rgb(0xFF, 0xFF, 0xFF));
        assert_eq!(sg.surface_subtle, Rgba::from_rgb(0xF1, 0xF4, 0xF8));
        assert_eq!(t.border, Rgba::from_rgb(0xD7, 0xDE, 0xE8));
        assert_eq!(sg.border_strong, Rgba::from_rgb(0xAE, 0xB9, 0xC8));
        assert_eq!(t.text_primary, Rgba::from_rgb(0x17, 0x20, 0x2B));
        assert_eq!(t.text_secondary, Rgba::from_rgb(0x52, 0x60, 0x71));
        assert_eq!(t.text_disabled, Rgba::from_rgb(0x89, 0x95, 0xA3));
        assert_eq!(t.accent, Rgba::from_rgb(0x00, 0x67, 0xC0));
        assert_eq!(t.accent_hover, Rgba::from_rgb(0x00, 0x5A, 0xAB));
        assert_eq!(sg.accent_pressed, Rgba::from_rgb(0x00, 0x4A, 0x8D));
    }

    #[test]
    fn dark_palette_matches_the_approved_signal_glass_hex_values() {
        let t = dark_tokens();
        let sg = t.signal_glass();
        assert_eq!(t.background, Rgba::from_rgb(0x10, 0x13, 0x1A));
        assert_eq!(t.surface, Rgba::from_rgb(0x17, 0x1C, 0x24));
        assert_eq!(t.surface_elevated, Rgba::from_rgb(0x20, 0x27, 0x35));
        assert_eq!(sg.surface_subtle, Rgba::from_rgb(0x1C, 0x22, 0x2D));
        assert_eq!(t.border, Rgba::from_rgb(0x34, 0x40, 0x52));
        assert_eq!(sg.border_strong, Rgba::from_rgb(0x53, 0x62, 0x76));
        assert_eq!(t.text_primary, Rgba::from_rgb(0xF5, 0xF7, 0xFA));
        assert_eq!(t.text_secondary, Rgba::from_rgb(0xAA, 0xB4, 0xC3));
        assert_eq!(t.text_disabled, Rgba::from_rgb(0x75, 0x81, 0x92));
        assert_eq!(t.accent, Rgba::from_rgb(0x3A, 0xA8, 0xFF));
        assert_eq!(t.accent_hover, Rgba::from_rgb(0x62, 0xB8, 0xFF));
        assert_eq!(sg.accent_pressed, Rgba::from_rgb(0x19, 0x8F, 0xEA));
    }

    // --- additive API compatibility ---------------------------------------------------

    #[test]
    fn theme_tokens_remain_constructible_with_only_legacy_fields() {
        // `ThemeTokens` is deliberately additive: a struct literal written
        // against the pre-Signal-Glass field set must keep compiling. New
        // tokens arrive through `signal_glass()` instead of new fields.
        let _ = ThemeTokens {
            is_dark: false,
            high_contrast: false,
            background: Rgba::BLACK,
            surface: Rgba::WHITE,
            surface_elevated: Rgba::WHITE,
            border: Rgba::from_rgb(0xCC, 0xCC, 0xCC),
            text_primary: Rgba::BLACK,
            text_secondary: Rgba::from_rgb(0x55, 0x55, 0x55),
            text_disabled: Rgba::from_rgb(0x88, 0x88, 0x88),
            text_on_accent: Rgba::WHITE,
            accent: Rgba::from_rgb(0x00, 0x67, 0xC0),
            accent_hover: Rgba::from_rgb(0x00, 0x5A, 0xAB),
            volume_threshold: VolumeThresholdColors::default(),
            focus: FocusTokens {
                ring: Rgba::from_rgb(0x00, 0x67, 0xC0),
                ring_width_px: 1.5,
                ring_gap_px: 3.0,
                inner_ring: Rgba::BLACK,
                inner_ring_width_px: 1.0,
                inner_ring_gap_px: 1.0,
            },
            error: ErrorTokens {
                text: Rgba::from_rgb(0xC4, 0x2B, 0x1C),
                border: Rgba::from_rgb(0xC4, 0x2B, 0x1C),
                surface: Rgba::from_rgb(0xFD, 0xE7, 0xE9),
            },
            spacing: SpacingTokens::default(),
            radii: RadiusTokens::default(),
            typography: TypographyTokens::default(),
            elevation: ElevationTokens::default(),
            hit_target: HitTargetTokens::default(),
            material: MaterialIntent {
                surface_alpha: 1.0,
                blur_enabled: false,
                blur_radius_px: 0.0,
            },
            animation: AnimationTokens::default(),
        };
    }

    // --- accent state distinction -----------------------------------------------------

    #[test]
    fn normal_themes_keep_accent_states_distinct() {
        for t in [light_tokens(), dark_tokens()] {
            let sg = t.signal_glass();
            assert_ne!(t.accent, t.accent_hover, "{:?}", t.accent);
            assert_ne!(t.accent, sg.accent_pressed, "{:?}", t.accent);
            assert_ne!(t.accent_hover, sg.accent_pressed, "{:?}", t.accent_hover);
        }
    }

    // --- contrast ---------------------------------------------------------------------

    #[test]
    fn light_and_dark_text_contrast_against_relevant_surfaces() {
        for t in [light_tokens(), dark_tokens()] {
            for bg in [t.background, t.surface] {
                assert!(
                    t.text_primary.contrast_ratio(bg) >= 7.0,
                    "primary vs {bg:?}: {:.2}:1",
                    t.text_primary.contrast_ratio(bg)
                );
                assert!(
                    t.text_secondary.contrast_ratio(bg) >= 4.5,
                    "secondary vs {bg:?}: {:.2}:1",
                    t.text_secondary.contrast_ratio(bg)
                );
            }
            assert!(
                t.text_disabled.contrast_ratio(t.surface) >= 3.0,
                "disabled vs surface: {:.2}:1",
                t.text_disabled.contrast_ratio(t.surface)
            );
            assert!(
                t.text_on_accent.contrast_ratio(t.accent) >= 3.0,
                "on-accent: {:.2}:1",
                t.text_on_accent.contrast_ratio(t.accent)
            );
            // Focus layers stay visible against the surface.
            assert!(
                t.focus.inner_ring.contrast_ratio(t.surface) >= 3.0,
                "inner ring vs surface: {:.2}:1",
                t.focus.inner_ring.contrast_ratio(t.surface)
            );
        }
    }

    #[test]
    fn every_accent_mode_keeps_readable_text_on_accent() {
        for theme in [ThemeMode::Light, ThemeMode::Dark] {
            for accent in [
                AccentMode::System,
                AccentMode::Blue,
                AccentMode::Green,
                AccentMode::Purple,
                AccentMode::Orange,
            ] {
                let t = tokens_for(theme, false, accent, || None);
                assert!(
                    t.text_on_accent.contrast_ratio(t.accent) >= 3.0,
                    "{accent:?} {theme:?}: {:.2}:1",
                    t.text_on_accent.contrast_ratio(t.accent)
                );
            }
        }
    }

    #[test]
    fn high_contrast_primary_and_state_text_contrast_is_maximal() {
        for system_dark in [Some(true), Some(false)] {
            let t = tokens(ThemeMode::System, true, system_dark);
            let sg = t.signal_glass();
            assert!(
                t.text_primary.contrast_ratio(t.background) >= 21.0 - 1e-6,
                "{:.2}:1",
                t.text_primary.contrast_ratio(t.background)
            );
            assert_eq!(sg.selected_text, t.text_primary);
            assert_eq!(sg.muted, t.text_primary);
            assert!(
                sg.selected_text.contrast_ratio(t.background) >= 21.0 - 1e-6,
                "{:.2}:1",
                sg.selected_text.contrast_ratio(t.background)
            );
        }
    }

    // --- high contrast -------------------------------------------------------------

    #[test]
    fn high_contrast_forces_opaque_surfaces_and_collapses_state_tokens() {
        for system_dark in [Some(true), Some(false)] {
            let t = tokens(ThemeMode::System, true, system_dark);
            let sg = t.signal_glass();
            for s in [
                t.background,
                t.surface,
                t.surface_elevated,
                sg.surface_subtle,
                sg.selected_surface,
                sg.disabled_surface,
            ] {
                assert!(s.is_opaque(), "{s:?}");
            }
            assert_eq!(t.material.surface_alpha, 1.0);
            assert!(!t.material.blur_enabled);
            // No tint-only meaning: surfaces collapse, state text collapses.
            assert_eq!(sg.surface_subtle, t.surface);
            assert_eq!(sg.selected_surface, t.surface);
            assert_eq!(sg.disabled_surface, t.surface);
            assert_eq!(sg.border_strong, t.border);
            assert_eq!(sg.selected_text, t.text_primary);
            assert_eq!(sg.disabled_text, t.text_primary);
            assert_eq!(sg.muted, t.text_primary);
            assert_eq!(sg.status_success, t.text_primary);
            assert_eq!(sg.status_warning, t.text_primary);
            assert_eq!(sg.status_info, t.text_primary);
            assert_eq!(sg.accent_pressed, t.accent);
            assert_eq!(t.accent_hover, t.accent);
        }
    }

    // --- two-layer focus ------------------------------------------------------------

    #[test]
    fn focus_ring_has_two_visible_layers() {
        for t in [light_tokens(), dark_tokens()] {
            let f = t.focus;
            assert!(f.ring_width_px > 0.0, "outer width");
            assert!(f.inner_ring_width_px > 0.0, "inner width");
            assert_ne!(f.ring, f.inner_ring, "layers must be distinct colors");
            assert_ne!(
                f.ring_width_px, f.inner_ring_width_px,
                "layers must have distinct widths"
            );
            assert!(f.inner_ring_gap_px > 0.0, "inner gap");
            // The inner ring + its gap sit inside the outer ring's gap, so
            // there is an air gap between the two layers.
            assert!(
                f.inner_ring_gap_px + f.inner_ring_width_px < f.ring_gap_px,
                "no gap between layers: {} + {} vs {}",
                f.inner_ring_gap_px,
                f.inner_ring_width_px,
                f.ring_gap_px
            );
        }
    }

    // --- SignalGlassTokens ------------------------------------------------------------

    #[test]
    fn signal_glass_tokens_match_approved_light_and_dark_values() {
        let light = light_tokens();
        let sg = light.signal_glass();
        assert_eq!(sg.surface_subtle, Rgba::from_rgb(0xF1, 0xF4, 0xF8));
        assert_eq!(sg.border_strong, Rgba::from_rgb(0xAE, 0xB9, 0xC8));
        assert_eq!(sg.accent_pressed, Rgba::from_rgb(0x00, 0x4A, 0x8D));
        assert_eq!(sg.selected_surface, light.accent);
        assert_eq!(sg.selected_text, light.text_on_accent);
        assert_eq!(sg.disabled_surface, sg.surface_subtle);
        assert_eq!(sg.disabled_text, light.text_disabled);
        assert_eq!(sg.muted, light.volume_threshold.muted);
        assert_eq!(sg.status_success, light.volume_threshold.low);
        assert_eq!(sg.status_warning, light.volume_threshold.high);
        assert_eq!(sg.status_info, light.volume_threshold.medium);

        let dark = dark_tokens();
        let sg = dark.signal_glass();
        assert_eq!(sg.surface_subtle, Rgba::from_rgb(0x1C, 0x22, 0x2D));
        assert_eq!(sg.border_strong, Rgba::from_rgb(0x53, 0x62, 0x76));
        assert_eq!(sg.accent_pressed, Rgba::from_rgb(0x19, 0x8F, 0xEA));
        assert_eq!(sg.selected_surface, dark.accent);
        assert_eq!(sg.selected_text, dark.text_on_accent);
        assert_eq!(sg.disabled_surface, sg.surface_subtle);
        assert_eq!(sg.disabled_text, dark.text_disabled);
        assert_eq!(sg.muted, Rgba::from_rgb(0x88, 0x88, 0x88));
        assert_eq!(sg.status_success, Rgba::from_rgb(0x27, 0xAE, 0x60));
        assert_eq!(sg.status_warning, Rgba::from_rgb(0xE0, 0x5C, 0x00));
        assert_eq!(sg.status_info, Rgba::from_rgb(0x00, 0x78, 0xD4));
    }

    // --- approved layout and typography roles ------------------------------------------

    #[test]
    fn approved_layout_and_typography_roles_are_present_and_ordered() {
        let t = dark_tokens();

        // Spacing: 4px grid through xxl.
        let spacing = [
            t.spacing.xs_px,
            t.spacing.sm_px,
            t.spacing.md_px,
            t.spacing.lg_px,
            t.spacing.xl_px,
            t.spacing.xxl_px,
        ];
        assert_eq!(spacing, [4.0, 8.0, 12.0, 16.0, 24.0, 32.0]);
        assert!(spacing.windows(2).all(|w| w[0] < w[1]));
        assert!(spacing.iter().all(|v| *v > 0.0));

        // Radius: legacy aliases plus the approved roles.
        assert_eq!(t.radii.control_px, 4.0);
        assert_eq!(t.radii.card_px, 8.0);
        assert_eq!(t.radii.surface_px, 12.0);
        assert_eq!(t.radii.pill_px, 999.0);
        let radii = [
            t.radii.control_px,
            t.radii.card_px,
            t.radii.surface_px,
            t.radii.pill_px,
        ];
        assert!(radii.windows(2).all(|w| w[0] < w[1]));
        assert!(radii.iter().all(|v| *v > 0.0));
        assert!(t.radii.small_px <= t.radii.medium_px && t.radii.medium_px <= t.radii.large_px);

        // Typography: approved roles with exact size/weight/face.
        assert_eq!(
            t.typography.display_value,
            TextRole {
                size_px: 28.0,
                weight: 600,
                monospace: false,
            }
        );
        assert_eq!(
            t.typography.surface_title,
            TextRole {
                size_px: 17.0,
                weight: 600,
                monospace: false,
            }
        );
        assert_eq!(
            t.typography.section_title,
            TextRole {
                size_px: 15.0,
                weight: 600,
                monospace: false,
            }
        );
        assert_eq!(
            t.typography.body,
            TextRole {
                size_px: 13.0,
                weight: 400,
                monospace: false,
            }
        );
        assert_eq!(
            t.typography.label,
            TextRole {
                size_px: 12.0,
                weight: 600,
                monospace: false,
            }
        );
        assert_eq!(
            t.typography.caption,
            TextRole {
                size_px: 11.0,
                weight: 400,
                monospace: false,
            }
        );
        assert_eq!(
            t.typography.keycap,
            TextRole {
                size_px: 12.0,
                weight: 600,
                monospace: true,
            }
        );
        // Legacy aliases stay aligned with the approved roles.
        assert_eq!(t.typography.display, t.typography.display_value);
        assert_eq!(t.typography.title, t.typography.surface_title);
        // Ordered hierarchy: caption is the smallest, display_value largest
        // (label and keycap legitimately share a 12px size).
        assert!(t.typography.caption.size_px < t.typography.label.size_px);
        assert!(t.typography.label.size_px < t.typography.body.size_px);
        assert!(t.typography.body.size_px < t.typography.surface_title.size_px);
        assert!(t.typography.surface_title.size_px < t.typography.display_value.size_px);
    }
}
