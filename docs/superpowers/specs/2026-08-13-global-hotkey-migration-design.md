# Global Hotkey Migration: rdev → global-hotkey 0.8.0

**Date:** 2026-08-13
**Status:** Approved (design review)
**Branch:** `refactor/hotkey-and-ci-fix`

## 1. Problem statement

`volumectl` listens for global keyboard shortcuts with `rdev` on every
platform (`crates/volumectl/src/hotkeys_rdev.rs`, ~500 lines). `rdev` has two
costs:

1. **Permission surface.** On macOS, `rdev::listen` uses a CGEventTap, which
   requires the Accessibility permission and a manual
   Settings → Privacy & Security grant. On Windows it installs a low-level
   keyboard hook (`WH_KEYBOARD_LL`). Neither is needed for this app's actual
   feature: a small fixed set of global hotkey combos.
2. **Blunt input model.** `rdev` observes every key event globally and the app
   re-derives combos from raw key state. The app cannot get OS-level conflict
   reporting for a combo.

`global-hotkey` 0.8.0 (the crate behind `tauri-plugin-global-shortcut`)
registers the same combos through native APIs — Windows `RegisterHotKey`
(hidden window, no hook), macOS Carbon `RegisterEventHotKey` (no Accessibility
permission), Linux X11 via pure-Rust `x11rb` XGrabKey — and reports per-combo
conflicts (`Err(AlreadyRegistered)`).

## 2. Goals

- Replace `rdev` with `global-hotkey` 0.8.0 on all platforms.
- **Preserve every existing feature and behavior:**
  - 8 actions: VolumeUp/Down, VolumeUpLarge/DownLarge (Shift variants),
    ToggleMute (M), Reset50 (R), OpenMixer (V), OpenMenu (Shift+M).
  - Hold-to-Repeat: first press fires immediately, then repeats every 50 ms
    until released (implemented by our own worker, not the OS).
  - Config modifier `CtrlAlt` / `Alt` / `Ctrl` with live reload on config
    change (`set_modifier`).
  - macOS dual-modifier acceptance: config `CtrlAlt` works as both `⌃+⌥`
    and `⌘+⌥` (register both combos).
  - Help surface status model (`HotkeyRegStatus`).
  - Windows mouse-wheel bridge (`wheel_win32.rs`) untouched.
- Step size default: `volume_step` 2 → **1** (the user-visible "1% per press"
  goal). `volume_step_large` stays **10**.
- Drop macOS Accessibility guidance (no longer required).

## 3. Non-goals

- No Tauri framework rewrite; the native UI stack (D2D / GTK4 / AppKit
  renderers, tray, winit) is untouched. `global-hotkey` is used as a plain
  Rust crate.
- No CI workflow changes: Ubuntu jobs already install the needed C libraries
  (`libpulse-dev libx11-dev libxi-dev libxtst-dev libgtk-4-dev
  libadwaita-1-dev xvfb`); `x11rb` is pure Rust, `windows-sys` needs nothing,
  Carbon is a system framework. CI is green today.
- No settings UI / OSD work (YAGNI).

## 4. Design

### 4.1 Dependency changes

- Workspace `Cargo.toml`: remove `rdev = "0.5.3"`; add
  `global-hotkey = "0.8"` (plain crate, no extra features).
- `crates/volumectl/Cargo.toml`: `rdev = { workspace = true }` →
  `global-hotkey = { workspace = true }`.
- `Cargo.lock`: regenerated (rdev + its tree removed; global-hotkey +
  `keyboard-types`, `keycode`, `x11rb-record` etc. added).

### 4.2 New hotkey backend: `hotkeys_global.rs`

Replaces `hotkeys_rdev.rs` (file removed). Public surface mirrors the current
host contracts:

- `GlobalHotkeys::new(initial_modifier: HotkeyModifier) -> Result<Self, String>`
  — builds the `GlobalHotKeyManager` and registers all combos for the
  modifier. On Windows the hidden window is created on the calling (main)
  thread, so the existing `GetMessage`/`DispatchMessage` host loop pumps its
  `WM_HOTKEY` messages; nothing else to wire.
