//! Help surface view: keyboard shortcuts reference, live status badges,
//! search filter, and conflict callout.

use eframe::egui::{self, Align, Layout, RichText};
use volumectl::config::{HotkeyBindings, Language};
use volumectl::hotkeys::{HotkeyAction, HotkeyRegResult, HotkeyRegStatus};

use crate::i18n::{tr, Key};
use crate::style::{self, Tokens};

/// View state passed from the host controller.
pub struct HelpView<'a> {
    pub language: Language,
    pub is_dark: bool,
    pub query: &'a mut String,
    pub bindings: &'a HotkeyBindings,
    pub status: &'a [HotkeyRegResult],
}

/// Intent emitted by user interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpIntent {
    OpenConfig,
    OpenSettings,
    Close,
}

pub const FOOTER_BUTTONS: [(Key, HelpIntent); 3] = [
    (Key::HelpEditConfig, HelpIntent::OpenConfig),
    (Key::SettingsTitle, HelpIntent::OpenSettings),
    (Key::CommonClose, HelpIntent::Close),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HelpGroup {
    Volume,
    Commands,
    Extended,
}

impl HelpGroup {
    pub const ALL: [Self; 3] = [Self::Volume, Self::Commands, Self::Extended];

    #[must_use]
    pub fn heading_key(self) -> Key {
        match self {
            Self::Volume => Key::HelpGroupVolume,
            Self::Commands => Key::HelpGroupCommands,
            Self::Extended => Key::HelpGroupExtended,
        }
    }
}

type SpecTuple = (
    HotkeyAction,
    HelpGroup,
    Key,
    Key,
    fn(&HotkeyBindings) -> &str,
);

const SPECS: [SpecTuple; 8] = [
    (
        HotkeyAction::VolumeUp,
        HelpGroup::Volume,
        Key::HotkeyVolumeUp,
        Key::HotkeyVolumeUpHelp,
        |b| &b.volume_up,
    ),
    (
        HotkeyAction::VolumeDown,
        HelpGroup::Volume,
        Key::HotkeyVolumeDown,
        Key::HotkeyVolumeDownHelp,
        |b| &b.volume_down,
    ),
    (
        HotkeyAction::ToggleMute,
        HelpGroup::Commands,
        Key::HotkeyToggleMute,
        Key::HotkeyToggleMuteHelp,
        |b| &b.toggle_mute,
    ),
    (
        HotkeyAction::Reset50,
        HelpGroup::Commands,
        Key::HotkeyReset50,
        Key::HotkeyReset50Help,
        |b| &b.reset_50,
    ),
    (
        HotkeyAction::OpenMixer,
        HelpGroup::Commands,
        Key::HotkeyOpenMixer,
        Key::HotkeyOpenMixerHelp,
        |b| &b.open_mixer,
    ),
    (
        HotkeyAction::VolumeUpLarge,
        HelpGroup::Extended,
        Key::HotkeyVolumeUpLarge,
        Key::HotkeyVolumeUpLargeHelp,
        |b| &b.volume_up_large,
    ),
    (
        HotkeyAction::VolumeDownLarge,
        HelpGroup::Extended,
        Key::HotkeyVolumeDownLarge,
        Key::HotkeyVolumeDownLargeHelp,
        |b| &b.volume_down_large,
    ),
    (
        HotkeyAction::OpenMenu,
        HelpGroup::Extended,
        Key::HotkeyOpenMenu,
        Key::HotkeyOpenMenuHelp,
        |b| &b.open_menu,
    ),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpRow {
    pub action: HotkeyAction,
    pub group: HelpGroup,
    pub label_key: Key,
    pub help_key: Key,
    pub label: String,
    pub help: String,
    pub binding: String,
    pub formatted_combo: String,
    pub chips: Vec<String>,
    pub status_key: Key,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpGroupSection {
    pub group: HelpGroup,
    pub heading_key: Key,
    pub rows: Vec<HelpRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictCalloutModel {
    pub title_key: Key,
    pub body_key: Key,
    pub action_key: Key,
    pub tail_key: Key,
    pub conflicted_bindings: Vec<String>,
}

#[must_use]
pub fn binding_chips(binding: &str) -> Vec<String> {
    if binding.is_empty() {
        return Vec::new();
    }
    binding
        .split('+')
        .map(|t| match t.trim() {
            "ArrowUp" => "↑".into(),
            "ArrowDown" => "↓".into(),
            "ArrowLeft" => "←".into(),
            "ArrowRight" => "→".into(),
            "Control" | "Ctrl" => "Ctrl".into(),
            "Option" | "Alt" => "Alt".into(),
            "Command" | "Cmd" | "Super" => "⌘".into(),
            "Shift" => "Shift".into(),
            "Space" => "Space".into(),
            "Escape" | "Esc" => "Esc".into(),
            s if s.starts_with("Key") && s.len() == 4 => s[3..].into(),
            s if s.starts_with("Digit") && s.len() == 6 => s[5..].into(),
            s => s.into(),
        })
        .collect()
}

#[must_use]
pub fn format_binding(binding: &str) -> String {
    binding_chips(binding).join("+")
}

#[must_use]
pub fn status_for_action(
    action: HotkeyAction,
    status: &[HotkeyRegResult],
) -> Option<&HotkeyRegStatus> {
    status
        .iter()
        .find(|r| r.action == action)
        .map(|r| &r.status)
}

#[must_use]
pub fn status_key(status: Option<&HotkeyRegStatus>) -> Key {
    match status {
        Some(HotkeyRegStatus::Registered) => Key::HelpStatusReady,
        Some(HotkeyRegStatus::Conflicted(_)) => Key::HelpStatusInUse,
        Some(HotkeyRegStatus::Disabled) => Key::HelpStatusDisabled,
        Some(HotkeyRegStatus::HookRouted) => Key::HelpStatusActive,
        None => Key::HelpStatusFallback,
    }
}

#[must_use]
pub fn status_key_for_action(action: HotkeyAction, status: &[HotkeyRegResult]) -> Key {
    status_key(status_for_action(action, status))
}

#[must_use]
pub fn rows(
    bindings: &HotkeyBindings,
    status: &[HotkeyRegResult],
    query: &str,
    lang: Language,
) -> Vec<HelpRow> {
    let q = query.trim().to_lowercase();
    SPECS
        .iter()
        .filter_map(|&(action, group, l_key, h_key, field)| {
            let binding = field(bindings).to_string();
            let chips = binding_chips(&binding);
            let formatted_combo = chips.join("+");
            let label = tr(l_key, lang, &[]).to_string();
            let help = tr(h_key, lang, &[]).to_string();

            let hit = q.is_empty()
                || [&label, &help, &binding, &formatted_combo]
                    .iter()
                    .any(|s| s.to_lowercase().contains(&q))
                || chips.iter().any(|c| c.to_lowercase().contains(&q));
            if !hit {
                return None;
            }

            Some(HelpRow {
                action,
                group,
                label_key: l_key,
                help_key: h_key,
                label,
                help,
                binding,
                formatted_combo,
                chips,
                status_key: status_key_for_action(action, status),
            })
        })
        .collect()
}

#[must_use]
pub fn grouped(rows: &[HelpRow]) -> Vec<HelpGroupSection> {
    HelpGroup::ALL
        .into_iter()
        .filter_map(|g| {
            let s_rows: Vec<HelpRow> = rows.iter().filter(|r| r.group == g).cloned().collect();
            (!s_rows.is_empty()).then(|| HelpGroupSection {
                group: g,
                heading_key: g.heading_key(),
                rows: s_rows,
            })
        })
        .collect()
}

#[must_use]
pub fn no_matches_text(query: &str, language: Language) -> String {
    format!("{} \"{query}\".", tr(Key::HelpNoMatches, language, &[]))
}

#[must_use]
pub fn conflict_callout(
    bindings: &HotkeyBindings,
    status: &[HotkeyRegResult],
) -> Option<ConflictCalloutModel> {
    let conflicted: Vec<String> = SPECS
        .iter()
        .filter_map(
            |&(action, _, _, _, field)| match status_for_action(action, status)? {
                HotkeyRegStatus::Conflicted(_) => {
                    let raw = field(bindings);
                    let fmt = format_binding(raw);
                    Some(if fmt.is_empty() { raw.into() } else { fmt })
                }
                _ => None,
            },
        )
        .collect();

    if conflicted.is_empty() {
        return None;
    }
    Some(ConflictCalloutModel {
        title_key: Key::HelpCalloutTitle,
        body_key: Key::HelpCalloutBody,
        action_key: Key::HelpCalloutAction,
        tail_key: if conflicted.len() == 1 {
            Key::HelpCalloutTailOne
        } else {
            Key::HelpCalloutTailMany
        },
        conflicted_bindings: conflicted,
    })
}

#[must_use]
pub fn conflict_sentence(callout: &ConflictCalloutModel, language: Language) -> String {
    let combos = &callout.conflicted_bindings;
    let names = match combos.len() {
        0 => String::new(),
        1 => combos[0].clone(),
        _ => format!(
            "{} and {}",
            combos[..combos.len() - 1].join(", "),
            combos.last().unwrap()
        ),
    };
    let tail = tr(callout.tail_key, language, &[]);
    if names.is_empty() {
        tail.to_string()
    } else {
        format!("{names} {tail}")
    }
}

pub fn show(ui: &mut egui::Ui, view: &mut HelpView<'_>) -> Vec<HelpIntent> {
    let mut intents = Vec::new();
    let tokens = style::tokens(view.is_dark);
    ui.spacing_mut().item_spacing = egui::vec2(style::SPACING_INNER, style::SPACING_INNER);

    let hint = tr(Key::HelpSearchPlaceholder, view.language, &[]).to_string();
    egui::Frame::NONE
        .fill(tokens.input)
        .stroke(egui::Stroke::new(1.0, tokens.border))
        .corner_radius(egui::CornerRadius::same(style::RADIUS_CONTROL))
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(view.query)
                    .frame(egui::Frame::NONE)
                    .hint_text(hint)
                    .desired_width(f32::INFINITY),
            );
        });
    ui.add_space(style::SPACE_2);

    let content_height = (ui.available_height() - 40.0).max(60.0);
    ui.allocate_ui_with_layout(
        egui::vec2(ui.available_width(), content_height),
        Layout::top_down(Align::Min),
        |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                if let Some(callout) = conflict_callout(view.bindings, view.status) {
                    render_callout(ui, &callout, tokens, view.language, &mut intents);
                    ui.add_space(6.0);
                }
                let filtered = rows(view.bindings, view.status, view.query, view.language);
                let sections = grouped(&filtered);
                if sections.is_empty() {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new(no_matches_text(view.query, view.language))
                            .small()
                            .color(tokens.muted),
                    );
                } else {
                    for section in &sections {
                        ui.label(
                            RichText::new(tr(section.heading_key, view.language, &[]))
                                .small()
                                .color(tokens.muted),
                        );
                        for row in &section.rows {
                            render_row(ui, row, tokens, view.language);
                            ui.add_space(2.0);
                        }
                        ui.add_space(4.0);
                    }
                }
            });
        },
    );

    ui.separator();
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            for &(key, intent) in FOOTER_BUTTONS.iter().rev() {
                if ui.button(tr(key, view.language, &[]).to_string()).clicked() {
                    intents.push(intent);
                }
            }
        });
    });

    intents
}

