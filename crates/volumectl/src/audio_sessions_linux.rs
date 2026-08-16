//! Linux per-app audio sessions over PulseAudio sink-inputs.
//!
//! [`PulseSessions`] implements the shared [`SessionsSource`] contract for
//! Linux: it enumerates PulseAudio sink-inputs (per-app playback streams) and
//! maps them to [`AudioSessionInfo`]. The backend is a thin, direct
//! `libpulse-sys` adapter (no third-party wrapper): a plain (non-threaded)
//! `pa_mainloop` connection to the default server, drained on the calling
//! thread with `pa_mainloop_iterate`, synchronous ops with a deadline, and a
//! pure mapping helper that is unit-tested without a Pulse server. The
//! non-threaded mainloop matches Pulse's canonical pattern: callbacks run
//! inline inside `iterate`, so teardown cannot race a callback or trip the
//! library's deferred-connect assertions (the threaded mainloop did).
//!
//! Failure is never fatal: an absent/unreachable server degrades to an empty
//! session list and `Err` from mutations (the host re-emits the fresh list,
//! the existing §9.6 stale-id contract).

use std::ffi::{c_char, c_void};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use libpulse_sys as pa;

use crate::host_core::{AudioSessionInfo, SessionsSource};

/// PulseAudio normal-volume scale (65536 = 100%).
const PA_VOLUME_NORM: u32 = 0x10000;
/// Upper bound for a single synchronous op (callbacks signal the mainloop,
/// so this only triggers when the server stops responding).
const OP_TIMEOUT: Duration = Duration::from_secs(2);
/// Upper bound for connecting to the server (a dead PULSE_SERVER fails
/// fast; a black-holed socket still cannot hang the app).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// Pure mapping from a Pulse sink-input snapshot to the shared session
/// contract. Kept dependency-free so the name fallback chain, percentage
/// math, and corked→active mapping are unit-testable without libpulse.
pub fn build_session_info(
    index: u32,
    app_name: Option<&str>,
    media_name: Option<&str>,
    stream_name: Option<&str>,
    volume_avg: u32,
    muted: bool,
    corked: bool,
) -> AudioSessionInfo {
    let name = app_name
        .filter(|s| !s.is_empty())
        .or_else(|| media_name.filter(|s| !s.is_empty()))
        .or_else(|| stream_name.filter(|s| !s.is_empty()))
        .unwrap_or("Unknown app")
        .to_string();
    AudioSessionInfo {
        id: index.to_string(),
        name,
        pct: (volume_avg.saturating_mul(100) / PA_VOLUME_NORM).min(100) as u8,
        muted,
        active: !corked,
    }
}

struct Connection {
    mainloop: *mut pa::pa_mainloop,
    context: *mut pa::pa_context,
}

// SAFETY: every Pulse access is serialized behind `PulseSessions.inner`, and
// the host serializes all AppCore calls behind its own mutex (the same
// precedent as `LinuxAudio`/`WindowsAudio`).
unsafe impl Send for Connection {}
unsafe impl Sync for Connection {}

impl Drop for Connection {
    fn drop(&mut self) {
        unsafe {
            if !self.mainloop.is_null() && !self.context.is_null() {
                pa::pa_context_disconnect(self.context);
                pa::pa_context_unref(self.context);
            }
            if !self.mainloop.is_null() {
                pa::pa_mainloop_free(self.mainloop);
            }
        }
    }
}

impl Connection {
    fn ready(&self) -> bool {
        unsafe { pa::pa_context_get_state(self.context) == pa::pa_context_state_t::Ready }
    }
}

/// Callback context for sink-input enumeration: the collection being filled.
struct ListCtx {
    sessions: *mut Vec<AudioSessionInfo>,
}

extern "C" fn sink_input_info_cb(
    _c: *mut pa::pa_context,
    info: *const pa::pa_sink_input_info,
    eol: i32,
    userdata: *mut c_void,
) {
    unsafe {
        let ctx = &mut *(userdata as *mut ListCtx);
        if eol == 0 && !info.is_null() {
            let info = &*info;
            let app_name = proplist_get(info.proplist, c"application.name".as_ptr());
            let media_name = proplist_get(info.proplist, c"media.name".as_ptr());
            let stream_name = if info.name.is_null() {
                None
            } else {
                Some(
                    std::ffi::CStr::from_ptr(info.name)
                        .to_string_lossy()
                        .into_owned(),
                )
            };
            let avg = pa::pa_cvolume_avg(&info.volume);
            (*ctx.sessions).push(build_session_info(
                info.index,
                app_name.as_deref(),
                media_name.as_deref(),
                stream_name.as_deref(),
                avg,
                info.mute != 0,
                info.corked != 0,
            ));
        }
    }
}

