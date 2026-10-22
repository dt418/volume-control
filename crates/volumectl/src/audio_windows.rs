//! Windows audio backend — WASAPI default render endpoint via raw COM.
//!
//! `windows-sys` exposes COM interfaces as opaque `*mut c_void` pointers, so
//! this module hand-rolls the vtable layouts for the interfaces we need:
//!
//!   IMMDeviceEnumerator → GetDefaultAudioEndpoint
//!   IMMDevice           → GetId / Activate
//!   IAudioEndpointVolume → scalar volume + mute get/set
//!
//! Vtable layouts follow the published Win32 headers (Microsoft Learn).
//! HRESULTs are i32; return codes are checked and mapped to [`AudioError`].
//!
//! The backend follows the current default render endpoint instead of pinning
//! the endpoint selected at startup. Every operation compares the stored
//! device id with the current default; a mismatch re-acquires the endpoint.
//! Failed calls additionally trigger one re-acquire + retry, covering device
//! removal races that happen between the id check and the call.
//!
//! Calling convention: a vtable *field* is a function pointer, so calls are
//! written `(vtbl::<I>(this).method)(this, args...)` — the extra parentheses
//! call the pointer stored in the field.

use std::ffi::c_void;
use std::sync::Mutex;

use windows_sys::core::{GUID, PCWSTR};
use windows_sys::Win32::{
    Media::Audio::{eConsole, eRender, EDataFlow, ERole},
    System::Com::{CoCreateInstance, CoTaskMemFree, CLSCTX_ALL},
};

use crate::audio::{AudioBackend, AudioError, VolumeState};

const CLSID_MMDEVICE_ENUMERATOR: GUID = GUID::from_u128(0xBCDE0395_E52F_467C_8E3D_C4579291692E);
const IID_IMMDEVICE_ENUMERATOR: GUID = GUID::from_u128(0xA95664D2_9614_4F35_A746_DE8DB63617E6);
const IID_IAUDIO_ENDPOINT_VOLUME: GUID = GUID::from_u128(0x5CDF2C82_841E_4546_9722_0CF74078229A);

#[allow(clippy::upper_case_acronyms)]
type HRESULT = i32;

/// `AUDCLNT_E_DEVICE_INVALIDATED` (audioclient.h): the endpoint was removed.
const AUDCLNT_E_DEVICE_INVALIDATED: HRESULT = 0x8889_0004u32 as i32;
/// `HRESULT_FROM_WIN32(ERROR_NOT_FOUND)`: no default endpoint.
const E_NOTFOUND: HRESULT = 0x8007_0490u32 as i32;

// ── COM vtable layouts (IUnknown prefix then interface methods) ────────────

#[repr(C)]
struct IMMDeviceEnumeratorVtbl {
    // IUnknown
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    // IMMDeviceEnumerator
    enum_audio_endpoints:
        unsafe extern "system" fn(*mut c_void, EDataFlow, u32, *mut *mut c_void) -> HRESULT,
    get_default_audio_endpoint:
        unsafe extern "system" fn(*mut c_void, EDataFlow, ERole, *mut *mut c_void) -> HRESULT,
    get_device: unsafe extern "system" fn(*mut c_void, PCWSTR, *mut *mut c_void) -> HRESULT,
    register_notify_callback: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    unregister_notify_callback: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
}

#[repr(C)]
struct IMMDeviceVtbl {
    // IUnknown
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    // IMMDevice
    activate: unsafe extern "system" fn(
        *mut c_void,
        *const GUID,
        u32,
        *mut c_void,
        *mut *mut c_void,
    ) -> HRESULT,
    open_property_store: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HRESULT,
    get_id: unsafe extern "system" fn(*mut c_void, *mut PCWSTR) -> HRESULT,
    get_state: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
}

