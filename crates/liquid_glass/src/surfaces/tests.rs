//! Unit tests for native Overlay HUD surface lifecycle, auto-hide, and scene generation.

use std::time::{Duration, Instant};

use crate::scene::graph::Scene;
use crate::surfaces::overlay::{
    OverlaySurface, DEVICE_TEXT_ID, GAUGE_FILL_ID, GAUGE_TRACK_ID, OVERLAY_HEIGHT, OVERLAY_ROOT_ID,
    OVERLAY_WIDTH, PERCENT_TEXT_ID,
};

#[test]
fn overlay_initial_state_and_dimensions() {
    let now = Instant::now();
    let overlay = OverlaySurface::new(now);

    assert!(!overlay.is_visible());
    assert_eq!(OVERLAY_WIDTH, 336.0);
    assert_eq!(OVERLAY_HEIGHT, 88.0);
}

#[test]
fn overlay_show_populates_scene_nodes_with_correct_geometry() {
    let now = Instant::now();
    let mut overlay = OverlaySurface::new(now);
    let mut scene = Scene::new();

    overlay.show(75, false, Some("Realtek Headphones"), now, false);
    assert!(overlay.is_visible());

    let t_settled = now + Duration::from_millis(600);
    overlay.populate_scene(&mut scene, t_settled);

    // Root node
    let root = scene.get(OVERLAY_ROOT_ID).expect("Root node must exist");
    assert!(root.visible);
    assert_eq!(root.layout_bounds.width, 336.0);
    assert_eq!(root.layout_bounds.height, 88.0);

    // Percent text
    let pct = scene.get(PERCENT_TEXT_ID).expect("Percent node must exist");
    assert_eq!(pct.text.as_deref(), Some("75%"));

    // Device text
    let dev = scene.get(DEVICE_TEXT_ID).expect("Device node must exist");
    assert_eq!(dev.text.as_deref(), Some("Realtek Headphones"));

    // Gauge track & fill
    let track = scene.get(GAUGE_TRACK_ID).expect("Track node must exist");
    assert_eq!(track.layout_bounds.width, 288.0);

    let fill = scene.get(GAUGE_FILL_ID).expect("Fill node must exist");
    let expected_fill_w = 288.0 * 0.75;
    assert!(
        (fill.layout_bounds.width - expected_fill_w).abs() < 0.05,
        "Expected fill width {}, got {}",
        expected_fill_w,
        fill.layout_bounds.width
    );
}

#[test]
fn overlay_mute_state_behavior() {
    let now = Instant::now();
    let mut overlay = OverlaySurface::new(now);
    let mut scene = Scene::new();

    overlay.show(50, true, Some("Speakers"), now, false);
    overlay.populate_scene(&mut scene, now);

    let pct = scene.get(PERCENT_TEXT_ID).expect("Percent node must exist");
    assert_eq!(pct.text.as_deref(), Some("Muted"));

    let fill = scene.get(GAUGE_FILL_ID).expect("Fill node must exist");
    assert_eq!(fill.layout_bounds.width, 0.0);
}

#[test]
fn overlay_auto_hide_and_idle_settling() {
    let now = Instant::now();
    let mut overlay = OverlaySurface::new(now);

    overlay.show(80, false, None, now, false);
    assert!(overlay.is_visible());

    // During active auto-hide window (< 1500ms)
    let t_mid = now + Duration::from_millis(500);
    let _ = overlay.update(t_mid);
    assert!(overlay.is_visible());

    // Past auto-hide timeout (> 1500ms) and past fade-out (> 2000ms)
    let t_hidden = now + Duration::from_millis(2500);
    let animating = overlay.update(t_hidden);

    assert!(
        !animating,
        "Overlay must settle and stop requesting redraws when hidden"
    );
    assert!(!overlay.is_visible(), "Overlay must hide after timeout");
}

