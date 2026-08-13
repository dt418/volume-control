# Global Hotkey Migration (rdev → global-hotkey) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the `rdev` global-keyboard listener with the `global-hotkey` 0.8.0 crate on every platform while preserving all existing hotkey behavior (8 actions, hold-to-repeat, Shift variants, macOS ⌘/⌃ dual modifier, per-action status), and set the default volume step to 1%.

**Architecture:** A new `crates/volumectl/src/hotkeys_global.rs` module owns the `GlobalHotKeyManager`, a combo → action table keyed by `HotKey::id()`, a listener thread draining `GlobalHotKeyEvent::receiver()`, and the existing condvar-based 50 ms repeat worker. Hosts (Windows `app.rs`, GTK `linux_host_core`, `macos_app.rs`, headless) keep their current drain loop via `try_recv()`; only the concrete backend type changes. `CapsLock` config falls back to `Ctrl+Alt` combos with a warning (native APIs cannot express CapsLock as a modifier).

**Tech Stack:** Rust 2021 (workspace, rust-version 1.82), `global-hotkey = "0.8"` (Windows `RegisterHotKey`, macOS Carbon, Linux X11 via `x11rb`), `rdev` removed, existing `windows-sys`/`tray-icon`/GTK/AppKit stack untouched.

## Global Constraints

- **Branch:** all work on `refactor/hotkey-and-ci-fix` (created from `master` before Task 1). No worktrees — user chose an in-checkout branch.
- **Dependency floor:** `global-hotkey = "0.8"` only; no extra features; `rdev` removed from workspace `Cargo.toml`, `crates/volumectl/Cargo.toml`, and regenerated `Cargo.lock`.
- **CI:** `.github/workflows/ci.yml` and `release.yml` are NOT modified (green today; `x11rb` is pure Rust, `windows-sys`/Carbon need nothing new).
- **Behavior preservation:** 8 actions (`VolumeUp/Down`, `VolumeUpLarge/DownLarge`, `ToggleMute`, `Reset50`, `OpenMixer`, `OpenMenu`); hold-to-repeat first-press-immediate then 50 ms; Shift = large step; macOS config `CtrlAlt` registers both `⌃+⌥` and `⌘+⌥`; `CapsLock` config → `Ctrl+Alt` combos + `log::warn!`; `HotkeyRegStatus` contract kept.
- **Accepted nuance (documented in docs/global-hotkeys.md):** release detection is combo-level — the hold ends when the hotkey's main key is released (Windows polls `GetAsyncKeyState` on that key). Releasing only a modifier early does not end the hold while the main key stays down.
- **Records guard (mandatory, every code commit):** every commit touching code must also stage `feature_list.json` (vol-029 entry/status) + `claude-progress.md` (Session 039 entry). `last_updated` bumped in `feature_list.json`.
- **Quality gate per commit:** `cargo fmt --all --check`, `git diff --check`, `cargo clippy --workspace --all-targets --no-default-features -- -D warnings` clean; full battery via `bash scripts/format-lint.sh` before ship.
- **Windows host:** run scripts with `bash` (Git Bash), never `sh`; cross-target checks need the pkg-config stub (`powershell -ExecutionPolicy Bypass -File .agents/skills/windows-host/scripts/ensure-pkg-config-stub.ps1` + `PKG_CONFIG`/`PKG_CONFIG_ALLOW_CROSS` env).
- **Conventional commits:** `feat:` / `refactor:` / `docs:`.

---

### Task 1: Default volume step → 1%

**Files:**
- Modify: `crates/volumectl/src/config.rs` (line 119 doc, line 121 doc, line 179 default)
- Modify: `README.md:13`
- Modify: `README.vi.md:14`
- Records: `feature_list.json`, `claude-progress.md`

**Interfaces:**
- Consumes: existing `Config::default()` / `normalize()` (unchanged).
- Produces: `Config::default().volume_step == 1` (was 2). `volume_step_large` stays 10. The old-JSON compat test (`old_json_without_appearance_uses_appearance_defaults`, which parses explicit `"volume_step": 2`) must keep passing unchanged.