extern "C" fn sink_input_mute_cb(
    _c: *mut pa::pa_context,
    info: *const pa::pa_sink_input_info,
    eol: i32,
    userdata: *mut c_void,
) {
    unsafe {
        let muted = &mut *(userdata as *mut bool);
        if eol == 0 && !info.is_null() {
            *muted = (*info).mute != 0;
        }
    }
}

extern "C" fn success_cb(_c: *mut pa::pa_context, success: i32, userdata: *mut c_void) {
    unsafe {
        *(userdata as *mut bool) = success != 0;
    }
}

fn proplist_get(plist: *mut pa::pa_proplist, key: *const c_char) -> Option<String> {
    if plist.is_null() {
        return None;
    }
    let ptr = unsafe { pa::pa_proplist_gets(plist, key) };
    if ptr.is_null() {
        return None;
    }
    Some(
        unsafe { std::ffi::CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned(),
    )
}

/// Drain the mainloop until `done()` returns true, the deadline passes, or
/// the context stops being Ready. Callbacks run inline inside `iterate` on
/// this thread; the 5 ms poll keeps the loop responsive without spinning.
fn drain(
    conn: &Connection,
    deadline: Instant,
    context_must_be_ready: bool,
    done: impl Fn() -> bool,
) -> bool {
    loop {
        if done() {
            return true;
        }
        if context_must_be_ready
            && unsafe { pa::pa_context_get_state(conn.context) } != pa::pa_context_state_t::Ready
        {
            return false;
        }
        if Instant::now() >= deadline {
            return false;
        }
        let mut retval = 0;
        unsafe { pa::pa_mainloop_iterate(conn.mainloop, 0, &mut retval) };
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Run one Pulse operation to completion (or deadline). Returns true when
/// the operation finished.
fn run_op(conn: &Connection, op: *mut pa::pa_operation) -> bool {
    if op.is_null() {
        return false;
    }
    let done = || {
        let state = unsafe { pa::pa_operation_get_state(op) };
        matches!(
            state,
            pa::pa_operation_state_t::Done | pa::pa_operation_state_t::Cancelled
        )
    };
    let ok = drain(conn, Instant::now() + OP_TIMEOUT, true, done);
    let state = unsafe { pa::pa_operation_get_state(op) };
    unsafe {
        if state == pa::pa_operation_state_t::Running {
            pa::pa_operation_cancel(op);
        }
        pa::pa_operation_unref(op);
    }
    ok && state == pa::pa_operation_state_t::Done
}

/// Open a plain-mainloop connection to the default Pulse server. Returns
/// None on failure (unreachable server, timeout) — never panics.
fn open_connection() -> Option<Connection> {
    unsafe {
        let mainloop = pa::pa_mainloop_new();
        if mainloop.is_null() {
            return None;
        }
        let api = pa::pa_mainloop_get_api(mainloop);
        let context = pa::pa_context_new(api, c"VolumeControl".as_ptr());
        if context.is_null() {
            pa::pa_mainloop_free(mainloop);
            return None;
        }
        let conn = Connection { mainloop, context };
        pa::pa_context_connect(
            context,
            std::ptr::null(),
            pa::PA_CONTEXT_NOFLAGS,
            std::ptr::null(),
        );

        let deadline = Instant::now() + CONNECT_TIMEOUT;
        let connected = drain(&conn, deadline, false, || {
            let state = pa::pa_context_get_state(context);
            matches!(
                state,
                pa::pa_context_state_t::Ready
                    | pa::pa_context_state_t::Failed
                    | pa::pa_context_state_t::Terminated
            )
        });
        let state = pa::pa_context_get_state(context);
        if connected && state == pa::pa_context_state_t::Ready {
            Some(conn)
        } else {
            None // conn dropped: disconnect + unref + free
        }
    }
}

/// Linux per-app session source (PulseAudio sink-inputs).
pub struct PulseSessions {
    inner: Mutex<Option<Connection>>,
}

unsafe impl Send for PulseSessions {}
unsafe impl Sync for PulseSessions {}

impl PulseSessions {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    /// Borrow the live connection, (re)connecting when missing or not Ready.
    fn connection(&self) -> std::sync::MutexGuard<'_, Option<Connection>> {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let needs_reconnect = match inner.as_ref() {
            Some(conn) => !conn.ready(),
            None => true,
        };
        if needs_reconnect {
            *inner = open_connection();
        }
        inner
    }
}

impl Default for PulseSessions {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionsSource for PulseSessions {
    fn supported(&self) -> bool {
        true
    }

    fn list(&self) -> Vec<AudioSessionInfo> {
        let mut inner = self.connection();
        let Some(conn) = inner.as_mut() else {
            return Vec::new();
        };
        let mut sessions: Vec<AudioSessionInfo> = Vec::new();
        let mut ctx = ListCtx {
            sessions: &mut sessions,
        };
        let op = unsafe {
            pa::pa_context_get_sink_input_info_list(
                conn.context,
                Some(sink_input_info_cb),
                &mut ctx as *mut ListCtx as *mut c_void,
            )
        };
        let ok = run_op(conn, op);
        if !ok {
            log::warn!("pulse: sink-input enumeration failed or timed out");
            sessions.clear();
        }
        sessions
    }

    fn set_volume(&self, id: &str, pct: u8) -> Result<(), String> {
        let mut inner = self.connection();
        let Some(conn) = inner.as_mut() else {
            return Err("pulse unavailable".to_string());
        };
        let index: u32 = id
            .parse()
            .map_err(|_| format!("session {id:?} is not a sink-input index"))?;
        let mut cvol = pa::pa_cvolume {
            channels: 1,
            values: [0; pa::PA_CHANNELS_MAX as usize],
        };
        let norm = (pct as u32 * PA_VOLUME_NORM).min(PA_VOLUME_NORM * 100) / 100;
        unsafe {
            pa::pa_cvolume_set(&mut cvol, 1, norm);
            let mut success = false;
            let op = pa::pa_context_set_sink_input_volume(
                conn.context,
                index,
                &cvol,
                Some(success_cb),
                &mut success as *mut bool as *mut c_void,
            );
            if !run_op(conn, op) {
                return Err(format!("session {id:?} no longer active"));
            }
            if !success {
                return Err(format!("session {id:?} no longer active"));
            }
        }
        Ok(())
    }

    fn mute(&self, id: &str) -> Result<(), String> {
        let mut inner = self.connection();
        let Some(conn) = inner.as_mut() else {
            return Err("pulse unavailable".to_string());
        };
        let index: u32 = id
            .parse()
            .map_err(|_| format!("session {id:?} is not a sink-input index"))?;
        // Read the current mute under the same connection, then flip it.
        let mut currently_muted = false;
        let op = unsafe {
            pa::pa_context_get_sink_input_info(
                conn.context,
                index,
                Some(sink_input_mute_cb),
                &mut currently_muted as *mut bool as *mut c_void,
            )
        };
        if !run_op(conn, op) {
            return Err(format!("session {id:?} no longer active"));
        }
        let mut success = false;
        let op = unsafe {
            pa::pa_context_set_sink_input_mute(
                conn.context,
                index,
                if currently_muted { 0 } else { 1 },
                Some(success_cb),
                &mut success as *mut bool as *mut c_void,
            )
        };
        if !run_op(conn, op) {
            return Err(format!("session {id:?} no longer active"));
        }
        if !success {
            return Err(format!("session {id:?} no longer active"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_prefers_application_name_then_media_then_stream_then_fallback() {
        let app = build_session_info(
            7,
            Some("Spotify"),
            Some("track.mp3"),
            None,
            32_768,
            false,
            false,
        );
        assert_eq!(app.id, "7");
        assert_eq!(app.name, "Spotify");
        assert_eq!(app.pct, 50);
        assert!(!app.muted);
        assert!(app.active);

        let media = build_session_info(
            8,
            None,
            Some("Firefox"),
            Some("stream"),
            65_536,
            true,
            false,
        );
        assert_eq!(media.name, "Firefox");
        assert_eq!(media.pct, 100);
        assert!(media.muted);

        let stream = build_session_info(10, None, None, Some("mpv"), 32_768, false, true);
        assert_eq!(stream.name, "mpv");
        assert!(!stream.active);

        let fallback = build_session_info(9, None, None, None, 0, false, true);
        assert_eq!(fallback.name, "Unknown app");
        assert_eq!(fallback.pct, 0);
        assert!(!fallback.active);
    }

    #[test]
    fn mapping_clamps_volume_to_100() {
        let loud = build_session_info(1, Some("A"), None, None, 200_000, false, false);
        assert_eq!(loud.pct, 100);
    }

    #[test]
    fn sessions_report_supported() {
        assert!(PulseSessions::new().supported());
    }

    #[test]
    fn unreachable_pulse_server_degrades_to_empty_list() {
        // Point Pulse at a dead socket so the test is hermetic regardless of
        // the runner's audio state; restore afterwards even on panic.
        let old = std::env::var_os("PULSE_SERVER");
        std::env::set_var("PULSE_SERVER", "tcp:127.0.0.1:1");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let deadline = Instant::now() + Duration::from_secs(5);
            let sessions = PulseSessions::new().list();
            assert!(
                Instant::now() < deadline,
                "list() must return quickly when Pulse is unreachable"
            );
            assert!(
                sessions.is_empty(),
                "unreachable Pulse must yield no sessions"
            );
        }));
        match old {
            Some(v) => std::env::set_var("PULSE_SERVER", v),
            None => std::env::remove_var("PULSE_SERVER"),
        }
        assert!(
            result.is_ok(),
            "unreachable-server degradation test panicked"
        );
    }
}
