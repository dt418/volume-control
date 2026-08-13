//! Windows WASAPI audio-session source for the per-app mixer.
//!
//! Enumerates the audio sessions of the default render device (one entry per
//! process with an open audio session) and exposes per-session volume/mute
//! control. Following the pattern of [`crate::audio_windows`], COM interfaces
//! are opaque `*mut c_void` pointers with hand-rolled vtable layouts matching
//! the published Win32 headers:
//!
//!   IMMDeviceEnumerator → GetDefaultAudioEndpoint
//!   IMMDevice           → Activate(IID_IAudioClient)
//!   IAudioClient        → GetService(IID_IAudioSessionManager2)
//!   IAudioSessionManager2 → GetSessionEnumerator
//!   IAudioSessionEnumerator → GetCount / GetSession
//!   IAudioSessionControl  → GetState / GetDisplayName
//!   IAudioSessionControl2 → GetProcessId
//!   ISimpleAudioVolume    → Get/SetMasterVolume, Get/SetMute
//!
//! A session's `id` is its process id as a string (the mixer frontend treats
//! ids as opaque). Stale ids (process no longer has a session) return `Err`
//! per spec §9.6; the caller re-emits `state://sessions` so the UI drops the
//! dead row. No device / no sessions / COM failure → empty list, never panic.

use std::ffi::c_void;

use windows_sys::core::GUID;
use windows_sys::Win32::{
    Foundation::CloseHandle,
    Media::Audio::{eConsole, eRender, EDataFlow, ERole},
    System::{
        Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL},
        Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION},
    },
};

use crate::host_core::{AudioSessionInfo, SessionsSource};

#[allow(clippy::upper_case_acronyms)]
type HRESULT = i32;

const CLSID_MMDEVICE_ENUMERATOR: GUID = GUID::from_u128(0xBCDE0395_E52F_467C_8E3D_C4579291692E);
const IID_IMMDEVICE_ENUMERATOR: GUID = GUID::from_u128(0xA95664D2_9614_4F35_A746_DE8DB63617E6);
const IID_IAUDIO_CLIENT: GUID = GUID::from_u128(0x1CB9AD4C_DBFA_4C32_B178_C2F568A703B2);
const IID_IAUDIO_SESSION_MANAGER2: GUID = GUID::from_u128(0x77AA99A0_1BD6_484F_8BC7_2C654C9A9B6F);
const IID_IAUDIO_SESSION_CONTROL2: GUID = GUID::from_u128(0xBFB7FF88_7239_4FC9_8FA2_07C950BE9C6D);
const IID_ISIMPLE_AUDIO_VOLUME: GUID = GUID::from_u128(0x87CE5498_68D6_44E5_9215_6DA47EF883D8);

/// `AudioSessionState::AudioSessionStateActive` (audiopolicy.h): the session
/// is producing audio right now. Inactive = 0, Expired = 2.
const AUDIO_SESSION_STATE_ACTIVE: i32 = 1;

// ── COM vtable layouts (IUnknown prefix, then interface methods, in order) ──

#[repr(C)]
struct IMMDeviceEnumeratorVtbl {
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    enum_audio_endpoints:
        unsafe extern "system" fn(*mut c_void, EDataFlow, u32, *mut *mut c_void) -> HRESULT,
    get_default_audio_endpoint:
        unsafe extern "system" fn(*mut c_void, EDataFlow, ERole, *mut *mut c_void) -> HRESULT,
    get_device: unsafe extern "system" fn(*mut c_void, *const u16, *mut *mut c_void) -> HRESULT,
    register_endpoint_notification_callback:
        unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    unregister_endpoint_notification_callback:
        unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
}

#[repr(C)]
struct IMMDeviceVtbl {
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    activate: unsafe extern "system" fn(
        *mut c_void,
        *const GUID,
        u32,
        *mut c_void,
        *mut *mut c_void,
    ) -> HRESULT,
    open_property_store: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HRESULT,
    get_id: unsafe extern "system" fn(*mut c_void, *mut *const u16) -> HRESULT,
    get_state: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
}