#[repr(C)]
struct IAudioEndpointVolumeVtbl {
    // IUnknown
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    // IAudioEndpointVolume
    register_control_change_notify: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    unregister_control_change_notify:
        unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    get_channel_count: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
    set_master_volume_level: unsafe extern "system" fn(*mut c_void, f32, *const GUID) -> HRESULT,
    set_master_volume_level_scalar:
        unsafe extern "system" fn(*mut c_void, f32, *const GUID) -> HRESULT,
    get_master_volume_level: unsafe extern "system" fn(*mut c_void, *mut f32) -> HRESULT,
    get_master_volume_level_scalar: unsafe extern "system" fn(*mut c_void, *mut f32) -> HRESULT,
    set_channel_volume_level:
        unsafe extern "system" fn(*mut c_void, u32, f32, *const GUID) -> HRESULT,
    set_channel_volume_level_scalar:
        unsafe extern "system" fn(*mut c_void, u32, f32, *const GUID) -> HRESULT,
    get_channel_volume_level: unsafe extern "system" fn(*mut c_void, u32, *mut f32) -> HRESULT,
    get_channel_volume_level_scalar:
        unsafe extern "system" fn(*mut c_void, u32, *mut f32) -> HRESULT,
    set_mute: unsafe extern "system" fn(*mut c_void, i32, *const GUID) -> HRESULT,
    get_mute: unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT,
    get_volume_step_info: unsafe extern "system" fn(*mut c_void, *mut u32, *mut u32) -> HRESULT,
    get_volume_range:
        unsafe extern "system" fn(*mut c_void, *mut f32, *mut f32, *mut f32) -> HRESULT,
}

/// Access the vtable of an opaque COM pointer.
unsafe fn vtbl<T>(this: *const c_void) -> &'static T {
    &**(this as *const *const T)
}

fn io_error(operation: &str, hr: HRESULT) -> AudioError {
    if hr == AUDCLNT_E_DEVICE_INVALIDATED || hr == E_NOTFOUND {
        AudioError::DeviceLost
    } else {
        AudioError::Io(format!("{operation}: 0x{hr:x}"))
    }
}

/// Owned endpoint pair (IAudioEndpointVolume + IMMDevice) plus the device id
/// used to detect default-device changes.
struct Endpoint {
    endpoint: *mut c_void,
    device: *mut c_void,
    device_id: Vec<u16>,
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        unsafe {
            if !self.endpoint.is_null() {
                (vtbl::<IAudioEndpointVolumeVtbl>(self.endpoint).release)(self.endpoint);
            }
            if !self.device.is_null() {
                (vtbl::<IMMDeviceVtbl>(self.device).release)(self.device);
            }
        }
    }
}

/// WASAPI default-output volume controller.
pub struct WindowsAudio {
    /// IMMDeviceEnumerator* (owned; released on Drop).
    enumerator: *mut c_void,
    /// Current default endpoint. `None` between a failed release and the next
    /// acquisition attempt.
    current: Mutex<Option<Endpoint>>,
}

// COM refcounts make these safe to move across threads; `current` serializes
// endpoint access.
unsafe impl Send for WindowsAudio {}
unsafe impl Sync for WindowsAudio {}

impl WindowsAudio {
    fn init_com() -> Result<crate::com_guard::ComGuard, AudioError> {
        crate::com_guard::ComGuard::init_apartment_sta()
            .map_err(|hr| AudioError::Init(format!("CoInitializeEx(STA): 0x{hr:x}")))
    }

    pub fn new() -> Result<Self, AudioError> {
        unsafe {
            // COM is initialized for this setup call only. Each later audio
            // operation initializes COM on its calling thread as well; the
            // backend is shared by Tauri commands and the host poll thread,
            // so a guard stored in this struct would be dropped on the wrong
            // thread and leave those calls outside an apartment.
            let _com = Self::init_com()?;

            let mut enumerator: *mut c_void = std::ptr::null_mut();
            let hr = CoCreateInstance(
                &CLSID_MMDEVICE_ENUMERATOR,
                std::ptr::null_mut(), // punkouter: IUnknown
                CLSCTX_ALL,
                &IID_IMMDEVICE_ENUMERATOR,
                &mut enumerator,
            );
            if hr < 0 {
                return Err(AudioError::Init(format!(
                    "CoCreateInstance(MMDeviceEnumerator): 0x{hr:x}"
                )));
            }

            let current = match Self::acquire_default(enumerator) {
                Ok(endpoint) => endpoint,
                Err(error) => {
                    (vtbl::<IMMDeviceEnumeratorVtbl>(enumerator).release)(enumerator);
                    return Err(error);
                }
            };

            Ok(Self {
                enumerator,
                current: Mutex::new(Some(current)),
            })
        }
    }

