//! Mixer model tests: sorting, filtering, labels, status, footer, band colors.

use volumecontrol_egui::i18n::{tr, Key};
use volumecontrol_egui::style;
use volumecontrol_egui::surfaces::mixer::{
    band_color, empty_text, matches_query, row_label, rows, status_text,
};
use volumectl::config::{AppLabelStyle, Language};
use volumectl::ui::{AppState, AudioSessionState};

fn session(id: &str, name: &str, process: &str, volume: u8, active: bool) -> AudioSessionState {
    AudioSessionState {
        id: id.to_string(),
        name: name.to_string(),
        process: process.to_string(),
        display_name: None,
        volume_percent: volume,
        muted: false,
        active,
        system: false,
    }
}

fn state_with(sessions: Vec<AudioSessionState>) -> AppState {
    AppState {
        sessions,
        ..AppState::default()
    }
}

#[test]
fn rows_sort_active_first_then_volume_descending() {
    let state = state_with(vec![
        session("a", "Alpha", "alpha.exe", 10, false),
        session("b", "Beta", "beta.exe", 80, true),
        session("c", "Gamma", "gamma.exe", 40, true),
        session("d", "Delta", "delta.exe", 90, false),
    ]);

    let ids: Vec<String> = rows(&state, "", AppLabelStyle::Friendly, Language::En)
        .into_iter()
        .map(|row| row.id)
        .collect();
    assert_eq!(ids, vec!["b", "c", "d", "a"]);
}

#[test]
fn search_matches_name_process_and_display_name_case_insensitively() {
    let mut spotify = session("s", "Spotify", "Spotify.exe", 50, true);
    spotify.display_name = Some("Spotify Music".into());

    assert!(matches_query(&spotify, "spot"));
    assert!(matches_query(&spotify, "EXE"));
    assert!(matches_query(&spotify, "music"));
    assert!(matches_query(&spotify, "  "));
    assert!(!matches_query(&spotify, "chrome"));
}

#[test]
fn filtering_keeps_only_matching_rows() {
    let state = state_with(vec![
        session("a", "Alpha", "alpha.exe", 10, true),
        session("b", "Beta", "beta.exe", 20, true),
    ]);

    let filtered = rows(&state, "BET", AppLabelStyle::Friendly, Language::En);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, "b");
}

#[test]
fn label_style_controls_friendly_process_and_combined_names() {
    let mut app = session("a", "Spotify", "spotify.exe", 50, true);
    app.display_name = Some("Spotify Music".into());

    assert_eq!(
        row_label(&app, AppLabelStyle::Friendly, Language::En),
        "Spotify Music"
    );
    assert_eq!(
        row_label(&app, AppLabelStyle::FriendlyAndProcess, Language::En),
        "Spotify Music (spotify.exe)"
    );
    assert_eq!(
        row_label(&app, AppLabelStyle::Process, Language::En),
        "spotify.exe"
    );
}

#[test]
fn system_sounds_uses_localized_label_and_hint() {
    let mut system = session("sys", "System Sounds", "", 0, true);
    system.system = true;

    let label = row_label(&system, AppLabelStyle::Process, Language::Vi);
    assert_eq!(label, "Âm thanh hệ thống");
}

#[test]
fn status_text_switches_to_muted_copy() {
    let mut state = AppState {
        volume_percent: 35,
        ..AppState::default()
    };
    assert_eq!(status_text(&state, Language::Vi), "Âm lượng 35%");

    state.muted = true;
    assert_eq!(
        status_text(&state, Language::Vi),
        tr(Key::CommonMuted, Language::Vi, &[])
    );
}

#[test]
fn empty_text_distinguishes_no_sessions_and_no_matches() {
    assert_eq!(
        empty_text(false, "", Language::Vi),
        tr(Key::MixerNoSessions, Language::Vi, &[])
    );
    assert_eq!(
        empty_text(true, "chrome", Language::En),
        tr(Key::MixerNoMatches, Language::En, &[])
    );
}

#[test]
fn band_color_boundaries_match_signal_rail() {
    let tokens = style::tokens(true);
    assert_eq!(band_color(39, 40, 75, false, &tokens), tokens.success);
    assert_eq!(band_color(40, 40, 75, false, &tokens), tokens.accent);
    assert_eq!(band_color(75, 40, 75, false, &tokens), tokens.accent);
    assert_eq!(band_color(76, 40, 75, false, &tokens), tokens.warning);
    assert_eq!(band_color(76, 40, 75, true, &tokens), tokens.muted);
}