- [ ] **Step 1: Write the failing test**

Append to the `#[cfg(test)] mod tests` block in `crates/volumectl/src/config.rs`:

```rust
#[test]
fn default_volume_step_is_one_percent() {
    let cfg = Config::default();
    assert_eq!(cfg.volume_step, 1, "small step must default to 1%");
    assert_eq!(cfg.volume_step_large, 10, "large step stays 10%");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p volumectl config::tests::default_volume_step_is_one_percent`
Expected: FAIL — `assertion failed: cfg.volume_step == 1` (actual 2).

- [ ] **Step 3: Implement the change**

In `crates/volumectl/src/config.rs`:
- Line 119 doc: `/// Small step percent (default 2).` → `/// Small step percent (default 1).`
- Line 179: `volume_step: 2,` → `volume_step: 1,`
- `MIN_VOLUME_STEP`/`MAX_VOLUME_STEP`/`normalize()` untouched (1 is already in range; `volume_step_large` clamp still yields 2 minimum).

In `README.md` line 13 and `README.vi.md` line 14: `volume ±2%` → `volume ±1%`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p volumectl config::tests::default_volume_step_is_one_percent`
Expected: PASS. Then `cargo test -p volumectl --no-default-features` — full crate suite green.

- [ ] **Step 5: Update records in the same commit**

- `feature_list.json`: add the new feature entry (next free id; the file currently ends at vol-028 — verify `max(id)` first, expect **vol-029**) with `status: "in_progress"`, `area: "hotkeys"`, title `Global hotkeys via global-hotkey crate (1% step)`; bump `last_updated`. Follow the exact JSON shape of an existing entry (id/priority/area/title/user_visible_behavior/status/verification/evidence/notes).
- `claude-progress.md`: open `## Session 039 (2026-08-13) - global-hotkey migration (rdev → global-hotkey, 1% step)` with a Goal line and a short "Task 1: default volume step 2 → 1" section (edit tool only — never PowerShell `Set-Content` on tracked .md).
- Stage all five files, commit:

```bash
git add crates/volumectl/src/config.rs README.md README.vi.md feature_list.json claude-progress.md
git commit -m "feat: default volume step to 1%"
```

Expected: pre-commit hook passes (fmt + whitespace + clippy + `check-records.sh --staged`).

---

### Task 2: Add `global-hotkey` dependency and the hotkey core (TDD)

**Files:**
- Modify: `Cargo.toml` (workspace `[workspace.dependencies]`)
- Modify: `crates/volumectl/Cargo.toml` (`[dependencies]`)
- Modify: `crates/volumectl/src/lib.rs` (module declaration)
- Create: `crates/volumectl/src/hotkeys_global.rs`
- Records: `feature_list.json`, `claude-progress.md`

**Interfaces:**
- Consumes: `crate::hotkeys::{hotkey_id, hotkey_from_id, HotkeyAction, HotkeyRegError, HotkeyRegResult, HotkeyRegStatus, ALL_HOTKEY_ACTIONS}` and `crate::config::HotkeyModifier` (all existing).
- Produces (used by Task 3 and hosts):
  - `pub fn combos_for(modifier: HotkeyModifier) -> Vec<(HotKey, HotkeyAction)>`
  - `pub struct GlobalHotkeys` with `pub fn new(initial_modifier: HotkeyModifier) -> Result<Self, String>`, `pub fn try_recv(&self) -> Option<HotkeyAction>`, `pub fn set_modifier(&self, modifier: HotkeyModifier)`, `pub fn listener_failure(&self) -> Option<String>`, `pub fn status(&self) -> Vec<HotkeyRegResult>`
  - `#[cfg(not(target_os = "windows"))] pub fn run_headless() -> Result<(), String>`
- Keeps `rdev` in Cargo.toml during this task (removed in Task 3) so `hotkeys_rdev.rs` still compiles.

