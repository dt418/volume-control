//! Settings model contracts: patch mapping, validation routing, recorder,
//! blacklist draft operations, and update phases.

use volumecontrol_egui::settings::blacklist;
use volumecontrol_egui::settings::model::{SettingsDraft, Tab};
use volumecontrol_egui::settings::recorder::{KeyEvent, Recorder, RecorderAction, RecorderEffect};
use volumecontrol_egui::settings::update_card::{UpdateAction, UpdateCard, UpdatePhase};
use volumectl::config::{AppLabelStyle, Config, HotkeyModifier, Language};
use volumectl::ui::{AccentMode, MaterialMode, MotionMode, ThemeMode};

#[test]
fn patch_maps_every_draft_field() {
    let config = Config::default();
    let mut draft = SettingsDraft::from_config(&config);
    draft.volume_step = 3;
    draft.volume_step_large = 15;
    draft.overlay_duration_ms = 2_500;
    draft.ui_font_size = 16;
    draft.language = Language::Vi;
    draft.app_label_style = AppLabelStyle::Process;
    draft.autostart = true;
    draft.theme = ThemeMode::Dark;
    draft.material = MaterialMode::Translucent;
    draft.motion = MotionMode::Reduced;
    draft.accent = AccentMode::Purple;
    draft.hotkeys.toggle_mute = "Ctrl+Shift+KeyM".into();
    draft.blacklist = vec!["chrome.exe".into()];
    draft.color_thresholds.green_up_to = 30;
    draft.color_thresholds.blue_up_to = 70;
    draft.beep.enabled = false;

    let patch = draft.patch();
    assert_eq!(patch.volume_step, Some(3));
    assert_eq!(patch.volume_step_large, Some(15));
    assert_eq!(patch.overlay_duration_ms, Some(2_500));
    assert_eq!(patch.ui_font_size, Some(16));
    assert_eq!(patch.language.as_deref(), Some("vi"));
    assert_eq!(patch.app_label_style.as_deref(), Some("process"));
    assert_eq!(patch.autostart, Some(true));
    assert_eq!(patch.theme.as_deref(), Some("Dark"));
    assert_eq!(patch.material.as_deref(), Some("Translucent"));
    assert_eq!(patch.motion.as_deref(), Some("Reduced"));
    assert_eq!(patch.accent.as_deref(), Some("Purple"));
    assert_eq!(
        patch.hotkeys.as_ref().unwrap().toggle_mute,
        "Ctrl+Shift+KeyM"
    );
    assert_eq!(
        patch.blacklist.as_ref().unwrap(),
        &vec!["chrome.exe".to_string()]
    );
    assert_eq!(
        patch.color_thresholds.as_ref().unwrap().green_up_to,
        Some(30)
    );
    assert_eq!(patch.beep.as_ref().unwrap().enabled, Some(false));
}

#[test]
fn pristine_draft_reports_no_changes_and_reset_discards_edits() {
    let config = Config::default();
    let mut draft = SettingsDraft::from_config(&config);
    assert!(draft.is_pristine());

    draft.overlay_duration_ms = 3_000;
    assert!(!draft.is_pristine());

    draft.reset();
    assert!(draft.is_pristine());
    assert_eq!(draft.overlay_duration_ms, config.overlay_duration_ms);
}

#[test]
fn validation_routes_fields_to_their_tab() {
    let config = Config::default();

    let mut draft = SettingsDraft::from_config(&config);
    draft.volume_step = 30;
    draft.volume_step_large = 20;
    let error = draft.validate().unwrap_err();
    assert_eq!(error.tab, Tab::General);
    assert_eq!(error.field, "volume_step_large");

    let mut draft = SettingsDraft::from_config(&config);
    draft.color_thresholds.green_up_to = 90;
    draft.color_thresholds.blue_up_to = 60;
    let error = draft.validate().unwrap_err();
    assert_eq!(error.tab, Tab::Appearance);

    let mut draft = SettingsDraft::from_config(&config);
    draft.beep.blocked_duration_ms = 9;
    let error = draft.validate().unwrap_err();
    assert_eq!(error.tab, Tab::Feedback);

    let mut draft = SettingsDraft::from_config(&config);
    draft.hotkeys.volume_up = "Ctrl+Alt+ArrowUp".into();
    draft.hotkeys.volume_down = "Ctrl+Alt+ArrowUp".into();
    let error = draft.validate().unwrap_err();
    assert_eq!(error.tab, Tab::Hotkeys);
}

#[test]
fn overlay_duration_below_500_is_rejected() {
    let mut draft = SettingsDraft::from_config(&Config::default());
    draft.overlay_duration_ms = 499;
    assert!(draft.validate().is_err());
    draft.overlay_duration_ms = 500;
    assert!(draft.validate().is_ok());
}

