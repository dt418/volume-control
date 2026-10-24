//! Settings surface: six-tab chrome, draft editors, picker, and update card.

use eframe::egui;
use egui::{Align, Layout, RichText};

use volumectl::config::{
    AppLabelStyle, ConfigLoadNotice, HotkeyBindings, HotkeyModifier, Language,
};
use volumectl::host_core::RunningApp;
use volumectl::hotkeys::{HotkeyAction, HotkeyRegResult, HotkeyRegStatus};
use volumectl::ui::{AccentMode, MaterialMode, MotionMode, ThemeMode};

use crate::i18n::{tr, Key};
use crate::settings::blacklist;
use crate::settings::model::{self, SettingsDraft, Tab};
use crate::settings::recorder::{KeyEvent, Recorder, RecorderAction, RecorderEffect};
use crate::settings::update_card::{UpdateAction, UpdateCard};
use crate::style;

/// Intent emitted by the Settings view.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingsIntent {
    Save,
    Cancel,
    Reset,
    ApplyPreset(HotkeyModifier),
    AddBlacklist(String),
    RemoveBlacklist(String),
    ToggleBlacklist(String),
    ClearBlacklist,
    ApplyRecommended,
    RestoreDefaults,
    ReloadSaved,
    RefreshRunningApps,
    OpenConfig,
    CheckUpdates,
    DownloadUpdate,
    InstallUpdate,
    DismissUpdate,
}

/// State of the running-app picker panel.
#[derive(Debug, Default)]
pub struct PickerState {
    pub open: bool,
    pub query: String,
    pub loading: bool,
    pub error: Option<String>,
    pub anchor: Option<egui::Pos2>,
}

pub struct SettingsView<'a> {
    pub language: Language,
    pub is_dark: bool,
    pub draft: &'a mut SettingsDraft,
    pub tab: &'a mut Tab,
    pub error: Option<&'a model::FieldError>,
    pub recorder: &'a mut Recorder,
    pub picker: &'a mut PickerState,
    pub running_apps: &'a [RunningApp],
    pub update: &'a UpdateCard,
    pub autostart_supported: bool,
    pub config_path: String,
    pub config_notice: Option<&'a str>,
    pub saved_blacklist: &'a [String],
    pub modifier: HotkeyModifier,
    pub hotkey_status: &'a [HotkeyRegResult],
}

pub fn show(ui: &mut egui::Ui, view: &mut SettingsView<'_>) -> Vec<SettingsIntent> {
    let mut intents = Vec::new();
    // One control rhythm for every tab: equal heights and 6 px vertical gaps.
    ui.spacing_mut().interact_size.y = style::BUTTON_HEIGHT;
    ui.spacing_mut().item_spacing.y = style::SPACE_3;
    if view.recorder.active().is_some() {
        capture_keys(ui, view.draft, view.recorder);
    }
    egui::Panel::top("settings-header")
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            header(ui, view, &mut intents);
        });
    egui::Panel::bottom("settings-footer")
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            footer(ui, view, &mut intents);
        });
    egui::Panel::left("settings-sidebar")
        .default_size(196.0)
        .size_range(196.0..=196.0)
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            sidebar(ui, view);
        });
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            let tokens = style::tokens(view.is_dark);
            egui::Frame::NONE
                .fill(tokens.card)
                .corner_radius(egui::CornerRadius::same(style::RADIUS_CARD))
                .inner_margin(egui::Margin::same(16))
                .show(ui, |ui| {
                    ui.set_min_size(ui.available_size());
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .scroll_bar_visibility(
                            egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                        )
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.set_width(ui.available_width());
                                match *view.tab {
                                    Tab::General => general_tab(ui, view, &mut intents),
                                    Tab::Hotkeys => hotkeys_tab(ui, view, &mut intents),
                                    Tab::Appearance => appearance_tab(ui, view),
                                    Tab::Blacklist => blacklist_tab(ui, view, &mut intents),
                                    Tab::Feedback => feedback_tab(ui, view),
                                    Tab::Storage => storage_tab(ui, view, &mut intents),
                                }
                            });
                        });
                });
        });
    intents
}

fn header(ui: &mut egui::Ui, view: &mut SettingsView<'_>, intents: &mut Vec<SettingsIntent>) {
    let tokens = style::tokens(view.is_dark);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(tr(Key::SettingsTitle, view.language, &[])).strong());
            ui.label(
                RichText::new(tr(Key::SettingsSubtitle, view.language, &[]))
                    .small()
                    .color(tokens.muted),
            );
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .add(
                    egui::Button::new(RichText::new(egui_phosphor::regular::X).size(14.0))
                        .frame(false),
                )
                .on_hover_text(tr(Key::CommonClose, view.language, &[]).to_string())
                .clicked()
            {
                intents.push(SettingsIntent::Cancel);
            }
        });
    });
}

