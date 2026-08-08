use std::sync::{Arc, Mutex};

use volumectl_lib::audio::{AudioBackend, AudioError, VolumeState};
use volumectl_lib::config::HotkeyModifier;
use volumectl_lib::hotkeys::HotkeyAction;
use volumectl_lib::linux_host_core::{HostCore, HotkeySource};
use volumectl_lib::ui::{AppAction, SurfaceId, UiStatus};

struct FakeAudio {
    state: Mutex<VolumeState>,
    fail_mutations: bool,
    fail_reads: bool,
}

impl FakeAudio {
    fn new(volume: f32, muted: bool) -> Self {
        Self {
            state: Mutex::new(VolumeState { volume, muted }),
            fail_mutations: false,
            fail_reads: false,
        }
    }

    fn failing_mutations(volume: f32, muted: bool) -> Self {
        Self {
            state: Mutex::new(VolumeState { volume, muted }),
            fail_mutations: true,
            fail_reads: false,
        }
    }

    fn failing_reads(volume: f32, muted: bool) -> Self {
        Self {
            state: Mutex::new(VolumeState { volume, muted }),
            fail_mutations: false,
            fail_reads: true,
        }
    }
}

impl AudioBackend for FakeAudio {
    fn get_state(&self) -> Result<VolumeState, AudioError> {
        if self.fail_reads {
            return Err(AudioError::DeviceLost);
        }
        Ok(*self.state.lock().expect("fake audio state lock"))
    }

    fn set_volume(&self, volume: f32) -> Result<(), AudioError> {
        if self.fail_mutations {
            return Err(AudioError::Io("set volume failed".into()));
        }
        self.state.lock().expect("fake audio state lock").volume = volume;
        Ok(())
    }

    fn toggle_mute(&self) -> Result<VolumeState, AudioError> {
        if self.fail_mutations {
            return Err(AudioError::Io("toggle mute failed".into()));
        }
        let mut state = self.state.lock().expect("fake audio state lock");
        state.muted = !state.muted;
        Ok(*state)
    }

    fn set_mute(&self, muted: bool) -> Result<(), AudioError> {
        if self.fail_mutations {
            return Err(AudioError::Io("set mute failed".into()));
        }
        self.state.lock().expect("fake audio state lock").muted = muted;
        Ok(())
    }
}

struct FakeHotkeys {
    queued: Mutex<Vec<HotkeyAction>>,
    failure: Mutex<Option<String>>,
    modifier: Mutex<Option<HotkeyModifier>>,
}

impl FakeHotkeys {
    fn with_action(action: HotkeyAction) -> Self {
        Self {
            queued: Mutex::new(vec![action]),
            failure: Mutex::new(None),
            modifier: Mutex::new(None),
        }
    }

    fn failing(message: &str) -> Self {
        Self {
            queued: Mutex::new(Vec::new()),
            failure: Mutex::new(Some(message.into())),
            modifier: Mutex::new(None),
        }
    }

    fn modifier(&self) -> Option<HotkeyModifier> {
        *self.modifier.lock().expect("hotkey modifier lock")
    }
}

impl HotkeySource for FakeHotkeys {
    fn try_recv(&self) -> Option<HotkeyAction> {
        self.queued.lock().expect("hotkey queue lock").pop()
    }

    fn listener_failure(&self) -> Option<String> {
        self.failure.lock().expect("hotkey failure lock").clone()
    }

    fn set_modifier(&self, modifier: HotkeyModifier) {
        *self.modifier.lock().expect("hotkey modifier lock") = Some(modifier);
    }
}

struct ArcHotkeys(Arc<FakeHotkeys>);

impl HotkeySource for ArcHotkeys {
    fn try_recv(&self) -> Option<HotkeyAction> {
        self.0.try_recv()
    }

    fn listener_failure(&self) -> Option<String> {
        self.0.listener_failure()
    }

