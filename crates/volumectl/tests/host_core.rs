//! Integration tests for the cross-platform [`volumectl_lib::host_core::AppCore`].

use std::sync::{Arc, Mutex};

use volumectl_lib::audio::{AudioBackend, AudioError, VolumeState};
use volumectl_lib::config::{Config, HotkeyBindings, HotkeyModifier};
use volumectl_lib::host_core::{AppCore, AudioSessionInfo, EventSink};
use volumectl_lib::hotkeys::HotkeyRegResult;
use volumectl_lib::ui::AppAction;

/// Serializes tests that mutate the process-global `VOLUMECTL_CONFIG_DIR` env
/// var (which `config_path()` reads).
///
/// WHY THE LOCK IS THE ROOT-CAUSE FIX (not a band-aid): the two mtime tests
/// run concurrently in this binary. Without the lock, one test's set/restore
/// of the env var interleaves with the other's save->resync->reload window,
/// so `config_path()` changes mid-test: `save_config` writes + resyncs the
/// mtime in dir A while `reload_config_if_changed` reads the mtime from dir B
/// (or the real user config), the mtimes differ, the reload fires spuriously
/// and the assertion panics. Reproduced: 5/8 runs failed with
/// "set_modifier must resync the config mtime (no spurious reload)" at the
/// reload assertion. The production save path is flush-safe (config_ini's
/// sibling temp file + write_all + sync_all + atomic replacement) and NTFS/
/// ext4 mtime resolution is fine-grained, so there is no secondary mtime
/// granularity or flush mechanism — the env race is the only failure mode.
/// The lock makes each test see a stable config path for its whole body.
static CONFIG_DIR_LOCK: Mutex<()> = Mutex::new(());

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
        let mut state = self.state.lock().unwrap();
        state.muted = !state.muted;
        Ok(*state)
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
    fn open_surface(&self, label: &str) {
        self.events.lock().unwrap().push(format!("open:{label}"));
    }
    fn toggle_surface(&self, label: &str) {
        self.events.lock().unwrap().push(format!("toggle:{label}"));
    }
    fn backend(&self, status: &volumectl_lib::host_core::BackendStatus) {
        use volumectl_lib::host_core::BackendStatus;
        let text = match status {
            BackendStatus::Ready => "backend:ready".to_string(),
            BackendStatus::Degraded { error } => {
                format!("backend:degraded:{error}")
            }
        };
        self.events.lock().unwrap().push(text);
    }
}

/// [`StubAudio`] with a controllable failure flag for degradation tests.
struct FlakyAudio {
    state: Arc<Mutex<VolumeState>>,
    failing: Arc<Mutex<bool>>,
}

impl AudioBackend for FlakyAudio {
    fn get_state(&self) -> Result<VolumeState, AudioError> {
        if *self.failing.lock().unwrap() {
            Err(AudioError::DeviceLost)
        } else {
            Ok(*self.state.lock().unwrap())
        }
    }
    fn set_volume(&self, volume: f32) -> Result<(), AudioError> {
        if *self.failing.lock().unwrap() {
            Err(AudioError::DeviceLost)
        } else {
            self.state.lock().unwrap().volume = volume;
            Ok(())
        }
    }
    fn toggle_mute(&self) -> Result<VolumeState, AudioError> {
        let mut state = self.state.lock().unwrap();
        state.muted = !state.muted;
        Ok(*state)
    }
    fn set_mute(&self, muted: bool) -> Result<(), AudioError> {
        self.state.lock().unwrap().muted = muted;
        Ok(())
    }
}

fn flaky_core(sink: Arc<RecordingSink>) -> (AppCore, Arc<Mutex<bool>>, Arc<Mutex<VolumeState>>) {
    let failing = Arc::new(Mutex::new(false));
    let state = Arc::new(Mutex::new(VolumeState {
        volume: 0.5,
        muted: false,
    }));
    let audio = FlakyAudio {
        state: state.clone(),
        failing: failing.clone(),
    };
    let core = AppCore::new_without_native_hotkeys(
        Box::new(audio),
        Config::default(),
        HotkeyModifier::CtrlAlt,
        sink,
        None,
    )
    .unwrap();
    (core, failing, state)
}