fn sidebar(ui: &mut egui::Ui, view: &mut SettingsView<'_>) {
    let tokens = style::tokens(view.is_dark);
    ui.allocate_ui_with_layout(
        egui::vec2(180.0, ui.available_height()),
        Layout::top_down(Align::Min),
        |ui| {
            for tab in Tab::ALL {
                let selected = *view.tab == tab;
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 36.0),
                    egui::Sense::click(),
                );
                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                let fill = if selected || response.hovered() {
                    tokens.input
                } else {
                    egui::Color32::TRANSPARENT
                };
                ui.painter().rect_filled(
                    rect,
                    egui::CornerRadius::same(style::RADIUS_CONTROL),
                    fill,
                );
                let text_color = if selected { tokens.text } else { tokens.muted };
                ui.painter().text(
                    rect.left_center() + egui::vec2(14.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    tr(tab_key(tab), view.language, &[]).to_string(),
                    egui::FontId::proportional(ui.style().text_styles[&egui::TextStyle::Body].size),
                    if selected { tokens.text } else { text_color },
                );
                if response.clicked() {
                    *view.tab = tab;
                }
            }
        },
    );
}

fn general_tab(ui: &mut egui::Ui, view: &mut SettingsView<'_>, intents: &mut Vec<SettingsIntent>) {
    language_combo(ui, view);
    label_style_combo(ui, view);
    let language = view.language;
    let is_dark = view.is_dark;
    let error = view.error;
    let mut font_size = u32::from(view.draft.ui_font_size);
    numeric_row(
        ui,
        language,
        is_dark,
        error,
        Key::SettingsFontSize,
        Key::SettingsFontSizeHelp,
        "ui_font_size",
        &mut font_size,
        9..=24,
    );
    view.draft.ui_font_size = font_size as u8;
    numeric_row(
        ui,
        language,
        is_dark,
        error,
        Key::SettingsVolumeStep,
        Key::SettingsVolumeStepHelp,
        "volume_step",
        &mut view.draft.volume_step,
        1..=50,
    );
    numeric_row(
        ui,
        language,
        is_dark,
        error,
        Key::SettingsVolumeStepLarge,
        Key::SettingsVolumeStepLargeHelp,
        "volume_step_large",
        &mut view.draft.volume_step_large,
        1..=50,
    );
    let mut duration = view.draft.overlay_duration_ms as u32;
    numeric_row(
        ui,
        language,
        is_dark,
        error,
        Key::SettingsOverlayDuration,
        Key::SettingsOverlayDurationHelp,
        "overlay_duration_ms",
        &mut duration,
        200..=10_000,
    );
    view.draft.overlay_duration_ms = u64::from(duration);
    let mut autostart = view.draft.autostart;
    ui.horizontal(|ui| {
        ui.add_enabled(
            view.autostart_supported,
            egui::Checkbox::new(
                &mut autostart,
                tr(Key::SettingsAutostart, view.language, &[]).to_string(),
            ),
        );
    });
    view.draft.autostart = autostart;
    if !view.autostart_supported {
        ui.small(tr(Key::SettingsAutostartUnavailable, view.language, &[]).to_string());
    }
    ui.add_space(10.0);
    update_card(ui, view, intents);
}

fn hotkeys_tab(ui: &mut egui::Ui, view: &mut SettingsView<'_>, intents: &mut Vec<SettingsIntent>) {
    let tokens = style::tokens(view.is_dark);
    ui.label(RichText::new(tr(Key::SettingsKeyboard, view.language, &[])).strong());
    ui.label(
        RichText::new(tr(Key::HotkeysSubtitle, view.language, &[]))
            .small()
            .color(tokens.muted),
    );
    ui.add_space(10.0);
    for action in RecorderAction::ALL {
        egui::Frame::NONE
            .stroke(egui::Stroke::new(1.0, tokens.border))
            .corner_radius(egui::CornerRadius::same(style::RADIUS_CONTROL))
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let available = ui.available_width();
                    let button_width = 50.0;
                    let label_width = (available * 0.32).clamp(140.0, 180.0);
                    let chip_width = (available - label_width - 2.0 * button_width - 24.0)
                        .max(style::HOTKEY_CHIP_MIN_WIDTH);
                    ui.allocate_ui_with_layout(
                        egui::vec2(label_width, 34.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.label(tr(action.label(), view.language, &[]).to_string());
                            ui.label(
                                RichText::new(tr(action.help(), view.language, &[]))
                                    .small()
                                    .color(tokens.muted),
                            );
                        },
                    );
                    let recording = view.recorder.active() == Some(action);
                    let binding = action.binding(&view.draft.hotkeys);
                    let display = if recording {
                        tr(Key::RecorderPressShortcut, view.language, &[]).to_string()
                    } else if binding.is_empty() {
                        tr(Key::RecorderNotSet, view.language, &[]).to_string()
                    } else {
                        binding.to_string()
                    };
                    if ui
                        .add_sized(
                            [chip_width, 28.0],
                            egui::Button::new(RichText::new(display).monospace().small())
                                .frame(true)
                                .min_size(egui::vec2(chip_width, 28.0)),
                        )
                        .clicked()
                    {
                        view.recorder.start(action);
                    }
                    if ui
                        .add_sized(
                            [button_width, 28.0],
                            egui::Button::new(
                                tr(Key::RecorderRecord, view.language, &[]).to_string(),
                            )
                            .min_size(egui::vec2(button_width, 28.0)),
                        )
                        .clicked()
                    {
                        view.recorder.start(action);
                    }
                    if ui
                        .add_enabled(
                            !binding.is_empty(),
                            egui::Button::new(
                                tr(Key::RecorderClear, view.language, &[]).to_string(),
                            )
                            .min_size(egui::vec2(button_width, 28.0)),
                        )
                        .clicked()
                    {
                        action.set_binding(&mut view.draft.hotkeys, String::new());
                    }
                });
                if hotkey_conflicted(action, view.hotkey_status) {
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(tr(Key::HotkeyConflictBadge, view.language, &[]))
                            .small()
                            .color(tokens.danger),
                    );
                }
            });
        ui.add_space(6.0);
    }
    if let Some(hint) = view.recorder.hint() {
        ui.colored_label(tokens.muted, hint.text(view.language));
    }
    ui.add_space(8.0);
    ui.add_space(4.0);
    ui.label(RichText::new(tr(Key::HotkeysPresets, view.language, &[])).strong());
    ui.horizontal(|ui| {
        for modifier in model::MODIFIER_PRESETS {
            let enabled = modifier != HotkeyModifier::CapsLock;
            let label = modifier_label(modifier, view.language);
            if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                intents.push(SettingsIntent::ApplyPreset(modifier));
            }
        }
    });
    ui.label(
        RichText::new(tr(Key::HelpCapsFallback, view.language, &[]))
            .small()
            .color(tokens.muted),
    );
}