- `try_recv() -> Option<HotkeyAction>` — drains
  `GlobalHotKeyEvent::receiver().try_recv()`.
- `set_modifier(modifier)` — unregister all, re-register all with the new
  modifier (live config reload).
- `listener_failure() -> Option<String>` — kept for host parity; with the new
  backend registration failures are immediate, so this reports the first
  registration error instead of an async thread error.
- `status() -> Vec<HotkeyRegResult>` — real per-action status:
  `Registered` on success, `Conflicted(HotkeyRegError)` when
  `register` returns `Err(AlreadyRegistered)` (first time the Help surface can
  show a true conflict).

**Combo construction** (pure, unit-tested `fn combos_for(modifier) -> Vec<(HotKey, HotkeyAction)>`):

| Action | Modifier combos |
|---|---|
| VolumeUp | `MOD+ArrowUp` |
| VolumeDown | `MOD+ArrowDown` |
| VolumeUpLarge | `MOD+Shift+ArrowUp` |
| VolumeDownLarge | `MOD+Shift+ArrowDown` |
| ToggleMute | `MOD+M` |
| Reset50 | `MOD+R` |
| OpenMixer | `MOD+V` |
| OpenMenu | `MOD+Shift+M` |

where `MOD = { CtrlAlt → CONTROL\|ALT, Alt → ALT, Ctrl → CONTROL }`. On macOS,
`CtrlAlt` additionally registers the `SUPER\|ALT` (⌘+⌥) spelling of every
combo, so the documented "either ⌘ or ⌃" behavior survives. `CapsLock` is
**unsupported by the crate** — see 4.4.

**Event handling** (replaces `process_event`):

- `Pressed` → map id → action; send once; if volume action, start the hold.
- `Released` → stop the hold.
- Repeated `Pressed` while a hold is active → ignored (same "no double-fire"
  guarantee as today; macOS Carbon auto-repeat is absorbed here).
- `id` → action lookup from a `HashMap<u32, HotkeyAction>` built at
  registration time from `HotKey::id()` (deterministic:
  `(mods.bits() << 16) | key as u32`).

**Hold-to-Repeat worker:** the existing condvar-based worker
(`run_repeat_worker`, 50 ms, `REPEAT_INTERVAL`) is kept as-is; only the
state machine that drives it changes (combo-level instead of raw key flags).

**Behavior nuance (accepted, documented):** release detection is combo-level
today, not per-key. Releasing one modifier early while still holding the
arrow key does not end the hold until the arrow key is released (Windows:
the crate polls `GetAsyncKeyState` on the non-modifier key). Previously any
combo key release ended the hold. This is more natural for "hold the arrow to
keep stepping" and is documented in `docs/global-hotkeys.md`.

### 4.3 Host wiring

- `src/app.rs` (Windows host): `RdevHotkeys::new(...)` →
  `GlobalHotkeys::new(...)`; the existing `drain_hotkeys` timer
  (`ID_TIMER_HOTKEY`) is unchanged.
- `src/linux_host_core.rs`: `impl HotkeySource for GlobalHotkeys` (Linux)
  replaces the `RdevHotkeys` impl.
- `src/linux_app.rs`, `src/macos_app.rs`: same rename.
- `src/main.rs`: `run_headless()` (non-Windows) uses `GlobalHotkeys`;
  banner text updated ("global-hotkey"); macOS Accessibility guidance and
  `print_permission_guidance` removed.
- `src/lib.rs`: `pub mod hotkeys_rdev` → `pub mod hotkeys_global` (with a
  compatibility `pub use` if any external test references the old name —
  audit first).
- `src/hotkeys/mod.rs`: unchanged (action/status model reused). Comments that
  say "rdev listener" are updated.

### 4.4 CapsLock modifier

`global-hotkey` supports only `SHIFT | CONTROL | ALT | SUPER` modifiers
(Windows `RegisterHotKey`, Carbon, X11 all lack CapsLock-as-modifier).

