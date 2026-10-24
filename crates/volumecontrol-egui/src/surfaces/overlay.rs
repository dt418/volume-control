//! HUD overlay: volume toast and short text payloads.

use eframe::egui;
use egui::{Align, Layout, RichText};

use volumectl::config::Language;

use crate::i18n::{tr, Key};
use crate::style::{self, Tokens};
use crate::surfaces::mixer;

/// Duration of the HUD slide/fade in milliseconds.
pub const OVERLAY_ANIM_MS: u64 = 180;

/// Ease-out progress for the HUD entrance (0 at start, 1 once settled).
#[must_use]
pub fn overlay_ease(elapsed_ms: u64) -> f32 {
    let t = (elapsed_ms as f32 / OVERLAY_ANIM_MS as f32).clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t)
}

pub struct OverlayView<'a> {
    pub language: Language,
    pub is_dark: bool,
    pub text: Option<&'a str>,
    pub volume: u8,
    pub muted: bool,
    pub device: Option<&'a str>,
    pub green_up_to: u8,
    pub blue_up_to: u8,
    /// Entrance progress 0..=1 (1 = fully settled, no offset).
    pub progress: f32,
}

pub fn show(ui: &mut egui::Ui, view: &OverlayView<'_>) {
    let tokens = style::tokens(view.is_dark);
    let opaque = {
        let color = tokens.card;
        egui::Color32::from_rgb(color.r(), color.g(), color.b())
    };
    egui::CentralPanel::default()
        .frame(
            egui::Frame::NONE
                .fill(opaque)
                .corner_radius(egui::CornerRadius::same(style::RADIUS_HUD))
                .inner_margin(egui::Margin::symmetric(12, 10)),
        )
        .show(ui, |ui| {
            ui.set_opacity(view.progress.clamp(0.0, 1.0));
            ui.add_space((1.0 - view.progress.clamp(0.0, 1.0)) * 10.0);
            match view.text {
                Some(text) => {
                    ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                        ui.add(egui::Label::new(RichText::new(text).small()).wrap());
                    });
                }
                None => volume_layout(ui, view, &tokens),
            }
        });
}

fn volume_layout(ui: &mut egui::Ui, view: &OverlayView<'_>, tokens: &Tokens) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(
                RichText::new(tr(Key::OverlayVolume, view.language, &[]))
                    .small()
                    .color(tokens.text),
            );
            ui.label(
                RichText::new(view.device.map_or_else(
                    || tr(Key::OverlaySystemOutput, view.language, &[]).to_string(),
                    ToString::to_string,
                ))
                .small()
                .color(tokens.muted),
            );
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let value = if view.muted {
                tr(Key::CommonMuted, view.language, &[]).to_string()
            } else {
                format!("{}%", view.volume)
            };
            ui.label(
                RichText::new(value)
                    .size(ui.style().text_styles[&egui::TextStyle::Heading].size + 6.0)
                    .strong(),
            );
        });
    });
    ui.add_space(5.0);
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 8.0), egui::Sense::hover());
    mixer::paint_rail(
        ui.painter(),
        rect,
        view.volume,
        view.muted,
        view.green_up_to,
        view.blue_up_to,
        tokens,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_ease_reaches_full_alpha_at_duration() {
        assert_eq!(overlay_ease(0), 0.0);
        assert!(overlay_ease(90) > 0.0 && overlay_ease(90) < 1.0);
        assert_eq!(overlay_ease(180), 1.0);
        assert_eq!(overlay_ease(10_000), 1.0);
    }
}