- [ ] **Step 1: Add the dependency (keep rdev for now)**

Workspace `Cargo.toml` — add to `[workspace.dependencies]`:

```toml
# Cross-platform global hotkey registration through native OS APIs (Windows
# RegisterHotKey, macOS Carbon, Linux X11 via x11rb). No low-level hooks and
# no macOS Accessibility permission required.
global-hotkey = "0.8"
```

`crates/volumectl/Cargo.toml` — add next to the rdev line:

```toml
global-hotkey = { workspace = true }
```

Run: `cargo check -p volumectl --no-default-features` — must stay green (rdev still present).

- [ ] **Step 2: Declare the module**

`crates/volumectl/src/lib.rs` — add after the `hotkeys` declaration:

```rust
pub mod hotkeys_global;
```

- [ ] **Step 3: Write the failing tests (RED)**

Create `crates/volumectl/src/hotkeys_global.rs` containing ONLY the module doc comment and this test module (no implementation yet — compile fails, which is the expected RED):

```rust
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

#[cfg(test)]
mod tests {
    use super::*;
    use global_hotkey::hotkey::{Code, HotKey, Modifiers};
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
            assert!(combos.iter().any(|(_, a)| *a == action), "missing {action:?}");
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
        let (hotkey, _) = combos()
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
        assert!(hold.holding.load(Ordering::Acquire), "volume hold continues");
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
```

- [ ] **Step 4: Run tests to verify they fail (RED)**

Run: `cargo test -p volumectl hotkeys_global`
Expected: compile error — `cannot find function combos_for in this scope` (and the other missing items). This is the intended RED.

- [ ] **Step 5: Implement the module (GREEN)**

Replace the file content with the full implementation below (keep the test module from Step 3 unchanged below it):

```rust
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
        HotkeyModifier::CtrlAlt | HotkeyModifier::CapsLock => {
            Modifiers::CONTROL | Modifiers::ALT
        }
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
    push(Code::ArrowDown, Modifiers::empty(), HotkeyAction::VolumeDown);
    push(Code::ArrowUp, Modifiers::SHIFT, HotkeyAction::VolumeUpLarge);
    push(Code::ArrowDown, Modifiers::SHIFT, HotkeyAction::VolumeDownLarge);
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
) -> (HashMap<u32, HotkeyAction>, Vec<HotKey>, Vec<HotkeyRegResult>) {
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
        if combos.iter().any(|(hotkey, action)| {
            *action == result.action && ids.contains_key(&hotkey.id())
        }) {
            result.status = HotkeyRegStatus::Registered;
        }
    }

    (ids, registered, reg_results)
}

/// Global hotkey backend built on `global-hotkey`.
///
/// One instance owns the native manager, the registered combos, the
/// id→action table, the listener thread and the repeat worker. Hosts drain
/// [`GlobalHotkeys::try_recv`] exactly as they drained the rdev backend.
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
            log::warn!(
                "CapsLock is not supported by global-hotkey; using Ctrl+Alt combos instead"
            );
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
            log::warn!(
                "CapsLock is not supported by global-hotkey; using Ctrl+Alt combos instead"
            );
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
```

- [ ] **Step 6: Run tests to verify they pass (GREEN)**

Run: `cargo test -p volumectl hotkeys_global`
Expected: 11 tests PASS. Then `cargo test -p volumectl --no-default-features` — full suite green.

- [ ] **Step 7: Update records + commit**

- `feature_list.json`: update the vol-029 entry — add to `verification` (list the unit-test command) and a `notes` line (`global-hotkey` 0.8.0 emits combo-level Pressed/Released; CapsLock falls back to Ctrl+Alt). Bump `last_updated`.
- `claude-progress.md`: append a "Task 2: global-hotkey dependency + hotkey core (11 unit tests)" section to Session 039 (edit tool).

```bash
git add Cargo.toml crates/volumectl/Cargo.toml crates/volumectl/src/lib.rs \
        crates/volumectl/src/hotkeys_global.rs feature_list.json claude-progress.md
git commit -m "feat: add global-hotkey hotkey core with unit tests"
```

