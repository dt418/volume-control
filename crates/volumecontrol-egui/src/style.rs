//! Liquid Glass design tokens for egui.
//!
//! One token set for all platforms, sampled from the legacy v0.1.11 design
//! (`docs/references/legacy-app/design-spec.md` §1). Fills stay translucent so
//! the OS backdrop (Mica/Acrylic/NSVisualEffectView) reads through the
//! surface; the glass look degrades gracefully to plain translucent panels on
//! compositors without native blur.

use egui::{Color32, CornerRadius, Stroke, Visuals};

/// Surface palette for one theme.
#[derive(Debug, Clone, Copy)]
pub struct Tokens {
    pub background: Color32,
    pub card: Color32,
    pub input: Color32,
    pub border: Color32,
    pub accent: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub danger: Color32,
    pub success: Color32,
}

/// Control corner radius, design-spec §1.
pub const RADIUS_CONTROL: u8 = 8;
/// Card corner radius.
pub const RADIUS_CARD: u8 = 12;
/// Mixer/overlay card corner radius.
pub const RADIUS_HUD: u8 = 16;
/// Intra-row spacing.
pub const SPACING_INNER: f32 = 8.0;
/// Card padding.
pub const SPACING_CARD: f32 = 16.0;

/// Palette for the requested theme.
#[must_use]
pub fn tokens(is_dark: bool) -> Tokens {
    if is_dark {
        Tokens {
            // Alpha ~180 keeps the vibrancy backdrop visible.
            background: Color32::from_rgba_unmultiplied(23, 24, 27, 180),
            card: Color32::from_rgba_unmultiplied(30, 32, 35, 180),
            input: Color32::from_rgba_unmultiplied(35, 37, 41, 200),
            border: Color32::from_rgb(46, 49, 52),
            accent: Color32::from_rgb(0x2F, 0x7F, 0xF7),
            text: Color32::from_rgb(0xE8, 0xEA, 0xED),
            muted: Color32::from_rgb(0x9A, 0xA0, 0xA6),
            danger: Color32::from_rgb(0xEF, 0x44, 0x44),
            success: Color32::from_rgb(0x22, 0xC5, 0x5E),
        }
    } else {
        Tokens {
            background: Color32::from_rgba_unmultiplied(245, 246, 248, 190),
            card: Color32::from_rgba_unmultiplied(255, 255, 255, 200),
            input: Color32::from_rgba_unmultiplied(237, 239, 242, 220),
            border: Color32::from_rgb(0xD8, 0xDC, 0xE1),
            accent: Color32::from_rgb(0x2F, 0x7F, 0xF7),
            text: Color32::from_rgb(0x1B, 0x1D, 0x20),
            muted: Color32::from_rgb(0x5F, 0x66, 0x70),
            danger: Color32::from_rgb(0xDC, 0x26, 0x26),
            success: Color32::from_rgb(0x16, 0xA3, 0x4A),
        }
    }
}

/// Install the token set into the egui context.
pub fn apply(ctx: &egui::Context, is_dark: bool) {
    let tokens = tokens(is_dark);
    let mut visuals = if is_dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    visuals.panel_fill = tokens.background;
    visuals.window_fill = tokens.background;
    visuals.extreme_bg_color = tokens.input;
    visuals.override_text_color = Some(tokens.text);
    visuals.selection.bg_fill = tokens.accent;
    visuals.selection.stroke = Stroke::new(1.0, tokens.text);
    visuals.window_stroke = Stroke::new(1.0, tokens.border);
    visuals.window_corner_radius = CornerRadius::same(RADIUS_CARD);

    let widgets = &mut visuals.widgets;
    widgets.noninteractive.bg_fill = tokens.card;
    widgets.noninteractive.weak_bg_fill = tokens.card;
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, tokens.border);
    widgets.noninteractive.corner_radius = CornerRadius::same(RADIUS_CONTROL);
    widgets.noninteractive.fg_stroke = Stroke::new(1.0, tokens.text);
    widgets.inactive.bg_fill = tokens.input;
    widgets.inactive.weak_bg_fill = tokens.input;
    widgets.inactive.bg_stroke = Stroke::new(1.0, tokens.border);
    widgets.inactive.corner_radius = CornerRadius::same(RADIUS_CONTROL);
    widgets.inactive.fg_stroke = Stroke::new(1.0, tokens.text);
    widgets.hovered.bg_fill = tokens.input;
    widgets.hovered.weak_bg_fill = tokens.accent.gamma_multiply(0.35);
    widgets.hovered.bg_stroke = Stroke::new(1.0, tokens.accent);
    widgets.hovered.corner_radius = CornerRadius::same(RADIUS_CONTROL);
    widgets.hovered.fg_stroke = Stroke::new(1.0, tokens.text);
    widgets.active.bg_fill = tokens.accent;
    widgets.active.weak_bg_fill = tokens.accent;
    widgets.active.bg_stroke = Stroke::new(1.0, tokens.accent);
    widgets.active.corner_radius = CornerRadius::same(RADIUS_CONTROL);
    widgets.active.fg_stroke = Stroke::new(1.0, tokens.text);

    ctx.set_visuals(visuals);
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(SPACING_INNER, SPACING_INNER);
        style.spacing.button_padding = egui::vec2(10.0, 4.0);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_cards_stay_translucent() {
        let dark = tokens(true);
        assert!(dark.background.a() < 255);
        assert!(dark.card.a() < 255);
        assert_eq!(dark.accent, Color32::from_rgb(0x2F, 0x7F, 0xF7));
    }

    #[test]
    fn light_and_dark_text_contrast_flip() {
        assert!(tokens(true).text.r() > tokens(true).background.r());
        assert!(tokens(false).text.r() < tokens(false).background.r());
    }
}