#[test]
fn backend_degradation_publishes_one_shot_status_and_recovers() {
    let sink = Arc::new(RecordingSink::default());
    let (mut core, failing, state) = flaky_core(sink.clone());

    // Healthy: no degraded event on the initial publish.
    assert!(!sink
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|e| e.starts_with("backend:")));

    // Fail the backend: the next probe emits exactly one degraded event and
    // does not publish a volume event (no fake values).
    *failing.lock().unwrap() = true;
    core.sync_external_state();
    let events = sink.events.lock().unwrap().clone();
    assert_eq!(
        events
            .iter()
            .filter(|e| e.starts_with("backend:"))
            .collect::<Vec<_>>(),
        vec!["backend:degraded:audio device stopped responding"]
    );
    // Only the constructor's startup publish exists; the degraded probe
    // must not fabricate a volume event.
    assert_eq!(
        events.iter().filter(|e| e.starts_with("volume:")).count(),
        1
    );

    // Repeat probes stay silent (one-shot transition), no spam.
    core.sync_external_state();
    core.publish_confirmed_state(true);
    let events = sink.events.lock().unwrap().clone();
    assert_eq!(
        events.iter().filter(|e| e.starts_with("backend:")).count(),
        1
    );

    // Recovery emits ready once and resumes publishing live values.
    *failing.lock().unwrap() = false;
    *state.lock().unwrap() = VolumeState {
        volume: 0.6,
        muted: false,
    };
    core.sync_external_state();
    let events = sink.events.lock().unwrap().clone();
    assert!(events.iter().any(|e| e == "backend:ready"));
    assert!(events.iter().any(|e| e == "volume:60:false"));
}

#[test]
fn bootstrap_reports_degraded_backend_when_the_probe_fails() {
    let sink = Arc::new(RecordingSink::default());
    let (mut core, failing, _state) = flaky_core(sink.clone());
    *failing.lock().unwrap() = true;

    let payload = core.bootstrap();
    use volumectl_lib::host_core::BackendStatus;
    assert_eq!(
        payload.backend_status,
        BackendStatus::Degraded {
            error: "audio device stopped responding".to_string()
        }
    );
}

fn core_with(sink: Arc<RecordingSink>) -> AppCore {
    // Host-core tests never create the OS-level hotkey manager: parallel
    // Carbon registration from test threads (and headless CI sessions) can
    // abort the test process natively. The degraded no-manager state covers
    // the same AppCore logic; native registration is exercised by the real
    // host and the E2E matrix instead.
    AppCore::new_without_native_hotkeys(
        Box::new(StubAudio {
            state: Mutex::new(VolumeState {
                volume: 0.5,
                muted: false,
            }),
        }),
        Config::default(),
        HotkeyModifier::CtrlAlt,
        sink,
        None,
    )
    .unwrap()
}

#[cfg(target_os = "linux")]
#[test]
fn linux_core_uses_a_supported_sessions_source() {
    // Point Pulse at a dead socket so the test is hermetic and fast even on
    // runners without a Pulse server; restore afterwards.
    let old = std::env::var_os("PULSE_SERVER");
    std::env::set_var("PULSE_SERVER", "tcp:127.0.0.1:1");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut core = core_with(Arc::new(RecordingSink::default()));
        // The Linux host wires PulseSessions: bootstrap reports per-app
        // sessions as supported even when the server is unreachable
        // (macOS/other targets intentionally report false).
        let payload = core.bootstrap();
        assert!(payload.sessions_supported);
        assert!(payload.sessions.is_empty());
    }));
    match old {
        Some(v) => std::env::set_var("PULSE_SERVER", v),
        None => std::env::remove_var("PULSE_SERVER"),
    }
    assert!(result.is_ok(), "linux sessions wiring test panicked");
}

#[test]
fn bootstrap_exposes_config_load_notice_without_changing_config_shape() {
    let sink = Arc::new(RecordingSink::default());
    let core = AppCore::new_without_native_hotkeys(
        Box::new(StubAudio {
            state: Mutex::new(VolumeState {
                volume: 0.5,
                muted: false,
            }),
        }),
        Config::default(),
        HotkeyModifier::CtrlAlt,
        sink,
        Some(volumectl_lib::config::ConfigLoadNotice::MigratedFromJson),
    )
    .unwrap();
    let mut core = core;
    let payload = core.bootstrap();

    assert_eq!(
        payload.config_notice,
        Some(volumectl_lib::config::ConfigLoadNotice::MigratedFromJson)
    );
    assert_eq!(payload.config.volume_step, 1);
}