fn appearance_tab(ui: &mut egui::Ui, view: &mut SettingsView<'_>) {
    theme_combo(ui, view);
    material_combo(ui, view);
    motion_combo(ui, view);
    accent_combo(ui, view);
    ui.add_space(8.0);
    preview_card(ui, view);
    ui.add_space(8.0);
    ui.label(
        RichText::new(tr(Key::ThresholdHeading, view.language, &[]))
            .size(11.0)
            .strong(),
    );
    let language = view.language;
    let is_dark = view.is_dark;
    let error = view.error;
    numeric_u8_row(
        ui,
        language,
        is_dark,
        error,
        Key::ThresholdGreen,
        "color_thresholds.green_up_to",
        &mut view.draft.color_thresholds.green_up_to,
    );
    numeric_u8_row(
        ui,
        language,
        is_dark,
        error,
        Key::ThresholdBlue,
        "color_thresholds.blue_up_to",
        &mut view.draft.color_thresholds.blue_up_to,
    );
    numeric_u8_row(
        ui,
        language,
        is_dark,
        error,
        Key::ThresholdOrange,
        "color_thresholds.orange_up_to",
        &mut view.draft.color_thresholds.orange_up_to,
    );
}

fn blacklist_tab(
    ui: &mut egui::Ui,
    view: &mut SettingsView<'_>,
    intents: &mut Vec<SettingsIntent>,
) {
    ui.heading(tr(Key::SettingsBlacklist, view.language, &[]).to_string());
    ui.small(tr(Key::BlacklistSubtitle, view.language, &[]).to_string());
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let mut entry = String::new();
        let field_width = (ui.available_width() - 68.0).max(140.0);
        let response = ui.add_sized(
            [field_width, style::INPUT_HEIGHT],
            egui::TextEdit::singleline(&mut entry).hint_text(tr(
                Key::BlacklistManualPlaceholder,
                view.language,
                &[],
            )),
        );
        let submit = response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        let clicked = ui
            .add_sized(
                [56.0, style::INPUT_HEIGHT],
                egui::Button::new(tr(Key::BlacklistAdd, view.language, &[]).to_string()),
            )
            .clicked();
        if (clicked || submit) && !entry.trim().is_empty() {
            intents.push(SettingsIntent::AddBlacklist(entry.clone()));
        }
    });
    ui.horizontal_wrapped(|ui| {
        let choose = ui.add_sized(
            [180.0, style::INPUT_HEIGHT],
            egui::Button::new(tr(Key::BlacklistChooseApps, view.language, &[]).to_string()),
        );
        let anchor = choose.rect.left_bottom() + egui::vec2(0.0, 4.0);
        if choose.clicked() {
            view.picker.open = !view.picker.open;
            if view.picker.open {
                view.picker.anchor = Some(anchor);
                view.picker.loading = true;
                intents.push(SettingsIntent::RefreshRunningApps);
            } else {
                view.picker.anchor = None;
            }
        }
        for (label, intent) in [
            (
                Key::BlacklistApplyRecommended,
                SettingsIntent::ApplyRecommended,
            ),
            (
                Key::BlacklistRestoreDefaults,
                SettingsIntent::RestoreDefaults,
            ),
            (Key::BlacklistReloadSaved, SettingsIntent::ReloadSaved),
            (Key::BlacklistClear, SettingsIntent::ClearBlacklist),
        ] {
            let enabled = intent != SettingsIntent::ReloadSaved
                || view.draft.blacklist != view.saved_blacklist;
            if ui
                .add_enabled(
                    enabled,
                    egui::Button::new(tr(label, view.language, &[]).to_string()),
                )
                .clicked()
            {
                intents.push(intent);
            }
        }
    });
    if view.picker.open {
        picker_panel(ui, view, intents);
    }
    ui.add_space(6.0);
    ui.small(tr(Key::BlacklistUnsavedHint, view.language, &[]).to_string());
    let tokens = style::tokens(view.is_dark);
    let mut remove: Option<String> = None;
    if view.draft.blacklist.is_empty() {
        ui.add_space(8.0);
        ui.label(
            RichText::new(tr(Key::BlacklistEmpty, view.language, &[]))
                .size(12.0)
                .strong(),
        );
        ui.label(
            RichText::new(tr(Key::BlacklistEmptyHint, view.language, &[]))
                .size(10.0)
                .color(tokens.muted),
        );
    } else {
        egui::ScrollArea::vertical()
            .max_height(ui.available_height() - 24.0)
            .show(ui, |ui| {
                for entry in &view.draft.blacklist {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(entry).size(11.0));
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .small_button(
                                    tr(Key::BlacklistRemove, view.language, &[]).to_string(),
                                )
                                .clicked()
                            {
                                remove = Some(entry.clone());
                            }
                        });
                    });
                }
            });
    }
    if let Some(entry) = remove {
        intents.push(SettingsIntent::RemoveBlacklist(entry));
    }
}

