//! Single-instance application guard and IPC signaling.
//!
//! Enforces VolumePro's `#SingleInstance Force` equivalent:
//! - A named Win32 mutex identifies the running instance.
//! - A second launch detects the mutex, signals a named auto-reset event to ask
//!   the primary instance to display its Mixer surface, and exits.
//! - The primary instance polls the event on its frame/event tick to reveal the
//!   Mixer without blocking the UI thread.
//! - The mutex and event are scoped to a [`PrimaryGuard`] whose RAII drop cleans
//!   up handles cleanly on shutdown.
//! - On non-Windows platforms, single-instance enforcement is a no-op that
//!   always returns [`Launch::Primary`].

#[cfg(target_os = "windows")]
use std::sync::RwLock;

/// Default Win32 named mutex identifying the running primary instance.
pub const DEFAULT_MUTEX_NAME: &str = "Local\\VolumeControl.Egui.SingleInstance";

/// Default Win32 named auto-reset event signaled to request showing the Mixer.
pub const DEFAULT_SHOW_MIXER_EVENT_NAME: &str = "Local\\VolumeControl.Egui.ShowMixer";

/// Deterministic fallback names used during testing.
pub const TEST_MUTEX_NAME: &str = "Local\\VolumeControl.Test.SingleInstance";
pub const TEST_SHOW_MIXER_EVENT_NAME: &str = "Local\\VolumeControl.Test.ShowMixer";

#[cfg(target_os = "windows")]
static CUSTOM_NAMES: RwLock<Option<(String, String)>> = RwLock::new(None);