#[test]
fn constructor_publishes_initial_confirmed_state() {
    let sink = Arc::new(RecordingSink::default());
    let _core = core_with(sink.clone());
    assert!(
        sink.events
            .lock()
            .unwrap()
            .iter()
            .any(|event| event == "volume:50:false"),
        "startup must publish the initial state so native surfaces are not blank"
    );
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
fn volume_actions_cover_small_large_reset_clamping_and_mute() {
    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink);

    core.handle_action(AppAction::AdjustVolume { delta_percent: 1 });
    assert_eq!(core.bootstrap().volume_pct, 51);

    core.handle_action(AppAction::AdjustVolume { delta_percent: -1 });
    assert_eq!(core.bootstrap().volume_pct, 50);

    core.handle_action(AppAction::AdjustVolume { delta_percent: 10 });
    assert_eq!(core.bootstrap().volume_pct, 60);

    core.handle_action(AppAction::AdjustVolume { delta_percent: -10 });
    assert_eq!(core.bootstrap().volume_pct, 50);

    core.handle_action(AppAction::SetVolumePercent { percent: 125 });
    assert_eq!(core.bootstrap().volume_pct, 100);
    core.handle_action(AppAction::AdjustVolume { delta_percent: 10 });
    assert_eq!(core.bootstrap().volume_pct, 100);

    core.handle_action(AppAction::SetVolumePercent { percent: 0 });
    core.handle_action(AppAction::AdjustVolume { delta_percent: -10 });
    assert_eq!(core.bootstrap().volume_pct, 0);

    core.handle_action(AppAction::ResetVolume);
    assert_eq!(core.bootstrap().volume_pct, 50);

    core.handle_action(AppAction::ToggleMute);
    assert!(core.bootstrap().muted);
    assert_eq!(core.bootstrap().volume_pct, 0);
    core.handle_action(AppAction::ToggleMute);
    assert!(!core.bootstrap().muted);
    assert_eq!(core.bootstrap().volume_pct, 50);
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
        ..Default::default()
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
fn update_settings_mutates_autostart_preference_without_touching_registry() {
    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink);

    core.update_settings(volumectl_lib::host_core::SettingsPatch {
        autostart: Some(true),
        ..Default::default()
    })
    .expect("auto-start preference is a valid boolean");

    assert!(core.bootstrap().config.autostart);
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
        })
        .unwrap_err();
    assert!(
        err.contains("greater than volume_step"),
        "unexpected error: {err}"
    );
}

#[test]
fn update_settings_mutates_overlay_beep_and_thresholds() {
    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink.clone());
    let patch = volumectl_lib::host_core::SettingsPatch {
        overlay_duration_ms: Some(5000),
        beep: Some(volumectl_lib::host_core::BeepPatch {
            enabled: Some(false),
            blocked_freq: Some(500),
            blocked_duration_ms: Some(120),
            limit_freq: Some(700),
            limit_duration_ms: Some(90),
        }),
        color_thresholds: Some(volumectl_lib::host_core::ColorThresholdsPatch {
            green_up_to: Some(30),
            blue_up_to: Some(60),
            orange_up_to: Some(90),
        }),
        ..Default::default()
    };
    core.update_settings(patch).unwrap();
    let cfg = core.bootstrap().config;
    assert_eq!(cfg.overlay_duration_ms, 5000);
    assert!(!cfg.beep.enabled);
    assert_eq!(cfg.beep.blocked_freq, 500);
    assert_eq!(cfg.beep.blocked_duration_ms, 120);
    assert_eq!(cfg.beep.limit_freq, 700);
    assert_eq!(cfg.beep.limit_duration_ms, 90);
    assert_eq!(cfg.color_thresholds.green_up_to, 30);
    assert_eq!(cfg.color_thresholds.blue_up_to, 60);
    assert_eq!(cfg.color_thresholds.orange_up_to, 90);
}

#[test]
fn update_settings_rejects_invalid_overlay_beep_and_thresholds() {
    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink.clone());

    // Overlay duration below the 200 ms floor.
    let err = core
        .update_settings(volumectl_lib::host_core::SettingsPatch {
            overlay_duration_ms: Some(50),
            ..Default::default()
        })
        .unwrap_err();
    assert_eq!(err, "overlay_duration_ms: must be between 200 and 10000");

    // Beep frequency above the 32,767 Hz ceiling.
    let err = core
        .update_settings(volumectl_lib::host_core::SettingsPatch {
            beep: Some(volumectl_lib::host_core::BeepPatch {
                blocked_freq: Some(40000),
                ..Default::default()
            }),
            ..Default::default()
        })
        .unwrap_err();
    assert_eq!(err, "beep.blocked_freq: must be between 37 and 32767");

    // Beep duration below the 10 ms floor.
    let err = core
        .update_settings(volumectl_lib::host_core::SettingsPatch {
            beep: Some(volumectl_lib::host_core::BeepPatch {
                limit_duration_ms: Some(5),
                ..Default::default()
            }),
            ..Default::default()
        })
        .unwrap_err();
    assert_eq!(err, "beep.limit_duration_ms: must be between 10 and 2000");

    // Threshold order violation (green above blue).
    let err = core
        .update_settings(volumectl_lib::host_core::SettingsPatch {
            color_thresholds: Some(volumectl_lib::host_core::ColorThresholdsPatch {
                green_up_to: Some(90),
                blue_up_to: Some(50),
                ..Default::default()
            }),
            ..Default::default()
        })
        .unwrap_err();
    assert_eq!(
        err,
        "color_thresholds.green_up_to: must not exceed blue_up_to"
    );

    // No partial application on any rejection.
    let payload = core.bootstrap();
    assert_eq!(payload.config.overlay_duration_ms, 1800);
    assert!(payload.config.beep.enabled);
    assert_eq!(payload.config.color_thresholds.green_up_to, 40);
}