fn card_frame(tokens: Tokens, border: egui::Color32) -> egui::Frame {
    egui::Frame::NONE
        .fill(tokens.card)
        .stroke(egui::Stroke::new(1.0, border))
        .corner_radius(egui::CornerRadius::same(style::RADIUS_CONTROL))
        .inner_margin(egui::Margin::symmetric(10, 6))
}

fn render_callout(
    ui: &mut egui::Ui,
    c: &ConflictCalloutModel,
    tokens: Tokens,
    lang: Language,
    intents: &mut Vec<HelpIntent>,
) {
    card_frame(tokens, tokens.warning).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.colored_label(tokens.warning, "⚠");
            ui.label(
                RichText::new(tr(c.title_key, lang, &[]))
                    .strong()
                    .color(tokens.warning),
            );
        });
        ui.add(egui::Label::new(RichText::new(tr(c.body_key, lang, &[])).small()).wrap());
        let summary = format!(
            "{} {}",
            c.conflicted_bindings.len(),
            tr(c.tail_key, lang, &[]),
        );
        ui.horizontal(|ui| {
            ui.label(RichText::new(summary).small().color(tokens.muted));
            if ui.link(tr(c.action_key, lang, &[]).as_ref()).clicked() {
                intents.push(HelpIntent::OpenSettings);
            }
        });
        ui.horizontal_wrapped(|ui| {
            for combo in c.conflicted_bindings.iter().take(3) {
                render_chip(ui, combo, tokens);
            }
            let remaining = c.conflicted_bindings.len().saturating_sub(3);
            if remaining > 0 {
                ui.label(
                    RichText::new(format!("+{remaining}"))
                        .small()
                        .color(tokens.muted),
                );
            }
        });
    });
}