---

### Task 3: Wire the backend and remove rdev

**Files:**
- Modify: `Cargo.toml` (remove rdev from `[workspace.dependencies]`)
- Modify: `crates/volumectl/Cargo.toml` (remove rdev line)
- Modify: `crates/volumectl/src/lib.rs` (remove `pub mod hotkeys_rdev;`)
- Modify: `crates/volumectl/src/app.rs` (lines 36, 100, 419 comment, 422)
- Modify: `crates/volumectl/src/linux_host_core.rs` (lines 357-368 impl)
- Modify: `crates/volumectl/src/linux_app.rs` (line 20, 298)
- Modify: `crates/volumectl/src/macos_app.rs` (lines 21, 34, 293)
- Modify: `crates/volumectl/src/main.rs` (line 60)
- Modify: `crates/volumectl/src/hotkeys/mod.rs` (line 60 comment)
- Modify: `crates/volumectl/src/wheel_win32.rs` (lines 3-4 comment)
- Delete: `crates/volumectl/src/hotkeys_rdev.rs`
- Modify: `CLAUDE.md` (lines 11, 75, 112-115, 128, 135)
- Records: `feature_list.json`, `claude-progress.md`

**Interfaces:**
- Consumes: Task 2's `GlobalHotkeys` (`new`/`try_recv`/`set_modifier`/`listener_failure`/`status`) and `run_headless`.
- Produces: `crate::hotkeys_global` (was `hotkeys_rdev`); `impl HotkeySource for GlobalHotkeys` under `#[cfg(target_os = "linux")]`; no remaining `rdev`/`RdevHotkeys` references in code.

- [ ] **Step 1: Remove the dependency and delete the old backend**

- Workspace `Cargo.toml`: delete the rdev entry and its comment block.
- `crates/volumectl/Cargo.toml`: delete `rdev = { workspace = true }`.
- `crates/volumectl/src/lib.rs`: delete `pub mod hotkeys_rdev;`.
- Delete the file: `git rm crates/volumectl/src/hotkeys_rdev.rs`

- [ ] **Step 2: Rewire the hosts**

`crates/volumectl/src/app.rs`:
- Line 36: `use crate::hotkeys_rdev::RdevHotkeys;` → `use crate::hotkeys_global::GlobalHotkeys;`
- Line 100: `hotkeys: RdevHotkeys,` → `hotkeys: GlobalHotkeys,`
- Line 419 comment: `// Cross-platform rdev keyboard listener + the Windows-only wheel bridge.` → `// Cross-platform global-hotkey listener + the Windows-only wheel bridge.`
- Line 422: `let hotkeys = RdevHotkeys::new(config.modifier)?;` → `let hotkeys = GlobalHotkeys::new(config.modifier)?;`

`crates/volumectl/src/linux_host_core.rs` — replace the impl block (lines 357-368):

```rust
#[cfg(target_os = "linux")]
impl HotkeySource for crate::hotkeys_global::GlobalHotkeys {
    fn try_recv(&self) -> Option<HotkeyAction> {
        self.try_recv()
    }

    fn listener_failure(&self) -> Option<String> {
        self.listener_failure()
    }

    fn set_modifier(&self, modifier: HotkeyModifier) {
        self.set_modifier(modifier)
    }
}
```

Also update the trait doc comment (lines 17-18): "without starting an rdev listener" → "without starting a global hotkey listener".

`crates/volumectl/src/linux_app.rs`:
- Line 20: `use crate::hotkeys_rdev::RdevHotkeys;` → `use crate::hotkeys_global::GlobalHotkeys;`
- Line 298: `match RdevHotkeys::new(config.modifier) {` → `match GlobalHotkeys::new(config.modifier) {`

