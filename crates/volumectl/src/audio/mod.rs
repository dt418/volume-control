//! Shared platform-agnostic audio abstraction.
//!
//! A `AudioBackend` exposes the operations the app needs regardless of OS:
//! reading the default output volume, muting it, and (optionally) listing
//! output devices. The Windows implementation lives in `audio_windows`;
//! macOS/Linux backends are wired in from their respective `#[cfg]` modules.

use std::fmt;

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
pub trait AudioBackend {
    /// Read the current default output state.
    fn get_state(&self) -> Result<VolumeState, AudioError>;
    /// Set absolute volume in 0.0–=1.0 range (impl should clamp + unmute).
    fn set_volume(&self, volume: f32) -> Result<(), AudioError>;
    /// Toggle mute state; returns the new state.
    fn toggle_mute(&self) -> Result<VolumeState, AudioError>;
    /// Set mute to an explicit value.
    fn set_mute(&self, muted: bool) -> Result<(), AudioError>;
}