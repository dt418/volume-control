//! Unit tests for the pure helpers of the Windows WASAPI audio-session
//! source (`volumectl_lib::audio_sessions_win32`).
//!
//! The full COM session enumeration cannot run headless (needs a live audio
//! device and processes with audio sessions), so the tests target the pure
//! parts: session-id formatting and the display-name fallback chain. The
//! live enumeration is exercised by the Task 3 manual smoke checklist.
//!
//! Windows-only: the module under test is `cfg(target_os = "windows")`.

#![cfg(target_os = "windows")]

use volumectl_lib::audio_sessions_win32::{resolved_session_name, session_id};

#[test]
fn resolved_session_name_prefers_non_empty_display_name() {
    assert_eq!(
        resolved_session_name("Spotify", "Spotify.exe", 1234),
        "Spotify"
    );
    // Surrounding whitespace in the OS display name is trimmed.
    assert_eq!(
        resolved_session_name("  Firefox  ", "firefox.exe", 55),
        "Firefox"
    );
}

#[test]
fn resolved_session_name_falls_back_to_process_base_name() {
    assert_eq!(resolved_session_name("", "chrome.exe", 42), "chrome.exe");
    assert_eq!(
        resolved_session_name("   ", "spotify.exe", 7),
        "spotify.exe"
    );
}

#[test]
fn resolved_session_name_uses_pid_placeholder_as_last_resort() {
    assert_eq!(resolved_session_name("", "", 4242), "Process 4242");
    assert_eq!(resolved_session_name("  ", "  ", 9), "Process 9");
}

#[test]
fn session_id_round_trips_as_pid_string() {
    assert_eq!(session_id(0), "0");
    assert_eq!(session_id(3556), "3556");
    assert_eq!(session_id(u32::MAX), "4294967295");
}
