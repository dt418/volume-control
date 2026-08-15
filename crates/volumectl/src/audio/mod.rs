//! Shared platform-agnostic audio abstraction.
//!
//! A `AudioBackend` exposes the operations the app needs regardless of OS:
//! reading the default output volume, muting it, and (optionally) listing
//! output devices. The Windows implementation lives in `audio_windows`;
//! macOS/Linux backends are wired in from their respective `#[cfg]` modules.

use std::fmt;

#[cfg(debug_assertions)]
use std::sync::Mutex;

/// The current volume state of the default output device.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolumeState {
    /// 0.0 – 1.0 linear volume.
    pub volume: f32,
    /// True when the output is muted.
    pub muted: bool,
}

impl VolumeState {
    pub fn percent(&self) -> u8 {
        (self.volume.clamp(0.0, 1.0) * 100.0).round() as u8
    }
}

/// Open the platform's default-output audio backend.
///
/// Resolves the concrete [`AudioBackend`] implementation for the current OS
/// (PulseAudio on Linux, CoreAudio on macOS). Windows keeps its own Win32
/// WASAPI host wiring and does not route through this factory.
///
/// # Errors
///
/// Returns [`AudioError::Init`] when no default output device can be opened.
#[cfg(target_os = "linux")]
pub fn default_backend() -> Result<Box<dyn AudioBackend>, AudioError> {
    crate::audio_linux::LinuxAudio::new().map(|b| Box::new(b) as Box<dyn AudioBackend>)
}

/// Open the platform's default-output audio backend (CoreAudio on macOS).
///
/// # Errors
///
/// Returns [`AudioError::Init`] when no default output device can be opened.
#[cfg(target_os = "macos")]
pub fn default_backend() -> Result<Box<dyn AudioBackend>, AudioError> {
    crate::audio_macos::MacAudio::new().map(|b| Box::new(b) as Box<dyn AudioBackend>)
}

/// Backend error type — intentionally coarse so each platform can map
/// its native errors without leaking them across the trait boundary.
#[derive(Debug, Clone)]
pub enum AudioError {
    /// The backend could not be initialised (no audio device, permission, …).
    Init(String),
    /// A device handle / session vanished while in use.
    DeviceLost,
    /// The underlying API call failed.
    Io(String),
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioError::Init(m) => write!(f, "audio init failed: {m}"),
            AudioError::DeviceLost => write!(f, "audio device stopped responding"),
            AudioError::Io(m) => write!(f, "audio i/o error: {m}"),
        }
    }
}

impl std::error::Error for AudioError {}

/// Controls the system volume / mute state.
///
/// `Send + Sync` so the core can own it behind a `Mutex` and poll it from a
/// background thread in the Tauri host.
pub trait AudioBackend: Send + Sync {
    /// Read the current default output state.
    fn get_state(&self) -> Result<VolumeState, AudioError>;
    /// Set absolute volume in 0.0–=1.0 range (impl should clamp + unmute).
    fn set_volume(&self, volume: f32) -> Result<(), AudioError>;
    /// Toggle mute state; returns the new state.
    fn toggle_mute(&self) -> Result<VolumeState, AudioError>;
    /// Set mute to an explicit value.
    fn set_mute(&self, muted: bool) -> Result<(), AudioError>;
}

/// Audio backend used when the host has no usable default output device.
///
/// Desktop runners and headless environments can legitimately start without
/// an audio endpoint. Keeping that condition behind the normal backend
/// contract lets the Tauri host finish booting and lets the UI explain that
/// mixer controls are unavailable instead of panicking during setup.
#[derive(Debug, Clone)]
pub struct UnavailableAudio {
    reason: String,
}

impl UnavailableAudio {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }

    fn error(&self) -> AudioError {
        AudioError::Init(self.reason.clone())
    }
}

impl AudioBackend for UnavailableAudio {
    fn get_state(&self) -> Result<VolumeState, AudioError> {
        Err(self.error())
    }

    fn set_volume(&self, _volume: f32) -> Result<(), AudioError> {
        Err(self.error())
    }

    fn toggle_mute(&self) -> Result<VolumeState, AudioError> {
        Err(self.error())
    }

    fn set_mute(&self, _muted: bool) -> Result<(), AudioError> {
        Err(self.error())
    }
}

/// Deterministic in-memory audio endpoint used only by the debug E2E harness.
///
/// CI runners do not promise a physical default output device, but the Tauri
/// E2E suite still needs to exercise the complete command/event/UI path. This
/// backend is compiled out of release builds and is only selected when the
/// host sees the explicit debug E2E marker.
#[cfg(debug_assertions)]
#[derive(Debug)]
pub struct E2eAudio {
    state: Mutex<VolumeState>,
}

#[cfg(debug_assertions)]
impl E2eAudio {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(VolumeState {
                volume: 0.5,
                muted: false,
            }),
        }
    }
}

#[cfg(debug_assertions)]
impl Default for E2eAudio {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(debug_assertions)]
impl AudioBackend for E2eAudio {
    fn get_state(&self) -> Result<VolumeState, AudioError> {
        Ok(*self.state.lock().expect("E2E audio state lock"))
    }

    fn set_volume(&self, volume: f32) -> Result<(), AudioError> {
        let mut state = self.state.lock().expect("E2E audio state lock");
        state.volume = volume.clamp(0.0, 1.0);
        if state.volume > 0.0 {
            state.muted = false;
        }
        Ok(())
    }

    fn toggle_mute(&self) -> Result<VolumeState, AudioError> {
        let mut state = self.state.lock().expect("E2E audio state lock");
        state.muted = !state.muted;
        Ok(*state)
    }

    fn set_mute(&self, muted: bool) -> Result<(), AudioError> {
        self.state.lock().expect("E2E audio state lock").muted = muted;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{AudioBackend, UnavailableAudio};

    #[test]
    fn unavailable_backend_returns_initialization_error_without_panicking() {
        let backend = UnavailableAudio::new("no default output");

        assert!(backend.get_state().is_err());
        assert!(backend.set_volume(0.5).is_err());
        assert!(backend.toggle_mute().is_err());
        assert!(backend.set_mute(true).is_err());
    }

    #[cfg(debug_assertions)]
    #[test]
    fn debug_e2e_audio_round_trips_scalar_volume_and_mute() {
        let backend = super::E2eAudio::new();

        assert_eq!(
            backend.get_state().unwrap(),
            super::VolumeState {
                volume: 0.5,
                muted: false,
            }
        );
        backend.set_mute(true).unwrap();
        backend.set_volume(0.0).unwrap();
        assert_eq!(
            backend.get_state().unwrap(),
            super::VolumeState {
                volume: 0.0,
                muted: true,
            }
        );
        backend.set_mute(false).unwrap();
        backend.set_volume(0.5).unwrap();
        assert_eq!(
            backend.get_state().unwrap(),
            super::VolumeState {
                volume: 0.5,
                muted: false,
            }
        );
    }
}
