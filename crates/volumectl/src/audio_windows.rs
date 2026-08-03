//! Windows audio backend — WASAPI default render endpoint via raw COM.
//!
//! `windows-sys` exposes COM interfaces as opaque `*mut c_void` pointers, so
//! this module hand-rolls the vtable layouts for the three interfaces we need:
//!
//!   IMMDeviceEnumerator → GetDefaultAudioEndpoint
//!   IMMDevice           → Activate
//!   IAudioEndpointVolume → scalar volume + mute get/set
//!
//! Vtable layouts follow the published Win32 headers (Microsoft Learn).
//! HRESULTs are i32; return codes are checked and mapped to [`AudioError`].
//!
//! Calling convention: a vtable *field* is a function pointer, so calls are
//! written `(vtbl::<I>(this).method)(this, args...)` — the extra parentheses
//! call the pointer stored in the field.

use std::ffi::c_void;

use windows_sys::core::{GUID, PCWSTR};
use windows_sys::Win32::{
    Media::Audio::{eConsole, eRender, EDataFlow, ERole},
    System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL},
};

use crate::audio::{AudioBackend, AudioError, VolumeState};

const CLSID_MMDEVICE_ENUMERATOR: GUID = GUID::from_u128(0xBCDE0395_E52F_467C_8E3D_C4579291692E);
const IID_IMMDEVICE_ENUMERATOR: GUID = GUID::from_u128(0xA95664D2_9614_4F35_A746_DE8DB63617E6);
const IID_IAUDIO_ENDPOINT_VOLUME: GUID = GUID::from_u128(0x5CDF2C82_841E_4546_9722_0CF74078229A);

type HRESULT = i32;

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

/// WASAPI default-output volume controller.
pub struct WindowsAudio {
    /// IAudioEndpointVolume* (owned; Release on Drop).
    endpoint: *mut c_void,
    /// IMMDeviceEnumerator* — released before the endpoint on Drop.
    enumerator: *mut c_void,
    device: *mut c_void,
}

// COM refcounts make these safe to move across threads.
unsafe impl Send for WindowsAudio {}
unsafe impl Sync for WindowsAudio {}

impl WindowsAudio {
    pub fn new() -> Result<Self, AudioError> {
        unsafe {
            // COINIT_MULTITHREADED = 0; dwcoinit is u32.
            let hr = CoInitializeEx(std::ptr::null(), 0);
            // S_OK(0) or S_FALSE(1) are acceptable; anything else is an error.
            if hr != 0 && hr != 1 {
                return Err(AudioError::Init(format!("CoInitializeEx: 0x{hr:x}")));
            }

            let mut enumerator: *mut c_void = std::ptr::null_mut();
            let hr = CoCreateInstance(
                &CLSID_MMDEVICE_ENUMERATOR,
                std::ptr::null_mut(), // punkouter: IUnknown
                CLSCTX_ALL,
                &IID_IMMDEVICE_ENUMERATOR,
                &mut enumerator,
            );
            if hr != 0 {
                return Err(AudioError::Init(format!(
                    "CoCreateInstance(MMDeviceEnumerator): 0x{hr:x}"
                )));
            }

            let mut device: *mut c_void = std::ptr::null_mut();
            let hr = (vtbl::<IMMDeviceEnumeratorVtbl>(enumerator).get_default_audio_endpoint)(
                enumerator,
                eRender,
                eConsole,
                &mut device,
            );
            if hr != 0 {
                (vtbl::<IMMDeviceEnumeratorVtbl>(enumerator).release)(enumerator);
                return Err(AudioError::Io(format!("GetDefaultAudioEndpoint: 0x{hr:x}")));
            }

            let mut endpoint: *mut c_void = std::ptr::null_mut();
            let hr = (vtbl::<IMMDeviceVtbl>(device).activate)(
                device,
                &IID_IAUDIO_ENDPOINT_VOLUME,
                CLSCTX_ALL,
                std::ptr::null_mut(),
                &mut endpoint,
            );
            if hr != 0 {
                (vtbl::<IMMDeviceVtbl>(device).release)(device);
                (vtbl::<IMMDeviceEnumeratorVtbl>(enumerator).release)(enumerator);
                return Err(AudioError::Io(format!(
                    "Activate(IAudioEndpointVolume): 0x{hr:x}"
                )));
            }

            Ok(Self {
                endpoint,
                enumerator,
                device,
            })
        }
    }
}

impl AudioBackend for WindowsAudio {
    fn get_state(&self) -> Result<VolumeState, AudioError> {
        unsafe {
            let v = vtbl::<IAudioEndpointVolumeVtbl>(self.endpoint);
            let mut vol = 0.0_f32;
            let hr = (v.get_master_volume_level_scalar)(self.endpoint, &mut vol);
            if hr != 0 {
                return Err(AudioError::Io(format!(
                    "GetMasterVolumeLevelScalar: 0x{hr:x}"
                )));
            }
            let mut muted = 0_i32;
            let hr = (v.get_mute)(self.endpoint, &mut muted);
            if hr != 0 {
                return Err(AudioError::Io(format!("GetMute: 0x{hr:x}")));
            }
            Ok(VolumeState {
                volume: vol,
                muted: muted != 0,
            })
        }
    }

    fn set_volume(&self, volume: f32) -> Result<(), AudioError> {
        let v = volume.clamp(0.0, 1.0);
        unsafe {
            let hr = (vtbl::<IAudioEndpointVolumeVtbl>(self.endpoint)
                .set_master_volume_level_scalar)(
                self.endpoint, v, std::ptr::null()
            );
            if hr != 0 {
                return Err(AudioError::Io(format!(
                    "SetMasterVolumeLevelScalar: 0x{hr:x}"
                )));
            }
        }
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
        unsafe {
            let hr = (vtbl::<IAudioEndpointVolumeVtbl>(self.endpoint).set_mute)(
                self.endpoint,
                muted as i32,
                std::ptr::null(),
            );
            if hr != 0 {
                return Err(AudioError::Io(format!("SetMute: 0x{hr:x}")));
            }
        }
        Ok(())
    }
}

impl Drop for WindowsAudio {
    fn drop(&mut self) {
        unsafe {
            if !self.endpoint.is_null() {
                (vtbl::<IAudioEndpointVolumeVtbl>(self.endpoint).release)(self.endpoint);
            }
            if !self.device.is_null() {
                (vtbl::<IMMDeviceVtbl>(self.device).release)(self.device);
            }
            if !self.enumerator.is_null() {
                (vtbl::<IMMDeviceEnumeratorVtbl>(self.enumerator).release)(self.enumerator);
            }
            CoUninitialize();
        }
    }
}
