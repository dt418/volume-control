//! Platform-neutral state and action contracts for UI surfaces.
//!
//! This module intentionally contains no platform or renderer imports. The host
//! owns audio/configuration mutation, publishes confirmed [`AppState`], and
//! handles [`AppAction`] values emitted by any renderer.

use serde::{Deserialize, Serialize};

/// A user-visible application surface that can be shown or hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SurfaceId {
    Overlay,
    Mixer,
    Settings,
    Help,
    Tray,
}

/// Requested application theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

/// Requested surface material treatment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MaterialMode {
    #[default]
    Auto,
    Translucent,
    Opaque,
}

/// Requested animation policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MotionMode {
    #[default]
    Full,
    Reduced,
    Disabled,
}

/// Visibility state for a surface, including transient overlay presentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SurfaceVisibility {
    #[default]
    Hidden,
    Visible,
}

impl SurfaceVisibility {
    pub const fn is_visible(self) -> bool {
        matches!(self, Self::Visible)
    }

    pub const fn is_hidden(self) -> bool {
        matches!(self, Self::Hidden)
    }
}

/// The host-reported status of the shared UI state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum UiStatus {
    #[default]
    Ready,
    Syncing,
    Error(String),
}

/// Confirmed state published by the application host to every renderer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppState {
    /// Confirmed default-output volume, normalized to an integer percentage.
    pub volume_percent: u8,
    /// Confirmed mute state.
    pub muted: bool,
    /// Stable identifier or user-facing name of the active output device.
    pub device: Option<String>,
    /// Current visibility for each supported UI surface.
    pub surfaces: SurfaceVisibilityState,
    /// Appearance preferences currently in effect.
    pub theme: ThemeMode,
    pub material: MaterialMode,
    pub motion: MotionMode,
    /// Host/audio synchronization status.
    pub status: UiStatus,
}

impl AppState {
    /// Construct confirmed state from an already-clamped volume percentage.
    pub fn from_audio(volume_percent: u8, muted: bool, device: Option<String>) -> Self {
        Self {
            volume_percent,
            muted,
            device,
            ..Self::default()
        }
    }

    pub fn is_visible(&self, surface: SurfaceId) -> bool {
        self.surfaces.get(surface).is_visible()
    }

    pub fn set_visibility(&mut self, surface: SurfaceId, visibility: SurfaceVisibility) {
        self.surfaces.set(surface, visibility);
    }

    pub fn show(&mut self, surface: SurfaceId) {
        self.set_visibility(surface, SurfaceVisibility::Visible);
    }

    pub fn hide(&mut self, surface: SurfaceId) {
        self.set_visibility(surface, SurfaceVisibility::Hidden);
    }

    pub fn toggle(&mut self, surface: SurfaceId) -> SurfaceVisibility {
        let visibility = if self.is_visible(surface) {
            SurfaceVisibility::Hidden
        } else {
            SurfaceVisibility::Visible
        };
        self.set_visibility(surface, visibility);
        visibility
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            volume_percent: 0,
            muted: false,
            device: None,
            surfaces: SurfaceVisibilityState::default(),
            theme: ThemeMode::default(),
            material: MaterialMode::default(),
            motion: MotionMode::default(),
            status: UiStatus::default(),
        }
    }
}

/// Compact visibility storage that keeps the public model serde-friendly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SurfaceVisibilityState {
    pub overlay: SurfaceVisibility,
    pub mixer: SurfaceVisibility,
    pub settings: SurfaceVisibility,
    pub help: SurfaceVisibility,
    pub tray: SurfaceVisibility,
}

impl SurfaceVisibilityState {
    pub const fn get(self, surface: SurfaceId) -> SurfaceVisibility {
        match surface {
            SurfaceId::Overlay => self.overlay,
            SurfaceId::Mixer => self.mixer,
            SurfaceId::Settings => self.settings,
            SurfaceId::Help => self.help,
            SurfaceId::Tray => self.tray,
        }
    }

    pub fn set(&mut self, surface: SurfaceId, visibility: SurfaceVisibility) {
        match surface {
            SurfaceId::Overlay => self.overlay = visibility,
            SurfaceId::Mixer => self.mixer = visibility,
            SurfaceId::Settings => self.settings = visibility,
            SurfaceId::Help => self.help = visibility,
            SurfaceId::Tray => self.tray = visibility,
        }
    }
}

/// Host-directed intent emitted by UI surfaces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AppAction {
    SetVolumePercent { percent: u16 },
    AdjustVolume { delta_percent: i16 },
    ToggleMute,
    SetMute { muted: bool },
    ResetVolume,
    ShowSurface(SurfaceId),
    HideSurface(SurfaceId),
    ToggleSurface(SurfaceId),
    ApplyConfig,
    CancelConfig,
    ResetConfig,
    SetTheme(ThemeMode),
    SetMaterial(MaterialMode),
    SetMotion(MotionMode),
    OpenConfigLocation,
    AddBlacklistEntry(String),
    RemoveBlacklistEntry(String),
    ClearBlacklist,
    ApplyRecommendedBlacklist,
    Exit,
}

impl AppAction {
    /// Return a volume percentage safe to pass to an audio host.
    pub fn normalized(&self) -> Option<u8> {
        match self {
            Self::SetVolumePercent { percent } => Some((*percent).min(100) as u8),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_audio_keeps_already_clamped_percentage_and_defaults_preferences() {
        let state = AppState::from_audio(73, true, Some("Speakers".into()));

        assert_eq!(state.volume_percent, 73);
        assert!(state.muted);
        assert_eq!(state.device.as_deref(), Some("Speakers"));
        assert_eq!(state.theme, ThemeMode::System);
        assert!(!state.is_visible(SurfaceId::Overlay));
    }

    #[test]
    fn visibility_helpers_change_only_the_requested_surface() {
        let mut state = AppState::default();

        state.show(SurfaceId::Mixer);
        assert!(state.is_visible(SurfaceId::Mixer));
        assert!(!state.is_visible(SurfaceId::Help));
        assert_eq!(state.toggle(SurfaceId::Mixer), SurfaceVisibility::Hidden);
        assert!(!state.is_visible(SurfaceId::Help));
    }

    #[test]
    fn set_volume_action_normalizes_above_maximum() {
        let action = AppAction::SetVolumePercent { percent: 250 };

        assert_eq!(action.normalized(), Some(100));
        assert_eq!(AppAction::ToggleMute.normalized(), None);
    }

    #[test]
    fn model_types_round_trip_through_serde() {
        let state = AppState::from_audio(42, false, None);
        let encoded = serde_json::to_string(&state).expect("state serializes");
        let decoded: AppState = serde_json::from_str(&encoded).expect("state deserializes");

        assert_eq!(decoded, state);
    }
}