    fn set_modifier(&self, modifier: HotkeyModifier) {
        self.0.set_modifier(modifier);
    }
}

#[test]
fn wayland_hotkeys_can_be_reported_unavailable_without_evdev() {
    let audio = Box::new(FakeAudio::new(0.5, false));
    let mut host = HostCore::new(
        Some(audio),
        None,
        volumectl_lib::config::Config::default(),
        test_caps_with_compositor(true),
    );

    host.mark_degraded("global hotkeys unavailable: current Linux provider requires X11");

    assert!(host.capabilities().compositor);
    assert!(host
        .degraded_reasons()
        .iter()
        .any(|reason| reason.contains("requires X11")));
    assert!(!host.quit_requested());
}

#[test]
fn hotkey_modifier_changes_are_synchronized_on_reload() {
    let audio = Box::new(FakeAudio::new(0.5, false));
    let hotkeys = Arc::new(FakeHotkeys::with_action(HotkeyAction::Reset50));
    let mut host = HostCore::new(
        Some(audio),
        Some(Box::new(ArcHotkeys(Arc::clone(&hotkeys)))),
        volumectl_lib::config::Config::default(),
        test_caps(),
    );
    let config = volumectl_lib::config::Config {
        modifier: HotkeyModifier::Alt,
        ..volumectl_lib::config::Config::default()
    };

    host.apply_reloaded_config(config, None);

    assert_eq!(hotkeys.modifier(), Some(HotkeyModifier::Alt));
}

#[test]
fn host_core_accepts_injected_audio_backend() {
    let audio = Box::new(FakeAudio::new(0.5, false));
    let host = HostCore::for_test(audio);

    assert_eq!(host.state().volume_percent, 50);
    assert!(!host.quit_requested());
}

#[test]
fn set_volume_clamps_and_refreshes_confirmed_state() {
    let audio = Box::new(FakeAudio::new(0.5, false));
    let mut host = HostCore::for_test(audio);

    host.apply_action(&AppAction::SetVolumePercent { percent: 125 });
    host.refresh_audio();

    assert_eq!(host.state().volume_percent, 100);
    assert_eq!(host.state().status, UiStatus::Ready);
}

#[test]
fn adjust_volume_reads_confirmed_value_before_mutating() {
    let audio = Box::new(FakeAudio::new(0.5, false));
    let mut host = HostCore::for_test(audio);

    host.apply_action(&AppAction::AdjustVolume { delta_percent: 10 });
    host.refresh_audio();

    assert_eq!(host.state().volume_percent, 60);
}

#[test]
fn failed_audio_mutation_keeps_previous_confirmed_state() {
    let audio = Box::new(FakeAudio::failing_mutations(0.5, false));
    let mut host = HostCore::for_test(audio);

    host.apply_action(&AppAction::SetVolumePercent { percent: 90 });

    assert_eq!(host.state().volume_percent, 50);
    assert!(matches!(host.state().status, UiStatus::Error(_)));
}

#[test]
fn tray_action_is_deferred_without_shutdown() {
    let audio = Box::new(FakeAudio::new(0.5, false));
    let mut host = HostCore::for_test(audio);

    host.apply_action(&AppAction::OpenTrayMenu);

    assert!(!host.quit_requested());
}

#[test]
fn exit_requests_shutdown() {
    let audio = Box::new(FakeAudio::new(0.5, false));
    let mut host = HostCore::for_test(audio);

    host.apply_action(&AppAction::Exit);

    assert!(host.quit_requested());
}

#[test]
fn reload_action_marks_config_for_the_next_host_poll() {
    let audio = Box::new(FakeAudio::new(0.5, false));
    let mut host = HostCore::for_test(audio);

    host.apply_action(&AppAction::ReloadConfig);

    assert!(host.take_config_reload_request());
    assert!(!host.take_config_reload_request());
}

