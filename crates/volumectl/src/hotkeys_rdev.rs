//! Cross-platform global keyboard shortcuts backed by `rdev`.
//!
//! The listener deliberately does not use the operating system's native key
//! repeat. The first press emits an action immediately, then a small worker
//! emits the same volume action every 50 ms until *any* key in the shortcut is
//! released. Command actions are emitted once per physical press, even when
//! the OS sends repeated `KeyPress` events.
//!
//! `rdev::listen` is a blocking listener with no portable unlisten operation.
//! The listener handle is therefore detached on drop; the callback observes
//! the stop flag and the process teardown removes the platform hook. The
//! repeat worker itself is always stopped and joined cleanly.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use rdev::{listen, EventType, Key};

use crate::config::HotkeyModifier;
use crate::hotkeys::{
    hotkey_id, HotkeyAction, HotkeyRegResult, HotkeyRegStatus, ALL_HOTKEY_ACTIONS,
};

const REPEAT_INTERVAL: Duration = Duration::from_millis(50);

/// Shared state used by the listener and the repeat worker.
struct KeyboardState {
    ctrl: AtomicBool,
    alt: AtomicBool,
    meta: AtomicBool,
    caps_lock: AtomicBool,
    shift: AtomicBool,
    up: AtomicBool,
    down: AtomicBool,
    mute: AtomicBool,
    reset: AtomicBool,
    mixer: AtomicBool,
    holding: AtomicBool,
    hold_action: AtomicU8,
    /// The worker waits on this pair instead of polling in a tight loop.
    wake_lock: Mutex<()>,
    wake: Condvar,
}

impl KeyboardState {
    fn new() -> Self {
        Self {
            ctrl: AtomicBool::new(false),
            alt: AtomicBool::new(false),
            meta: AtomicBool::new(false),
            caps_lock: AtomicBool::new(false),
            shift: AtomicBool::new(false),
            up: AtomicBool::new(false),
            down: AtomicBool::new(false),
            mute: AtomicBool::new(false),
            reset: AtomicBool::new(false),
            mixer: AtomicBool::new(false),
            holding: AtomicBool::new(false),
            hold_action: AtomicU8::new(0),
            wake_lock: Mutex::new(()),
            wake: Condvar::new(),
        }
    }

    /// Update a physical key flag and return whether this was a new state.
    fn update_key(&self, key: Key, pressed: bool) -> bool {
        let flag = match key {
            Key::ControlLeft | Key::ControlRight => Some(&self.ctrl),
            Key::Alt | Key::AltGr => Some(&self.alt),
            Key::MetaLeft | Key::MetaRight => Some(&self.meta),
            Key::CapsLock => Some(&self.caps_lock),
            Key::ShiftLeft | Key::ShiftRight => Some(&self.shift),
            Key::UpArrow => Some(&self.up),
            Key::DownArrow => Some(&self.down),
            Key::KeyM => Some(&self.mute),
            Key::KeyR => Some(&self.reset),
            Key::KeyV => Some(&self.mixer),
            _ => None,
        };
        flag.map(|value| value.swap(pressed, Ordering::AcqRel) != pressed)
            .unwrap_or(true)
    }

    fn stop_hold(&self) {
        self.holding.store(false, Ordering::Release);
        self.hold_action.store(0, Ordering::Release);
        self.wake.notify_one();
    }

    fn start_hold(&self, action: HotkeyAction) {
        self.hold_action
            .store(hotkey_id(action) as u8, Ordering::Release);
        self.holding.store(true, Ordering::Release);
        self.wake.notify_one();
    }

    fn stop_worker(&self) {
        self.stop_hold();
        self.wake.notify_one();
    }
}

fn modifier_code(modifier: HotkeyModifier) -> u8 {
    match modifier {
        HotkeyModifier::CtrlAlt => 0,
        HotkeyModifier::Alt => 1,
        HotkeyModifier::Ctrl => 2,
        HotkeyModifier::CapsLock => 3,
    }
}

fn modifier_from_code(code: u8) -> HotkeyModifier {
    match code {
        1 => HotkeyModifier::Alt,
        2 => HotkeyModifier::Ctrl,
        3 => HotkeyModifier::CapsLock,
        _ => HotkeyModifier::CtrlAlt,
    }
}

#[cfg(target_os = "macos")]
fn primary_modifier_held(state: &KeyboardState) -> bool {
    state.meta.load(Ordering::Acquire)
}

#[cfg(not(target_os = "macos"))]
fn primary_modifier_held(state: &KeyboardState) -> bool {
    state.ctrl.load(Ordering::Acquire)
}

