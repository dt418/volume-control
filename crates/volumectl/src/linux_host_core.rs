//! Platform-neutral Linux host state and action reducer.
//!
//! GTK and the concrete Linux renderer stay in `linux_app`; this module owns
//! only confirmed state, backend actions, hotkey capability state, and
//! configuration-facing host behavior so it can be tested without a display.

use std::path::Path;
use std::time::SystemTime;

use crate::audio::{AudioBackend, AudioError};
use crate::config::{self, Config, HotkeyModifier};
use crate::hotkeys::HotkeyAction;
use crate::ui::{AppAction, AppState, SurfaceId, SurfaceVisibility, UiCapabilities, UiStatus};

/// The small interface the host needs from a global-hotkey provider.
///
/// Keeping this seam local to the Linux host allows deterministic tests without
/// starting an rdev listener or requiring an X11 display.
pub trait HotkeySource {
    fn try_recv(&self) -> Option<HotkeyAction>;
    fn listener_failure(&self) -> Option<String>;
    fn set_modifier(&self, modifier: HotkeyModifier);
}

/// Main-thread-owned, platform-neutral Linux host state.
const MAX_AUDIO_RETRIES: u8 = 20;

pub struct HostCore {
    audio: Option<Box<dyn AudioBackend>>,
    hotkeys: Option<Box<dyn HotkeySource>>,
    config: Config,
    last_config_mtime: Option<SystemTime>,
    config_reload_requested: bool,
    audio_retry_attempts: u8,
    state: AppState,
    caps: UiCapabilities,
    quit_requested: bool,
    degraded: Vec<String>,
}

impl HostCore {
    /// Construct a host from optional production capabilities.
    pub fn new(
        audio: Option<Box<dyn AudioBackend>>,
        hotkeys: Option<Box<dyn HotkeySource>>,
        config: Config,
        caps: UiCapabilities,
    ) -> Self {
        let mut host = Self {
            audio,
            hotkeys,
            config,
            last_config_mtime: None,
            config_reload_requested: false,
            audio_retry_attempts: 0,
            state: AppState::default(),
            caps,
            quit_requested: false,
            degraded: Vec::new(),
        };
        host.sync_appearance();
        host.refresh_audio();
        host
    }

    /// Minimal constructor used by platform-neutral host tests.
    pub fn for_test(audio: Box<dyn AudioBackend>) -> Self {
        Self::new(
            Some(audio),
            None,
            Config::default(),
            UiCapabilities {
                compositor: false,
                blur: false,
                high_contrast: false,
                reduced_motion: false,
                dpi_scale: 1.0,
                work_area: crate::ui::WorkArea::new(0, 0, 1600, 900),
            },
        )
    }