/// Override the mutex and event names used by [`acquire`] and [`signal_show_mixer`].
///
/// Intended for automated test isolation and multi-instance test harnesses. Pass
/// `(None, None)` to clear overrides.
pub fn set_custom_names_for_test(mutex_name: Option<String>, event_name: Option<String>) {
    #[cfg(target_os = "windows")]
    if let Ok(mut lock) = CUSTOM_NAMES.write() {
        if let (Some(m), Some(e)) = (mutex_name, event_name) {
            *lock = Some((m, e));
        } else {
            *lock = None;
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (mutex_name, event_name);
    }
}

/// Result of attempting to acquire the single-instance lock.
#[derive(Debug)]
pub enum Launch {
    /// This process is the primary instance and owns the instance lock.
    Primary(PrimaryGuard),
    /// Another instance is already running; the caller should signal the
    /// primary to show the mixer and terminate.
    Secondary,
}

/// RAII guard holding open handles for the primary single-instance mutex and event.
///
/// On Windows, dropping this guard releases ownership of the mutex and closes
/// both the mutex and event handles so the OS destroys the named kernel objects.
#[derive(Debug)]
pub struct PrimaryGuard {
    #[cfg(target_os = "windows")]
    mutex: windows_sys::Win32::Foundation::HANDLE,
    #[cfg(target_os = "windows")]
    event: windows_sys::Win32::Foundation::HANDLE,
}

// Win32 handles are process-wide identifiers and safe to transfer across threads.
unsafe impl Send for PrimaryGuard {}
unsafe impl Sync for PrimaryGuard {}

impl PrimaryGuard {
    /// Non-blocking check for a pending "show mixer" request signaled by a
    /// secondary launch.
    ///
    /// Returns `true` if signaled. Because the event is auto-reset, consuming
    /// the signal clears it back to non-signaled immediately.
    pub fn poll_show_mixer(&self) -> bool {
        #[cfg(target_os = "windows")]
        {
            if self.event == 0 {
                return false;
            }
            // SAFETY: `self.event` is a valid handle created by `CreateEventW`
            // with 0 (non-blocking) timeout; does not block the caller.
            unsafe {
                windows_sys::Win32::System::Threading::WaitForSingleObject(self.event, 0)
                    == windows_sys::Win32::Foundation::WAIT_OBJECT_0
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            false
        }
    }
}

impl Drop for PrimaryGuard {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        // SAFETY: Closing handles held by this guard and releasing mutex ownership.
        // If handles are zero, operations are skipped.
        unsafe {
            if self.event != 0 {
                windows_sys::Win32::Foundation::CloseHandle(self.event);
                self.event = 0;
            }
            if self.mutex != 0 {
                windows_sys::Win32::System::Threading::ReleaseMutex(self.mutex);
                windows_sys::Win32::Foundation::CloseHandle(self.mutex);
                self.mutex = 0;
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn to_wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
fn resolve_names() -> (String, String) {
    if let Ok(lock) = CUSTOM_NAMES.read() {
        if let Some((m, e)) = &*lock {
            return (m.clone(), e.clone());
        }
    }
    if let (Ok(m), Ok(e)) = (
        std::env::var("VOLUMECONTROL_MUTEX_NAME"),
        std::env::var("VOLUMECONTROL_SHOW_MIXER_EVENT_NAME"),
    ) {
        return (m, e);
    }
    #[cfg(test)]
    {
        (
            TEST_MUTEX_NAME.to_string(),
            TEST_SHOW_MIXER_EVENT_NAME.to_string(),
        )
    }
    #[cfg(not(test))]
    {
        (
            DEFAULT_MUTEX_NAME.to_string(),
            DEFAULT_SHOW_MIXER_EVENT_NAME.to_string(),
        )
    }
}

/// Attempt to acquire the application single-instance lock.
///
/// On Windows, creates or opens the named mutex. If the mutex is already held
/// by another instance, returns [`Launch::Secondary`].
///
/// On non-Windows platforms, always returns [`Launch::Primary`].
pub fn acquire() -> Launch {
    #[cfg(target_os = "windows")]
    {
        let (mutex_name, event_name) = resolve_names();
        acquire_named(&mutex_name, &event_name)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Launch::Primary(PrimaryGuard {})
    }
}

/// Attempt to acquire the single-instance lock with explicitly specified object names.
pub fn acquire_named(mutex_name: &str, event_name: &str) -> Launch {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS};
        use windows_sys::Win32::System::Threading::{CreateEventW, CreateMutexW};

        let wide_mutex = to_wide_null(mutex_name);
        // SAFETY: `wide_mutex` is null-terminated and valid for read.
        // `bInitialOwner` is TRUE (1) to request initial ownership.
        unsafe {
            let mutex = CreateMutexW(std::ptr::null(), 1, wide_mutex.as_ptr());

            if mutex == 0 {
                log::warn!("could not create single-instance mutex; allowing launch");
                return Launch::Primary(PrimaryGuard { mutex: 0, event: 0 });
            }

            if GetLastError() == ERROR_ALREADY_EXISTS {
                CloseHandle(mutex);
                log::info!("another instance is already running");
                return Launch::Secondary;
            }

            let wide_event = to_wide_null(event_name);
            // SAFETY: `wide_event` is null-terminated and valid.
            // `bManualReset` = 0 (auto-reset), `bInitialState` = 0 (non-signaled).
            let event = CreateEventW(std::ptr::null(), 0, 0, wide_event.as_ptr());

            if event == 0 {
                log::warn!(
                    "could not create show-mixer event; secondary launches will stay silent"
                );
            }

            Launch::Primary(PrimaryGuard { mutex, event })
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (mutex_name, event_name);
        Launch::Primary(PrimaryGuard {})
    }
}

/// Signal the running primary instance to reveal the Mixer surface.
///
/// Called by a secondary launch before exiting. Returns `true` if the event
/// was found and signaled, `false` if no listening primary instance was reached.
pub fn signal_show_mixer() -> bool {
    #[cfg(target_os = "windows")]
    {
        let (_, event_name) = resolve_names();
        signal_show_mixer_named(&event_name)
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

/// Signal a specific named show-mixer event.
pub fn signal_show_mixer_named(event_name: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{OpenEventW, SetEvent, EVENT_MODIFY_STATE};

        let wide_event = to_wide_null(event_name);
        // SAFETY: `wide_event` is null-terminated and valid.
        // Retries briefly to allow the primary startup window between mutex and event creation.
        unsafe {
            for _ in 0..50 {
                let event = OpenEventW(EVENT_MODIFY_STATE, 0, wide_event.as_ptr());
                if event != 0 {
                    let success = SetEvent(event) != 0;
                    CloseHandle(event);
                    return success;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            log::warn!("running instance did not expose the show-mixer event");
            false
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = event_name;
        false
    }
}
