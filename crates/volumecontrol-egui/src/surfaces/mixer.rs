//! Mixer surface: legacy v0.1.11 parity (header, search, system row, session
//! rows, footer) on egui primitives.

use eframe::egui;
use egui::{Color32, RichText, Sense, Shape, Stroke, WidgetInfo};

use volumectl::config::{AppLabelStyle, Language};
use volumectl::ui::{AppAction, AppState, AudioSessionState};

use crate::i18n::{tr, Key};
use crate::style::{self, Tokens};

/// Intent emitted by the mixer view.
#[derive(Debug, Clone, PartialEq)]
pub enum MixerIntent {
    Action(AppAction),
    /// Re-probe the audio backend / session list.
    RetryBackend,
}

/// Rendering options that do not belong to [`AppState`].
pub struct MixerOptions {
    pub language: Language,
    pub is_dark: bool,
    pub green_up_to: u8,
    pub blue_up_to: u8,
    pub app_label_style: AppLabelStyle,
    pub sessions_supported: bool,
    pub error: Option<String>,
}

/// Filtered, sorted session list for rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixerRow {
    pub id: String,
    pub label: String,
    pub hint: Option<String>,
    pub volume: u8,
    pub muted: bool,
    pub system: bool,
}

/// Rows in legacy order: active first, then volume descending, then label.
#[must_use]
pub fn rows(
    state: &AppState,
    query: &str,
    style: AppLabelStyle,
    language: Language,
) -> Vec<MixerRow> {
    let mut sessions: Vec<&AudioSessionState> = state
        .sessions
        .iter()
        .filter(|session| matches_query(session, query))
        .collect();
    sessions.sort_by(|left, right| {
        right
            .active
            .cmp(&left.active)
            .then(right.volume_percent.cmp(&left.volume_percent))
            .then_with(|| row_label(left, style, language).cmp(&row_label(right, style, language)))
    });
    sessions
        .into_iter()
        .map(|session| MixerRow {
            id: session.id.clone(),
            label: row_label(session, style, language),
            hint: session
                .system
                .then(|| tr(Key::MixerSystemSoundsHint, language, &[]).to_string()),
            volume: session.volume_percent,
            muted: session.muted,
            system: session.system,
        })
        .collect()
}

/// Case-insensitive match over name, process, and display name.
#[must_use]
pub fn matches_query(session: &AudioSessionState, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }
    session.name.to_lowercase().contains(&query)
        || session.process.to_lowercase().contains(&query)
        || session
            .display_name
            .as_deref()
            .is_some_and(|name| name.to_lowercase().contains(&query))
}

/// Row label resolved through the configured app-label style.
#[must_use]
pub fn row_label(session: &AudioSessionState, style: AppLabelStyle, language: Language) -> String {
    if session.system {
        return tr(Key::MixerSystemSounds, language, &[]).to_string();
    }
    let friendly = session
        .display_name
        .as_deref()
        .unwrap_or(session.name.as_str());
    let process = if session.process.is_empty() {
        session.name.as_str()
    } else {
        session.process.as_str()
    };
    match style {
        AppLabelStyle::Friendly => friendly.to_string(),
        AppLabelStyle::FriendlyAndProcess if friendly.eq_ignore_ascii_case(process) => {
            friendly.to_string()
        }
        AppLabelStyle::FriendlyAndProcess => format!("{friendly} ({process})"),
        AppLabelStyle::Process => process.to_string(),
    }
}

/// Band color for a volume value (SignalRail semantics).
#[must_use]
pub fn band_color(
    value: u8,
    green_up_to: u8,
    blue_up_to: u8,
    muted: bool,
    tokens: &Tokens,
) -> Color32 {
    if muted {
        tokens.muted
    } else if value < green_up_to {
        tokens.success
    } else if value <= blue_up_to {
        tokens.accent
    } else {
        tokens.warning
    }
}

/// Header status text: confirmed percentage or the muted label.
#[must_use]
pub fn status_text(state: &AppState, language: Language) -> String {
    if state.muted {
        tr(Key::CommonMuted, language, &[]).to_string()
    } else {
        format!(
            "{} {}%",
            tr(Key::CommonVolume, language, &[]),
            state.volume_percent
        )
    }
}

/// Footer summary: `"{n} app sessions"` composed from legacy copy.
#[must_use]
pub fn footer_text(count: usize, language: Language) -> String {
    format!("{count} {}", tr(Key::MixerAppSessions, language, &[]))
}

/// Empty-state copy for the session list.
#[must_use]
pub fn empty_text(has_sessions: bool, query: &str, language: Language) -> String {
    if !has_sessions || query.trim().is_empty() {
        tr(Key::MixerNoSessions, language, &[]).to_string()
    } else {
        tr(Key::MixerNoMatches, language, &[]).to_string()
    }
}

