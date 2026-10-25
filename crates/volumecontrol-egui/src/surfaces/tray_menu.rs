//! Egui-rendered tray context menu.
//!
//! The OS menu cannot follow the app theme or be exercised by automation, so
//! the tray icon opens this borderless popup instead. It mirrors the legacy
//! 10-item structure and dispatches the same [`TrayAction`]s.

use egui_phosphor::regular as icon;
use volumectl::config::Language;

use super::tray::TrayAction;
use crate::style::{self, Tokens};

/// Fixed popup width (logical points).
pub const WIDTH: f32 = 268.0;
/// Row height used by both layout and the popup-size calculation.
const ROW_HEIGHT: f32 = 32.0;
const SEPARATOR_HEIGHT: f32 = 7.0;
const ROW_COUNT: f32 = 9.0;
const SEPARATOR_COUNT: f32 = 3.0;

/// Popup height derived from the current type size instead of a hardcoded
/// value: margins + header + separators + every row.
#[must_use]
pub fn height(base_font: f32) -> f32 {
    10.0 + (base_font + 2.0) + SEPARATOR_COUNT * SEPARATOR_HEIGHT + ROW_COUNT * ROW_HEIGHT
}

/// State needed to draw the menu.
#[derive(Clone, Copy)]
pub struct MenuState {
    pub language: Language,
    pub is_dark: bool,
    pub volume_pct: u8,
    pub muted: bool,
}

/// Draw the menu and return the chosen action, if any.
pub fn show(ui: &mut egui::Ui, state: MenuState) -> Option<TrayAction> {
    let tokens = style::tokens(state.is_dark);
    let mut row_ids: Vec<egui::Id> = Vec::new();
    // Rows already carry their own 32 px height; default item spacing would
    // add 8 px per row and push the last items out of the fixed popup.
    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
    let s = volumectl::locale::strings(state.language);
    let mut action = None;

    egui::Frame::NONE
        .fill(tokens.card)
        .stroke(egui::Stroke::new(1.0, tokens.border))
        .inner_margin(egui::Margin::same(5))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                egui::RichText::new(format!(
                    "{} — {}%",
                    crate::i18n::tr(crate::i18n::Key::AppName, state.language, &[]),
                    state.volume_pct
                ))
                .size(13.0)
                .strong()
                .color(tokens.text),
            );
            separator(ui, tokens);
            let mute_icon = if state.muted {
                icon::SPEAKER_SLASH
            } else {
                icon::SPEAKER_HIGH
            };
            {
                let mut push = |clicked: bool, id: egui::Id, act: TrayAction| {
                    row_ids.push(id);
                    if clicked {
                        action = Some(act);
                    }
                };
                let (clicked, id) = row(ui, tokens, mute_icon, s.tray_mute, state.muted);
                push(clicked, id, TrayAction::ToggleMute);
                let (clicked, id) = row(
                    ui,
                    tokens,
                    icon::ARROW_COUNTER_CLOCKWISE,
                    s.tray_reset,
                    false,
                );
                push(clicked, id, TrayAction::Reset50);
                let (clicked, id) = row(ui, tokens, icon::SLIDERS, s.tray_mixer, false);
                push(clicked, id, TrayAction::ShowMixer);
                separator(ui, tokens);
                let (clicked, id) = row(ui, tokens, icon::GEAR, s.tray_settings, false);
                push(clicked, id, TrayAction::ShowSettings);
                let (clicked, id) = row(ui, tokens, icon::QUESTION, s.tray_help, false);
                push(clicked, id, TrayAction::ShowHelp);
                let (clicked, id) = row(
                    ui,
                    tokens,
                    icon::ARROWS_CLOCKWISE,
                    s.tray_check_updates,
                    false,
                );
                push(clicked, id, TrayAction::CheckUpdates);
                let (clicked, id) = row(ui, tokens, icon::ARROW_CLOCKWISE, s.tray_reload, false);
                push(clicked, id, TrayAction::ReloadConfig);
                let (clicked, id) = row(ui, tokens, icon::FOLDER_OPEN, s.tray_edit, false);
                push(clicked, id, TrayAction::OpenConfig);
                separator(ui, tokens);
                let (clicked, id) = row(ui, tokens, icon::POWER, s.tray_exit, false);
                push(clicked, id, TrayAction::Exit);
            }
        });

    focus_first_row(ui, &row_ids);
    action
}

/// Focus the first row on open so the popup is keyboard-usable; egui's own
/// focus navigation then moves between rows with Up/Down.
fn focus_first_row(ui: &egui::Ui, ids: &[egui::Id]) {
    if ids.is_empty() {
        return;
    }
    // Focus is context-wide, so a widget in another viewport can own it; only
    // a row of this popup counts.
    let owns_focus = ui
        .memory(|memory| memory.focused())
        .is_some_and(|id| ids.contains(&id));
    if !owns_focus {
        ui.memory_mut(|memory| memory.request_focus(ids[0]));
    }
}

fn separator(ui: &mut egui::Ui, tokens: Tokens) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), SEPARATOR_HEIGHT),
        egui::Sense::hover(),
    );
    let line = egui::Rect::from_center_size(rect.center(), egui::vec2(rect.width(), 1.0));
    ui.painter()
        .rect_filled(line, egui::CornerRadius::ZERO, tokens.border);
}

fn row(
    ui: &mut egui::Ui,
    tokens: Tokens,
    glyph: &str,
    label: &str,
    checked: bool,
) -> (bool, egui::Id) {
    let width = ui.available_width();
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, ROW_HEIGHT), egui::Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(style::RADIUS_SM),
            tokens.input,
        );
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::same(style::RADIUS_SM),
            egui::Stroke::new(1.0, tokens.accent),
            egui::StrokeKind::Inside,
        );
    }
    let font = egui::FontId::proportional(ui.style().text_styles[&egui::TextStyle::Body].size);
    // Order: check column, icon, label.
    if checked {
        ui.painter().text(
            rect.left_center() + egui::vec2(10.0, 0.0),
            egui::Align2::LEFT_CENTER,
            icon::CHECK,
            egui::FontId::proportional(13.0),
            tokens.accent,
        );
    }
    ui.painter().text(
        rect.left_center() + egui::vec2(32.0, 0.0),
        egui::Align2::LEFT_CENTER,
        glyph,
        font.clone(),
        tokens.muted,
    );
    ui.painter().text(
        rect.left_center() + egui::vec2(56.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        font,
        tokens.text,
    );
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(egui::accesskit::Role::MenuItem);
        node.set_label(label.to_owned());
    });
    let activated = response.clicked()
        || (response.has_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)));
    (activated, response.id)
}