fn modifier_held(state: &KeyboardState, modifier: HotkeyModifier) -> bool {
    let alt = state.alt.load(Ordering::Acquire);
    let primary = primary_modifier_held(state);
    match modifier {
        HotkeyModifier::CtrlAlt => primary && alt,
        HotkeyModifier::Alt => alt,
        HotkeyModifier::Ctrl => primary,
        HotkeyModifier::CapsLock => state.caps_lock.load(Ordering::Acquire),
    }
}

fn is_volume_action(action: HotkeyAction) -> bool {
    matches!(
        action,
        HotkeyAction::VolumeUp
            | HotkeyAction::VolumeDown
            | HotkeyAction::VolumeUpLarge
            | HotkeyAction::VolumeDownLarge
    )
}

fn action_for_key(
    state: &KeyboardState,
    modifier: HotkeyModifier,
    key: Key,
) -> Option<HotkeyAction> {
    if !modifier_held(state, modifier) {
        return None;
    }

    let shifted = state.shift.load(Ordering::Acquire);
    Some(match key {
        Key::UpArrow => {
            if shifted {
                HotkeyAction::VolumeUpLarge
            } else {
                HotkeyAction::VolumeUp
            }
        }
        Key::DownArrow => {
            if shifted {
                HotkeyAction::VolumeDownLarge
            } else {
                HotkeyAction::VolumeDown
            }
        }
        Key::KeyM => {
            if shifted {
                HotkeyAction::OpenMenu
            } else {
                HotkeyAction::ToggleMute
            }
        }
        Key::KeyR => HotkeyAction::Reset50,
        Key::KeyV => HotkeyAction::OpenMixer,
        _ => return None,
    })
}

fn is_hold_release_key(key: Key) -> bool {
    matches!(
        key,
        Key::ControlLeft
            | Key::ControlRight
            | Key::Alt
            | Key::AltGr
            | Key::MetaLeft
            | Key::MetaRight
            | Key::CapsLock
            | Key::ShiftLeft
            | Key::ShiftRight
            | Key::UpArrow
            | Key::DownArrow
    )
}

fn is_shortcut_key(key: Key) -> bool {
    matches!(
        key,
        Key::ControlLeft
            | Key::ControlRight
            | Key::Alt
            | Key::AltGr
            | Key::MetaLeft
            | Key::MetaRight
            | Key::CapsLock
            | Key::ShiftLeft
            | Key::ShiftRight
            | Key::UpArrow
            | Key::DownArrow
            | Key::KeyM
            | Key::KeyR
            | Key::KeyV
    )
}

fn process_event(
    event: EventType,
    modifier: &AtomicU8,
    state: &KeyboardState,
    tx: &Sender<HotkeyAction>,
) {
    match event {
        EventType::KeyPress(key) => {
            if is_shortcut_key(key) {
                log::debug!("rdev key press: {key:?}");
            }
            // rdev may report repeated KeyPress events while a key is held.
            // Only the first physical press can trigger a command or start a
            // repeat; the worker owns the cadence after that.
            if !state.update_key(key, true) {
                return;
            }

            let modifier = modifier_from_code(modifier.load(Ordering::Acquire));
            let Some(action) = action_for_key(state, modifier, key) else {
                return;
            };
            log::debug!("global hotkey action: {action:?}");
            let _ = tx.send(action); // instant feedback for every action
            if is_volume_action(action) {
                state.start_hold(action);
            }
        }
        EventType::KeyRelease(key) => {
            if is_shortcut_key(key) {
                log::debug!("rdev key release: {key:?}");
            }
            state.update_key(key, false);
            // Releasing either modifier, Shift, or the arrow ends the current
            // hold immediately. A command key was never put into hold state.
            if is_hold_release_key(key) {
                state.stop_hold();
            }
        }
        _ => {}
    }
}

fn run_repeat_worker(
    state: Arc<KeyboardState>,
    tx: Sender<HotkeyAction>,
    listener_stop: Arc<AtomicBool>,
) {
    loop {
        if listener_stop.load(Ordering::Acquire) {
            break;
        }

        if !state.holding.load(Ordering::Acquire) {
            let guard = state.wake_lock.lock().expect("hotkey wake mutex poisoned");
            let _guard = state
                .wake
                .wait_while(guard, |_| {
                    !state.holding.load(Ordering::Acquire) && !listener_stop.load(Ordering::Acquire)
                })
                .expect("hotkey wake mutex poisoned");
            continue;
        }

        let action = state.hold_action.load(Ordering::Acquire) as i32;
        if let Some(action) = crate::hotkeys::hotkey_from_id(action) {
            let _ = tx.send(action);
        }

        let guard = state.wake_lock.lock().expect("hotkey wake mutex poisoned");
        let (_guard, _) = state
            .wake
            .wait_timeout(guard, REPEAT_INTERVAL)
            .expect("hotkey wake mutex poisoned");
    }
}