#[test]
fn overlay_text_notification_mode() {
    use crate::surfaces::overlay::{OverlaySurface, NOTIFICATION_TEXT_ID};

    let now = Instant::now();
    let mut overlay = OverlaySurface::new(now);
    let mut scene = Scene::new();

    overlay.show_notification("Update available: v0.2.0", now);
    assert_eq!(
        overlay.notification_text(),
        Some("Update available: v0.2.0")
    );
    assert!(overlay.is_visible());

    overlay.populate_scene(&mut scene, now);
    let text = scene
        .get(NOTIFICATION_TEXT_ID)
        .expect("Toast notification text node must exist");
    assert_eq!(text.text.as_deref(), Some("Update available: v0.2.0"));
    assert!(scene.get(GAUGE_TRACK_ID).is_none());
}

#[test]
fn mixer_initial_state_and_dimensions() {
    use crate::surfaces::mixer::{MIXER_HEIGHT, MIXER_WIDTH};
    assert_eq!(MIXER_WIDTH, 400.0);
    assert_eq!(MIXER_HEIGHT, 224.0);
}

#[test]
fn mixer_state_sync_and_filtering() {
    use crate::surfaces::mixer::MixerSurface;
    use volumectl_lib::ui::{AppState, AudioSessionState};

    let mut mixer = MixerSurface::new();
    let mut state = AppState::from_audio(80, false, Some("Realtek Speakers".into()));
    state.sessions = vec![
        AudioSessionState {
            id: "spotify.exe".into(),
            name: "Spotify".into(),
            process: "spotify.exe".into(),
            display_name: Some("Spotify Music Player".into()),
            volume_percent: 65,
            muted: false,
            active: true,
            system: false,
        },
        AudioSessionState {
            id: "discord.exe".into(),
            name: "Discord".into(),
            process: "discord.exe".into(),
            display_name: Some("Discord Voice".into()),
            volume_percent: 50,
            muted: true,
            active: true,
            system: false,
        },
    ];

    mixer.sync_state(&state);
    assert_eq!(mixer.filtered_sessions().len(), 2);

    // Filter by process name
    mixer.set_search_query("spotify");
    let filtered = mixer.filtered_sessions();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, "spotify.exe");

    // Filter by display name
    mixer.set_search_query("voice");
    let filtered_voice = mixer.filtered_sessions();
    assert_eq!(filtered_voice.len(), 1);
    assert_eq!(filtered_voice[0].id, "discord.exe");

    // Clear filter
    mixer.set_search_query("");
    assert_eq!(mixer.filtered_sessions().len(), 2);
}

#[test]
fn mixer_interaction_action_dispatch() {
    use crate::scene::node::SemanticId;
    use crate::surfaces::mixer::{MixerSurface, MASTER_MUTE_ID, MASTER_SLIDER_ID};
    use volumectl_lib::ui::{AppAction, AppState, AudioSessionState};

    let mut mixer = MixerSurface::new();
    let state = AppState {
        sessions: vec![AudioSessionState {
            id: "game.exe".into(),
            name: "Game".into(),
            process: "game.exe".into(),
            display_name: None,
            volume_percent: 100,
            muted: false,
            active: true,
            system: false,
        }],
        ..Default::default()
    };
    mixer.sync_state(&state);

    // Master mute click
    assert_eq!(
        mixer.handle_click(MASTER_MUTE_ID),
        Some(AppAction::ToggleMute)
    );

    // Master slider drag (75%)
    assert_eq!(
        mixer.handle_slider_drag(MASTER_SLIDER_ID, 0.75),
        Some(AppAction::SetVolumePercent { percent: 75 })
    );

    // Session 0 slider drag (ID = 1001)
    assert_eq!(
        mixer.handle_slider_drag(SemanticId(1001), 0.40),
        Some(AppAction::SetSessionVolume {
            id: "game.exe".into(),
            percent: 40
        })
    );

    // Session 0 mute click (ID = 1002)
    assert_eq!(
        mixer.handle_click(SemanticId(1002)),
        Some(AppAction::ToggleSessionMute {
            id: "game.exe".into()
        })
    );
}