`crates/volumectl/src/macos_app.rs`:
- Line 21: `use crate::hotkeys_rdev::RdevHotkeys;` → `use crate::hotkeys_global::GlobalHotkeys;`
- Line 34: `hotkeys: RdevHotkeys,` → `hotkeys: GlobalHotkeys,`
- Line 293: `let hotkeys = RdevHotkeys::new(config.modifier).map_err(|error| error.to_string())?;` → `let hotkeys = GlobalHotkeys::new(config.modifier).map_err(|error| error.to_string())?;`

`crates/volumectl/src/main.rs` line 60: `volumectl_lib::hotkeys_rdev::run_headless()` → `volumectl_lib::hotkeys_global::run_headless()`

`crates/volumectl/src/hotkeys/mod.rs` line 60: `/// The action is handled by the global `rdev` listener.` → `/// The action is handled by the global hotkey listener.`

`crates/volumectl/src/wheel_win32.rs` lines 3-4: `//! Keyboard shortcuts are handled by the cross-platform `rdev` backend.` → `//! Keyboard shortcuts are handled by the cross-platform `global-hotkey` backend.`

- [ ] **Step 3: Update CLAUDE.md architecture notes**

- Line 11: replace both `rdev` tokens with `global-hotkey`.
- Line 75: `libraries required by rdev:` → `libraries required by the global hotkey and X11 backends:`.
- Line 112: `starts the headless `hotkeys_rdev` host` → `starts the headless `hotkeys_global` host`.
- Line 114: `Linux rdev global hotkeys currently require X11.` → `Linux global hotkeys currently require X11.`
- Line 128: `are in `hotkeys_rdev.rs`.` → `are in `hotkeys_global.rs`.`
- Line 135: `CoreAudio, rdev actions, mtime` → `CoreAudio, global-hotkey actions, mtime`

- [ ] **Step 4: Verify the build and full test suite**

Run (Windows host):
```bash
cargo build
cargo test --workspace --no-default-features
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo fmt --all --check
git diff --check
```
Expected: all green, zero warnings. Then confirm no rdev remnants:
```bash
rg -rn "rdev|RdevHotkeys" --glob '!target/**' --glob '!Cargo.lock' --glob '!docs/**' --glob '!feature_list.json' --glob '!claude-progress.md' --glob '!session-handoff.md'
```
Expected: no hits in `crates/`, `Cargo.toml`, `CLAUDE.md`, `scripts/`, `.github/` (historical mentions may remain only in old spec/plan docs, records, and `scripts/verify-vol011.ps1` — those are historical and intentionally left).

- [ ] **Step 5: Cross-target compile checks (Windows host)**

```powershell
powershell -ExecutionPolicy Bypass -File .agents/skills/windows-host/scripts/ensure-pkg-config-stub.ps1
$env:PKG_CONFIG = "$env:TEMP\rtk-stub-bin\pkg-config.cmd"; $env:PKG_CONFIG_ALLOW_CROSS = '1'
cargo check --target x86_64-unknown-linux-gnu -p volumectl --tests --no-default-features --features gtk-renderer
cargo check --target x86_64-apple-darwin -p volumectl --tests --no-default-features
```
Expected: both compile clean (Linux pulls in `hotkeys_global` + `x11rb`; macOS pulls in Carbon path + the `⌘+⌥` combo branch).

- [ ] **Step 6: Update records + commit**

- `feature_list.json`: vol-029 — add to `verification` (cross-target check commands + rg-clean result); `notes`: rdev fully removed; CI unchanged.
- `claude-progress.md`: append "Task 3: wire global-hotkey backend, remove rdev (all hosts, cross-target checks green)" to Session 039.

```bash
git add -A
git commit -m "refactor: migrate global hotkeys from rdev to global-hotkey"
```

Expected: pre-commit hook passes. (`git add -A` is safe here — verify `git status --short` first; if any unrelated file appears, stage explicitly instead.)

---

### Task 4: Documentation, records finalize, full verification, ship