/// Cross-platform global keyboard listener and Hold-to-Repeat controller.
pub struct RdevHotkeys {
    modifier: Arc<AtomicU8>,
    state: Arc<KeyboardState>,
    listener_stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    /// `rdev::listen` blocks and has no portable cancellation API. Dropping
    /// this handle detaches it; the callback is disabled by `listener_stop`.
    listener: Option<JoinHandle<()>>,
    rx: Receiver<HotkeyAction>,
}

impl RdevHotkeys {
    pub fn new(initial_modifier: HotkeyModifier) -> Result<Self, String> {
        let (tx, rx) = mpsc::channel();
        let modifier = Arc::new(AtomicU8::new(modifier_code(initial_modifier)));
        let state = Arc::new(KeyboardState::new());
        let listener_stop = Arc::new(AtomicBool::new(false));

        let worker_state = Arc::clone(&state);
        let worker_stop = Arc::clone(&listener_stop);
        let worker_tx = tx.clone();
        let worker = thread::Builder::new()
            .name("volumectl-hotkey-repeat".into())
            .spawn(move || run_repeat_worker(worker_state, worker_tx, worker_stop))
            .map_err(|error| format!("start hotkey repeat worker: {error}"))?;

        let listener_modifier = Arc::clone(&modifier);
        let listener_state = Arc::clone(&state);
        let listener_stop_flag = Arc::clone(&listener_stop);
        let listener = thread::Builder::new()
            .name("volumectl-rdev-listener".into())
            .spawn(move || {
                log::info!("rdev global keyboard listener starting");
                let result = listen(move |event| {
                    if !listener_stop_flag.load(Ordering::Acquire) {
                        process_event(event.event_type, &listener_modifier, &listener_state, &tx);
                    }
                });
                if let Err(error) = result {
                    log::error!("global keyboard listener stopped: {error:?}");
                }
            })
            .map_err(|error| format!("start rdev listener: {error}"))?;

        Ok(Self {
            modifier,
            state,
            listener_stop,
            worker: Some(worker),
            listener: Some(listener),
            rx,
        })
    }

    /// Apply a config change without restarting the listener.
    pub fn set_modifier(&self, modifier: HotkeyModifier) {
        self.state.stop_hold();
        self.modifier
            .store(modifier_code(modifier), Ordering::Release);
    }

