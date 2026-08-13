//! Integration tests for the cross-platform [`volumectl_lib::host_core::AppCore`].

use std::sync::{Arc, Mutex};

use volumectl_lib::audio::{AudioBackend, AudioError, VolumeState};
use volumectl_lib::config::{Config, HotkeyModifier};
use volumectl_lib::host_core::{AppCore, AudioSessionInfo, EventSink};
use volumectl_lib::hotkeys::HotkeyRegResult;
use volumectl_lib::ui::AppAction;

struct StubAudio {
    state: Mutex<VolumeState>,
}

impl AudioBackend for StubAudio {
    fn get_state(&self) -> Result<VolumeState, AudioError> {
        Ok(*self.state.lock().unwrap())
    }
    fn set_volume(&self, volume: f32) -> Result<(), AudioError> {
        self.state.lock().unwrap().volume = volume;
        Ok(())
    }
    fn toggle_mute(&self) -> Result<VolumeState, AudioError> {
        Ok(*self.state.lock().unwrap())
    }
    fn set_mute(&self, muted: bool) -> Result<(), AudioError> {
        self.state.lock().unwrap().muted = muted;
        Ok(())
    }
}

#[derive(Default)]
struct RecordingSink {
    events: Mutex<Vec<String>>,
}

impl EventSink for RecordingSink {
    fn volume(&self, pct: u8, muted: bool) {
        self.events
            .lock()
            .unwrap()
            .push(format!("volume:{pct}:{muted}"));
    }
    fn hotkeys(&self, _status: &[HotkeyRegResult]) {}
    fn sessions(&self, _sessions: &[AudioSessionInfo]) {}
    fn overlay(&self, text: Option<String>, _state: VolumeState, _config: Config) {
        self.events
            .lock()
            .unwrap()
            .push(format!("overlay:{}", text.unwrap_or_default()));
    }
}

fn core_with(sink: Arc<RecordingSink>) -> AppCore {
    AppCore::new(
        Box::new(StubAudio {
            state: Mutex::new(VolumeState {
                volume: 0.5,
                muted: false,
            }),
        }),
        Config::default(),
        HotkeyModifier::CtrlAlt,
        sink,
    )
    .unwrap()
}

#[test]
fn adjust_volume_emits_volume_event() {
    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink.clone());
    core.apply_hotkey(volumectl_lib::hotkeys::HotkeyAction::VolumeUp);
    assert!(sink
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|e| e.starts_with("volume:51:")));
}

#[test]
fn handle_action_adjust_volume_emits_volume_event() {
    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink.clone());
    core.handle_action(AppAction::AdjustVolume { delta_percent: 1 });
    assert!(sink
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|e| e.starts_with("volume:51:")));
}

#[test]
fn session_commands_follow_the_platform_contract() {
    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink.clone());
    if cfg!(target_os = "windows") {
        // Task 2b: the WASAPI source is live on Windows; a stale id must be
        // rejected (the command layer then re-emits state://sessions).
        assert!(core.set_session_volume("stale-id", 40).is_err());
        assert!(core.mute_session("stale-id").is_err());
    } else {
        // No per-app sessions on other platforms: no-op Ok contract.
        assert_eq!(core.sessions(), vec![]);
        assert!(core.set_session_volume("stale-id", 40).is_ok());
        assert!(core.mute_session("stale-id").is_ok());
    }
}

#[test]
fn bootstrap_reports_default_step_and_session_support() {
    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink.clone());
    let payload = core.bootstrap();
    assert_eq!(payload.config.volume_step, 1);
    // Windows wires the WASAPI source; other platforms report unsupported.
    assert_eq!(payload.sessions_supported, cfg!(target_os = "windows"));
    assert_eq!(payload.hotkey_status.len(), 8);
}

#[test]
fn update_settings_mutates_steps_and_appearance_without_writing_disk() {
    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink.clone());
    let patch = volumectl_lib::host_core::SettingsPatch {
        volume_step: Some(5),
        volume_step_large: Some(20), // within the 1..=50 range, > volume_step
        theme: Some("Dark".into()),
        material: Some("Opaque".into()),
        motion: Some("Reduced".into()),
        accent: Some("Purple".into()),
    };
    core.update_settings(patch).unwrap();
    let payload = core.bootstrap();
    assert_eq!(payload.config.volume_step, 5);
    assert_eq!(payload.config.volume_step_large, 20);
    assert_eq!(
        payload.config.appearance.theme,
        volumectl_lib::ui::ThemeMode::Dark
    );
    assert_eq!(
        payload.config.appearance.material,
        volumectl_lib::ui::MaterialMode::Opaque
    );
    assert_eq!(
        payload.config.appearance.motion,
        volumectl_lib::ui::MotionMode::Reduced
    );
    assert_eq!(
        payload.config.appearance.accent,
        volumectl_lib::ui::AccentMode::Purple
    );
}