fn picker_panel(ui: &mut egui::Ui, view: &mut SettingsView<'_>, intents: &mut Vec<SettingsIntent>) {
    let Some(anchor) = view.picker.anchor else {
        return;
    };
    let tokens = style::tokens(view.is_dark);
    egui::Area::new(egui::Id::new("running-app-picker"))
        .order(egui::Order::Foreground)
        .fixed_pos(anchor)
        .show(ui.ctx(), |ui| {
            egui::Frame::NONE
                .fill({
                    let card = tokens.card;
                    egui::Color32::from_rgb(card.r(), card.g(), card.b())
                })
                .stroke(egui::Stroke::new(1.0, tokens.border))
                .corner_radius(egui::CornerRadius::same(style::RADIUS_CARD))
                .inner_margin(egui::Margin::same(10))
                .show(ui, |ui| {
                    ui.set_width(style::PICKER_WIDTH);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(tr(Key::BlacklistAppPicker, view.language, &[])).strong(),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .small_button(
                                    tr(Key::BlacklistDone, view.language, &[]).to_string(),
                                )
                                .clicked()
                            {
                                view.picker.open = false;
                                view.picker.anchor = None;
                            }
                            if ui
                                .small_button(
                                    tr(Key::BlacklistRefresh, view.language, &[]).to_string(),
                                )
                                .clicked()
                            {
                                view.picker.loading = true;
                                intents.push(SettingsIntent::RefreshRunningApps);
                            }
                        });
                    });
                    ui.add(
                        egui::TextEdit::singleline(&mut view.picker.query).hint_text(tr(
                            Key::BlacklistSearchApps,
                            view.language,
                            &[],
                        )),
                    );
                    if view.picker.loading {
                        ui.small(tr(Key::BlacklistLoadingApps, view.language, &[]).to_string());
                        return;
                    }
                    if let Some(error) = &view.picker.error {
                        ui.colored_label(tokens.danger, error);
                    }
                    let query = view.picker.query.to_lowercase();
                    let mut matched = 0;
                    egui::ScrollArea::vertical()
                        .max_height(style::PICKER_HEIGHT)
                        .show(ui, |ui| {
                            for app in view.running_apps {
                                let haystack = format!(
                                    "{} {} {}",
                                    app.process,
                                    app.title,
                                    app.display_name.as_deref().unwrap_or("")
                                )
                                .to_lowercase();
                                if !query.is_empty() && !haystack.contains(&query) {
                                    continue;
                                }
                                matched += 1;
                                let mut checked =
                                    blacklist::is_blocked(&view.draft.blacklist, &app.process);
                                let name = app.display_name.as_deref().unwrap_or(&app.title);
                                ui.horizontal(|ui| {
                                    if let Some(texture) = running_app_icon(ui, app) {
                                        ui.add(egui::Image::new(egui::load::SizedTexture::new(
                                            texture.id(),
                                            egui::vec2(16.0, 16.0),
                                        )));
                                    }
                                    if ui
                                        .checkbox(&mut checked, format!("{name} ({})", app.process))
                                        .changed()
                                    {
                                        intents.push(SettingsIntent::ToggleBlacklist(
                                            app.process.clone(),
                                        ));
                                    }
                                });
                            }
                            if matched == 0 {
                                ui.small(
                                    tr(
                                        if view.running_apps.is_empty() {
                                            Key::BlacklistNoRunningApps
                                        } else {
                                            Key::BlacklistNoAppMatches
                                        },
                                        view.language,
                                        &[],
                                    )
                                    .to_string(),
                                );
                            }
                        });
                    ui.small(format!(
                        "{}: {}",
                        tr(Key::BlacklistSelected, view.language, &[]),
                        view.draft.blacklist.len()
                    ));
                });
        });
}