#[repr(C)]
struct IAudioClientVtbl {
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    initialize: unsafe extern "system" fn(
        *mut c_void,
        u32,
        u32,
        i64,
        i64,
        *const c_void,
        *const GUID,
    ) -> HRESULT,
    get_buffer_size: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
    get_stream_latency: unsafe extern "system" fn(*mut c_void, *mut i64) -> HRESULT,
    get_current_padding: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
    is_format_supported:
        unsafe extern "system" fn(*mut c_void, *const c_void, *mut *mut c_void) -> HRESULT,
    get_mix_format: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    get_device_period: unsafe extern "system" fn(*mut c_void, *mut i64, *mut i64) -> HRESULT,
    start: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    stop: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    reset: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    set_event_handle: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    get_service: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
}

#[repr(C)]
struct IAudioSessionManager2Vtbl {
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_audio_session_control:
        unsafe extern "system" fn(*mut c_void, *const GUID, u32, *mut *mut c_void) -> HRESULT,
    get_simple_audio_volume:
        unsafe extern "system" fn(*mut c_void, *const GUID, u32, *mut *mut c_void) -> HRESULT,
    get_session_enumerator: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    register_session_notification: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    unregister_session_notification: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    register_duck_notification:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut c_void) -> HRESULT,
    unregister_duck_notification: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
}

#[repr(C)]
struct IAudioSessionEnumeratorVtbl {
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_count: unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT,
    get_session: unsafe extern "system" fn(*mut c_void, i32, *mut *mut c_void) -> HRESULT,
}

#[repr(C)]
struct IAudioSessionControlVtbl {
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_state: unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT,
    get_display_name: unsafe extern "system" fn(*mut c_void, *mut *mut u16) -> HRESULT,
    set_display_name: unsafe extern "system" fn(*mut c_void, *const u16, *const GUID) -> HRESULT,
    get_icon_path: unsafe extern "system" fn(*mut c_void, *mut *mut u16) -> HRESULT,
    set_icon_path: unsafe extern "system" fn(*mut c_void, *const u16, *const GUID) -> HRESULT,
    get_grouping_param: unsafe extern "system" fn(*mut c_void, *mut *const u8) -> HRESULT,
    set_grouping_param: unsafe extern "system" fn(*mut c_void, *const u8, *const GUID) -> HRESULT,
    register_audio_session_notification:
        unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    unregister_audio_session_notification:
        unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
}

#[repr(C)]
struct IAudioSessionControl2Vtbl {
    // IAudioSessionControl (slots 0..11), then the Control2 extensions.
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_state: unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT,
    get_display_name: unsafe extern "system" fn(*mut c_void, *mut *mut u16) -> HRESULT,
    set_display_name: unsafe extern "system" fn(*mut c_void, *const u16, *const GUID) -> HRESULT,
    get_icon_path: unsafe extern "system" fn(*mut c_void, *mut *mut u16) -> HRESULT,
    set_icon_path: unsafe extern "system" fn(*mut c_void, *const u16, *const GUID) -> HRESULT,
    get_grouping_param: unsafe extern "system" fn(*mut c_void, *mut *const u8) -> HRESULT,
    set_grouping_param: unsafe extern "system" fn(*mut c_void, *const u8, *const GUID) -> HRESULT,
    register_audio_session_notification:
        unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    unregister_audio_session_notification:
        unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    get_session_identifier: unsafe extern "system" fn(*mut c_void, *mut *mut u16) -> HRESULT,
    get_session_instance_identifier:
        unsafe extern "system" fn(*mut c_void, *mut *mut u16) -> HRESULT,
    get_process_id: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
    is_system_sounds_session: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    set_ducking_preference: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
}

#[repr(C)]
struct ISimpleAudioVolumeVtbl {
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    set_master_volume: unsafe extern "system" fn(*mut c_void, f32, *const GUID) -> HRESULT,
    get_master_volume: unsafe extern "system" fn(*mut c_void, *mut f32) -> HRESULT,
    set_mute: unsafe extern "system" fn(*mut c_void, i32, *const GUID) -> HRESULT,
    get_mute: unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT,
}