/// Draw the mixer and collect intents. The caller applies `AppAction`s and
/// handles [`MixerIntent::RetryBackend`].
pub fn show(
    ui: &mut egui::Ui,
    state: &AppState,
    query: &mut String,
    options: &MixerOptions,
    intents: &mut Vec<MixerIntent>,
) {
    let language = options.language;
    let tokens = style::tokens(options.is_dark);

    header(ui, state, options, tokens, intents);
    ui.add_space(6.0);
    search(ui, query, tokens, language);
    ui.add_space(6.0);
    system_row(ui, state, options, tokens, intents);
    ui.add_space(6.0);

    if let Some(error) = &options.error {
        ui.horizontal(|ui| {
            ui.colored_label(tokens.danger, error);
            if ui
                .small_button(tr(Key::MixerRetryConnection, language, &[]).to_string())
                .clicked()
            {
                intents.push(MixerIntent::RetryBackend);
            }
        });
    }

    let visible_rows = rows(state, query, options.app_label_style, language);
    let footer_height = 18.0;
    if options.sessions_supported {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height((ui.available_height() - footer_height).max(24.0))
            .show(ui, |ui| {
                if visible_rows.is_empty() {
                    ui.label(
                        RichText::new(empty_text(!state.sessions.is_empty(), query, language))
                            .size(11.0)
                            .color(tokens.muted),
                    );
                } else {
                    for row in &visible_rows {
                        session_row(ui, row, options, tokens, intents);
                    }
                }
            });
    } else {
        ui.label(
            RichText::new(tr(Key::MixerUnsupported, language, &[]))
                .size(11.0)
                .color(tokens.muted),
        );
    }

    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(footer_text(visible_rows.len(), language))
                    .size(10.0)
                    .color(tokens.muted),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(tr(Key::MixerEscToClose, language, &[]))
                        .size(10.0)
                        .color(tokens.muted),
                );
            });
        });
    });
}

fn header(
    ui: &mut egui::Ui,
    state: &AppState,
    options: &MixerOptions,
    tokens: Tokens,
    intents: &mut Vec<MixerIntent>,
) {
    let language = options.language;
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(tr(Key::MixerTitle, language, &[]))
                .size(13.0)
                .strong(),
        );
        ui.label(
            RichText::new(status_text(state, language))
                .size(11.0)
                .color(tokens.muted),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icon_button(
                ui,
                egui_phosphor::regular::X,
                tr(Key::CommonClose, language, &[]),
            )
            .clicked()
            {
                intents.push(MixerIntent::Action(AppAction::HideSurface(
                    volumectl::ui::SurfaceId::Mixer,
                )));
            }
            if icon_button(
                ui,
                egui_phosphor::regular::GEAR,
                tr(Key::SettingsTitle, language, &[]),
            )
            .clicked()
            {
                intents.push(MixerIntent::Action(AppAction::ShowSurface(
                    volumectl::ui::SurfaceId::Settings,
                )));
            }
            if icon_button(
                ui,
                egui_phosphor::regular::QUESTION,
                tr(Key::HelpTitle, language, &[]),
            )
            .clicked()
            {
                intents.push(MixerIntent::Action(AppAction::ShowSurface(
                    volumectl::ui::SurfaceId::Help,
                )));
            }
        });
    });
}

fn search(ui: &mut egui::Ui, query: &mut String, tokens: Tokens, language: Language) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS)
                .size(11.0)
                .color(tokens.muted),
        );
        let edit = egui::TextEdit::singleline(query)
            .hint_text(tr(Key::MixerSearchPlaceholder, language, &[]))
            .desired_width(ui.available_width() - 22.0);
        ui.add(edit);
        if !query.is_empty()
            && icon_button(
                ui,
                egui_phosphor::regular::X,
                tr(Key::MixerClearSearch, language, &[]),
            )
            .clicked()
        {
            query.clear();
        }
    });
}

fn system_row(
    ui: &mut egui::Ui,
    state: &AppState,
    options: &MixerOptions,
    tokens: Tokens,
    intents: &mut Vec<MixerIntent>,
) {
    let language = options.language;
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(104.0, 30.0),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                ui.label(RichText::new(tr(Key::MixerSystemOutput, language, &[])).size(11.0));
                ui.label(
                    RichText::new(if state.muted {
                        tr(Key::CommonMuted, language, &[]).to_string()
                    } else {
                        format!("{}%", state.volume_percent)
                    })
                    .size(9.0)
                    .color(tokens.muted),
                );
            },
        );
        let mut volume = state.volume_percent;
        let width = (ui.available_width() - 56.0).max(80.0);
        let label = tr(Key::MixerSystemOutput, language, &[]).to_string();
        let response = signal_slider(
            ui,
            &mut volume,
            state.muted,
            options.green_up_to,
            options.blue_up_to,
            tokens,
            width,
            &label,
        );
        if response.changed() {
            intents.push(MixerIntent::Action(AppAction::SetVolumePercent {
                percent: u16::from(volume),
            }));
        }
        let mute_icon = if state.muted {
            egui_phosphor::regular::SPEAKER_SLASH
        } else {
            egui_phosphor::regular::SPEAKER_HIGH
        };
        let mute_hint = if state.muted {
            tr(Key::MixerUnmuteSystemOutput, language, &[])
        } else {
            tr(Key::MixerMuteSystemOutput, language, &[])
        };
        if icon_button(ui, mute_icon, mute_hint).clicked() {
            intents.push(MixerIntent::Action(AppAction::ToggleMute));
        }
        if icon_button(
            ui,
            egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE,
            tr(Key::MixerResetVolume, language, &[]),
        )
        .clicked()
        {
            intents.push(MixerIntent::Action(AppAction::ResetVolume));
        }
    });
}