fn feedback_tab(ui: &mut egui::Ui, view: &mut SettingsView<'_>) {
    ui.checkbox(
        &mut view.draft.beep.enabled,
        tr(Key::FeedbackBeepEnabled, view.language, &[]).to_string(),
    );
    let language = view.language;
    let is_dark = view.is_dark;
    let error = view.error;
    numeric_beep_row(
        ui,
        language,
        is_dark,
        error,
        Key::FeedbackBlockedFreq,
        Key::FeedbackBlockedFreqHelp,
        "beep.blocked_freq",
        &mut view.draft.beep.blocked_freq,
        37..=32_767,
    );
    numeric_beep_row(
        ui,
        language,
        is_dark,
        error,
        Key::FeedbackBlockedDuration,
        Key::FeedbackBlockedDurationHelp,
        "beep.blocked_duration_ms",
        &mut view.draft.beep.blocked_duration_ms,
        10..=2_000,
    );
    numeric_beep_row(
        ui,
        language,
        is_dark,
        error,
        Key::FeedbackLimitFreq,
        Key::FeedbackLimitFreqHelp,
        "beep.limit_freq",
        &mut view.draft.beep.limit_freq,
        37..=32_767,
    );
    numeric_beep_row(
        ui,
        language,
        is_dark,
        error,
        Key::FeedbackLimitDuration,
        Key::FeedbackLimitDurationHelp,
        "beep.limit_duration_ms",
        &mut view.draft.beep.limit_duration_ms,
        10..=2_000,
    );
}

fn storage_tab(ui: &mut egui::Ui, view: &mut SettingsView<'_>, intents: &mut Vec<SettingsIntent>) {
    let tokens = style::tokens(view.is_dark);
    egui::Frame::NONE
        .fill(tokens.card)
        .stroke(egui::Stroke::new(1.0, tokens.border))
        .corner_radius(egui::CornerRadius::same(style::RADIUS_CARD))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.small(tr(Key::StorageUiOnly, view.language, &[]).to_string());
            ui.add_space(4.0);
            ui.label(
                RichText::new(&view.config_path)
                    .monospace()
                    .size(10.0)
                    .color(tokens.muted),
            );
            if ui
                .button(tr(Key::SettingsOpenConfig, view.language, &[]).to_string())
                .clicked()
            {
                intents.push(SettingsIntent::OpenConfig);
            }
            ui.small(tr(Key::StorageEditingWarning, view.language, &[]).to_string());
            if let Some(notice) = view.config_notice {
                ui.colored_label(tokens.warning, notice);
            }
        });
}

fn update_card(ui: &mut egui::Ui, view: &mut SettingsView<'_>, intents: &mut Vec<SettingsIntent>) {
    let tokens = style::tokens(view.is_dark);
    egui::Frame::NONE
        .fill(tokens.card)
        .stroke(egui::Stroke::new(1.0, tokens.border))
        .corner_radius(egui::CornerRadius::same(style::RADIUS_CARD))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.label(
                RichText::new(tr(Key::SettingsUpdatesTitle, view.language, &[]))
                    .size(12.0)
                    .strong(),
            );
            ui.small(tr(Key::SettingsUpdatesSubtitle, view.language, &[]).to_string());
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    view.update
                        .phase
                        .status_text(view.language, &view.update.current_version),
                )
                .size(11.0),
            );
            if let Some(action) = view.update.phase.action() {
                let (label, intent) = match action {
                    UpdateAction::Check => {
                        (Key::SettingsUpdatesCheckBtn, SettingsIntent::CheckUpdates)
                    }
                    UpdateAction::Download => {
                        (Key::SettingsUpdatesInstall, SettingsIntent::DownloadUpdate)
                    }
                    UpdateAction::ConfirmInstall => (
                        Key::SettingsUpdatesConfirmInstall,
                        SettingsIntent::InstallUpdate,
                    ),
                    UpdateAction::Retry => {
                        (Key::SettingsUpdatesRetry, SettingsIntent::CheckUpdates)
                    }
                };
                ui.horizontal(|ui| {
                    if ui
                        .button(tr(label, view.language, &[]).to_string())
                        .clicked()
                    {
                        intents.push(intent);
                    }
                    if ui
                        .button(tr(Key::SettingsUpdatesLater, view.language, &[]).to_string())
                        .clicked()
                    {
                        intents.push(SettingsIntent::DismissUpdate);
                    }
                });
            }
        });
}

fn footer(ui: &mut egui::Ui, view: &mut SettingsView<'_>, intents: &mut Vec<SettingsIntent>) {
    let pristine = view.draft.is_pristine();
    ui.add_space(6.0);
    ui.separator();
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                !pristine,
                egui::Button::new(tr(Key::CommonReset, view.language, &[]).to_string()),
            )
            .clicked()
        {
            intents.push(SettingsIntent::Reset);
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .add_enabled(
                    !pristine,
                    egui::Button::new(
                        RichText::new(tr(Key::CommonSaveChanges, view.language, &[])).strong(),
                    )
                    .fill(style::tokens(view.is_dark).accent),
                )
                .clicked()
            {
                intents.push(SettingsIntent::Save);
            }
            if ui
                .button(tr(Key::CommonCancel, view.language, &[]).to_string())
                .clicked()
            {
                intents.push(SettingsIntent::Cancel);
            }
        });
    });
}