    /// Read the id of the current default render device. The id buffer is
    /// allocated by COM and freed with `CoTaskMemFree`.
    unsafe fn current_default_id(enumerator: *mut c_void) -> Result<Vec<u16>, AudioError> {
        let mut device: *mut c_void = std::ptr::null_mut();
        let hr = (vtbl::<IMMDeviceEnumeratorVtbl>(enumerator).get_default_audio_endpoint)(
            enumerator,
            eRender,
            eConsole,
            &mut device,
        );
        if hr < 0 {
            return Err(io_error("GetDefaultAudioEndpoint", hr));
        }
        let id = Self::copy_device_id(device);
        (vtbl::<IMMDeviceVtbl>(device).release)(device);
        id
    }

    unsafe fn copy_device_id(device: *mut c_void) -> Result<Vec<u16>, AudioError> {
        let mut id: PCWSTR = std::ptr::null();
        let hr = (vtbl::<IMMDeviceVtbl>(device).get_id)(device, &mut id);
        if hr < 0 {
            return Err(io_error("GetId", hr));
        }
        if id.is_null() {
            return Err(AudioError::Io("GetId returned a null id".into()));
        }
        let mut len = 0usize;
        while *id.add(len) != 0 {
            len += 1;
        }
        let value = std::slice::from_raw_parts(id, len + 1).to_vec();
        CoTaskMemFree(id as *const c_void);
        Ok(value)
    }

    unsafe fn acquire_default(enumerator: *mut c_void) -> Result<Endpoint, AudioError> {
        let mut device: *mut c_void = std::ptr::null_mut();
        let hr = (vtbl::<IMMDeviceEnumeratorVtbl>(enumerator).get_default_audio_endpoint)(
            enumerator,
            eRender,
            eConsole,
            &mut device,
        );
        if hr < 0 {
            return Err(io_error("GetDefaultAudioEndpoint", hr));
        }

        let device_id = match Self::copy_device_id(device) {
            Ok(id) => id,
            Err(error) => {
                (vtbl::<IMMDeviceVtbl>(device).release)(device);
                return Err(error);
            }
        };

        let mut endpoint: *mut c_void = std::ptr::null_mut();
        let hr = (vtbl::<IMMDeviceVtbl>(device).activate)(
            device,
            &IID_IAUDIO_ENDPOINT_VOLUME,
            CLSCTX_ALL,
            std::ptr::null_mut(),
            &mut endpoint,
        );
        if hr < 0 {
            (vtbl::<IMMDeviceVtbl>(device).release)(device);
            return Err(io_error("Activate(IAudioEndpointVolume)", hr));
        }

        Ok(Endpoint {
            endpoint,
            device,
            device_id,
        })
    }

    /// Drop the cached endpoint when the default changed, then acquire the
    /// new default when needed.
    fn ensure_current(&self, slot: &mut Option<Endpoint>) -> Result<(), AudioError> {
        if let Some(existing) = slot.as_ref() {
            let default_id = unsafe { Self::current_default_id(self.enumerator) }?;
            if existing.device_id == default_id {
                return Ok(());
            }
            log::warn!("default audio endpoint changed; re-acquiring");
            *slot = None;
        }
        *slot = Some(unsafe { Self::acquire_default(self.enumerator) }?);
        Ok(())
    }

