//! Task 5b tests: CLI verbs module.
//!
//! Validates verb parsing (case-insensitivity, parameter handling, rejection
//! of out-of-range values or malformed invocations), Verb -> AppAction mapping,
//! and percentage rendering.

#[path = "../src/cli.rs"]
mod cli;

use cli::{parse, parse_slice, render_get, render_get_state, Verb};
use volumectl::audio::VolumeState;
use volumectl::ui::AppAction;

fn s(val: &str) -> String {
    val.to_string()
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().copied().map(s).collect()
}

#[test]
fn parse_get_verb() {
    for input in &["get", "GET", "Get", "gEt"] {
        let args = strings(&[input]);
        assert_eq!(parse(&args), Some(Verb::Get), "failed for {input}");
        assert_eq!(parse_slice(&[*input]), Some(Verb::Get));
    }
}

#[test]
fn parse_set_verb_valid_range() {
    let cases = [("0", 0u8), ("1", 1), ("50", 50), ("99", 99), ("100", 100)];

    for (raw, expected) in cases {
        for verb in &["set", "SET", "Set"] {
            let args = strings(&[verb, raw]);
            assert_eq!(
                parse(&args),
                Some(Verb::Set(expected)),
                "failed for {verb} {raw}"
            );
            assert_eq!(parse_slice(&[*verb, raw]), Some(Verb::Set(expected)));
        }
    }
}

#[test]
fn parse_set_verb_rejects_out_of_range_and_malformed() {
    let invalid = [
        // Out of range (101 is explicitly tested per requirements)
        "101", "102", "200", "255", "256", "1000", "-1", "-50", // Non-numeric
        "abc", "50%", "one", "", // Whitespace/formatting
        " 50", "50 ", "+50",
    ];

    for val in invalid {
        let args = strings(&["set", val]);
        assert_eq!(parse(&args), None, "expected 'set {val}' to be rejected");
        assert_eq!(parse_slice(&["set", val]), None);
    }

    // Missing value
    assert_eq!(parse(&strings(&["set"])), None);
    assert_eq!(parse_slice(&["set"]), None);

    // Extra trailing arguments
    assert_eq!(parse(&strings(&["set", "50", "extra"])), None);
    assert_eq!(parse_slice(&["set", "50", "extra"]), None);
}

#[test]
fn parse_mute_verb() {
    // Bare mute (toggle)
    for verb in &["mute", "MUTE", "Mute"] {
        let args = strings(&[verb]);
        assert_eq!(parse(&args), Some(Verb::Mute(None)), "failed for {verb}");
        assert_eq!(parse_slice(&[*verb]), Some(Verb::Mute(None)));
    }

    // Mute on
    for verb in &["mute", "MUTE", "Mute"] {
        for on in &["on", "ON", "On"] {
            let args = strings(&[verb, on]);
            assert_eq!(
                parse(&args),
                Some(Verb::Mute(Some(true))),
                "failed for {verb} {on}"
            );
            assert_eq!(parse_slice(&[*verb, *on]), Some(Verb::Mute(Some(true))));
        }
    }

    // Mute off
    for verb in &["mute", "MUTE", "Mute"] {
        for off in &["off", "OFF", "Off"] {
            let args = strings(&[verb, off]);
            assert_eq!(
                parse(&args),
                Some(Verb::Mute(Some(false))),
                "failed for {verb} {off}"
            );
            assert_eq!(parse_slice(&[*verb, *off]), Some(Verb::Mute(Some(false))));
        }
    }
}

#[test]
fn parse_mute_rejects_malformed() {
    let invalid = ["toggle", "1", "0", "true", "false", "yes", "no", "extra"];

    for val in invalid {
        let args = strings(&["mute", val]);
        assert_eq!(parse(&args), None, "expected 'mute {val}' to be rejected");
        assert_eq!(parse_slice(&["mute", val]), None);
    }

    // Extra trailing arguments
    assert_eq!(parse(&strings(&["mute", "on", "extra"])), None);
    assert_eq!(parse(&strings(&["mute", "off", "extra"])), None);
    assert_eq!(parse(&strings(&["mute", "extra1", "extra2"])), None);
}

#[test]
fn parse_rejects_unknown_verbs_and_empty_args() {
    assert_eq!(parse(&[]), None);
    assert_eq!(parse_slice::<&str>(&[]), None);

    for verb in &[
        "unknown", "status", "volume", "vol", "help", "show", "toggle",
    ] {
        assert_eq!(parse(&strings(&[verb])), None);
        assert_eq!(parse_slice(&[*verb]), None);
    }

    // Get with extra arguments
    assert_eq!(parse(&strings(&["get", "extra"])), None);
    assert_eq!(parse_slice(&["get", "extra"]), None);
}

#[test]
fn render_get_formats_percentage() {
    assert_eq!(render_get(0), "0");
    assert_eq!(render_get(42), "42");
    assert_eq!(render_get(100), "100");
}

#[test]
fn render_get_state_formats_volume_state() {
    let state = VolumeState {
        volume: 0.42,
        muted: false,
    };
    assert_eq!(render_get_state(&state), "42");

    let zero = VolumeState {
        volume: 0.0,
        muted: true,
    };
    assert_eq!(render_get_state(&zero), "0");

    let full = VolumeState {
        volume: 1.0,
        muted: false,
    };
    assert_eq!(render_get_state(&full), "100");
}

#[test]
fn verb_maps_to_expected_app_action() {
    assert_eq!(Verb::Get.to_app_action(), None);
    assert_eq!(
        Verb::Set(0).to_app_action(),
        Some(AppAction::SetVolumePercent { percent: 0 })
    );
    assert_eq!(
        Verb::Set(42).to_app_action(),
        Some(AppAction::SetVolumePercent { percent: 42 })
    );
    assert_eq!(
        Verb::Set(100).to_app_action(),
        Some(AppAction::SetVolumePercent { percent: 100 })
    );
    assert_eq!(
        Verb::Mute(None).to_app_action(),
        Some(AppAction::ToggleMute)
    );
    assert_eq!(
        Verb::Mute(Some(true)).to_app_action(),
        Some(AppAction::SetMute { muted: true })
    );
    assert_eq!(
        Verb::Mute(Some(false)).to_app_action(),
        Some(AppAction::SetMute { muted: false })
    );
}