fn capture_keys(ui: &egui::Ui, draft: &mut SettingsDraft, recorder: &mut Recorder) {
    let events = ui.input(|input| input.events.clone());
    for event in events {
        let egui::Event::Key {
            key,
            pressed: true,
            repeat: false,
            modifiers,
            ..
        } = event
        else {
            continue;
        };
        let key_event = match key {
            egui::Key::Escape => KeyEvent::Escape,
            egui::Key::Backspace | egui::Key::Delete => KeyEvent::Backspace,
            other => KeyEvent::Key {
                code: key_code(other).to_string(),
                ctrl: modifiers.ctrl,
                alt: modifiers.alt,
                shift: modifiers.shift,
                meta: modifiers.command,
            },
        };
        match recorder.feed(key_event) {
            RecorderEffect::Captured { action, binding } => {
                action.set_binding(&mut draft.hotkeys, binding);
            }
            RecorderEffect::Cleared { action } => {
                action.set_binding(&mut draft.hotkeys, String::new());
            }
            RecorderEffect::NeedModifier | RecorderEffect::Waiting | RecorderEffect::Cancelled => {}
        }
    }
}

/// Portable `Code` spelling for the portable global-hotkey format.
fn key_code(key: egui::Key) -> &'static str {
    match key {
        egui::Key::ArrowUp => "ArrowUp",
        egui::Key::ArrowDown => "ArrowDown",
        egui::Key::ArrowLeft => "ArrowLeft",
        egui::Key::ArrowRight => "ArrowRight",
        egui::Key::Backspace => "Backspace",
        egui::Key::Delete => "Delete",
        egui::Key::Enter => "Enter",
        egui::Key::Space => "Space",
        egui::Key::Tab => "Tab",
        egui::Key::Home => "Home",
        egui::Key::End => "End",
        egui::Key::PageUp => "PageUp",
        egui::Key::PageDown => "PageDown",
        egui::Key::Insert => "Insert",
        egui::Key::A => "KeyA",
        egui::Key::B => "KeyB",
        egui::Key::C => "KeyC",
        egui::Key::D => "KeyD",
        egui::Key::E => "KeyE",
        egui::Key::F => "KeyF",
        egui::Key::G => "KeyG",
        egui::Key::H => "KeyH",
        egui::Key::I => "KeyI",
        egui::Key::J => "KeyJ",
        egui::Key::K => "KeyK",
        egui::Key::L => "KeyL",
        egui::Key::M => "KeyM",
        egui::Key::N => "KeyN",
        egui::Key::O => "KeyO",
        egui::Key::P => "KeyP",
        egui::Key::Q => "KeyQ",
        egui::Key::R => "KeyR",
        egui::Key::S => "KeyS",
        egui::Key::T => "KeyT",
        egui::Key::U => "KeyU",
        egui::Key::V => "KeyV",
        egui::Key::W => "KeyW",
        egui::Key::X => "KeyX",
        egui::Key::Y => "KeyY",
        egui::Key::Z => "KeyZ",
        egui::Key::Num0 => "Digit0",
        egui::Key::Num1 => "Digit1",
        egui::Key::Num2 => "Digit2",
        egui::Key::Num3 => "Digit3",
        egui::Key::Num4 => "Digit4",
        egui::Key::Num5 => "Digit5",
        egui::Key::Num6 => "Digit6",
        egui::Key::Num7 => "Digit7",
        egui::Key::Num8 => "Digit8",
        egui::Key::Num9 => "Digit9",
        egui::Key::F1 => "F1",
        egui::Key::F2 => "F2",
        egui::Key::F3 => "F3",
        egui::Key::F4 => "F4",
        egui::Key::F5 => "F5",
        egui::Key::F6 => "F6",
        egui::Key::F7 => "F7",
        egui::Key::F8 => "F8",
        egui::Key::F9 => "F9",
        egui::Key::F10 => "F10",
        egui::Key::F11 => "F11",
        egui::Key::F12 => "F12",
        _ => "Unidentified",
    }
}

fn tab_key(tab: Tab) -> Key {
    match tab {
        Tab::General => Key::SettingsGeneral,
        Tab::Hotkeys => Key::SettingsHotkeys,
        Tab::Appearance => Key::SettingsAppearance,
        Tab::Blacklist => Key::SettingsBlacklist,
        Tab::Feedback => Key::SettingsFeedback,
        Tab::Storage => Key::SettingsStorage,
    }
}

fn modifier_label(modifier: HotkeyModifier, language: Language) -> String {
    let _ = language;
    match modifier {
        HotkeyModifier::CtrlAlt => "Ctrl + Alt",
        HotkeyModifier::Alt => "Alt",
        HotkeyModifier::Ctrl => "Ctrl",
        HotkeyModifier::CapsLock => "Caps Lock",
    }
    .to_string()
}

fn row_label(ui: &mut egui::Ui, language: Language, is_dark: bool, label: Key, help: Key) {
    let tokens = style::tokens(is_dark);
    ui.vertical(|ui| {
        ui.label(RichText::new(tr(label, language, &[])).size(12.0));
        ui.label(
            RichText::new(tr(help, language, &[]))
                .size(9.0)
                .color(tokens.muted),
        );
    });
}