    /// Return the next action, if the listener has queued one.
    pub fn try_recv(&self) -> Option<HotkeyAction> {
        match self.rx.try_recv() {
            Ok(action) => Some(action),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }

    /// The rdev listener owns all configured actions, so there is no per-key
    /// OS registration conflict to report. This preserves the existing Help
    /// surface contract while accurately showing every action as active.
    pub fn status(&self) -> Vec<HotkeyRegResult> {
        ALL_HOTKEY_ACTIONS
            .into_iter()
            .map(|action| HotkeyRegResult {
                action,
                status: HotkeyRegStatus::Registered,
            })
            .collect()
    }
}

impl Drop for RdevHotkeys {
    fn drop(&mut self) {
        self.listener_stop.store(true, Ordering::Release);
        self.state.stop_worker();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        // Do not join `rdev::listen`: on supported platforms it blocks until
        // the process exits. Dropping the handle detaches the listener thread.
        let _ = self.listener.take();
    }
}

/// Run the non-Windows fallback host with global hotkeys enabled. This keeps
/// macOS useful even before a native AppKit surface is opened, and gives a
/// Linux build without GTK a real long-running hotkey mode instead of silently
/// falling back to a one-shot CLI.
#[cfg(not(target_os = "windows"))]
pub fn run_headless() -> Result<(), String> {
    let config = crate::config::load();
    let audio = crate::audio::default_backend().map_err(|error| error.to_string())?;
    let hotkeys = RdevHotkeys::new(config.modifier)?;
    log::info!(
        "volumectl global hotkeys running (rdev; modifier={:?})",
        config.modifier
    );

    loop {
        while let Some(action) = hotkeys.try_recv() {
            use HotkeyAction as H;
            match action {
                H::VolumeUp => adjust_headless(&*audio, config.volume_step as i16),
                H::VolumeDown => adjust_headless(&*audio, -(config.volume_step as i16)),
                H::VolumeUpLarge => adjust_headless(&*audio, config.volume_step_large as i16),
                H::VolumeDownLarge => adjust_headless(&*audio, -(config.volume_step_large as i16)),
                H::ToggleMute => {
                    if let Err(error) = audio.toggle_mute() {
                        log::warn!("toggle mute failed: {error}");
                    }
                }
                H::Reset50 => {
                    if let Err(error) = audio.set_volume(0.5) {
                        log::warn!("reset volume failed: {error}");
                    }
                }
                H::OpenMixer | H::OpenMenu => {
                    log::debug!("{action:?} is unavailable in headless mode");
                }
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(not(target_os = "windows"))]
fn adjust_headless(audio: &dyn crate::audio::AudioBackend, delta_percent: i16) {
    let Ok(current) = audio.get_state() else {
        return;
    };
    let target = crate::core::step_volume(current.volume, delta_percent as f32);
    if let Err(error) = audio.set_volume(target) {
        log::warn!("adjust volume failed: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(state: &KeyboardState, modifier: &AtomicU8, tx: &Sender<HotkeyAction>, key: Key) {
        process_event(EventType::KeyPress(key), modifier, state, tx);
    }

    fn release(state: &KeyboardState, modifier: &AtomicU8, tx: &Sender<HotkeyAction>, key: Key) {
        process_event(EventType::KeyRelease(key), modifier, state, tx);
    }

    #[test]
    fn ctrl_alt_volume_hold_emits_once_and_ignores_native_repeat() {
        let state = KeyboardState::new();
        let modifier = AtomicU8::new(modifier_code(HotkeyModifier::CtrlAlt));
        let (tx, rx) = mpsc::channel();

        press(&state, &modifier, &tx, Key::ControlLeft);
        press(&state, &modifier, &tx, Key::Alt);
        press(&state, &modifier, &tx, Key::DownArrow);
        press(&state, &modifier, &tx, Key::DownArrow);

        assert_eq!(
            rx.try_iter().collect::<Vec<_>>(),
            vec![HotkeyAction::VolumeDown]
        );
        assert!(state.holding.load(Ordering::Acquire));

        release(&state, &modifier, &tx, Key::Alt);
        assert!(!state.holding.load(Ordering::Acquire));
    }

    #[test]
    fn command_key_repeat_is_one_shot() {
        let state = KeyboardState::new();
        let modifier = AtomicU8::new(modifier_code(HotkeyModifier::CtrlAlt));
        let (tx, rx) = mpsc::channel();

        press(&state, &modifier, &tx, Key::ControlLeft);
        press(&state, &modifier, &tx, Key::Alt);
        press(&state, &modifier, &tx, Key::KeyR);
        press(&state, &modifier, &tx, Key::KeyR);

        assert_eq!(
            rx.try_iter().collect::<Vec<_>>(),
            vec![HotkeyAction::Reset50]
        );
    }

    #[test]
    fn releasing_any_combo_key_stops_a_hold() {
        let state = KeyboardState::new();
        let modifier = AtomicU8::new(modifier_code(HotkeyModifier::CtrlAlt));
        let (tx, _rx) = mpsc::channel();

        press(&state, &modifier, &tx, Key::ControlLeft);
        press(&state, &modifier, &tx, Key::Alt);
        press(&state, &modifier, &tx, Key::UpArrow);
        assert!(state.holding.load(Ordering::Acquire));

        release(&state, &modifier, &tx, Key::ControlLeft);
        assert!(!state.holding.load(Ordering::Acquire));
    }

    #[test]
    fn modifier_is_read_from_config_state() {
        let state = KeyboardState::new();
        let modifier = AtomicU8::new(modifier_code(HotkeyModifier::Alt));
        let (tx, rx) = mpsc::channel();

        press(&state, &modifier, &tx, Key::Alt);
        press(&state, &modifier, &tx, Key::UpArrow);
        assert_eq!(
            rx.try_iter().collect::<Vec<_>>(),
            vec![HotkeyAction::VolumeUp]
        );

        state.stop_hold();
        release(&state, &modifier, &tx, Key::Alt);
        modifier.store(modifier_code(HotkeyModifier::Ctrl), Ordering::Release);
        press(&state, &modifier, &tx, Key::ControlLeft);
        press(&state, &modifier, &tx, Key::DownArrow);
        assert_eq!(
            rx.try_iter().collect::<Vec<_>>(),
            vec![HotkeyAction::VolumeDown]
        );
    }
}
