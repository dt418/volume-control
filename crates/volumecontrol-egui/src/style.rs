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
    pub warning: Color32,
}

/// Small radius: chips, pills.
pub const RADIUS_SM: u8 = 4;
/// Control corner radius: buttons, inputs, list cards.
pub const RADIUS_CONTROL: u8 = 8;
/// Card corner radius.
pub const RADIUS_CARD: u8 = 12;
/// Mixer/overlay shell radius.
pub const RADIUS_HUD: u8 = 16;

/// Spacing scale (design tokens).
pub const SPACE_1: f32 = 2.0;
pub const SPACE_2: f32 = 4.0;
pub const SPACE_3: f32 = 6.0;
pub const SPACE_4: f32 = 8.0;
pub const SPACE_5: f32 = 12.0;
pub const SPACE_6: f32 = 16.0;
pub const SPACE_7: f32 = 24.0;
pub const SPACE_8: f32 = 32.0;

/// Intra-row spacing (alias of SPACE_4).
pub const SPACING_INNER: f32 = SPACE_4;
/// Card padding (alias of SPACE_6).
pub const SPACING_CARD: f32 = SPACE_6;

/// Surface geometry (logical points), shared with `placement.rs`.
pub const MIXER_WIDTH: f32 = 400.0;
pub const MIXER_HEIGHT: f32 = 240.0;
pub const SETTINGS_DEFAULT_WIDTH: f32 = 760.0;
pub const SETTINGS_DEFAULT_HEIGHT: f32 = 620.0;
pub const SETTINGS_MIN_WIDTH: f32 = 620.0;
pub const SETTINGS_MIN_HEIGHT: f32 = 520.0;
pub const HELP_WIDTH: f32 = 520.0;
pub const HELP_HEIGHT: f32 = 500.0;
pub const OVERLAY_WIDTH: f32 = 336.0;
pub const OVERLAY_HEIGHT: f32 = 88.0;

/// Docking margins (logical points).
pub const OVERLAY_MARGIN_X: f32 = 20.0;
pub const OVERLAY_MARGIN_Y: f32 = 40.0;
pub const MIXER_OVERLAY_GAP: f32 = 16.0;
pub const HELP_MARGIN_X: f32 = 24.0;
pub const HELP_MARGIN_Y: f32 = 48.0;

/// Component geometry shared by views.
pub const MIXER_FOOTER_HEIGHT: f32 = 22.0;
pub const MIXER_SESSION_LABEL_WIDTH: f32 = 140.0;
pub const MIXER_SESSION_ROW_HEIGHT: f32 = 30.0;
pub const MIXER_CONTROLS_WIDTH: f32 = 62.0;
pub const HOTKEY_LABEL_WIDTH: f32 = 210.0;
pub const HOTKEY_ROW_HEIGHT: f32 = 44.0;
pub const HOTKEY_CHIP_MIN_WIDTH: f32 = 120.0;
pub const HOTKEY_BUTTON_WIDTH: f32 = 46.0;
pub const PICKER_WIDTH: f32 = 320.0;
pub const PICKER_HEIGHT: f32 = 180.0;

/// Control sizes.
pub const ICON_BUTTON: f32 = 24.0;
pub const BUTTON_HEIGHT: f32 = 28.0;
pub const INPUT_HEIGHT: f32 = 30.0;
pub const ROW_HEIGHT: f32 = 32.0;

/// Motion durations.
pub const DURATION_FAST: f32 = 0.12;
pub const DURATION_NORMAL: f32 = 0.18;
pub const DURATION_SLOW: f32 = 0.24;

/// Typography scale derived from the user font size.
#[derive(Debug, Clone, Copy)]
pub struct TypeScale {
    pub caption: f32,
    pub small: f32,
    pub body: f32,
    pub title: f32,
    pub headline: f32,
    pub display: f32,
}

/// Build the type scale for a base font size (9..=24).
#[must_use]
pub fn type_scale(base: u8) -> TypeScale {
    let base = f32::from(base.clamp(9, 24));
    TypeScale {
        caption: (base - 4.0).max(8.0),
        small: (base - 2.0).max(9.0),
        body: base,
        title: base + 4.0,
        headline: base + 8.0,
        display: base + 14.0,
    }
}

/// Palette for the requested theme.
#[must_use]
pub fn tokens(is_dark: bool) -> Tokens {
    if is_dark {
        Tokens {
            // Alpha ~180 keeps the vibrancy backdrop visible.
            background: Color32::from_rgba_unmultiplied(23, 24, 27, 235),
            card: Color32::from_rgba_unmultiplied(30, 32, 35, 232),
            input: Color32::from_rgba_unmultiplied(35, 37, 41, 240),
            border: Color32::from_rgb(46, 49, 52),
            accent: Color32::from_rgb(0x2F, 0x7F, 0xF7),
            text: Color32::from_rgb(0xE8, 0xEA, 0xED),
            muted: Color32::from_rgb(0x9A, 0xA0, 0xA6),
            danger: Color32::from_rgb(0xEF, 0x44, 0x44),
            success: Color32::from_rgb(0x22, 0xC5, 0x5E),
            warning: Color32::from_rgb(0xF5, 0x9E, 0x0B),
        }
    } else {
        Tokens {
            background: Color32::from_rgba_unmultiplied(245, 246, 248, 245),
            card: Color32::from_rgba_unmultiplied(255, 255, 255, 240),
            input: Color32::from_rgba_unmultiplied(237, 239, 242, 245),
            border: Color32::from_rgb(0xD8, 0xDC, 0xE1),
            accent: Color32::from_rgb(0x2F, 0x7F, 0xF7),
            text: Color32::from_rgb(0x1B, 0x1D, 0x20),
            muted: Color32::from_rgb(0x5F, 0x66, 0x70),
            danger: Color32::from_rgb(0xDC, 0x26, 0x26),
            success: Color32::from_rgb(0x16, 0xA3, 0x4A),
            warning: Color32::from_rgb(0xB4, 0x53, 0x09),
        }
    }
}

/// Install the token set into the egui context.
pub fn apply(ctx: &egui::Context, is_dark: bool, font_size: u8) {
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
    let base = f32::from(font_size.clamp(9, 24));
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(SPACING_INNER, SPACING_INNER);
        style.spacing.button_padding = egui::vec2(10.0, 4.0);
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(base));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(base));
        style.text_styles.insert(
            egui::TextStyle::Small,
            egui::FontId::proportional((base - 3.0).max(8.0)),
        );
        style.text_styles.insert(
            egui::TextStyle::Heading,
            egui::FontId::proportional(base + 5.0),
        );
        style.text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::monospace((base - 1.0).max(8.0)),
        );
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
