//! Single-instance launch tests.
//!
//! On Windows:
//!   - The first acquire returns [`Launch::Primary`].
//!   - A second acquire in the same process returns [`Launch::Secondary`].
//!   - A secondary launch can call [`signal_show_mixer`] to wake the primary.
//!   - The primary instance's [`PrimaryGuard::poll_show_mixer`] consumes the signal.
//!   - Dropping [`PrimaryGuard`] releases the named mutex and event via RAII.
//!
//! On non-Windows:
//!   - [`acquire`] always returns [`Launch::Primary`].

#![cfg_attr(not(target_os = "windows"), allow(dead_code, unused_imports))]

#[path = "../src/single_instance.rs"]
mod single_instance;

use single_instance::{
    acquire, acquire_named, set_custom_names_for_test, signal_show_mixer, signal_show_mixer_named,
    Launch, PrimaryGuard, DEFAULT_MUTEX_NAME, DEFAULT_SHOW_MIXER_EVENT_NAME, TEST_MUTEX_NAME,
    TEST_SHOW_MIXER_EVENT_NAME,
};
use std::sync::Mutex;

/// Global test lock to serialize tests using the default test instance names
/// across parallel test worker threads in the same process.
static TEST_SERIAL_LOCK: Mutex<()> = Mutex::new(());

const DETERMINISTIC_MUTEX_A: &str = "Local\\VolumeControl.Test.InstanceA.SingleInstance";
const DETERMINISTIC_EVENT_A: &str = "Local\\VolumeControl.Test.InstanceA.ShowMixer";

const DETERMINISTIC_MUTEX_B: &str = "Local\\VolumeControl.Test.InstanceB.SingleInstance";
const DETERMINISTIC_EVENT_B: &str = "Local\\VolumeControl.Test.InstanceB.ShowMixer";

#[test]
fn default_constants_match_legacy_host_contract() {
    assert_eq!(
        DEFAULT_MUTEX_NAME,
        "Local\\VolumeControl.Egui.SingleInstance"
    );
    assert_eq!(
        DEFAULT_SHOW_MIXER_EVENT_NAME,
        "Local\\VolumeControl.Egui.ShowMixer"
    );
    assert_eq!(TEST_MUTEX_NAME, "Local\\VolumeControl.Test.SingleInstance");
    assert_eq!(
        TEST_SHOW_MIXER_EVENT_NAME,
        "Local\\VolumeControl.Test.ShowMixer"
    );
}

#[test]
#[cfg(target_os = "windows")]
fn windows_acquire_twice_returns_primary_then_secondary() {
    let _lock = TEST_SERIAL_LOCK.lock().unwrap();

    // Use deterministic names isolated from any running desktop app.
    set_custom_names_for_test(
        Some(DETERMINISTIC_MUTEX_A.to_string()),
        Some(DETERMINISTIC_EVENT_A.to_string()),
    );

    let first = acquire();
    let primary_guard: PrimaryGuard = match first {
        Launch::Primary(guard) => guard,
        Launch::Secondary => panic!("first acquire must return Launch::Primary"),
    };

    let second = acquire();
    match second {
        Launch::Secondary => {}
        Launch::Primary(_) => {
            panic!("second acquire while holding guard must return Launch::Secondary")
        }
    }

    // Drop the guard to verify RAII cleanup.
    drop(primary_guard);

    // After RAII drop, a new acquire must succeed as Primary again.
    let reacquired = acquire();
    assert!(
        matches!(reacquired, Launch::Primary(_)),
        "acquire after primary guard drop must succeed as Launch::Primary"
    );

    // Clean up
    set_custom_names_for_test(None, None);
}

#[test]
#[cfg(target_os = "windows")]
fn windows_signal_show_mixer_observed_by_primary() {
    let _lock = TEST_SERIAL_LOCK.lock().unwrap();

    set_custom_names_for_test(
        Some(DETERMINISTIC_MUTEX_B.to_string()),
        Some(DETERMINISTIC_EVENT_B.to_string()),
    );

    let primary = acquire();
    let guard: PrimaryGuard = match primary {
        Launch::Primary(guard) => guard,
        Launch::Secondary => panic!("acquire must return Launch::Primary"),
    };

    // Initially there is no pending signal.
    assert!(
        !guard.poll_show_mixer(),
        "freshly created primary guard must have no pending show_mixer signals"
    );

    // Secondary signals the event.
    assert!(
        signal_show_mixer(),
        "signal_show_mixer must succeed when primary is listening"
    );

    // Primary observes the signal.
    assert!(
        guard.poll_show_mixer(),
        "poll_show_mixer must observe the signaled event"
    );

    // Event is auto-reset: subsequent poll must return false immediately.
    assert!(
        !guard.poll_show_mixer(),
        "auto-reset event must clear immediately after observation"
    );

    // Clean up
    drop(guard);
    set_custom_names_for_test(None, None);
}

#[test]
#[cfg(target_os = "windows")]
fn windows_acquire_named_and_raii_cleanup() {
    let _lock = TEST_SERIAL_LOCK.lock().unwrap();

    let mutex_name = "Local\\VolumeControl.Test.Named.SingleInstance";
    let event_name = "Local\\VolumeControl.Test.Named.ShowMixer";

    let first = acquire_named(mutex_name, event_name);
    let guard: PrimaryGuard = match first {
        Launch::Primary(g) => g,
        Launch::Secondary => panic!("acquire_named must succeed as primary"),
    };

    let second = acquire_named(mutex_name, event_name);
    assert!(
        matches!(second, Launch::Secondary),
        "acquire_named must return Secondary while first guard is held"
    );

    assert!(signal_show_mixer_named(event_name));
    assert!(guard.poll_show_mixer());
    assert!(!guard.poll_show_mixer());

    drop(guard);

    // After drop, acquiring again succeeds as primary.
    let third = acquire_named(mutex_name, event_name);
    assert!(matches!(third, Launch::Primary(_)));
}

#[test]
#[cfg(not(target_os = "windows"))]
fn non_windows_acquire_returns_primary() {
    let first = acquire();
    let guard: PrimaryGuard = match first {
        Launch::Primary(guard) => guard,
        Launch::Secondary => panic!("non-windows acquire must always return Launch::Primary"),
    };

    assert!(!guard.poll_show_mixer());
    assert!(!signal_show_mixer());
}
