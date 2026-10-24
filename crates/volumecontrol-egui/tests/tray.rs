//! Tests for the native tray module (Task 5a).
//!
//! Contract:
//! - `menu_items` returns exactly 10 items (1 disabled header + 9 actionable items in exact order)
//! - Header reads "Volume Control — {pct}%" and reflects the given percentage
//! - Localized for Vietnamese (Vi) and English (En) matching legacy app strings
//! - Action mapping is stable and bijective with legacy menu IDs
//! - Icon builder loads and decodes the 32x32 PNG asset
//! - No OS tray is created in tests to prevent side effects in CI / headless environments

#[path = "../src/surfaces/tray.rs"]
#[allow(dead_code)]
mod tray;

use tray::{build_icon, menu_items, TrayAction};
use volumectl::config::Language;

#[test]
fn menu_items_returns_header_and_9_actionable_items_in_exact_order() {
    let items = menu_items(Language::Vi, 50);
    assert_eq!(items.len(), 10, "tray menu must have exactly 10 items");

    // Header item is first and non-actionable
    assert_eq!(items[0].0, TrayAction::Header);
    assert!(!items[0].0.is_actionable());

    // Remaining 9 items are actionable and in exact legacy order
    let expected_actions = [
        TrayAction::ToggleMute,
        TrayAction::Reset50,
        TrayAction::ShowMixer,
        TrayAction::ShowSettings,
        TrayAction::ShowHelp,
        TrayAction::CheckUpdates,
        TrayAction::ReloadConfig,
        TrayAction::OpenConfig,
        TrayAction::Exit,
    ];

    for (idx, &expected) in expected_actions.iter().enumerate() {
        let (action, _) = &items[idx + 1];
        assert_eq!(*action, expected, "mismatch at position {}", idx + 1);
        assert!(
            action.is_actionable(),
            "item {:?} should be actionable",
            action
        );
    }
}

#[test]
fn menu_items_localized_for_vi() {
    let items = menu_items(Language::Vi, 75);

    assert_eq!(items[0].1, "Volume Control — 75%");
    assert_eq!(items[1].1, "Tắt tiếng");
    assert_eq!(items[2].1, "Đặt lại 50%");
    assert_eq!(items[3].1, "Mở bộ trộn âm thanh");
    assert_eq!(items[4].1, "Cài đặt");
    assert_eq!(items[5].1, "Trợ giúp");
    assert_eq!(items[6].1, "Kiểm tra cập nhật");
    assert_eq!(items[7].1, "Tải lại cấu hình");
    assert_eq!(items[8].1, "Mở file cấu hình");
    assert_eq!(items[9].1, "Thoát Điều khiển âm lượng");
}

#[test]
fn menu_items_localized_for_en() {
    let items = menu_items(Language::En, 100);

    assert_eq!(items[0].1, "Volume Control — 100%");
    assert_eq!(items[1].1, "Mute");
    assert_eq!(items[2].1, "Reset to 50%");
    assert_eq!(items[3].1, "Open mixer");
    assert_eq!(items[4].1, "Settings");
    assert_eq!(items[5].1, "Help");
    assert_eq!(items[6].1, "Check for updates");
    assert_eq!(items[7].1, "Reload configuration");
    assert_eq!(items[8].1, "Open config file");
    assert_eq!(items[9].1, "Exit Volume Control");
}

#[test]
fn header_contains_percentage() {
    for pct in [0, 1, 42, 50, 75, 99, 100] {
        let vi_items = menu_items(Language::Vi, pct);
        let en_items = menu_items(Language::En, pct);

        let expected = format!("Volume Control — {pct}%");
        assert_eq!(vi_items[0].1, expected);
        assert_eq!(en_items[0].1, expected);
        assert!(vi_items[0].1.contains(&format!("{pct}%")));
        assert!(en_items[0].1.contains(&format!("{pct}%")));
    }
}

#[test]
fn action_mapping_is_stable_across_languages() {
    let vi = menu_items(Language::Vi, 50);
    let en = menu_items(Language::En, 50);

    assert_eq!(vi.len(), en.len());
    for i in 0..vi.len() {
        assert_eq!(
            vi[i].0, en[i].0,
            "action at index {i} must match between languages"
        );
    }
}

#[test]
fn stable_menu_id_round_trip() {
    let cases = [
        ("volume", TrayAction::Header),
        ("header", TrayAction::Header),
        ("mute", TrayAction::ToggleMute),
        ("reset", TrayAction::Reset50),
        ("mixer", TrayAction::ShowMixer),
        ("settings", TrayAction::ShowSettings),
        ("help", TrayAction::ShowHelp),
        ("check_updates", TrayAction::CheckUpdates),
        ("reload", TrayAction::ReloadConfig),
        ("edit", TrayAction::OpenConfig),
        ("exit", TrayAction::Exit),
    ];

    for (id, expected_action) in cases {
        assert_eq!(
            TrayAction::from_menu_id(id),
            Some(expected_action),
            "ID {id} must decode to {:?}",
            expected_action
        );
        assert_eq!(
            TrayAction::from_menu_id(expected_action.menu_id()),
            Some(expected_action),
            "action {:?} menu_id {:?} must round-trip",
            expected_action,
            expected_action.menu_id()
        );
    }

    assert_eq!(TrayAction::from_menu_id("unknown_id"), None);
    assert_eq!(TrayAction::from_menu_id(""), None);
}

#[test]
fn build_icon_loads_valid_icon() {
    let icon = build_icon();
    assert!(icon.is_some(), "32x32 tray icon should decode successfully");
}