#[test]
fn update_settings_rejects_unknown_enum_strings() {
    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink.clone());
    let err = core
        .update_settings(volumectl_lib::host_core::SettingsPatch {
            volume_step: None,
            volume_step_large: None,
            theme: Some("Neon".into()),
            material: None,
            motion: None,
            accent: None,
        })
        .unwrap_err();
    assert!(err.contains("theme"), "unexpected error: {err}");
    // No partial application on error: the step stays at the default.
    assert_eq!(core.bootstrap().config.volume_step, 1);
}

#[test]
fn update_settings_rejects_out_of_range_or_unordered_steps() {
    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink.clone());

    // Small step above the config.rs ceiling (1..=50).
    let err = core
        .update_settings(volumectl_lib::host_core::SettingsPatch {
            volume_step: Some(51),
            volume_step_large: None,
            theme: None,
            material: None,
            motion: None,
            accent: None,
        })
        .unwrap_err();
    assert!(err.contains("volume_step"), "unexpected error: {err}");

    // Large step not greater than the small step.
    let err = core
        .update_settings(volumectl_lib::host_core::SettingsPatch {
            volume_step: Some(10),
            volume_step_large: Some(10),
            theme: None,
            material: None,
            motion: None,
            accent: None,
        })
        .unwrap_err();
    assert!(
        err.contains("greater than volume_step"),
        "unexpected error: {err}"
    );

    // No partial application on any rejection: defaults are untouched.
    let payload = core.bootstrap();
    assert_eq!(payload.config.volume_step, 1);
    assert_eq!(payload.config.volume_step_large, 10);
}

#[test]
fn update_settings_validates_large_against_patched_small() {
    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink.clone());

    // Patching only the large step below the current small step must reject.
    let err = core
        .update_settings(volumectl_lib::host_core::SettingsPatch {
            volume_step: None,
            volume_step_large: Some(1),
            theme: None,
            material: None,
            motion: None,
            accent: None,
        })
        .unwrap_err();
    assert!(
        err.contains("greater than volume_step"),
        "unexpected error: {err}"
    );
}

#[test]
fn volume_action_shows_overlay() {
    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink.clone());
    core.apply_hotkey(volumectl_lib::hotkeys::HotkeyAction::VolumeUp);
    assert!(
        sink.events
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.starts_with("overlay:")),
        "hotkey volume change must show the HUD overlay (legacy parity), got {:?}",
        *sink.events.lock().unwrap()
    );
    assert!(
        sink.events
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.starts_with("volume:")),
        "volume event must also be emitted"
    );
}

#[test]
fn config_only_paths_do_not_show_overlay() {
    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink.clone());
    // A non-volume action (open a surface) must not flash the HUD overlay.
    core.handle_action(AppAction::ShowSurface(
        volumectl_lib::ui::SurfaceId::Settings,
    ));
    assert!(
        !sink
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.starts_with("overlay:")),
        "non-volume actions must not show the overlay, got {:?}",
        *sink.events.lock().unwrap()
    );
}

#[test]
fn save_config_resyncs_mtime_so_reload_does_not_echo() {
    // Point the config path at a temp dir so the test never touches the real
    // user config.
    let tmp = std::env::temp_dir().join(format!("volumectl-hostcore-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let old = std::env::var_os("APPDATA");
    std::env::set_var("APPDATA", &tmp);

    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink.clone());

    // A save adopts the config and must resync the mtime: the next
    // reload_config_if_changed() call must NOT fire a spurious reload (which
    // would re-show the HUD + re-register hotkeys after every Settings save).
    core.save_config().unwrap();
    assert!(
        !core.reload_config_if_changed(),
        "save must resync the config mtime (no spurious reload)"
    );

    // Restore the environment.
    match old {
        Some(v) => std::env::set_var("APPDATA", v),
        None => std::env::remove_var("APPDATA"),
    }
    let _ = std::fs::remove_dir_all(&tmp);
}
