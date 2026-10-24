//! Unit tests for semantic hit testing, keyboard focus cycling, and IME state tracking.

use crate::input::focus::FocusManager;
use crate::input::hit_test::hit_test;
use crate::input::ime::{ImeAction, ImeState};
use crate::material::MaterialRole;
use crate::scene::geometry::{Point, Rect};
use crate::scene::graph::Scene;
use crate::scene::node::{SceneNode, SemanticId};

#[test]
fn hit_test_evaluates_top_most_z_order_and_clipping() {
    let mut scene = Scene::new();

    // Background card (z = 0)
    let card = SceneNode::new(
        SemanticId(1),
        MaterialRole::Panel,
        Rect::new(0.0, 0.0, 400.0, 300.0),
        0,
    );

    // Button on top of card (z = 10)
    let button = SceneNode::new(
        SemanticId(2),
        MaterialRole::Control,
        Rect::new(50.0, 50.0, 100.0, 32.0),
        10,
    );

    // Clipped control partially visible (z = 20)
    let clipped = SceneNode::new(
        SemanticId(3),
        MaterialRole::Control,
        Rect::new(200.0, 50.0, 100.0, 32.0),
        20,
    )
    .with_clip(Some(Rect::new(200.0, 50.0, 50.0, 32.0))); // Only left half visible

    scene.insert(card);
    scene.insert(button);
    scene.insert(clipped);

    // Point on button -> must hit button (z=10 over z=0)
    assert_eq!(
        hit_test(&scene, Point::new(60.0, 60.0)),
        Some(SemanticId(2))
    );

    // Point on card but outside button -> hits card (z=0)
    assert_eq!(
        hit_test(&scene, Point::new(10.0, 10.0)),
        Some(SemanticId(1))
    );

    // Point inside clipped control's bounds but outside its clip rect -> falls through to card
    assert_eq!(
        hit_test(&scene, Point::new(270.0, 60.0)),
        Some(SemanticId(1))
    );

    // Point inside clipped control's clip rect -> hits clipped control (z=20)
    assert_eq!(
        hit_test(&scene, Point::new(220.0, 60.0)),
        Some(SemanticId(3))
    );

    // Point outside everything
    assert_eq!(hit_test(&scene, Point::new(500.0, 500.0)), None);
}

#[test]
fn focus_manager_cycles_in_visual_order() {
    let mut scene = Scene::new();

    // Top-left control
    let n1 = SceneNode::new(
        SemanticId(1),
        MaterialRole::Control,
        Rect::new(10.0, 10.0, 50.0, 30.0),
        0,
    )
    .with_focusable(true);

    // Top-right control
    let n2 = SceneNode::new(
        SemanticId(2),
        MaterialRole::Control,
        Rect::new(100.0, 10.0, 50.0, 30.0),
        0,
    )
    .with_focusable(true);

    // Non-focusable middle banner
    let banner = SceneNode::new(
        SemanticId(99),
        MaterialRole::Panel,
        Rect::new(10.0, 50.0, 200.0, 40.0),
        0,
    );

    // Bottom control
    let n3 = SceneNode::new(
        SemanticId(3),
        MaterialRole::Control,
        Rect::new(10.0, 100.0, 50.0, 30.0),
        0,
    )
    .with_focusable(true);

    scene.insert(n1);
    scene.insert(n2);
    scene.insert(banner);
    scene.insert(n3);

    let mut focus_mgr = FocusManager::new();

    // Tab forward navigation: 1 -> 2 -> 3 -> 1
    assert_eq!(focus_mgr.focus_next(&scene), Some(SemanticId(1)));
    assert_eq!(focus_mgr.focus_next(&scene), Some(SemanticId(2)));
    assert_eq!(focus_mgr.focus_next(&scene), Some(SemanticId(3)));
    assert_eq!(focus_mgr.focus_next(&scene), Some(SemanticId(1)));

    // Shift+Tab backward navigation: 1 -> 3 -> 2 -> 1
    assert_eq!(focus_mgr.focus_prev(&scene), Some(SemanticId(3)));
    assert_eq!(focus_mgr.focus_prev(&scene), Some(SemanticId(2)));
    assert_eq!(focus_mgr.focus_prev(&scene), Some(SemanticId(1)));
}

#[test]
fn ime_state_lifecycle() {
    let mut ime = ImeState::new();
    assert!(!ime.is_enabled);
    assert!(!ime.is_composing());

    // Enable
    let act = ime.handle_event(&winit::event::Ime::Enabled);
    assert_eq!(act, ImeAction::Enabled);
    assert!(ime.is_enabled);

    // Preedit Vietnamese composition
    let preedit_ev = winit::event::Ime::Preedit("việt".into(), Some((0, 4)));
    let act_pre = ime.handle_event(&preedit_ev);
    assert_eq!(
        act_pre,
        ImeAction::PreeditChanged {
            text: "việt".into(),
            cursor: Some((0, 4))
        }
    );
    assert!(ime.is_composing());
    assert_eq!(ime.preedit_text(), "việt");

    // Commit
    let commit_ev = winit::event::Ime::Commit("việt".into());
    let act_com = ime.handle_event(&commit_ev);
    assert_eq!(act_com, ImeAction::Commit("việt".into()));
    assert!(!ime.is_composing());
    assert_eq!(ime.preedit_text(), "");

    // Disable
    let act_dis = ime.handle_event(&winit::event::Ime::Disabled);
    assert_eq!(act_dis, ImeAction::Disabled);
    assert!(!ime.is_enabled);
}

#[test]
fn mixer_surface_keyboard_search_and_ime_commit() {
    use crate::surfaces::mixer::MixerSurface;
    use volumectl_lib::ui::AudioSessionState;

    let mut mixer = MixerSurface::new();
    mixer.sync_state(&volumectl_lib::ui::AppState {
        sessions: vec![
            AudioSessionState {
                id: "1".into(),
                name: "Spotify".into(),
                process: "spotify.exe".into(),
                display_name: Some("Spotify Music".into()),
                ..Default::default()
            },
            AudioSessionState {
                id: "2".into(),
                name: "Discord".into(),
                process: "discord.exe".into(),
                display_name: Some("Discord Voice".into()),
                ..Default::default()
            },
        ],
        ..Default::default()
    });

    mixer.append_search_char('d');
    mixer.append_search_char('i');
    mixer.append_search_char('s');
    assert_eq!(mixer.search_query(), "dis");
    assert_eq!(mixer.filtered_sessions().len(), 1);
    assert_eq!(mixer.filtered_sessions()[0].name, "Discord");

    mixer.pop_search_char();
    assert_eq!(mixer.search_query(), "di");

    mixer.commit_search_text("cord app");
    assert_eq!(mixer.search_query(), "dicord app");

    mixer.clear_search();
    assert_eq!(mixer.search_query(), "");
    assert_eq!(mixer.filtered_sessions().len(), 2);
}