fn field_error(ui: &mut egui::Ui, is_dark: bool, error: Option<&model::FieldError>, field: &str) {
    if let Some(error) = error {
        if error.field == field {
            ui.colored_label(
                style::tokens(is_dark).danger,
                RichText::new(&error.message).size(9.0),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn numeric_row(
    ui: &mut egui::Ui,
    language: Language,
    is_dark: bool,
    error: Option<&model::FieldError>,
    label: Key,
    help: Key,
    field: &str,
    value: &mut u32,
    range: std::ops::RangeInclusive<u32>,
) {
    row_label(ui, language, is_dark, label, help);
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.x = ui.available_width();
        ui.add(egui::DragValue::new(value).range(range).speed(1.0));
    });
    field_error(ui, is_dark, error, field);
    ui.add_space(8.0);
}

fn numeric_u8_row(
    ui: &mut egui::Ui,
    language: Language,
    is_dark: bool,
    error: Option<&model::FieldError>,
    label: Key,
    field: &str,
    value: &mut u8,
) {
    ui.label(RichText::new(tr(label, language, &[])).size(12.0));
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.x = ui.available_width();
        ui.add(egui::DragValue::new(value).range(0..=100).speed(1.0));
    });
    field_error(ui, is_dark, error, field);
    ui.add_space(8.0);
}

#[allow(clippy::too_many_arguments)]
fn numeric_beep_row(
    ui: &mut egui::Ui,
    language: Language,
    is_dark: bool,
    error: Option<&model::FieldError>,
    label: Key,
    help: Key,
    field: &str,
    value: &mut u32,
    range: std::ops::RangeInclusive<u32>,
) {
    row_label(ui, language, is_dark, label, help);
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.x = ui.available_width();
        ui.add(egui::DragValue::new(value).range(range).speed(1.0));
    });
    field_error(ui, is_dark, error, field);
    ui.add_space(8.0);
}

fn language_combo(ui: &mut egui::Ui, view: &mut SettingsView<'_>) {
    ui.label(RichText::new(tr(Key::AppLanguage, view.language, &[])).size(12.0));
    egui::ComboBox::from_id_salt("language")
        .width(ui.available_width())
        .selected_text(match view.draft.language {
            Language::En => tr(Key::AppLanguageEn, view.language, &[]).to_string(),
            Language::Vi => tr(Key::AppLanguageVi, view.language, &[]).to_string(),
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut view.draft.language,
                Language::En,
                tr(Key::AppLanguageEn, view.language, &[]).to_string(),
            );
            ui.selectable_value(
                &mut view.draft.language,
                Language::Vi,
                tr(Key::AppLanguageVi, view.language, &[]).to_string(),
            );
        });
    ui.add_space(8.0);
}

fn label_style_combo(ui: &mut egui::Ui, view: &mut SettingsView<'_>) {
    ui.label(RichText::new(tr(Key::SettingsAppLabelStyle, view.language, &[])).size(12.0));
    ui.label(
        RichText::new(tr(Key::SettingsAppLabelStyleHelp, view.language, &[]))
            .size(10.0)
            .color(style::tokens(view.is_dark).muted),
    );
    egui::ComboBox::from_id_salt("app_label_style")
        .width(ui.available_width())
        .selected_text(label_style_text(view.draft.app_label_style, view.language))
        .show_ui(ui, |ui| {
            for style in [
                AppLabelStyle::Friendly,
                AppLabelStyle::FriendlyAndProcess,
                AppLabelStyle::Process,
            ] {
                ui.selectable_value(
                    &mut view.draft.app_label_style,
                    style,
                    label_style_text(style, view.language),
                );
            }
        });
    ui.add_space(8.0);
}

fn label_style_text(style: AppLabelStyle, language: Language) -> String {
    match style {
        AppLabelStyle::Friendly => tr(Key::SettingsAppLabelFriendly, language, &[]),
        AppLabelStyle::FriendlyAndProcess => {
            tr(Key::SettingsAppLabelFriendlyAndProcess, language, &[])
        }
        AppLabelStyle::Process => tr(Key::SettingsAppLabelProcess, language, &[]),
    }
    .to_string()
}

fn theme_combo(ui: &mut egui::Ui, view: &mut SettingsView<'_>) {
    combo_row(
        ui,
        view.language,
        Key::SettingsTheme,
        "theme",
        &mut view.draft.theme,
        &[
            (ThemeMode::System, Key::ThemeSystem),
            (ThemeMode::Light, Key::ThemeLight),
            (ThemeMode::Dark, Key::ThemeDark),
        ],
    );
}

fn material_combo(ui: &mut egui::Ui, view: &mut SettingsView<'_>) {
    combo_row(
        ui,
        view.language,
        Key::SettingsMaterial,
        "material",
        &mut view.draft.material,
        &[
            (MaterialMode::Auto, Key::MaterialAuto),
            (MaterialMode::Translucent, Key::MaterialTranslucent),
            (MaterialMode::Opaque, Key::MaterialOpaque),
        ],
    );
}