#[inline]
unsafe fn vtbl<T>(this: *const c_void) -> &'static T {
    &**(this as *const *const T)
}

// ── Pure helpers (unit-tested; no device required) ─────────────────────────

/// The mixer-facing session id: the process id as a string.
pub fn session_id(pid: u32) -> String {
    pid.to_string()
}

/// Final display name for a session: prefer the OS display name, then the
/// process image base name, then a pid placeholder.
pub fn resolved_session_name(display: &str, process_base: &str, pid: u32) -> String {
    let display = display.trim();
    let process_base = process_base.trim();
    if !display.is_empty() {
        display.to_string()
    } else if !process_base.is_empty() {
        process_base.to_string()
    } else {
        format!("Process {pid}")
    }
}

/// The Windows WASAPI session source (default render device).
pub struct WindowsSessions;

impl SessionsSource for WindowsSessions {
    fn supported(&self) -> bool {
        true
    }

    fn list(&self) -> Vec<AudioSessionInfo> {
        match SessionChain::acquire() {
            Ok(chain) => unsafe { enumerate_sessions(&chain) },
            Err(e) => {
                log::debug!("audio session enumeration unavailable: {e}");
                Vec::new()
            }
        }
    }

    fn set_volume(&self, id: &str, pct: u8) -> Result<(), String> {
        let chain = SessionChain::acquire()?;
        unsafe {
            with_session(&chain, id, |_control, simple| {
                let target = (pct as f32).clamp(0.0, 100.0) / 100.0;
                let hr = (vtbl::<ISimpleAudioVolumeVtbl>(simple).set_master_volume)(
                    simple,
                    target,
                    std::ptr::null(),
                );
                if hr != 0 {
                    return Err(format!("SetMasterVolume: 0x{hr:x}"));
                }
                Ok(())
            })
        }
    }

    fn mute(&self, id: &str) -> Result<(), String> {
        let chain = SessionChain::acquire()?;
        unsafe {
            with_session(&chain, id, |_control, simple| {
                let sv = vtbl::<ISimpleAudioVolumeVtbl>(simple);
                let mut muted = 0_i32;
                let hr = (sv.get_mute)(simple, &mut muted);
                if hr != 0 {
                    return Err(format!("GetMute: 0x{hr:x}"));
                }
                let hr = (sv.set_mute)(simple, if muted == 0 { 1 } else { 0 }, std::ptr::null());
                if hr != 0 {
                    return Err(format!("SetMute: 0x{hr:x}"));
                }
                Ok(())
            })
        }
    }
}

// ── COM acquisition / enumeration ──────────────────────────────────────────

/// Owned COM chain for one enumeration round: enumerator → device → client →
/// session manager → session enumerator. Released in reverse on drop, then
/// the COM apartment is uninitialized.
struct SessionChain {
    enumerator: *mut c_void,
    device: *mut c_void,
    client: *mut c_void,
    manager: *mut c_void,
    sessions: *mut c_void,
}