    pub fn state(&self) -> &AppState {
        &self.state
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn capabilities(&self) -> &UiCapabilities {
        &self.caps
    }

    pub fn quit_requested(&self) -> bool {
        self.quit_requested
    }

    pub fn degraded_reasons(&self) -> &[String] {
        &self.degraded
    }

    pub fn mark_degraded(&mut self, reason: impl Into<String>) {
        self.mark_degraded_inner(reason.into());
    }

    pub fn audio_available(&self) -> bool {
        self.audio.is_some()
    }

    pub fn set_audio_backend(&mut self, audio: Box<dyn AudioBackend>) {
        self.audio = Some(audio);
        self.audio_retry_attempts = 0;
        self.refresh_audio();
    }

    pub fn can_retry_audio(&self) -> bool {
        self.audio.is_none() && self.audio_retry_attempts < MAX_AUDIO_RETRIES
    }

    pub fn record_audio_retry(&mut self) {
        self.audio_retry_attempts = self.audio_retry_attempts.saturating_add(1);
    }

    pub fn config_mtime(&self) -> Option<SystemTime> {
        self.last_config_mtime
    }

    pub fn set_config_mtime(&mut self, mtime: Option<SystemTime>) {
        self.last_config_mtime = mtime;
    }

    /// Consume an explicit reload request queued by a renderer or tray action.
    pub fn take_config_reload_request(&mut self) -> bool {
        std::mem::take(&mut self.config_reload_requested)
    }

    pub fn set_hotkeys(&mut self, hotkeys: Option<Box<dyn HotkeySource>>) {
        self.hotkeys = hotkeys;
    }

    /// Apply one renderer or translated hotkey intent.
    pub fn apply_action(&mut self, action: &AppAction) {
        match action {
            AppAction::SetVolumePercent { percent } => {
                let target = (*percent).min(100) as f32 / 100.0;
                self.mutate_audio(|audio| audio.set_volume(target));
            }
            AppAction::AdjustVolume { delta_percent } => {
                let Some(current) = self.read_volume_for_adjustment() else {
                    return;
                };
                let target = (current * 100.0 + *delta_percent as f32).clamp(0.0, 100.0) / 100.0;
                self.mutate_audio(|audio| audio.set_volume(target));
            }
            AppAction::ToggleMute => self.mutate_audio(|audio| audio.toggle_mute().map(|_| ())),
            AppAction::SetMute { muted } => {
                self.mutate_audio(|audio| audio.set_mute(*muted));
            }
            AppAction::ResetVolume => {
                self.mutate_audio(|audio| audio.set_volume(0.5));
            }
            AppAction::ShowSurface(surface) => {
                self.state
                    .set_visibility(*surface, SurfaceVisibility::Visible);
            }
            AppAction::HideSurface(surface) => {
                self.state
                    .set_visibility(*surface, SurfaceVisibility::Hidden);
            }
            AppAction::ToggleSurface(surface) => {
                self.state.toggle(*surface);
            }
            AppAction::SetTheme(theme) => {
                self.config.appearance.theme = *theme;
                self.sync_appearance();
            }
            AppAction::SetMaterial(material) => {
                self.config.appearance.material = *material;
                self.sync_appearance();
            }
            AppAction::SetMotion(motion) => {
                self.config.appearance.motion = *motion;
                self.sync_appearance();
            }
            AppAction::OpenTrayMenu => {
                log::info!("Linux tray/menu unavailable in this host");
            }
            AppAction::OpenConfigLocation
            | AppAction::ApplyConfig
            | AppAction::CancelConfig
            | AppAction::ResetConfig
            | AppAction::AddBlacklistEntry(_)
            | AppAction::RemoveBlacklistEntry(_)
            | AppAction::ClearBlacklist
            | AppAction::ApplyRecommendedBlacklist => {
                log::info!("Linux configuration action deferred in this host: {action:?}");
            }
            AppAction::ReloadConfig => {
                self.config_reload_requested = true;
            }
            AppAction::Exit => {
                self.quit_requested = true;
            }
        }
    }

    /// Drain all queued hotkeys and translate them using current settings.
    pub fn poll_hotkeys(&mut self) {
        loop {
            let action = self.hotkeys.as_ref().and_then(|source| source.try_recv());
            let Some(action) = action else { break };
            let action = hotkey_to_action(action, &self.config);
            self.apply_action(&action);
        }

        if let Some(error) = self
            .hotkeys
            .as_ref()
            .and_then(|source| source.listener_failure())
        {
            self.mark_degraded(format!("global hotkeys unavailable: {error}"));
        }
    }

    /// Read authoritative backend state and publish it into the host model.
    pub fn refresh_audio(&mut self) {
        let result = self.audio.as_ref().map(|audio| audio.get_state());
        match result {
            None => self.mark_audio_error("audio backend unavailable".to_string()),
            Some(Ok(volume)) => {
                self.state.volume_percent = volume.percent();
                self.state.muted = volume.muted;
                self.state.status = UiStatus::Ready;
                self.clear_degraded_prefix("audio:");
            }
            Some(Err(error)) => {
                if matches!(error, AudioError::DeviceLost) {
                    self.audio = None;
                }
                self.mark_audio_error(error.to_string());
            }
        }
        self.sync_appearance();
    }

    /// Replace the current configuration after an already-loaded valid edit.
    ///
    /// The GTK adapter owns mtime/file I/O. This method keeps replacement and
    /// hotkey modifier synchronization deterministic and testable.
    pub fn apply_reloaded_config(&mut self, config: Config, mtime: Option<SystemTime>) {
        let modifier_changed = self.config.modifier != config.modifier;
        self.config = config;
        self.last_config_mtime = mtime;
        self.sync_appearance();
        if modifier_changed {
            if let Some(source) = self.hotkeys.as_ref() {
                source.set_modifier(self.config.modifier);
            }
        }
    }

    /// Read a valid config at `path` without writing defaults or replacing the
    /// active config on malformed input. The GTK adapter calls this when the
    /// tracked mtime changes.
    pub fn reload_config_at(
        &mut self,
        path: &Path,
        mtime: Option<SystemTime>,
    ) -> Result<(), String> {
        let raw = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        let config = serde_json::from_str::<Config>(&raw).map_err(|error| error.to_string())?;
        self.apply_reloaded_config(config::normalize(config), mtime);
        Ok(())
    }

    fn read_volume_for_adjustment(&mut self) -> Option<f32> {
        let result = self.audio.as_ref().map(|audio| audio.get_state());
        match result {
            None => {
                self.mark_audio_error("audio backend unavailable".to_string());
                None
            }
            Some(Ok(state)) => Some(state.volume.clamp(0.0, 1.0)),
            Some(Err(error)) => {
                if matches!(error, AudioError::DeviceLost) {
                    self.audio = None;
                }
                self.mark_audio_error(error.to_string());
                None
            }
        }
    }

    fn mutate_audio<F>(&mut self, operation: F)
    where
        F: FnOnce(&dyn AudioBackend) -> Result<(), AudioError>,
    {
        let result = self.audio.as_ref().map(|audio| operation(audio.as_ref()));
        match result {
            None => self.mark_audio_error("audio backend unavailable".to_string()),
            Some(Ok(())) => self.state.status = UiStatus::Syncing,
            Some(Err(error)) => {
                if matches!(error, AudioError::DeviceLost) {
                    self.audio = None;
                }
                self.mark_audio_error(error.to_string());
            }
        }
    }

    fn mark_audio_error(&mut self, message: String) {
        self.state.status = UiStatus::Error(message.clone());
        self.mark_degraded_inner(format!("audio: {message}"));
    }

    fn mark_degraded_inner(&mut self, reason: String) {
        if !self.degraded.iter().any(|existing| existing == &reason) {
            self.degraded.push(reason);
        }
    }

    fn clear_degraded_prefix(&mut self, prefix: &str) {
        self.degraded.retain(|reason| !reason.starts_with(prefix));
    }

    fn sync_appearance(&mut self) {
        self.state.theme = self.config.appearance.theme;
        self.state.material = self.config.appearance.material;
        self.state.motion = self.config.appearance.motion;
    }
}

fn hotkey_to_action(action: HotkeyAction, config: &Config) -> AppAction {
    use HotkeyAction as H;
    match action {
        H::VolumeUp => AppAction::AdjustVolume {
            delta_percent: config.volume_step as i16,
        },
        H::VolumeDown => AppAction::AdjustVolume {
            delta_percent: -(config.volume_step as i16),
        },
        H::VolumeUpLarge => AppAction::AdjustVolume {
            delta_percent: config.volume_step_large as i16,
        },
        H::VolumeDownLarge => AppAction::AdjustVolume {
            delta_percent: -(config.volume_step_large as i16),
        },
        H::ToggleMute => AppAction::ToggleMute,
        H::Reset50 => AppAction::ResetVolume,
        H::OpenMixer => AppAction::ToggleSurface(SurfaceId::Mixer),
        H::OpenMenu => AppAction::OpenTrayMenu,
    }
}

#[cfg(target_os = "linux")]
impl HotkeySource for crate::hotkeys_rdev::RdevHotkeys {
    fn try_recv(&self) -> Option<HotkeyAction> {
        self.try_recv()
    }

    fn listener_failure(&self) -> Option<String> {
        self.listener_failure()
    }

    fn set_modifier(&self, modifier: HotkeyModifier) {
        self.set_modifier(modifier)
    }
}
