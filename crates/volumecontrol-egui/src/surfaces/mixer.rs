//! Mixer surface: legacy v0.1.11 parity (header, search, system row, session
//! rows, footer) on egui primitives.

use eframe::egui;
use egui::{Color32, RichText, Sense, Shape, Stroke, WidgetInfo};

use volumectl::config::{AppLabelStyle, Language};
use volumectl::ui::{AppAction, AppState, AudioSessionState, SurfaceId};

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
    /// Update banner copy (available/downloading/ready), localized by the host.
    pub banner: Option<String>,
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

/// Speaker glyph for the current mute level: muted, low, medium, high.
#[must_use]
pub fn speaker_icon(volume: u8, muted: bool) -> &'static str {
    if muted {
        egui_phosphor::regular::SPEAKER_SLASH
    } else if volume == 0 {
        egui_phosphor::regular::SPEAKER_NONE
    } else if volume <= 33 {
        egui_phosphor::regular::SPEAKER_SIMPLE_LOW
    } else if volume <= 66 {
        egui_phosphor::regular::SPEAKER_LOW
    } else {
        egui_phosphor::regular::SPEAKER_HIGH
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
    let visible_rows = rows(state, query, options.app_label_style, language);

    egui::Panel::bottom("mixer-footer")
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            ui.painter().hline(
                ui.max_rect().x_range(),
                ui.max_rect().top(),
                egui::Stroke::new(1.0, tokens.border),
            );
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(footer_text(visible_rows.len(), language))
                        .small()
                        .color(tokens.muted),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(tr(Key::MixerEscToClose, language, &[]))
                            .small()
                            .color(tokens.muted),
                    );
                });
            });
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            header(ui, state, options, tokens, intents);
            ui.add_space(style::SPACE_1);
            search(ui, query, tokens, language);
            ui.add_space(style::SPACE_2);
            system_row(ui, state, options, tokens, intents);
            ui.add_space(style::SPACE_2);

            if let Some(banner) = &options.banner {
                egui::Frame::NONE
                    .fill(tokens.accent.gamma_multiply(0.18))
                    .corner_radius(egui::CornerRadius::same(style::RADIUS_CONTROL))
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(banner).small());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .small_button(
                                            tr(Key::SettingsUpdatesViewUpdate, language, &[])
                                                .to_string(),
                                        )
                                        .clicked()
                                    {
                                        intents.push(MixerIntent::Action(AppAction::ShowSurface(
                                            SurfaceId::Settings,
                                        )));
                                    }
                                },
                            );
                        });
                    });
                ui.add_space(style::SPACE_2);
            }

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

            if options.sessions_supported {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height(ui.available_height().max(24.0))
                    .scroll_bar_visibility(
                        egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                    )
                    .show(ui, |ui| {
                        if visible_rows.is_empty() {
                            ui.label(
                                RichText::new(empty_text(
                                    !state.sessions.is_empty(),
                                    query,
                                    language,
                                ))
                                .small()
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
                        .small()
                        .color(tokens.muted),
                );
            }
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
                egui_phosphor::regular::QUESTION,
                tr(Key::HelpTitle, language, &[]),
            )
            .clicked()
            {
                intents.push(MixerIntent::Action(AppAction::ShowSurface(
                    volumectl::ui::SurfaceId::Help,
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
    egui::Frame::NONE
        .fill(tokens.card)
        .stroke(egui::Stroke::new(1.0, tokens.border))
        .corner_radius(egui::CornerRadius::same(style::RADIUS_CONTROL))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(tr(Key::MixerSystemOutput, language, &[])).strong());
                ui.label(
                    RichText::new(if state.muted {
                        tr(Key::CommonMuted, language, &[]).to_string()
                    } else {
                        format!("{}%", state.volume_percent)
                    })
                    .small()
                    .color(tokens.muted),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_button(
                        ui,
                        egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE,
                        tr(Key::MixerResetVolume, language, &[]),
                    )
                    .clicked()
                    {
                        intents.push(MixerIntent::Action(AppAction::ResetVolume));
                    }
                    let mute_hint = if state.muted {
                        tr(Key::MixerUnmuteSystemOutput, language, &[])
                    } else {
                        tr(Key::MixerMuteSystemOutput, language, &[])
                    };
                    if icon_button(
                        ui,
                        speaker_icon(state.volume_percent, state.muted),
                        mute_hint,
                    )
                    .clicked()
                    {
                        intents.push(MixerIntent::Action(AppAction::ToggleMute));
                    }
                });
            });
            ui.add_space(2.0);
            let mut volume = state.volume_percent;
            let width = ui.available_width();
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
    egui::Frame::NONE
        .fill(tokens.card)
        .stroke(egui::Stroke::new(1.0, tokens.border))
        .corner_radius(egui::CornerRadius::same(style::RADIUS_CONTROL))
        .inner_margin(egui::Margin::symmetric(10, 2))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(
                        style::MIXER_SESSION_LABEL_WIDTH,
                        style::MIXER_SESSION_ROW_HEIGHT,
                    ),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.add(egui::Label::new(RichText::new(&row.label)).truncate());
                        if let Some(hint) = &row.hint {
                            ui.add(
                                egui::Label::new(RichText::new(hint).small().color(tokens.muted))
                                    .truncate(),
                            );
                        }
                    },
                );
                let mut volume = row.volume;
                let controls_width = 34.0;
                let width = (ui.available_width() - controls_width).max(60.0);
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
                let hint = if row.muted {
                    tr(Key::MixerUnmute, language, &[])
                } else {
                    tr(Key::MixerMute, language, &[])
                };
                if icon_button(ui, speaker_icon(row.volume, row.muted), hint).clicked() {
                    intents.push(MixerIntent::Action(AppAction::ToggleSessionMute {
                        id: row.id.clone(),
                    }));
                }
            });
        });
    ui.add_space(4.0);
}

fn icon_button(
    ui: &mut egui::Ui,
    icon: &str,
    tooltip: impl Into<egui::WidgetText>,
) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(icon).size(12.0)).frame(false))
        .on_hover_text(tooltip)
}

/// Draw the SignalRail track/fill/thumb into `track`.
pub fn paint_rail(
    painter: &egui::Painter,
    track: egui::Rect,
    value: u8,
    muted: bool,
    green_up_to: u8,
    blue_up_to: u8,
    tokens: &Tokens,
) {
    let color = band_color(value, green_up_to, blue_up_to, muted, tokens);
    painter.rect_filled(track, egui::CornerRadius::same(3), tokens.input);
    let fill_width = track.width() * f32::from(value) / 100.0;
    if fill_width > 0.0 {
        let fill = egui::Rect::from_min_size(track.min, egui::vec2(fill_width, track.height()));
        painter.rect_filled(fill, egui::CornerRadius::same(3), color);
    }
    let thumb_center = egui::pos2(track.left() + fill_width, track.center().y);
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

    paint_rail(
        ui.painter(),
        track,
        *value,
        muted,
        green_up_to,
        blue_up_to,
        &tokens,
    );
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