impl SessionChain {
    fn acquire() -> Result<Self, String> {
        unsafe {
            let hr = CoInitializeEx(std::ptr::null(), 0);
            if hr != 0 && hr != 1 {
                return Err(format!("CoInitializeEx: 0x{hr:x}"));
            }
            let mut enumerator: *mut c_void = std::ptr::null_mut();
            let hr = CoCreateInstance(
                &CLSID_MMDEVICE_ENUMERATOR,
                std::ptr::null_mut(),
                CLSCTX_ALL,
                &IID_IMMDEVICE_ENUMERATOR,
                &mut enumerator,
            );
            if hr != 0 {
                CoUninitialize();
                return Err(format!("CoCreateInstance(MMDeviceEnumerator): 0x{hr:x}"));
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
                CoUninitialize();
                return Err(format!("GetDefaultAudioEndpoint: 0x{hr:x}"));
            }
            let mut client: *mut c_void = std::ptr::null_mut();
            let hr = (vtbl::<IMMDeviceVtbl>(device).activate)(
                device,
                &IID_IAUDIO_CLIENT,
                CLSCTX_ALL,
                std::ptr::null_mut(),
                &mut client,
            );
            if hr != 0 {
                (vtbl::<IMMDeviceVtbl>(device).release)(device);
                (vtbl::<IMMDeviceEnumeratorVtbl>(enumerator).release)(enumerator);
                CoUninitialize();
                return Err(format!("Activate(IAudioClient): 0x{hr:x}"));
            }
            let mut manager: *mut c_void = std::ptr::null_mut();
            let hr = (vtbl::<IAudioClientVtbl>(client).get_service)(
                client,
                &IID_IAUDIO_SESSION_MANAGER2,
                &mut manager,
            );
            if hr != 0 {
                (vtbl::<IAudioClientVtbl>(client).release)(client);
                (vtbl::<IMMDeviceVtbl>(device).release)(device);
                (vtbl::<IMMDeviceEnumeratorVtbl>(enumerator).release)(enumerator);
                CoUninitialize();
                return Err(format!("GetService(IAudioSessionManager2): 0x{hr:x}"));
            }
            let mut sessions: *mut c_void = std::ptr::null_mut();
            let hr = (vtbl::<IAudioSessionManager2Vtbl>(manager).get_session_enumerator)(
                manager,
                &mut sessions,
            );
            if hr != 0 {
                (vtbl::<IAudioSessionManager2Vtbl>(manager).release)(manager);
                (vtbl::<IAudioClientVtbl>(client).release)(client);
                (vtbl::<IMMDeviceVtbl>(device).release)(device);
                (vtbl::<IMMDeviceEnumeratorVtbl>(enumerator).release)(enumerator);
                CoUninitialize();
                return Err(format!("GetSessionEnumerator: 0x{hr:x}"));
            }
            Ok(Self {
                enumerator,
                device,
                client,
                manager,
                sessions,
            })
        }
    }
}