#[test]
fn settings_dimensions_and_min_bounds() {
    use crate::surfaces::settings::{
        SETTINGS_DEFAULT_HEIGHT, SETTINGS_DEFAULT_WIDTH, SETTINGS_MIN_HEIGHT, SETTINGS_MIN_WIDTH,
    };
    assert_eq!(SETTINGS_DEFAULT_WIDTH, 760.0);
    assert_eq!(SETTINGS_DEFAULT_HEIGHT, 620.0);
    assert_eq!(SETTINGS_MIN_WIDTH, 620.0);
    assert_eq!(SETTINGS_MIN_HEIGHT, 520.0);
}

#[test]
fn settings_draft_apply_and_cancel_lifecycle() {
    use crate::surfaces::settings::SettingsSurface;
    use volumectl_lib::ui::{AppAction, AppState, MaterialMode, ThemeMode};

    let confirmed = AppState {
        theme: ThemeMode::Dark,
        material: MaterialMode::Auto,
        ..Default::default()
    };

    let mut settings = SettingsSurface::new(&confirmed);
    assert!(!settings.has_unsaved_changes());
    assert_eq!(settings.draft().theme, ThemeMode::Dark);
    assert_eq!(settings.draft().material, MaterialMode::Auto);

    // Edit draft
    settings.set_theme(ThemeMode::Light);
    settings.set_material(MaterialMode::Translucent);
    assert!(settings.has_unsaved_changes());
    assert_eq!(settings.draft().theme, ThemeMode::Light);
    assert_eq!(settings.draft().material, MaterialMode::Translucent);

    // Cancel reverts to confirmed
    let cancel_action = settings.cancel();
    assert_eq!(cancel_action, AppAction::CancelConfig);
    assert!(!settings.has_unsaved_changes());
    assert_eq!(settings.draft().theme, ThemeMode::Dark);
    assert_eq!(settings.draft().material, MaterialMode::Auto);

    // Edit again and save through host-owned SettingsPatch
    settings.set_theme(ThemeMode::Light);
    let apply_action = settings
        .handle_click(crate::surfaces::settings::BTN_SAVE_ID)
        .expect("save action");
    let AppAction::ApplyConfigDraft(patch) = apply_action else {
        panic!("expected typed SettingsPatch commit");
    };
    assert_eq!(patch.theme.as_deref(), Some("Light"));
    assert!(!settings.has_unsaved_changes());
    assert_eq!(settings.draft().theme, ThemeMode::Light);

    // A later confirmed publication updates the baseline, not the draft.
    let confirmed_after = AppState {
        theme: ThemeMode::Light,
        material: MaterialMode::Opaque,
        ..Default::default()
    };
    settings.sync_confirmed(&confirmed_after);
    assert_eq!(settings.draft().theme, ThemeMode::Light);
    assert_eq!(settings.draft().material, MaterialMode::Opaque);

    // Reset to defaults
    let reset_action = settings.reset_defaults();
    assert_eq!(reset_action, AppAction::ResetConfig);
    assert!(settings.has_unsaved_changes());
    assert_eq!(settings.draft().theme, ThemeMode::System);
}