#[test]
fn update_settings_blacklist_replace_normalizes_entries() {
    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink.clone());
    core.update_settings(volumectl_lib::host_core::SettingsPatch {
        blacklist: Some(vec![" Code ".into(), "notepad++".into()]),
        ..Default::default()
    })
    .unwrap();
    let list = core.bootstrap().config.blacklist;
    #[cfg(target_os = "windows")]
    assert_eq!(
        list,
        vec!["code.exe".to_string(), "notepad++.exe".to_string()]
    );
    #[cfg(target_os = "macos")]
    assert_eq!(
        list,
        vec!["code.app".to_string(), "notepad++.app".to_string()]
    );
    #[cfg(not(target_os = "windows"))]
    #[cfg(not(target_os = "macos"))]
    assert_eq!(list, vec!["code".to_string(), "notepad++".to_string()]);
}

#[test]
fn update_settings_adopts_recorded_hotkey_bindings() {
    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink);
    let bindings = HotkeyBindings {
        toggle_mute: "Ctrl+Shift+KeyU".into(),
        ..HotkeyBindings::default()
    };

    core.update_settings(volumectl_lib::host_core::SettingsPatch {
        hotkeys: Some(bindings.clone()),
        ..Default::default()
    })
    .expect("valid recorded bindings are accepted");

    assert_eq!(core.bootstrap().config.hotkeys, Some(bindings));
}

