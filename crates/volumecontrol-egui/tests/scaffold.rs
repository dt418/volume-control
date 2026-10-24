//! Task 1 contract: i18n tables, sink label mapping, viewport builder flags.

use volumecontrol_egui::app::builder_for;
use volumecontrol_egui::i18n::{table, tr, Key};
use volumecontrol_egui::placement::{frame_for, WorkArea};
use volumecontrol_egui::sink::surface_for_label;
use volumectl::config::Language;
use volumectl::ui::SurfaceId;

const AREA: WorkArea = WorkArea::new(0.0, 0.0, 1600.0, 900.0);

#[test]
fn legacy_copy_ports_verbatim() {
    assert_eq!(tr(Key::MixerTitle, Language::Vi, &[]), "Bộ trộn âm lượng");
    assert_eq!(
        tr(Key::SettingsUpdatesUpToDate, Language::En, &[]),
        "Volume Control is up to date"
    );
    assert_eq!(
        tr(Key::SettingsUpdatesAvailable, Language::Vi, &["0.2.0"]),
        "Có phiên bản mới: 0.2.0"
    );
}

#[test]
fn every_key_exists_in_both_languages() {
    let english = table(Language::En);
    let vietnamese = table(Language::Vi);
    assert_eq!(english.len(), Key::ALL.len());
    assert_eq!(vietnamese.len(), Key::ALL.len());
    for (index, key) in Key::ALL.iter().enumerate() {
        assert!(
            !english[index].trim().is_empty(),
            "missing EN copy for {key:?}"
        );
        assert!(
            !vietnamese[index].trim().is_empty(),
            "missing VI copy for {key:?}"
        );
    }
}

#[test]
fn placeholders_match_between_languages() {
    let english = table(Language::En);
    let vietnamese = table(Language::Vi);
    for (index, key) in Key::ALL.iter().enumerate() {
        let en = placeholders(english[index]);
        let vi = placeholders(vietnamese[index]);
        assert_eq!(en, vi, "placeholder parity broke for {key:?}");
    }
}

fn placeholders(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let bytes = text.as_bytes();
    let mut index = 0;
    while index + 2 < bytes.len() {
        if bytes[index] == b'{' && bytes[index + 1].is_ascii_digit() && bytes[index + 2] == b'}' {
            found.push(text[index..=index + 2].to_string());
            index += 3;
        } else {
            index += 1;
        }
    }
    found
}

#[test]
fn surface_labels_match_the_legacy_host_contract() {
    assert_eq!(surface_for_label("window-mixer"), Some(SurfaceId::Mixer));
    assert_eq!(
        surface_for_label("window-settings"),
        Some(SurfaceId::Settings)
    );
    assert_eq!(surface_for_label("window-help"), Some(SurfaceId::Help));
    assert_eq!(surface_for_label("window-overlay"), None);
    assert_eq!(surface_for_label("tray-menu"), None);
}

#[test]
fn mixer_is_a_borderless_always_on_top_hud() {
    let builder = builder_for(SurfaceId::Mixer, 1.0, &AREA, Language::En);
    let frame = frame_for(SurfaceId::Mixer, &AREA, 1.0);
    assert_eq!(builder.title.as_deref(), Some("Volume Mixer"));
    assert_eq!(builder.window_level, Some(egui::WindowLevel::AlwaysOnTop));
    assert_eq!(builder.taskbar, Some(false));
    assert_eq!(builder.decorations, Some(false));
    assert_eq!(builder.transparent, Some(true));
    assert_eq!(builder.resizable, Some(false));
    assert_eq!(
        builder.position,
        Some(egui::pos2(frame.x as f32, frame.y as f32))
    );
    assert_eq!(builder.inner_size, Some(egui::vec2(400.0, 280.0)));
}

#[test]
fn settings_is_a_resizable_taskbar_window_with_min_size() {
    let builder = builder_for(SurfaceId::Settings, 1.0, &AREA, Language::En);
    let frame = frame_for(SurfaceId::Settings, &AREA, 1.0);
    assert_eq!(builder.title.as_deref(), Some("Volume Control Settings"));
    assert_eq!(builder.window_level, None);
    assert_eq!(builder.taskbar, Some(true));
    assert_eq!(builder.resizable, Some(true));
    assert_eq!(builder.transparent, Some(true));
    assert_eq!(builder.min_inner_size, Some(egui::vec2(620.0, 520.0)));
    assert_eq!(
        builder.position,
        Some(egui::pos2(frame.x as f32, frame.y as f32))
    );
    assert_eq!(builder.inner_size, Some(egui::vec2(760.0, 620.0)));
}

#[test]
fn help_is_a_resizable_taskbar_window() {
    let builder = builder_for(SurfaceId::Help, 1.0, &AREA, Language::En);
    assert_eq!(builder.title.as_deref(), Some("Volume Control Help"));
    assert_eq!(builder.taskbar, Some(true));
    assert_eq!(builder.resizable, Some(true));
    assert_eq!(builder.inner_size, Some(egui::vec2(520.0, 500.0)));
}

#[test]
fn overlay_is_a_click_through_hud() {
    let builder = builder_for(SurfaceId::Overlay, 1.0, &AREA, Language::En);
    assert_eq!(builder.title.as_deref(), Some("Volume"));
    assert_eq!(builder.window_level, Some(egui::WindowLevel::AlwaysOnTop));
    assert_eq!(builder.taskbar, None);
    assert_eq!(builder.decorations, Some(false));
    assert_eq!(builder.transparent, Some(true));
    assert_eq!(builder.mouse_passthrough, None);
    assert_eq!(builder.inner_size, Some(egui::vec2(336.0, 88.0)));
}

#[test]
fn builder_positions_follow_dpi_scale() {
    let builder = builder_for(SurfaceId::Overlay, 2.0, &AREA, Language::En);
    let frame = frame_for(SurfaceId::Overlay, &AREA, 2.0);
    assert_eq!(
        builder.position,
        Some(egui::pos2(
            (frame.x as f32 / 2.0).round(),
            (frame.y as f32 / 2.0).round()
        ))
    );
}