fn render_row(ui: &mut egui::Ui, row: &HelpRow, tokens: Tokens, lang: Language) {
    card_frame(tokens, tokens.border).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(&row.label);
                ui.label(RichText::new(&row.help).small().color(tokens.muted));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                for chip in row.chips.iter().rev() {
                    render_chip(ui, chip, tokens);
                }
                render_status_pill(ui, row.status_key, tokens, lang);
            });
        });
    });
}

fn render_chip(ui: &mut egui::Ui, chip: &str, tokens: Tokens) {
    egui::Frame::NONE
        .fill(tokens.input)
        .stroke(egui::Stroke::new(1.0, tokens.border))
        .corner_radius(egui::CornerRadius::same(4))
        .inner_margin(egui::Margin::symmetric(5, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(chip).monospace().small().color(tokens.text))
        });
}

fn render_status_pill(ui: &mut egui::Ui, key: Key, tokens: Tokens, lang: Language) {
    let tint = match key {
        Key::HelpStatusReady => tokens.success,
        Key::HelpStatusInUse => tokens.danger,
        Key::HelpStatusDisabled => tokens.muted,
        Key::HelpStatusActive => tokens.accent,
        _ => tokens.warning,
    };
    egui::Frame::NONE
        .stroke(egui::Stroke::new(1.0, tint))
        .corner_radius(egui::CornerRadius::same(style::RADIUS_CONTROL))
        .inner_margin(egui::Margin::symmetric(5, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(tr(key, lang, &[])).small().color(tint))
        });
}
