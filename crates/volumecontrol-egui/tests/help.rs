//! Help view model tests: groups, search filtering, no-match copy,
//! status pills, and footer intent order.

pub use volumecontrol_egui::{i18n, style};

#[path = "../src/surfaces/help.rs"]
#[allow(dead_code)]
pub mod help;

use help::{
    conflict_callout, grouped, no_matches_text, rows, status_key, status_key_for_action, HelpGroup,
    HelpIntent, FOOTER_BUTTONS,
};
use volumecontrol_egui::i18n::{tr, Key};
use volumectl::config::{HotkeyBindings, Language};
use volumectl::hotkeys::{HotkeyAction, HotkeyRegError, HotkeyRegResult, HotkeyRegStatus};

fn sample_status() -> Vec<HotkeyRegResult> {
    vec![
        HotkeyRegResult {
            action: HotkeyAction::VolumeUp,
            status: HotkeyRegStatus::Registered,
        },
        HotkeyRegResult {
            action: HotkeyAction::VolumeDown,
            status: HotkeyRegStatus::Registered,
        },
        HotkeyRegResult {
            action: HotkeyAction::ToggleMute,
            status: HotkeyRegStatus::Registered,
        },
        HotkeyRegResult {
            action: HotkeyAction::Reset50,
            status: HotkeyRegStatus::Registered,
        },
        HotkeyRegResult {
            action: HotkeyAction::OpenMixer,
            status: HotkeyRegStatus::Registered,
        },
        HotkeyRegResult {
            action: HotkeyAction::VolumeUpLarge,
            status: HotkeyRegStatus::HookRouted,
        },
        HotkeyRegResult {
            action: HotkeyAction::VolumeDownLarge,
            status: HotkeyRegStatus::Disabled,
        },
        // OpenMenu omitted to test fallback/missing status
    ]
}

#[test]
fn group_assignment_eight_rows_in_three_groups() {
    let bindings = HotkeyBindings::default();
    let status = sample_status();
    let all_rows = rows(&bindings, &status, "", Language::En);

    assert_eq!(all_rows.len(), 8, "must produce exactly 8 rows");

    let sections = grouped(&all_rows);
    assert_eq!(sections.len(), 3, "must partition rows into 3 groups");

    // Group 1: Volume (VolumeUp, VolumeDown)
    assert_eq!(sections[0].group, HelpGroup::Volume);
    assert_eq!(sections[0].heading_key, Key::HelpGroupVolume);
    assert_eq!(sections[0].rows.len(), 2);
    assert_eq!(sections[0].rows[0].action, HotkeyAction::VolumeUp);
    assert_eq!(sections[0].rows[1].action, HotkeyAction::VolumeDown);

    // Group 2: Commands (ToggleMute, Reset50, OpenMixer)
    assert_eq!(sections[1].group, HelpGroup::Commands);
    assert_eq!(sections[1].heading_key, Key::HelpGroupCommands);
    assert_eq!(sections[1].rows.len(), 3);
    assert_eq!(sections[1].rows[0].action, HotkeyAction::ToggleMute);
    assert_eq!(sections[1].rows[1].action, HotkeyAction::Reset50);
    assert_eq!(sections[1].rows[2].action, HotkeyAction::OpenMixer);

    // Group 3: Extended (VolumeUpLarge, VolumeDownLarge, OpenMenu)
    assert_eq!(sections[2].group, HelpGroup::Extended);
    assert_eq!(sections[2].heading_key, Key::HelpGroupExtended);
    assert_eq!(sections[2].rows.len(), 3);
    assert_eq!(sections[2].rows[0].action, HotkeyAction::VolumeUpLarge);
    assert_eq!(sections[2].rows[1].action, HotkeyAction::VolumeDownLarge);
    assert_eq!(sections[2].rows[2].action, HotkeyAction::OpenMenu);

    let total_grouped: usize = sections.iter().map(|s| s.rows.len()).sum();
    assert_eq!(total_grouped, 8);
}

#[test]
fn search_matches_label_help_and_binding_case_insensitively() {
    let bindings = HotkeyBindings::default();
    let status = sample_status();

    // Match label in English
    let matches = rows(&bindings, &status, "Volume Up", Language::En);
    assert_eq!(matches.len(), 2); // Volume Up and Volume Up (large)
    assert_eq!(matches[0].action, HotkeyAction::VolumeUp);
    assert_eq!(matches[1].action, HotkeyAction::VolumeUpLarge);

    // Match label in Vietnamese
    let matches_vi = rows(&bindings, &status, "Tăng âm lượng", Language::Vi);
    assert_eq!(matches_vi.len(), 2);

    // Match help text
    let matches_help = rows(&bindings, &status, "small step", Language::En);
    assert_eq!(matches_help.len(), 2); // Volume Up and Volume Down

    // Match binding token (case-insensitive)
    let matches_key = rows(&bindings, &status, "arrowdown", Language::En);
    assert_eq!(matches_key.len(), 2); // VolumeDown and VolumeDownLarge

    let matches_mute = rows(&bindings, &status, "keym", Language::En);
    assert_eq!(matches_mute.len(), 2); // ToggleMute and OpenMenu (Shift+KeyM)

    // Formatted combo symbol match
    let matches_arrow = rows(&bindings, &status, "↓", Language::En);
    assert_eq!(matches_arrow.len(), 2);
}