fn motion_combo(ui: &mut egui::Ui, view: &mut SettingsView<'_>) {
    combo_row(
        ui,
        view.language,
        Key::SettingsMotion,
        "motion",
        &mut view.draft.motion,
        &[
            (MotionMode::Full, Key::MotionFull),
            (MotionMode::Reduced, Key::MotionReduced),
            (MotionMode::Disabled, Key::MotionDisabled),
        ],
    );
}

fn accent_combo(ui: &mut egui::Ui, view: &mut SettingsView<'_>) {
    combo_row(
        ui,
        view.language,
        Key::SettingsAccent,
        "accent",
        &mut view.draft.accent,
        &[
            (AccentMode::System, Key::AccentSystem),
            (AccentMode::Blue, Key::AccentBlue),
            (AccentMode::Green, Key::AccentGreen),
            (AccentMode::Purple, Key::AccentPurple),
            (AccentMode::Orange, Key::AccentOrange),
        ],
    );
}

fn combo_row<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    language: Language,
    label: Key,
    id: &str,
    value: &mut T,
    options: &[(T, Key)],
) {
    ui.label(RichText::new(tr(label, language, &[])).size(12.0));
    let selected = options
        .iter()
        .find(|(option, _)| option == value)
        .map(|(_, key)| tr(*key, language, &[]).to_string())
        .unwrap_or_default();
    egui::ComboBox::from_id_salt(id)
        .width(ui.available_width())
        .selected_text(selected)
        .show_ui(ui, |ui| {
            for (option, key) in options {
                ui.selectable_value(value, *option, tr(*key, language, &[]).to_string());
            }
        });
    ui.add_space(8.0);
}

fn preview_card(ui: &mut egui::Ui, view: &mut SettingsView<'_>) {
    let tokens = style::tokens(view.is_dark);
    egui::Frame::NONE
        .fill(tokens.input)
        .stroke(egui::Stroke::new(1.0, tokens.border))
        .corner_radius(egui::CornerRadius::same(style::RADIUS_CONTROL))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.label(
                RichText::new(tr(Key::AppName, view.language, &[]))
                    .size(11.0)
                    .strong(),
            );
            let color = crate::surfaces::mixer::band_color(
                55,
                view.draft.color_thresholds.green_up_to,
                view.draft.color_thresholds.blue_up_to,
                false,
                &tokens,
            );
            let (rect, _) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), 8.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(4), tokens.border);
            let fill = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width() * 0.55, 8.0));
            ui.painter()
                .rect_filled(fill, egui::CornerRadius::same(4), color);
            ui.small("preview tracks the draft; only Save applies");
        });
}

/// Notice text for a config load notice.
#[must_use]
pub fn notice_text(notice: ConfigLoadNotice, language: Language) -> String {
    let key = match notice {
        ConfigLoadNotice::MigratedFromJson => Key::StorageMigratedFromJson,
        ConfigLoadNotice::RecoveredFromJson => Key::StorageRecoveredFromJson,
        ConfigLoadNotice::DefaultsAfterError => Key::StorageDefaultsAfterError,
        ConfigLoadNotice::InvalidExternalEdit => Key::StorageInvalidExternalEdit,
    };
    tr(key, language, &[]).to_string()
}

/// Apply a modifier preset to every shortcut in the draft.
pub fn apply_preset(draft: &mut SettingsDraft, modifier: HotkeyModifier) {
    draft.hotkeys = HotkeyBindings::for_modifier(modifier);
}

/// Whether the Settings window must keep Escape for the active recorder.
#[must_use]
pub fn capturing(recorder: &Recorder) -> bool {
    recorder.active().is_some()
}

fn hotkey_action(action: RecorderAction) -> HotkeyAction {
    match action {
        RecorderAction::VolumeUp => HotkeyAction::VolumeUp,
        RecorderAction::VolumeDown => HotkeyAction::VolumeDown,
        RecorderAction::VolumeUpLarge => HotkeyAction::VolumeUpLarge,
        RecorderAction::VolumeDownLarge => HotkeyAction::VolumeDownLarge,
        RecorderAction::ToggleMute => HotkeyAction::ToggleMute,
        RecorderAction::Reset50 => HotkeyAction::Reset50,
        RecorderAction::OpenMixer => HotkeyAction::OpenMixer,
        RecorderAction::OpenMenu => HotkeyAction::OpenMenu,
    }
}

fn hotkey_conflicted(action: RecorderAction, status: &[HotkeyRegResult]) -> bool {
    status.iter().any(|result| {
        result.action == hotkey_action(action)
            && matches!(result.status, HotkeyRegStatus::Conflicted(_))
    })
}

/// Decode and cache a running-app icon texture for the picker.
fn running_app_icon(ui: &egui::Ui, app: &RunningApp) -> Option<egui::TextureHandle> {
    let data = app.icon.as_deref()?;
    let id = egui::Id::new(("running-app-icon", &app.process));
    if let Some(texture) = ui.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(texture);
    }
    let image = crate::icon::decode_data_url(data)?;
    let texture = ui.ctx().load_texture(
        format!("running-app-{}", app.process),
        image,
        egui::TextureOptions::LINEAR,
    );
    ui.data_mut(|d| d.insert_temp(id, texture.clone()));
    Some(texture)
}