fn session_row(
    ui: &mut egui::Ui,
    row: &MixerRow,
    options: &MixerOptions,
    tokens: Tokens,
    intents: &mut Vec<MixerIntent>,
) {
    let language = options.language;
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(104.0, 34.0),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                ui.add(egui::Label::new(RichText::new(&row.label).size(11.0)).truncate());
                if let Some(hint) = &row.hint {
                    ui.add(
                        egui::Label::new(RichText::new(hint).size(9.0).color(tokens.muted))
                            .truncate(),
                    );
                }
            },
        );
        let mut volume = row.volume;
        let width = (ui.available_width() - 30.0).max(60.0);
        let response = signal_slider(
            ui,
            &mut volume,
            row.muted,
            options.green_up_to,
            options.blue_up_to,
            tokens,
            width,
            &row.label,
        );
        if response.changed() {
            intents.push(MixerIntent::Action(AppAction::SetSessionVolume {
                id: row.id.clone(),
                percent: volume,
            }));
        }
        let icon = if row.muted {
            egui_phosphor::regular::SPEAKER_SLASH
        } else {
            egui_phosphor::regular::SPEAKER_HIGH
        };
        let hint = if row.muted {
            tr(Key::MixerUnmute, language, &[])
        } else {
            tr(Key::MixerMute, language, &[])
        };
        if icon_button(ui, icon, hint).clicked() {
            intents.push(MixerIntent::Action(AppAction::ToggleSessionMute {
                id: row.id.clone(),
            }));
        }
    });
}

fn icon_button(
    ui: &mut egui::Ui,
    icon: &str,
    tooltip: impl Into<egui::WidgetText>,
) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(icon).size(12.0)).frame(false))
        .on_hover_text(tooltip)
}

/// SignalRail-style slider: one value-width fill in the current band color,
/// gray fill plus an outlined-diamond thumb while muted.
#[allow(clippy::too_many_arguments)]
pub fn signal_slider(
    ui: &mut egui::Ui,
    value: &mut u8,
    muted: bool,
    green_up_to: u8,
    blue_up_to: u8,
    tokens: Tokens,
    width: f32,
    label: &str,
) -> egui::Response {
    let (rect, mut response) =
        ui.allocate_exact_size(egui::vec2(width, 18.0), Sense::click_and_drag());
    let track_height = 6.0;
    let track = egui::Rect::from_center_size(
        egui::pos2(rect.center().x, rect.center().y),
        egui::vec2(rect.width(), track_height),
    );
    let radius = egui::CornerRadius::same(3);

    if response.is_pointer_button_down_on() || response.clicked() {
        if let Some(position) = response.interact_pointer_pos() {
            let pct = (((position.x - track.left()) / track.width()) * 100.0)
                .clamp(0.0, 100.0)
                .round() as u8;
            if pct != *value {
                *value = pct;
                response.mark_changed();
            }
        }
    }

    let color = band_color(*value, green_up_to, blue_up_to, muted, &tokens);
    let painter = ui.painter();
    painter.rect_filled(track, radius, tokens.input);
    let fill_width = track.width() * f32::from(*value) / 100.0;
    if fill_width > 0.0 {
        let fill = egui::Rect::from_min_size(track.min, egui::vec2(fill_width, track.height()));
        painter.rect_filled(fill, radius, color);
    }
    let thumb_x = track.left() + fill_width;
    let thumb_center = egui::pos2(thumb_x, track.center().y);
    if muted {
        let r = 5.0;
        let points = vec![
            egui::pos2(thumb_center.x, thumb_center.y - r),
            egui::pos2(thumb_center.x + r, thumb_center.y),
            egui::pos2(thumb_center.x, thumb_center.y + r),
            egui::pos2(thumb_center.x - r, thumb_center.y),
        ];
        painter.add(Shape::closed_line(points, Stroke::new(1.5, tokens.muted)));
    } else {
        painter.circle_filled(thumb_center, 5.0, color);
        painter.circle_stroke(thumb_center, 5.0, Stroke::new(1.0, tokens.background));
    }

    response.widget_info(|| WidgetInfo::slider(true, f64::from(*value), label));
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footer_uses_legacy_session_copy() {
        assert_eq!(footer_text(3, Language::Vi), "3 phiên ứng dụng");
        assert_eq!(footer_text(1, Language::En), "1 app sessions");
    }

    #[test]
    fn empty_states_distinguish_no_sessions_from_no_matches() {
        assert_eq!(
            empty_text(false, "", Language::Vi),
            "Không có phiên âm thanh"
        );
        assert_eq!(empty_text(true, "zz", Language::En), "No matching apps");
    }
}
