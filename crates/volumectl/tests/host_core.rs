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
fn session_commands_are_noop_on_unsupported_platform() {
    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink.clone());
    // Task 2 interface contract: no-op until the Windows WASAPI source (2b).
    assert_eq!(core.sessions(), vec![]);
    assert!(core.set_session_volume("stale-id", 40).is_ok());
    assert!(core.mute_session("stale-id").is_ok());
}

#[test]
fn bootstrap_reports_default_step_and_empty_sessions() {
    let sink = Arc::new(RecordingSink::default());
    let mut core = core_with(sink.clone());
    let payload = core.bootstrap();
    assert_eq!(payload.config.volume_step, 1);
    assert!(!payload.sessions_supported);
    assert!(payload.sessions.is_empty());
    assert_eq!(payload.hotkey_status.len(), 8);
}
