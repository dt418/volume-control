//! macOS audio backend (CoreAudio) implementing the shared [`AudioBackend`]
//! contract.
//!
//! The `volumecontrol` crate selects its backend per target at compile time —
//! on macOS it always builds the real CoreAudio backend (see that crate's
//! `volumecontrol-macos` with the `coreaudio` feature), which is a pure
//! `objc2-core-audio` FFI with no extra system packages. No feature flag is
//! needed here.
//!
//! Like the Linux backend this is a thin adapter: it maps the 0..=100
//! percentage + mute state reported by CoreAudio onto the shared
//! [`VolumeState`] (0.0..=1.0 + muted) and clamps absolute sets, mapping every
//! error to the coarse [`AudioError`].

use crate::audio::{AudioBackend, AudioError, VolumeState};
use volumecontrol::AudioDevice;

/// CoreAudio-backed default output device.
#[derive(Debug)]
pub struct MacAudio {
    device: AudioDevice,
}

fn map_err(e: volumecontrol::AudioError) -> AudioError {
    AudioError::Io(e.to_string())
}

impl MacAudio {
    /// Open the system default output device.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::Init`] if no default device can be resolved.
    pub fn new() -> Result<Self, AudioError> {
        let device = AudioDevice::from_default().map_err(|e| AudioError::Init(e.to_string()))?;
        Ok(Self { device })
    }
}

impl AudioBackend for MacAudio {
    fn get_state(&self) -> Result<VolumeState, AudioError> {
        let vol = self.device.get_vol().map_err(map_err)?;
        let muted = self.device.is_mute().map_err(map_err)?;
        Ok(VolumeState {
            volume: f32::from(vol) / 100.0,
            muted,
        })
    }

    fn set_volume(&self, volume: f32) -> Result<(), AudioError> {
        let pct = (volume.clamp(0.0, 1.0) * 100.0).round() as u8;
        self.device.set_vol(pct).map_err(map_err)
    }

    fn toggle_mute(&self) -> Result<VolumeState, AudioError> {
        let muted = self.device.is_mute().map_err(map_err)?;
        self.device.set_mute(!muted).map_err(map_err)?;
        self.get_state()
    }

    fn set_mute(&self, muted: bool) -> Result<(), AudioError> {
        self.device.set_mute(muted).map_err(map_err)
    }
}