#[test]
fn no_match_copy_when_filter_finds_nothing() {
    let bindings = HotkeyBindings::default();
    let status = sample_status();

    let query = "nonexistent_chord_xyz";
    let matches = rows(&bindings, &status, query, Language::En);
    assert!(matches.is_empty());

    let sections = grouped(&matches);
    assert!(sections.is_empty());

    let copy_en = no_matches_text(query, Language::En);
    assert!(copy_en.contains(tr(Key::HelpNoMatches, Language::En, &[]).as_ref()));
    assert!(copy_en.contains(query));

    let copy_vi = no_matches_text(query, Language::Vi);
    assert!(copy_vi.contains(tr(Key::HelpNoMatches, Language::Vi, &[]).as_ref()));
    assert!(copy_vi.contains(query));
}

#[test]
fn status_pill_mapping_handles_all_variants_and_missing() {
    // Registered -> Ready (green)
    assert_eq!(
        status_key(Some(&HotkeyRegStatus::Registered)),
        Key::HelpStatusReady
    );

    // Conflicted -> InUse (red)
    let conf = HotkeyRegStatus::Conflicted(HotkeyRegError {
        error_code: 1400,
        message: "occupied".into(),
    });
    assert_eq!(status_key(Some(&conf)), Key::HelpStatusInUse);

    // Disabled -> Disabled (muted)
    assert_eq!(
        status_key(Some(&HotkeyRegStatus::Disabled)),
        Key::HelpStatusDisabled
    );

    // HookRouted -> Active
    assert_eq!(
        status_key(Some(&HotkeyRegStatus::HookRouted)),
        Key::HelpStatusActive
    );

    // None (missing/unknown) -> Fallback
    assert_eq!(status_key(None), Key::HelpStatusFallback);

    // Test status_key_for_action lookup
    let results = sample_status();
    assert_eq!(
        status_key_for_action(HotkeyAction::VolumeUp, &results),
        Key::HelpStatusReady
    );
    assert_eq!(
        status_key_for_action(HotkeyAction::VolumeUpLarge, &results),
        Key::HelpStatusActive
    );
    assert_eq!(
        status_key_for_action(HotkeyAction::VolumeDownLarge, &results),
        Key::HelpStatusDisabled
    );
    assert_eq!(
        status_key_for_action(HotkeyAction::OpenMenu, &results),
        Key::HelpStatusFallback
    );
}

#[test]
fn footer_intent_order_matches_specification() {
    assert_eq!(FOOTER_BUTTONS.len(), 3);
    assert_eq!(
        FOOTER_BUTTONS[0],
        (Key::HelpEditConfig, HelpIntent::OpenConfig)
    );
    assert_eq!(
        FOOTER_BUTTONS[1],
        (Key::SettingsTitle, HelpIntent::OpenSettings)
    );
    assert_eq!(FOOTER_BUTTONS[2], (Key::CommonClose, HelpIntent::Close));
}

#[test]
fn conflict_callout_detects_conflicts_and_selects_tail() {
    let bindings = HotkeyBindings::default();

    // No conflict case
    let clean_status = vec![HotkeyRegResult {
        action: HotkeyAction::VolumeUp,
        status: HotkeyRegStatus::Registered,
    }];
    assert!(conflict_callout(&bindings, &clean_status).is_none());

    // Single conflict
    let single_conflict = vec![HotkeyRegResult {
        action: HotkeyAction::VolumeUp,
        status: HotkeyRegStatus::Conflicted(HotkeyRegError {
            error_code: 1,
            message: "in use".into(),
        }),
    }];
    let callout = conflict_callout(&bindings, &single_conflict).expect("callout on conflict");
    assert_eq!(callout.title_key, Key::HelpCalloutTitle);
    assert_eq!(callout.body_key, Key::HelpCalloutBody);
    assert_eq!(callout.action_key, Key::HelpCalloutAction);
    assert_eq!(callout.tail_key, Key::HelpCalloutTailOne);
    assert_eq!(callout.conflicted_bindings.len(), 1);

    // Multiple conflicts
    let multi_conflict = vec![
        HotkeyRegResult {
            action: HotkeyAction::VolumeUp,
            status: HotkeyRegStatus::Conflicted(HotkeyRegError {
                error_code: 1,
                message: "in use".into(),
            }),
        },
        HotkeyRegResult {
            action: HotkeyAction::VolumeDown,
            status: HotkeyRegStatus::Conflicted(HotkeyRegError {
                error_code: 1,
                message: "in use".into(),
            }),
        },
    ];
    let callout_multi = conflict_callout(&bindings, &multi_conflict).expect("callout on multi");
    assert_eq!(callout_multi.tail_key, Key::HelpCalloutTailMany);
    assert_eq!(callout_multi.conflicted_bindings.len(), 2);
}