#[test]
fn settings_tab_switching_and_live_preview_workflow() {
    use crate::material::MaterialRole;
    use crate::surfaces::settings::{
        SettingsSurface, SettingsTab, BTN_CANCEL_ID, BTN_SAVE_ID, OPT_ACC_BLUE,
        OPT_MAT_TRANSLUCENT, OPT_THEME_LIGHT, TAB_APPEARANCE_ID, TAB_GENERAL_ID,
    };
    use volumectl_lib::ui::{AccentMode, AppAction, AppState, MaterialMode, ThemeMode};

    let confirmed = AppState {
        theme: ThemeMode::Dark,
        material: MaterialMode::Auto,
        accent: AccentMode::System,
        ..Default::default()
    };

    let mut settings = SettingsSurface::new(&confirmed);
    assert_eq!(settings.current_tab(), SettingsTab::General);

    // Switch to Appearance tab
    assert_eq!(settings.handle_click(TAB_APPEARANCE_ID), None);
    assert_eq!(settings.current_tab(), SettingsTab::Appearance);

    // Initial preview matches confirmed
    let init_tokens = settings.preview_tokens(MaterialRole::Hud);
    assert!(!init_tokens.is_opaque);
    assert_eq!(init_tokens.blur_radius, 28.0);

    // Click Light Theme option
    assert_eq!(settings.handle_click(OPT_THEME_LIGHT), None);
    assert_eq!(settings.draft().theme, ThemeMode::Light);
    assert!(settings.has_unsaved_changes());

    // Click Translucent Material option
    assert_eq!(settings.handle_click(OPT_MAT_TRANSLUCENT), None);
    assert_eq!(settings.draft().material, MaterialMode::Translucent);

    // Click Blue Accent option
    assert_eq!(settings.handle_click(OPT_ACC_BLUE), None);
    assert_eq!(settings.draft().accent, AccentMode::Blue);

    // Live preview tokens immediately reflect modified draft
    let preview_tokens = settings.preview_tokens(MaterialRole::Hud);
    assert_eq!(
        settings.preview_appearance().color_scheme,
        crate::material::ColorScheme::Light
    );
    assert!(
        preview_tokens.tint_color.x > 0.9,
        "Light theme tint should be bright"
    );

    // Cancel button discards draft and reverts
    let cancel_res = settings.handle_click(BTN_CANCEL_ID);
    assert_eq!(cancel_res, Some(AppAction::CancelConfig));
    assert!(!settings.has_unsaved_changes());
    assert_eq!(settings.draft().theme, ThemeMode::Dark);
    assert_eq!(settings.draft().material, MaterialMode::Auto);
    assert_eq!(settings.draft().accent, AccentMode::System);

    // Edit again and Save
    settings.handle_click(OPT_THEME_LIGHT);
    let save_res = settings.handle_click(BTN_SAVE_ID);
    assert!(matches!(save_res, Some(AppAction::ApplyConfigDraft(_))));
    assert!(!settings.has_unsaved_changes());
    assert_eq!(settings.draft().theme, ThemeMode::Light);

    // Switch back to General tab
    settings.handle_click(TAB_GENERAL_ID);
    assert_eq!(settings.current_tab(), SettingsTab::General);
}

#[test]
fn settings_scene_population_active_tab_controls() {
    use crate::scene::graph::Scene;
    use crate::surfaces::settings::{
        SettingsSurface, SettingsTab, BTN_CANCEL_ID, BTN_RESET_ID, BTN_SAVE_ID, OPT_GEN_AUTOSTART,
        OPT_GEN_STEP_2, OPT_MAT_TRANSLUCENT, OPT_THEME_LIGHT, PREVIEW_CARD_ID,
        SETTINGS_ACTION_BAR_ID, SETTINGS_CONTENT_ID, SETTINGS_ROOT_ID, SETTINGS_SIDEBAR_ID,
        TAB_APPEARANCE_ID, TAB_GENERAL_ID,
    };
    use volumectl_lib::ui::AppState;

    let state = AppState::default();
    let mut settings = SettingsSurface::new(&state);

    // 1. General tab population
    let mut scene_gen = Scene::new();
    settings.populate_scene(&mut scene_gen, 760.0, 620.0);

    assert!(scene_gen.get(SETTINGS_ROOT_ID).is_some());
    assert!(scene_gen.get(SETTINGS_SIDEBAR_ID).is_some());
    assert!(scene_gen.get(SETTINGS_CONTENT_ID).is_some());
    assert!(scene_gen.get(SETTINGS_ACTION_BAR_ID).is_some());
    assert!(scene_gen.get(TAB_GENERAL_ID).is_some());
    assert!(scene_gen.get(TAB_APPEARANCE_ID).is_some());
    assert!(
        scene_gen.get(OPT_GEN_AUTOSTART).is_some(),
        "Autostart control must exist"
    );
    assert!(
        scene_gen.get(OPT_GEN_STEP_2).is_some(),
        "Volume step control must exist"
    );
    assert!(scene_gen.get(BTN_SAVE_ID).is_some());
    assert!(scene_gen.get(BTN_CANCEL_ID).is_some());
    assert!(scene_gen.get(BTN_RESET_ID).is_some());

    // 2. Appearance tab population
    settings.select_tab(SettingsTab::Appearance);
    let mut scene_app = Scene::new();
    settings.populate_scene(&mut scene_app, 760.0, 620.0);

    assert!(
        scene_app.get(OPT_THEME_LIGHT).is_some(),
        "Theme selector must exist"
    );
    assert!(
        scene_app.get(OPT_MAT_TRANSLUCENT).is_some(),
        "Material selector must exist"
    );
    assert!(
        scene_app.get(PREVIEW_CARD_ID).is_some(),
        "Live preview card must exist"
    );
}