**Files:**
- Modify: `docs/global-hotkeys.md` (rewrite)
- Modify: `feature_list.json` (vol-029 → passing + evidence, `last_updated`)
- Modify: `claude-progress.md` (Session 039 finalize)
- Modify: `session-handoff.md` (refresh counts per windows-host skill)

**Interfaces:**
- Consumes: the migrated backend from Tasks 2-3; the accepted behavior nuances.
- Produces: final docs, evidence-backed records, shipped branch.

- [ ] **Step 1: Rewrite `docs/global-hotkeys.md`**

Rewrite the document to describe the `global-hotkey` backend:
- Registration: Windows `RegisterHotKey` (hidden window, no low-level hook, no extra permission), macOS Carbon `RegisterEventHotKey` (**no Accessibility permission** — delete the whole Accessibility/permission section), Linux X11 via `x11rb` XGrabKey (X11 `DISPLAY` required; same Wayland limitation as before — no Wayland backend in global-hotkey 0.8).
- Combo layout: `MOD+↑/↓`, `MOD+Shift+↑/↓`, `MOD+M`, `MOD+Shift+M`, `MOD+R`, `MOD+V`; `MOD` = configured modifier; macOS `CtrlAlt` also registers `⌘+⌥` spellings; `CapsLock` config falls back to `Ctrl+Alt` with a warning.
- Hold-to-Repeat: unchanged (first press immediate, 50 ms worker, combo-level release ends it). Document the accepted nuance: releasing only a modifier early does not end the hold while the main key stays down (Windows release detection polls the hotkey's own key via `GetAsyncKeyState`).
- Conflict reporting: per-combo `Err(AlreadyRegistered)` → Help surface shows `Conflicted`.

- [ ] **Step 2: Finalize records**

- `feature_list.json`: vol-029 → `status: "passing"`; `verification` = the exact commands actually run and passed (unit tests, full suite, clippy, cross-target checks, CI green after push); `evidence` = CI run URL(s) + any manual smoke notes; bump `last_updated`.
- `claude-progress.md`: append "Task 4: docs + verification + ship" with the verification evidence.
- `session-handoff.md`: update session number (039), feature count, unit-test count, and enforcement self-test counts per the current state.

- [ ] **Step 3: Full enforcement battery (Windows host, Git Bash)**

```bash
bash scripts/check-records.sh --branch
bash scripts/format-lint.sh          # full gate: fmt, whitespace, clippy, tests
bash scripts/test-check-records.sh
bash scripts/test-format-lint.sh
bash scripts/test-ship.sh
```
Expected: all pass, exit 0. Index must be clean before `test-format-lint.sh` (it needs an empty staged set — `git reset` first if anything is staged).

- [ ] **Step 4: Commit and push**

```bash
git add docs/global-hotkeys.md feature_list.json claude-progress.md session-handoff.md
git commit -m "docs: document global-hotkey backend and 1% step"
powershell -ExecutionPolicy Bypass -File scripts/ship.ps1 -Push
```
Expected: ship runs check-records (`--branch` + `--staged`), full format-lint gate, guard self-tests, then commits and pushes `refactor/hotkey-and-ci-fix` to origin.

- [ ] **Step 5: Verify CI + open PR**

- Watch GitHub Actions for the pushed branch: the `checks` (Ubuntu 24.04), `windows`, `macos`, and `ubuntu` (GTK) jobs must all be green (no workflow changes needed — verify the claim).
- Open a Pull Request `refactor/hotkey-and-ci-fix` → `master` describing the migration; after CI is green, merge per the user's plan.

- [ ] **Step 6: Manual smoke (post-merge, optional but recommended)**

- Windows: run the built binary; press `Ctrl+Alt+↑/↓` — volume changes exactly 1 % per press, holds repeat at 50 ms; `Ctrl+Alt+Shift+↑` = +10 %; `Ctrl+Alt+M/R/V` work; Task Manager shows ~0 % idle CPU.
- macOS (if available): verify hotkeys work with NO Accessibility permission granted (Carbon).
- Linux X11: `Ctrl+Alt+↑/↓` under an X session.
