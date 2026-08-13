//! Cross-platform global keyboard shortcuts backed by `global-hotkey`.
//!
//! A fixed set of hotkey combos is registered through the operating system's
//! native APIs — Windows `RegisterHotKey` (hidden window, no low-level hook),
//! macOS Carbon `RegisterEventHotKey` (no Accessibility permission), Linux
//! X11 via pure-Rust `x11rb` XGrabKey. Press and release are reported per
//! combo, and Hold-to-Repeat is implemented here: the first press emits
//! immediately, then a worker repeats the volume action every 50 ms until the
//! combo is released.
//!
//! The `CapsLock` modifier is not expressible in any of the native
//! registration APIs, so it falls back to the `Ctrl+Alt` combos with a
//! warning (see [`combos_for`] and [`GlobalHotkeys::new`]).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use crate::config::HotkeyModifier;
use crate::hotkeys::{
    hotkey_from_id, hotkey_id, HotkeyAction, HotkeyRegError, HotkeyRegResult, HotkeyRegStatus,
    ALL_HOTKEY_ACTIONS,
};

const REPEAT_INTERVAL: Duration = Duration::from_millis(50);
const LISTENER_POLL: Duration = Duration::from_millis(5);

/// Build the hotkey combos for one configured modifier.
///
/// Every action maps to a `(HotKey, HotkeyAction)` pair. `CapsLock` is not
/// expressible in the OS registration APIs and falls back to the `Ctrl+Alt`
/// combos. On macOS the `CtrlAlt` (and CapsLock fallback) modifier also
/// registers the `⌘+⌥` spelling of every combo, preserving the documented
/// "either ⌘ or ⌃" behavior.
pub fn combos_for(modifier: HotkeyModifier) -> Vec<(HotKey, HotkeyAction)> {
    let base = match modifier {
        HotkeyModifier::CtrlAlt | HotkeyModifier::CapsLock => Modifiers::CONTROL | Modifiers::ALT,
        HotkeyModifier::Alt => Modifiers::ALT,
        HotkeyModifier::Ctrl => Modifiers::CONTROL,
    };
    let mut combos = Vec::with_capacity(16);
    let mut push = |key: Code, extra: Modifiers, action: HotkeyAction| {
        combos.push((HotKey::new(Some(base | extra), key), action));
        #[cfg(target_os = "macos")]
        if matches!(modifier, HotkeyModifier::CtrlAlt | HotkeyModifier::CapsLock) {
            combos.push((
                HotKey::new(Some(Modifiers::SUPER | Modifiers::ALT | extra), key),
                action,
            ));
        }
    };
    push(Code::ArrowUp, Modifiers::empty(), HotkeyAction::VolumeUp);
    push(
        Code::ArrowDown,
        Modifiers::empty(),
        HotkeyAction::VolumeDown,
    );
    push(Code::ArrowUp, Modifiers::SHIFT, HotkeyAction::VolumeUpLarge);
    push(
        Code::ArrowDown,
        Modifiers::SHIFT,
        HotkeyAction::VolumeDownLarge,
    );
    push(Code::KeyM, Modifiers::empty(), HotkeyAction::ToggleMute);
    push(Code::KeyM, Modifiers::SHIFT, HotkeyAction::OpenMenu);
    push(Code::KeyR, Modifiers::empty(), HotkeyAction::Reset50);
    push(Code::KeyV, Modifiers::empty(), HotkeyAction::OpenMixer);
    combos
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

/// Hold state shared by the listener thread and the repeat worker.
struct HotkeyHold {
    holding: AtomicBool,
    hold_combo: AtomicU32,
    hold_action: AtomicU8,
    combo_down: AtomicU32,
    wake_lock: Mutex<()>,
    wake: Condvar,
}

impl HotkeyHold {
    fn new() -> Self {
        Self {
            holding: AtomicBool::new(false),
            hold_combo: AtomicU32::new(0),
            hold_action: AtomicU8::new(0),
            combo_down: AtomicU32::new(0),
            wake_lock: Mutex::new(()),
            wake: Condvar::new(),
        }
    }

    fn start(&self, combo: u32, action: HotkeyAction) {
        self.hold_combo.store(combo, Ordering::Release);
        self.hold_action
            .store(hotkey_id(action) as u8, Ordering::Release);
        self.holding.store(true, Ordering::Release);
        self.wake.notify_one();
    }

    fn stop(&self) {
        self.holding.store(false, Ordering::Release);
        self.hold_combo.store(0, Ordering::Release);
        self.hold_action.store(0, Ordering::Release);
        self.wake.notify_one();
    }
}

/// Translate one platform event into an action the host should apply.
///
/// Pure and unit-testable. Returns `Some(action)` exactly when the action
/// should be emitted now; `None` for auto-repeat presses, releases, and
/// unknown combos.
fn on_event(
    hold: &HotkeyHold,
    combo: u32,
    state: HotKeyState,
    action: HotkeyAction,
) -> Option<HotkeyAction> {
    match state {
        HotKeyState::Pressed => {
            let down = hold.combo_down.swap(combo, Ordering::AcqRel);
            if down == combo {
                // The OS re-delivered the same combo (macOS Carbon
                // auto-repeat). Volume repeats are owned by the worker and
                // command actions are one-shot.
                return None;
            }
            if is_volume_action(action) {
                hold.start(combo, action);
            }
            Some(action)
        }
        HotKeyState::Released => {
            if hold.combo_down.load(Ordering::Acquire) == combo {
                hold.combo_down.store(0, Ordering::Release);
            }
            if hold.holding.load(Ordering::Acquire)
                && hold.hold_combo.load(Ordering::Acquire) == combo
            {
                hold.stop();
            }
            None
        }
    }
}

/// Drain the platform event channel into the action channel while driving
/// hold/repeat state.
fn run_listener(
    ids: Arc<RwLock<HashMap<u32, HotkeyAction>>>,
    hold: Arc<HotkeyHold>,
    tx: Sender<HotkeyAction>,
    stop: Arc<AtomicBool>,
) {
    while !stop.load(Ordering::Acquire) {
        match GlobalHotKeyEvent::receiver().try_recv() {
            Ok(event) => {
                let action = ids
                    .read()
                    .expect("hotkey ids poisoned")
                    .get(&event.id())
                    .copied();
                if let Some(action) =
                    action.and_then(|action| on_event(&hold, event.id(), event.state(), action))
                {
                    let _ = tx.send(action);
                }
            }
            Err(_) => thread::sleep(LISTENER_POLL),
        }
    }
}

/// Emit the held volume action every `interval` while a hold is active.
fn run_repeat_worker(
    hold: Arc<HotkeyHold>,
    tx: Sender<HotkeyAction>,
    stop: Arc<AtomicBool>,
    interval: Duration,
) {
    loop {
        if stop.load(Ordering::Acquire) {
            break;
        }
        if !hold.holding.load(Ordering::Acquire) {
            let guard = hold.wake_lock.lock().expect("hotkey wake mutex poisoned");
            let _guard = hold
                .wake
                .wait_while(guard, |_| {
                    !hold.holding.load(Ordering::Acquire) && !stop.load(Ordering::Acquire)
                })
                .expect("hotkey wake mutex poisoned");
            continue;
        }
        // Wait a full interval before the first repeat: the physical press
        // already emitted the first action.
        let guard = hold.wake_lock.lock().expect("hotkey wake mutex poisoned");
        let (_guard, _) = hold
            .wake
            .wait_timeout(guard, interval)
            .expect("hotkey wake mutex poisoned");
        if !hold.holding.load(Ordering::Acquire) {
            continue;
        }
        let action = hold.hold_action.load(Ordering::Acquire);
        if let Some(action) = hotkey_from_id(action as i32) {
            let _ = tx.send(action);
        }
    }
}

/// Map a registration failure to the shared status model.
fn registration_error(error: &global_hotkey::Error) -> HotkeyRegError {
    match error {
        global_hotkey::Error::AlreadyRegistered(_) => HotkeyRegError {
            error_code: 1409, // ERROR_HOTKEY_ALREADY_REGISTERED
            message: "hotkey already registered by another application".into(),
        },
        other => HotkeyRegError {
            error_code: 0,
            message: format!("{other}"),
        },
    }
}

/// Register `combos` and report per-action status. Conflicts are skipped with
/// a warning and the remaining combos still register (resilient, matching the
/// previous backend's behavior).
fn register_combos(
    manager: &GlobalHotKeyManager,
    combos: &[(HotKey, HotkeyAction)],
) -> (
    HashMap<u32, HotkeyAction>,
    Vec<HotKey>,
    Vec<HotkeyRegResult>,
) {
    let mut ids = HashMap::with_capacity(combos.len());
    let mut registered = Vec::with_capacity(combos.len());
    let mut reg_results = ALL_HOTKEY_ACTIONS
        .iter()
        .map(|&action| HotkeyRegResult {
            action,
            status: HotkeyRegStatus::Conflicted(HotkeyRegError {
                error_code: 0,
                message: "registration pending".into(),
            }),
        })
        .collect::<Vec<_>>();

    for (hotkey, action) in combos {
        match manager.register(*hotkey) {
            Ok(()) => {
                ids.insert(hotkey.id(), *action);
                registered.push(*hotkey);
                log::debug!("registered {hotkey} -> {action:?}");
            }
            Err(error) => {
                log::warn!("hotkey registration failed for {action:?}: {error}");
                for result in &mut reg_results {
                    if result.action == *action {
                        result.status = HotkeyRegStatus::Conflicted(registration_error(&error));
                    }
                }
            }
        }
    }

    // An action is active if at least one of its combos registered.
    for result in &mut reg_results {
        if combos
            .iter()
            .any(|(hotkey, action)| *action == result.action && ids.contains_key(&hotkey.id()))
        {
            result.status = HotkeyRegStatus::Registered;
        }
    }

    (ids, registered, reg_results)
}

/// Global hotkey backend built on `global-hotkey`.
///
/// One instance owns the native manager, the registered combos, the
/// id→action table, the listener thread and the repeat worker. Hosts drain
/// [`GlobalHotkeys::try_recv`] exactly as hosts drained the previous backend.
pub struct GlobalHotkeys {
    manager: GlobalHotKeyManager,
    registered: Mutex<Vec<HotKey>>,
    ids: Arc<RwLock<HashMap<u32, HotkeyAction>>>,
    hold: Arc<HotkeyHold>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    listener: Option<JoinHandle<()>>,
    reg_results: Mutex<Vec<HotkeyRegResult>>,
    rx: Receiver<HotkeyAction>,
}

impl GlobalHotkeys {
    pub fn new(initial_modifier: HotkeyModifier) -> Result<Self, String> {
        if initial_modifier == HotkeyModifier::CapsLock {
            log::warn!("CapsLock is not supported by global-hotkey; using Ctrl+Alt combos instead");
        }
        let manager = GlobalHotKeyManager::new()
            .map_err(|error| format!("create global hotkey manager: {error}"))?;
        let combos = combos_for(initial_modifier);
        let (ids, registered, reg_results) = register_combos(&manager, &combos);

        let (tx, rx) = mpsc::channel();
        let ids = Arc::new(RwLock::new(ids));
        let hold = Arc::new(HotkeyHold::new());
        let stop = Arc::new(AtomicBool::new(false));

        let worker = thread::Builder::new()
            .name("volumectl-hotkey-repeat".into())
            .spawn({
                let hold = Arc::clone(&hold);
                let tx = tx.clone();
                let stop = Arc::clone(&stop);
                move || run_repeat_worker(hold, tx, stop, REPEAT_INTERVAL)
            })
            .map_err(|error| format!("start hotkey repeat worker: {error}"))?;

        let listener = thread::Builder::new()
            .name("volumectl-hotkey-listener".into())
            .spawn({
                let ids = Arc::clone(&ids);
                let hold = Arc::clone(&hold);
                let stop = Arc::clone(&stop);
                move || run_listener(ids, hold, tx, stop)
            })
            .map_err(|error| format!("start hotkey listener: {error}"))?;

        Ok(Self {
            manager,
            registered: Mutex::new(registered),
            ids,
            hold,
            stop,
            worker: Some(worker),
            listener: Some(listener),
            reg_results: Mutex::new(reg_results),
            rx,
        })
    }

    /// Apply a config change: unregister and re-register every combo for the
    /// new modifier without restarting the host.
    pub fn set_modifier(&self, modifier: HotkeyModifier) {
        self.hold.stop();
        if modifier == HotkeyModifier::CapsLock {
            log::warn!("CapsLock is not supported by global-hotkey; using Ctrl+Alt combos instead");
        }
        {
            let registered = self.registered.lock().expect("hotkey list poisoned");
            for hotkey in registered.iter() {
                let _ = self.manager.unregister(*hotkey);
            }
        }
        let combos = combos_for(modifier);
        let (ids, registered, reg_results) = register_combos(&self.manager, &combos);
        *self.ids.write().expect("hotkey ids poisoned") = ids;
        *self.registered.lock().expect("hotkey list poisoned") = registered;
        *self.reg_results.lock().expect("hotkey results poisoned") = reg_results;
    }

    /// Return the next action, if the listener has queued one.
    pub fn try_recv(&self) -> Option<HotkeyAction> {
        self.rx.try_recv().ok()
    }

    /// With `global-hotkey`, registration failures are per-combo and surface
    /// through [`GlobalHotkeys::status`]; the event listener cannot fail once
    /// the manager exists, so this always reports `None`.
    pub fn listener_failure(&self) -> Option<String> {
        None
    }

    /// Per-action registration status for the Help surface.
    pub fn status(&self) -> Vec<HotkeyRegResult> {
        self.reg_results
            .lock()
            .expect("hotkey results poisoned")
            .clone()
    }
}

impl Drop for GlobalHotkeys {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.hold.stop();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        if let Some(listener) = self.listener.take() {
            let _ = listener.join();
        }
        let registered = self.registered.lock().expect("hotkey list poisoned");
        for hotkey in registered.iter() {
            let _ = self.manager.unregister(*hotkey);
        }
    }
}

