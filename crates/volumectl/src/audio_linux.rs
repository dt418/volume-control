//! Linux audio backend (PulseAudio) implementing the shared [`AudioBackend`]
//! contract.
//!
//! The `volumecontrol` crate selects its backend per target at compile time —
//! on Linux it always builds the real PulseAudio backend (see that crate's
//! `volumecontrol-linux` with the `pulseaudio` feature), so no feature flag is
//! needed here. Building for Linux therefore requires the `libpulse-dev`
//! system package (the CI jobs install it).
//!
//! The backend is intentionally a thin adapter: it converts the 0..=100
//! percentage + mute state reported by PulseAudio into the shared
//! [`VolumeState`] (0.0..=1.0 + muted) and clamps/allows the host to set an
//! absolute volume. All errors are mapped to the coarse [`AudioError`] so no
//! PulseAudio type leaks across the trait boundary.

use crate::audio::{AudioBackend, AudioError, VolumeState};
use volumecontrol::AudioDevice;

/// PulseAudio-backed default output device.
#[derive(Debug)]
pub struct LinuxAudio {
    device: AudioDevice,
}

fn map_err(e: volumecontrol::AudioError) -> AudioError {
    AudioError::Io(e.to_string())
}

impl LinuxAudio {
    /// Open the system default output device.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::Init`] if no default device can be resolved
    /// (no PulseAudio server / sink reachable, missing permission, …).
    pub fn new() -> Result<Self, AudioError> {
        let device = AudioDevice::from_default().map_err(|e| AudioError::Init(e.to_string()))?;
        Ok(Self { device })
    }
}

impl AudioBackend for LinuxAudio {
    fn get_state(&self) -> Result<VolumeState, AudioError> {
        let vol = self.device.get_vol().map_err(map_err)?;
        let muted = self.device.is_mute().map_err(map_err)?;
        Ok(VolumeState {
            volume: f32::from(vol) / 100.0,
            muted,
        })
    }

    fn set_volume(&self, volume: f32) -> Result<(), AudioError> {
        // The host passes 0.0..=1.0; PulseAudio takes an integer 0..=100.
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