#[test]
fn surface_and_appearance_actions_update_host_state() {
    let audio = Box::new(FakeAudio::new(0.5, false));
    let mut host = HostCore::for_test(audio);

    host.apply_action(&AppAction::ShowSurface(SurfaceId::Mixer));

    assert!(host.state().is_visible(SurfaceId::Mixer));
}

#[test]
fn hotkey_opens_mixer_through_host_core() {
    let audio = Box::new(FakeAudio::new(0.5, false));
    let hotkeys = FakeHotkeys::with_action(HotkeyAction::OpenMixer);
    let mut host = HostCore::new(
        Some(audio),
        Some(Box::new(hotkeys)),
        volumectl_lib::config::Config::default(),
        test_caps(),
    );

    host.poll_hotkeys();

    assert!(host.state().is_visible(SurfaceId::Mixer));
}

#[test]
fn listener_failure_marks_hotkeys_degraded_but_keeps_host_alive() {
    let audio = Box::new(FakeAudio::new(0.5, false));
    let hotkeys = FakeHotkeys::failing("permission denied");
    let mut host = HostCore::new(
        Some(audio),
        Some(Box::new(hotkeys)),
        volumectl_lib::config::Config::default(),
        test_caps(),
    );

    host.poll_hotkeys();

    assert!(!host.quit_requested());
    assert!(host
        .degraded_reasons()
        .iter()
        .any(|reason| reason == "global hotkeys unavailable: permission denied"));
}

#[test]
fn read_failure_does_not_adjust_from_fake_zero() {
    let audio = Box::new(FakeAudio::failing_reads(0.5, false));
    let mut host = HostCore::for_test(audio);

    host.apply_action(&AppAction::AdjustVolume { delta_percent: 10 });

    assert_eq!(host.state().volume_percent, 0);
    assert!(matches!(host.state().status, UiStatus::Error(_)));
    assert!(host
        .degraded_reasons()
        .iter()
        .any(|reason| reason.starts_with("audio:")));
}

#[test]
fn device_loss_drops_backend_for_bounded_recovery() {
    let audio = Box::new(FakeAudio::failing_reads(0.5, false));
    let host = HostCore::for_test(audio);

    assert!(!host.audio_available());
    assert!(host.can_retry_audio());
}

#[test]
fn audio_retry_budget_is_bounded_when_backend_stays_unavailable() {
    let mut host = HostCore::new(
        None,
        None,
        volumectl_lib::config::Config::default(),
        test_caps(),
    );
    assert!(host.can_retry_audio());

    for _ in 0..20 {
        host.record_audio_retry();
    }

    assert!(!host.can_retry_audio());
}

#[test]
fn malformed_config_reload_preserves_previous_config() {
    let audio = Box::new(FakeAudio::new(0.5, false));
    let mut host = HostCore::for_test(audio);
    let before = host.config().clone();
    let path = std::env::temp_dir().join(format!("volumectl-invalid-{}.json", std::process::id()));
    std::fs::write(&path, b"{").expect("write malformed config");

    assert!(host.reload_config_at(&path, None).is_err());
    assert_eq!(host.config(), &before);
    let _ = std::fs::remove_file(path);
}

fn test_caps_with_compositor(compositor: bool) -> volumectl_lib::ui::UiCapabilities {
    volumectl_lib::ui::UiCapabilities {
        compositor,
        blur: compositor,
        high_contrast: false,
        reduced_motion: false,
        dpi_scale: 1.0,
        work_area: volumectl_lib::ui::WorkArea::new(0, 0, 1600, 900),
    }
}

fn test_caps() -> volumectl_lib::ui::UiCapabilities {
    volumectl_lib::ui::UiCapabilities {
        compositor: false,
        blur: false,
        high_contrast: false,
        reduced_motion: false,
        dpi_scale: 1.0,
        work_area: volumectl_lib::ui::WorkArea::new(0, 0, 1600, 900),
    }
}