**Decision:** keep the `HotkeyModifier::CapsLock` serde variant so persisted
configs deserialize without error (100 % backward compatibility). At
registration time, `CapsLock` is unsupported → log a warning and fall back to
`CtrlAlt` combos (recommended blacklist for CtrlAlt is empty, same as
CapsLock, so no blacklist migration needed). `recommended_blacklist` already
treats CapsLock and CtrlAlt identically.

### 4.5 Step size

- `src/config.rs`: `volume_step` default 2 → **1**; update doc comments.
- `volume_step_large` stays 10 (ratio 1:10 keeps Shift distinct).
- Validation bounds already allow 1 (`MIN_VOLUME_STEP = 1`); `normalize`
  already clamps. Config tests updated for the new default.

### 4.6 CI

No workflow changes. The Ubuntu `checks`/`ubuntu` jobs and `release.yml`
already install everything the new dependency tree needs. `rdev` removal does
not require dropping any installed package (keep the lists as-is; they are
shared with other X11 work such as foreground detection).

### 4.7 Documentation

- `docs/global-hotkeys.md`: rewrite the backend section (global-hotkey,
  no Accessibility permission on macOS, X11-only on Linux — same as rdev),
  document the combo-level release nuance, hold-to-repeat (unchanged), and
  the CapsLock fallback.
- Any user-facing strings mentioning rdev / Accessibility updated.

### 4.8 Testing strategy (TDD)

Red first: swap the dependency and delete `hotkeys_rdev.rs` → the build
breaks (expected RED). Then implement against tests:

Unit tests (pure, no OS):
- `combos_for(modifier)` — combos per modifier; Shift variants present; macOS
  dual ⌘/⌃ duplicates; CapsLock → CtrlAlt fallback.
- id → action round-trip through `HotKey::id()`.
- Action-level state machine: first Pressed sends once + starts hold; repeat
  Pressed ignored while holding; Released stops the hold; M/R/V one-shot.
- Repeat worker cadence (existing test pattern, re-pointed at the new state
  machine).
- Config: `volume_step` default 1; `volume_step_large` 10; CapsLock
  deserializes from persisted JSON without error.

Integration / cross-target (Windows host, per windows-host skill):
- `cargo check --target x86_64-unknown-linux-gnu -p volumectl --tests
  --no-default-features --features gtk-renderer`
- `cargo check --target x86_64-apple-darwin -p volumectl --tests
  --no-default-features`
  (after `ensure-pkg-config-stub.ps1` + `PKG_CONFIG` env).
- Native Windows: `cargo build` + manual smoke (press combos, confirm 1 %
  step, confirm CPU idle).

Existing `tests/linux_host_core.rs` uses `FakeHotkeys` against the
`HotkeySource` trait — unaffected. `tests/gtk_smoke.rs` /
`linux_host_smoke.rs` / `macos_host_smoke.rs` compile paths verified.

## 5. Verification (DoD)

1. `cargo fmt --all --check`, `git diff --check`
2. `cargo clippy --workspace --all-targets --no-default-features -- -D warnings`
3. `cargo test --workspace --no-default-features` (Windows host)
4. Cross-target checks (§4.8)
5. `bash scripts/format-lint.sh`, `bash scripts/check-records.sh --branch`,
   self-tests (`test-check-records.sh`, `test-format-lint.sh`, `test-ship.sh`)
6. Records updated in the same commit: `feature_list.json` (update vol-003 +
   new feature entry) and `claude-progress.md`
7. Push via `ship.sh`/`ship.ps1`; GitHub Actions green on all three OS jobs
8. Manual smoke: each combo changes volume by exactly 1 % (10 % with Shift);
   idle CPU ~0 %; macOS needs no Accessibility grant

## 6. Risks / rollback

- **Release nuance change** (§4.2): early modifier release no longer ends a
  hold while the arrow stays down. Accepted; documented.
- **CapsLock users** silently move to CtrlAlt (warning logged). Config still
  parses.
- **X11-only Linux** (no Wayland) — identical to today's rdev default; no
  regression.
- Rollback: revert the commit(s); rdev stays available in the workspace
  history (`Cargo.lock` regenerated by `cargo`).