#[test]
fn blacklist_app_actions_mutate_and_persist() {
    let _guard = CONFIG_DIR_LOCK.lock().unwrap();
    let tmp = std::env::temp_dir().join(format!("volumectl-blacklist-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let old = std::env::var_os("VOLUMECTL_CONFIG_DIR");
    std::env::set_var("VOLUMECTL_CONFIG_DIR", &tmp);

    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink.clone());

    // Add: normalized, deduplicated, persisted.
    core.handle_action(AppAction::AddBlacklistEntry("Code".into()));
    core.handle_action(AppAction::AddBlacklistEntry("Code".into()));
    let list = core.bootstrap().config.blacklist;
    #[cfg(target_os = "windows")]
    assert_eq!(list, vec!["code.exe".to_string()]);
    #[cfg(target_os = "macos")]
    assert_eq!(list, vec!["code.app".to_string()]);
    #[cfg(not(target_os = "windows"))]
    #[cfg(not(target_os = "macos"))]
    assert_eq!(list, vec!["code".to_string()]);

    // Remove: only the matching normalized entry disappears.
    core.handle_action(AppAction::AddBlacklistEntry("Chrome".into()));
    core.handle_action(AppAction::RemoveBlacklistEntry("chrome".into()));
    #[cfg(target_os = "windows")]
    assert_eq!(
        core.bootstrap().config.blacklist,
        vec!["code.exe".to_string()]
    );
    #[cfg(target_os = "macos")]
    assert_eq!(
        core.bootstrap().config.blacklist,
        vec!["code.app".to_string()]
    );
    #[cfg(not(target_os = "windows"))]
    #[cfg(not(target_os = "macos"))]
    assert_eq!(core.bootstrap().config.blacklist, vec!["code".to_string()]);

    // Apply Recommended: merges the modifier's presets (dedupe) and persists.
    core.handle_action(AppAction::ApplyRecommendedBlacklist);
    let after = core.bootstrap().config.blacklist.clone();
    let saved = volumectl_lib::config_ini::load_ini(&volumectl_lib::config::config_path())
        .expect("load persisted INI");
    assert_eq!(
        saved.blacklist, after,
        "persisted list must match in-memory"
    );
    assert!(!after.is_empty());

    // Clear: empties the list and persists.
    core.handle_action(AppAction::ClearBlacklist);
    assert!(core.bootstrap().config.blacklist.is_empty());
    let saved = volumectl_lib::config_ini::load_ini(&volumectl_lib::config::config_path())
        .expect("load persisted INI");
    assert!(saved.blacklist.is_empty());

    match old {
        Some(v) => std::env::set_var("VOLUMECTL_CONFIG_DIR", v),
        None => std::env::remove_var("VOLUMECTL_CONFIG_DIR"),
    }
    let _ = std::fs::remove_dir_all(&tmp);
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
    let _guard = CONFIG_DIR_LOCK.lock().unwrap();
    // Point the config path at a temp dir (cross-platform `VOLUMECTL_CONFIG_DIR`
    // override) so the test never touches the real user config.
    let tmp = std::env::temp_dir().join(format!("volumectl-hostcore-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let old = std::env::var_os("VOLUMECTL_CONFIG_DIR");
    std::env::set_var("VOLUMECTL_CONFIG_DIR", &tmp);

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
        Some(v) => std::env::set_var("VOLUMECTL_CONFIG_DIR", v),
        None => std::env::remove_var("VOLUMECTL_CONFIG_DIR"),
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn set_modifier_resyncs_mtime_so_reload_does_not_echo() {
    let _guard = CONFIG_DIR_LOCK.lock().unwrap();
    let tmp = std::env::temp_dir().join(format!("volumectl-modifier-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let old = std::env::var_os("VOLUMECTL_CONFIG_DIR");
    std::env::set_var("VOLUMECTL_CONFIG_DIR", &tmp);

    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink.clone());

    // Changing the modifier persists + adopts; the next reload must NOT fire a
    // spurious reload (regression: set_modifier previously saved without
    // resyncing the mtime, flashing the HUD ~150 ms after every pick).
    core.set_modifier(HotkeyModifier::Alt).unwrap();
    assert!(
        !core.reload_config_if_changed(),
        "set_modifier must resync the config mtime (no spurious reload)"
    );

    match old {
        Some(v) => std::env::set_var("VOLUMECTL_CONFIG_DIR", v),
        None => std::env::remove_var("VOLUMECTL_CONFIG_DIR"),
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn reload_preserves_previous_config_on_malformed_ini_then_accepts_valid_edit() {
    let _guard = CONFIG_DIR_LOCK.lock().unwrap();
    let tmp = std::env::temp_dir().join(format!("volumectl-reload-ini-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let old = std::env::var_os("VOLUMECTL_CONFIG_DIR");
    std::env::set_var("VOLUMECTL_CONFIG_DIR", &tmp);

    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink);
    std::fs::write(
        volumectl_lib::config::config_path(),
        "[general]\nvolume_step=not-a-number\n",
    )
    .unwrap();

    assert!(!core.reload_config_if_changed());
    assert_eq!(core.bootstrap().config.volume_step, 1);

    let edited = Config {
        volume_step: 5,
        volume_step_large: 20,
        ..Config::default()
    };
    std::fs::write(
        volumectl_lib::config::config_path(),
        volumectl_lib::config_ini::serialize_ini(&edited).unwrap(),
    )
    .unwrap();
    core.force_reload_config();
    assert_eq!(core.bootstrap().config.volume_step, 5);
    assert_eq!(core.bootstrap().config.volume_step_large, 20);
    assert!(!core.reload_config_if_changed());

    match old {
        Some(v) => std::env::set_var("VOLUMECTL_CONFIG_DIR", v),
        None => std::env::remove_var("VOLUMECTL_CONFIG_DIR"),
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn toggle_mixer_routes_to_toggle_surface_but_show_routes_to_open() {
    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink.clone());
    // The Mixer hotkey is ToggleSurface: it must toggle (open if closed,
    // close if open) — legacy app.rs behavior, spec §5.1.
    core.handle_action(volumectl_lib::ui::AppAction::ToggleSurface(
        volumectl_lib::ui::SurfaceId::Mixer,
    ));
    // Explicit Show stays an open.
    core.handle_action(volumectl_lib::ui::AppAction::ShowSurface(
        volumectl_lib::ui::SurfaceId::Mixer,
    ));
    let events = sink.events.lock().unwrap().clone();
    assert!(
        events.iter().any(|e| e == "toggle:window-mixer"),
        "{events:?}"
    );
    assert!(
        events.iter().any(|e| e == "open:window-mixer"),
        "{events:?}"
    );
}