/// Human-readable label for the configured modifier.
#[cfg(not(target_os = "windows"))]
#[cfg(target_os = "macos")]
fn modifier_label(modifier: HotkeyModifier) -> &'static str {
    match modifier {
        HotkeyModifier::CtrlAlt | HotkeyModifier::CapsLock => "⌘/⌃+⌥",
        HotkeyModifier::Alt => "⌥",
        HotkeyModifier::Ctrl => "⌘/⌃",
    }
}

#[cfg(not(target_os = "windows"))]
#[cfg(not(target_os = "macos"))]
fn modifier_label(modifier: HotkeyModifier) -> &'static str {
    match modifier {
        HotkeyModifier::CtrlAlt | HotkeyModifier::CapsLock => "Ctrl+Alt",
        HotkeyModifier::Alt => "Alt",
        HotkeyModifier::Ctrl => "Ctrl",
    }
}

/// Run the non-Windows fallback host with global hotkeys enabled.
#[cfg(not(target_os = "windows"))]
pub fn run_headless() -> Result<(), String> {
    let config = crate::config::load();
    let audio = crate::audio::default_backend().map_err(|error| error.to_string())?;
    let hotkeys = GlobalHotkeys::new(config.modifier)?;

    let combo = modifier_label(config.modifier);
    eprintln!(
        "VolumeControl global hotkeys running (global-hotkey).\n\
         config: {}\n\
         modifier: {combo} — hold {combo}↑/↓ to repeat, {combo}M mutes,\n\
         {combo}R resets to 50%, {combo}V opens the mixer (headless: no-op).",
        crate::config::config_path().display(),
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
    use global_hotkey::hotkey::{HotKey, Modifiers};
    use global_hotkey::HotKeyState;
    use std::sync::atomic::Ordering;
    use std::sync::mpsc;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    fn combos() -> Vec<(HotKey, HotkeyAction)> {
        combos_for(HotkeyModifier::CtrlAlt)
    }

    fn combo_of(combos: &[(HotKey, HotkeyAction)], action: HotkeyAction) -> u32 {
        combos
            .iter()
            .find(|(_, a)| *a == action)
            .map(|(hotkey, _)| hotkey.id)
            .expect("combo exists")
    }

    #[test]
    fn combos_for_ctrl_alt_covers_every_action() {
        let combos = combos();
        for action in ALL_HOTKEY_ACTIONS {
            assert!(
                combos.iter().any(|(_, a)| *a == action),
                "missing {action:?}"
            );
        }
        #[cfg(not(target_os = "macos"))]
        assert_eq!(combos.len(), 8);
        #[cfg(target_os = "macos")]
        assert_eq!(combos.len(), 16, "macOS also registers the ⌘+⌥ spelling");
    }

    #[test]
    fn combos_for_caps_lock_falls_back_to_ctrl_alt() {
        assert_eq!(combos_for(HotkeyModifier::CapsLock), combos());
    }

    #[test]
    fn alt_and_ctrl_modifiers_use_their_own_base() {
        let alt_up = combos_for(HotkeyModifier::Alt)
            .iter()
            .find(|(_, a)| *a == HotkeyAction::VolumeUp)
            .map(|(hotkey, _)| hotkey.mods)
            .expect("combo exists");
        assert_eq!(alt_up, Modifiers::ALT);

        let ctrl_up = combos_for(HotkeyModifier::Ctrl)
            .iter()
            .find(|(_, a)| *a == HotkeyAction::VolumeUp)
            .map(|(hotkey, _)| hotkey.mods)
            .expect("combo exists");
        assert_eq!(ctrl_up, Modifiers::CONTROL);

        let ctrl_alt_up = combos()
            .iter()
            .find(|(_, a)| *a == HotkeyAction::VolumeUp)
            .map(|(hotkey, _)| hotkey.mods)
            .expect("combo exists");
        assert_eq!(ctrl_alt_up, Modifiers::CONTROL | Modifiers::ALT);
    }

    #[test]
    fn shift_variants_carry_the_shift_modifier() {
        let combos = combos();
        let (hotkey, _) = combos
            .iter()
            .find(|(_, a)| *a == HotkeyAction::VolumeUpLarge)
            .expect("large combo exists");
        assert!(hotkey.mods.contains(Modifiers::SHIFT));
        assert!(hotkey.mods.contains(Modifiers::CONTROL));
        assert!(hotkey.mods.contains(Modifiers::ALT));
    }

    #[test]
    fn first_press_emits_once_and_starts_the_hold() {
        let hold = HotkeyHold::new();
        let up = combo_of(&combos(), HotkeyAction::VolumeUp);
        assert_eq!(
            on_event(&hold, up, HotKeyState::Pressed, HotkeyAction::VolumeUp),
            Some(HotkeyAction::VolumeUp)
        );
        assert!(hold.holding.load(Ordering::Acquire));
    }

    #[test]
    fn auto_repeat_press_of_the_same_combo_is_ignored() {
        let hold = HotkeyHold::new();
        let up = combo_of(&combos(), HotkeyAction::VolumeUp);
        on_event(&hold, up, HotKeyState::Pressed, HotkeyAction::VolumeUp);
        assert_eq!(
            on_event(&hold, up, HotKeyState::Pressed, HotkeyAction::VolumeUp),
            None,
            "OS auto-repeat must not double-fire"
        );
    }

    #[test]
    fn released_combo_ends_the_hold_and_next_press_emits() {
        let hold = HotkeyHold::new();
        let up = combo_of(&combos(), HotkeyAction::VolumeUp);
        on_event(&hold, up, HotKeyState::Pressed, HotkeyAction::VolumeUp);
        assert_eq!(
            on_event(&hold, up, HotKeyState::Released, HotkeyAction::VolumeUp),
            None
        );
        assert!(!hold.holding.load(Ordering::Acquire));
        assert_eq!(
            on_event(&hold, up, HotKeyState::Pressed, HotkeyAction::VolumeUp),
            Some(HotkeyAction::VolumeUp)
        );
    }

    #[test]
    fn command_action_is_one_shot_under_auto_repeat() {
        let hold = HotkeyHold::new();
        let mute = combo_of(&combos(), HotkeyAction::ToggleMute);
        assert_eq!(
            on_event(&hold, mute, HotKeyState::Pressed, HotkeyAction::ToggleMute),
            Some(HotkeyAction::ToggleMute)
        );
        assert_eq!(
            on_event(&hold, mute, HotKeyState::Pressed, HotkeyAction::ToggleMute),
            None,
            "command actions must be one-shot"
        );
        assert!(!hold.holding.load(Ordering::Acquire));
    }

    #[test]
    fn command_press_during_a_volume_hold_leaves_the_hold_running() {
        let hold = HotkeyHold::new();
        let combos = combos();
        let up = combo_of(&combos, HotkeyAction::VolumeUp);
        let mute = combo_of(&combos, HotkeyAction::ToggleMute);
        on_event(&hold, up, HotKeyState::Pressed, HotkeyAction::VolumeUp);
        assert_eq!(
            on_event(&hold, mute, HotKeyState::Pressed, HotkeyAction::ToggleMute),
            Some(HotkeyAction::ToggleMute)
        );
        assert!(hold.holding.load(Ordering::Acquire));
        on_event(&hold, mute, HotKeyState::Released, HotkeyAction::ToggleMute);
        assert!(
            hold.holding.load(Ordering::Acquire),
            "volume hold continues"
        );
        on_event(&hold, up, HotKeyState::Released, HotkeyAction::VolumeUp);
        assert!(!hold.holding.load(Ordering::Acquire));
    }

    #[test]
    fn volume_hold_switches_to_a_newly_pressed_volume_combo() {
        let hold = HotkeyHold::new();
        let combos = combos();
        let up = combo_of(&combos, HotkeyAction::VolumeUp);
        let down = combo_of(&combos, HotkeyAction::VolumeDown);
        on_event(&hold, up, HotKeyState::Pressed, HotkeyAction::VolumeUp);
        assert_eq!(
            on_event(&hold, down, HotKeyState::Pressed, HotkeyAction::VolumeDown),
            Some(HotkeyAction::VolumeDown)
        );
        assert_eq!(hold.hold_combo.load(Ordering::Acquire), down);
        on_event(&hold, up, HotKeyState::Released, HotkeyAction::VolumeUp);
        assert!(hold.holding.load(Ordering::Acquire));
        on_event(&hold, down, HotKeyState::Released, HotkeyAction::VolumeDown);
        assert!(!hold.holding.load(Ordering::Acquire));
    }

    #[test]
    fn repeat_worker_waits_a_full_interval_before_the_first_repeat() {
        let hold = Arc::new(HotkeyHold::new());
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker = {
            let hold = Arc::clone(&hold);
            let tx = tx.clone();
            let stop = Arc::clone(&stop);
            thread::spawn(move || run_repeat_worker(hold, tx, stop, Duration::from_secs(60)))
        };
        let up = combo_of(&combos(), HotkeyAction::VolumeUp);
        on_event(&hold, up, HotKeyState::Pressed, HotkeyAction::VolumeUp);
        thread::sleep(Duration::from_millis(100));
        hold.stop();
        stop.store(true, Ordering::Release);
        hold.wake.notify_one();
        let _ = worker.join();
        assert!(
            rx.try_iter().collect::<Vec<_>>().is_empty(),
            "a physical press must emit exactly one action (no early repeat)"
        );
    }
}