#[test]
fn settings_updates_and_blacklist_actions() {
    use crate::scene::graph::Scene;
    use crate::surfaces::settings::{
        SettingsSurface, SettingsTab, BTN_APPLY_REC_BLACKLIST_ID, BTN_CHECK_UPDATES_ID,
        BTN_CLEAR_BLACKLIST_ID,
    };
    use volumectl_lib::ui::{AppAction, AppState};

    let state = AppState::default();
    let mut settings = SettingsSurface::new(&state);

    settings.select_tab(SettingsTab::Updates);
    let mut updates_scene = Scene::new();
    settings.populate_scene(&mut updates_scene, 760.0, 620.0);
    assert!(updates_scene.get(BTN_CHECK_UPDATES_ID).is_some());
    assert_eq!(
        settings.handle_click(BTN_CHECK_UPDATES_ID),
        Some(AppAction::CheckUpdates)
    );

    settings.select_tab(SettingsTab::Blacklist);
    let mut blacklist_scene = Scene::new();
    settings.populate_scene(&mut blacklist_scene, 760.0, 620.0);
    assert!(blacklist_scene.get(BTN_APPLY_REC_BLACKLIST_ID).is_some());
    assert!(blacklist_scene.get(BTN_CLEAR_BLACKLIST_ID).is_some());
    assert_eq!(
        settings.handle_click(BTN_APPLY_REC_BLACKLIST_ID),
        Some(AppAction::ApplyRecommendedBlacklist)
    );
    assert_eq!(
        settings.handle_click(BTN_CLEAR_BLACKLIST_ID),
        Some(AppAction::ClearBlacklist)
    );
}

#[test]
fn help_surface_dimensions_and_bilingual_shortcuts() {
    use crate::surfaces::help::{HelpSurface, DEFAULT_SHORTCUTS, HELP_HEIGHT, HELP_WIDTH};
    use volumectl_lib::config::Language;

    assert_eq!(HELP_WIDTH, 520.0);
    assert_eq!(HELP_HEIGHT, 500.0);

    let mut help = HelpSurface::new(Language::En);
    assert!(!help.is_open());

    help.open();
    assert!(help.is_open());

    // English description
    let s0 = &DEFAULT_SHORTCUTS[0];
    assert_eq!(s0.description(Language::En), "Volume Up (+2%)");

    // Switch to Vietnamese
    help.set_language(Language::Vi);
    assert_eq!(s0.description(Language::Vi), "Tăng âm lượng (+2%)");

    help.close();
    assert!(!help.is_open());
}

#[test]
fn help_scene_generation() {
    use crate::scene::graph::Scene;
    use crate::surfaces::help::{HelpSurface, HELP_CLOSE_BTN_ID, HELP_ROOT_ID};
    use volumectl_lib::config::Language;

    let mut help = HelpSurface::new(Language::Vi);
    let mut scene = Scene::new();

    // Closed -> should not generate visible nodes
    help.populate_scene(&mut scene);
    assert!(scene.get(HELP_ROOT_ID).is_none());

    // Open -> generates dialog
    help.open();
    help.populate_scene(&mut scene);

    let root = scene.get(HELP_ROOT_ID).expect("Help root must exist");
    assert!(root.visible);
    assert_eq!(root.layout_bounds.width, 520.0);
    assert_eq!(root.layout_bounds.height, 500.0);

    assert!(scene.get(HELP_CLOSE_BTN_ID).is_some());
}