#[test]
fn recorder_captures_modifier_chords_and_rejects_bare_keys() {
    let mut recorder = Recorder::new();
    recorder.start(RecorderAction::ToggleMute);

    let effect = recorder.feed(KeyEvent::Key {
        code: "KeyM".into(),
        ctrl: false,
        alt: false,
        shift: false,
        meta: false,
    });
    assert_eq!(effect, RecorderEffect::NeedModifier);
    assert_eq!(recorder.active(), Some(RecorderAction::ToggleMute));

    let effect = recorder.feed(KeyEvent::Key {
        code: "KeyM".into(),
        ctrl: true,
        alt: true,
        shift: false,
        meta: false,
    });
    assert_eq!(
        effect,
        RecorderEffect::Captured {
            action: RecorderAction::ToggleMute,
            binding: "Ctrl+Alt+KeyM".into(),
        }
    );
    assert_eq!(recorder.active(), None);
}

#[test]
fn recorder_escape_cancels_and_backspace_clears() {
    let mut recorder = Recorder::new();
    recorder.start(RecorderAction::VolumeUp);
    assert_eq!(recorder.feed(KeyEvent::Escape), RecorderEffect::Cancelled);
    assert_eq!(recorder.active(), None);

    recorder.start(RecorderAction::VolumeUp);
    assert_eq!(
        recorder.feed(KeyEvent::Backspace),
        RecorderEffect::Cleared {
            action: RecorderAction::VolumeUp
        }
    );
}

#[test]
fn blacklist_add_normalizes_and_rejects_duplicates() {
    let mut list = Vec::new();
    assert!(blacklist::add(&mut list, "  CHROME.EXE "));
    assert_eq!(list, vec!["chrome.exe"]);
    assert!(!blacklist::add(&mut list, "chrome.exe"));
    assert!(!blacklist::add(&mut list, "   "));
}

#[test]
fn blacklist_merge_recommended_never_duplicates_existing_entries() {
    let mut list = vec!["chrome.exe".into()];
    blacklist::merge_recommended(&mut list, HotkeyModifier::Ctrl);
    let occurrences = list
        .iter()
        .filter(|entry| entry.eq_ignore_ascii_case("chrome.exe"))
        .count();
    assert_eq!(occurrences, 1);
}

#[test]
fn blacklist_reload_and_clear_report_changes() {
    let mut list = vec!["a.exe".into()];
    let saved = vec!["b.exe".into()];
    assert!(blacklist::reload_saved(&mut list, &saved));
    assert!(!blacklist::reload_saved(&mut list, &saved));
    assert!(blacklist::clear(&mut list));
    assert!(!blacklist::clear(&mut list));
}

#[test]
fn update_card_walks_the_legacy_phase_machine() {
    let mut card = UpdateCard::new("0.1.11");
    assert_eq!(card.phase.action(), Some(UpdateAction::Check));

    card.check_started();
    assert_eq!(card.phase, UpdatePhase::Checking);
    assert_eq!(card.phase.action(), None);

    card.check_finished(Some("0.2.0".into()));
    assert!(matches!(card.phase, UpdatePhase::Available { .. }));

    card.download_progress(40);
    assert_eq!(card.phase, UpdatePhase::Downloading { percent: 40 });

    card.download_ready("0.2.0");
    assert_eq!(card.phase.action(), Some(UpdateAction::ConfirmInstall));

    card.install_started("0.2.0");
    assert!(matches!(card.phase, UpdatePhase::Installing { .. }));

    card.failed("network down");
    assert_eq!(card.phase.action(), Some(UpdateAction::Retry));

    card.dismiss();
    assert_eq!(card.phase, UpdatePhase::Idle);
}

#[test]
fn update_status_localizes_versions_and_errors() {
    let mut card = UpdateCard::new("0.1.11");
    card.check_finished(None);
    assert_eq!(
        card.phase.status_text(Language::Vi, "0.1.11"),
        "Điều khiển âm lượng đã là bản mới nhất"
    );

    card.check_finished(Some("0.2.0".into()));
    assert_eq!(
        card.phase.status_text(Language::Vi, "0.1.11"),
        "Có phiên bản mới: 0.2.0"
    );

    card.failed("boom");
    assert!(card
        .phase
        .status_text(Language::En, "0.1.11")
        .contains("boom"));
}

#[test]
fn managed_builds_show_no_install_action() {
    let mut card = UpdateCard::new("0.1.11");
    card.managed_externally();
    assert_eq!(card.phase.action(), None);
    assert!(card
        .phase
        .status_text(Language::En, "0.1.11")
        .contains("managed externally"));
}