impl Drop for SessionChain {
    fn drop(&mut self) {
        unsafe {
            if !self.sessions.is_null() {
                (vtbl::<IAudioSessionEnumeratorVtbl>(self.sessions).release)(self.sessions);
            }
            if !self.manager.is_null() {
                (vtbl::<IAudioSessionManager2Vtbl>(self.manager).release)(self.manager);
            }
            if !self.client.is_null() {
                (vtbl::<IAudioClientVtbl>(self.client).release)(self.client);
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

/// Build one `AudioSessionInfo` per enumerated session. COM failures on an
/// individual session (QI, getters) degrade to defaults, never a panic.
unsafe fn enumerate_sessions(chain: &SessionChain) -> Vec<AudioSessionInfo> {
    let mut sessions = Vec::new();
    let evt = vtbl::<IAudioSessionEnumeratorVtbl>(chain.sessions);
    let mut count: i32 = 0;
    if (evt.get_count)(chain.sessions, &mut count) != 0 {
        return sessions;
    }
    for i in 0..count {
        let mut control: *mut c_void = std::ptr::null_mut();
        if (evt.get_session)(chain.sessions, i, &mut control) != 0 {
            continue;
        }
        let cv = vtbl::<IAudioSessionControlVtbl>(control);
        let mut control2: *mut c_void = std::ptr::null_mut();
        let mut simple: *mut c_void = std::ptr::null_mut();
        let _ = (cv.query_interface)(control, &IID_IAUDIO_SESSION_CONTROL2, &mut control2);
        let _ = (cv.query_interface)(control, &IID_ISIMPLE_AUDIO_VOLUME, &mut simple);

        let mut state: i32 = -1;
        let _ = (cv.get_state)(control, &mut state);

        let mut display_ptr: *mut u16 = std::ptr::null_mut();
        let _ = (cv.get_display_name)(control, &mut display_ptr);
        let display = if display_ptr.is_null() {
            String::new()
        } else {
            let s = wide_to_string(display_ptr);
            CoTaskMemFree(display_ptr as *const c_void);
            s
        };

        let mut pid: u32 = 0;
        if !control2.is_null() {
            let _ =
                (vtbl::<IAudioSessionControl2Vtbl>(control2).get_process_id)(control2, &mut pid);
        }

        let mut pct = 0.0_f32;
        let mut muted = 0_i32;
        if !simple.is_null() {
            let sv = vtbl::<ISimpleAudioVolumeVtbl>(simple);
            let _ = (sv.get_master_volume)(simple, &mut pct);
            let _ = (sv.get_mute)(simple, &mut muted);
        }

        let process_base = process_image_base(pid).unwrap_or_default();
        sessions.push(AudioSessionInfo {
            id: session_id(pid),
            name: resolved_session_name(&display, &process_base, pid),
            pct: (pct.clamp(0.0, 1.0) * 100.0).round() as u8,
            muted: muted != 0,
            active: state == AUDIO_SESSION_STATE_ACTIVE,
        });

        if !simple.is_null() {
            (vtbl::<ISimpleAudioVolumeVtbl>(simple).release)(simple);
        }
        if !control2.is_null() {
            (vtbl::<IAudioSessionControl2Vtbl>(control2).release)(control2);
        }
        (cv.release)(control);
    }
    sessions
}

/// Find the session whose pid string equals `id` and run `f` on its
/// `ISimpleAudioVolume`. Returns `Err` (never panics) when the session is
/// gone — the caller re-emits `state://sessions` so the UI drops the dead row.
unsafe fn with_session<F>(chain: &SessionChain, id: &str, f: F) -> Result<(), String>
where
    F: FnOnce(*mut c_void, *mut c_void) -> Result<(), String>,
{
    let evt = vtbl::<IAudioSessionEnumeratorVtbl>(chain.sessions);
    let mut count: i32 = 0;
    let hr = (evt.get_count)(chain.sessions, &mut count);
    if hr != 0 {
        return Err(format!("GetCount: 0x{hr:x}"));
    }
    for i in 0..count {
        let mut control: *mut c_void = std::ptr::null_mut();
        if (evt.get_session)(chain.sessions, i, &mut control) != 0 {
            continue;
        }
        let cv = vtbl::<IAudioSessionControlVtbl>(control);
        let mut control2: *mut c_void = std::ptr::null_mut();
        let mut simple: *mut c_void = std::ptr::null_mut();
        let _ = (cv.query_interface)(control, &IID_IAUDIO_SESSION_CONTROL2, &mut control2);
        let _ = (cv.query_interface)(control, &IID_ISIMPLE_AUDIO_VOLUME, &mut simple);

        let mut matched = false;
        if !control2.is_null() {
            let mut pid: u32 = 0;
            let _ =
                (vtbl::<IAudioSessionControl2Vtbl>(control2).get_process_id)(control2, &mut pid);
            matched = session_id(pid) == id;
        }

        if matched {
            let result = if simple.is_null() {
                Err(format!("session has no volume control: {id}"))
            } else {
                f(control, simple)
            };
            if !simple.is_null() {
                (vtbl::<ISimpleAudioVolumeVtbl>(simple).release)(simple);
            }
            if !control2.is_null() {
                (vtbl::<IAudioSessionControl2Vtbl>(control2).release)(control2);
            }
            (cv.release)(control);
            return result;
        }

        if !simple.is_null() {
            (vtbl::<ISimpleAudioVolumeVtbl>(simple).release)(simple);
        }
        if !control2.is_null() {
            (vtbl::<IAudioSessionControl2Vtbl>(control2).release)(control2);
        }
        (cv.release)(control);
    }
    Err(format!("session not found: {id}"))
}

/// Read a null-terminated UTF-16 string owned by the OS.
unsafe fn wide_to_string(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    while *ptr.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
}

/// Lowercase base name (e.g. `spotify.exe`) of a process, for sessions whose
/// OS display name is empty. Mirrors `host_core::foreground_process`.
fn process_image_base(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle == 0 {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(handle, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(handle);
        if ok == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        let base = path.rsplit('\\').next().unwrap_or(&path);
        Some(base.to_lowercase())
    }
}