    /// Run `op` against the current endpoint. A failed call drops the endpoint
    /// and retries once against a freshly acquired default. This handles both
    /// a default-device switch and a device disappearing between the id check
    /// and the call.
    fn with_endpoint<T>(
        &self,
        op: impl Fn(*mut c_void) -> Result<T, AudioError>,
    ) -> Result<T, AudioError> {
        let _com = Self::init_com()?;
        let mut slot = self
            .current
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.ensure_current(&mut slot)?;

        let endpoint = slot
            .as_ref()
            .expect("ensure_current always installs an endpoint")
            .endpoint;
        match op(endpoint) {
            Ok(value) => Ok(value),
            Err(first_error) => {
                log::warn!("audio endpoint call failed ({first_error}); re-acquiring default");
                *slot = None;
                self.ensure_current(&mut slot)?;
                let endpoint = slot
                    .as_ref()
                    .expect("ensure_current always installs an endpoint")
                    .endpoint;
                op(endpoint)
            }
        }
    }
}

impl AudioBackend for WindowsAudio {
    fn get_state(&self) -> Result<VolumeState, AudioError> {
        self.with_endpoint(|endpoint| unsafe {
            let v = vtbl::<IAudioEndpointVolumeVtbl>(endpoint);
            let mut vol = 0.0_f32;
            let hr = (v.get_master_volume_level_scalar)(endpoint, &mut vol);
            if hr < 0 {
                return Err(io_error("GetMasterVolumeLevelScalar", hr));
            }
            let mut muted = 0_i32;
            let hr = (v.get_mute)(endpoint, &mut muted);
            if hr < 0 {
                return Err(io_error("GetMute", hr));
            }
            Ok(VolumeState {
                volume: vol,
                muted: muted != 0,
            })
        })
    }

    fn set_volume(&self, volume: f32) -> Result<(), AudioError> {
        let v = volume.clamp(0.0, 1.0);
        self.with_endpoint(|endpoint| unsafe {
            let hr = (vtbl::<IAudioEndpointVolumeVtbl>(endpoint).set_master_volume_level_scalar)(
                endpoint,
                v,
                std::ptr::null(),
            );
            if hr < 0 {
                return Err(io_error("SetMasterVolumeLevelScalar", hr));
            }
            Ok(())
        })?;
        // Raising the volume unmutes, matching VolumePro's behaviour.
        if v > 0.0 {
            let _ = self.set_mute(false);
        }
        Ok(())
    }

    fn toggle_mute(&self) -> Result<VolumeState, AudioError> {
        let mut state = self.get_state()?;
        state.muted = !state.muted;
        self.set_mute(state.muted)?;
        Ok(state)
    }

    fn set_mute(&self, muted: bool) -> Result<(), AudioError> {
        self.with_endpoint(|endpoint| unsafe {
            let hr = (vtbl::<IAudioEndpointVolumeVtbl>(endpoint).set_mute)(
                endpoint,
                muted as i32,
                std::ptr::null(),
            );
            if hr < 0 {
                return Err(io_error("SetMute", hr));
            }
            Ok(())
        })
    }
}

impl Drop for WindowsAudio {
    fn drop(&mut self) {
        let mut current = self
            .current
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Endpoint::drop releases both COM interfaces.
        *current = None;
        unsafe {
            if !self.enumerator.is_null() {
                (vtbl::<IMMDeviceEnumeratorVtbl>(self.enumerator).release)(self.enumerator);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalidated_and_missing_devices_map_to_device_lost() {
        assert!(matches!(
            io_error("x", AUDCLNT_E_DEVICE_INVALIDATED),
            AudioError::DeviceLost
        ));
        assert!(matches!(io_error("x", E_NOTFOUND), AudioError::DeviceLost));
    }

    #[test]
    fn other_hresults_keep_the_operation_and_code() {
        match io_error("GetMute", 0x8000_4005u32 as i32) {
            AudioError::Io(message) => {
                assert!(message.contains("GetMute"));
                assert!(message.contains("80004005"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
