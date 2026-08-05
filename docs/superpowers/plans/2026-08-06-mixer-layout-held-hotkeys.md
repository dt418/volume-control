# Mixer Layout and Held Hotkeys Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the Windows mixer reset-button clipping and allow held volume shortcuts to repeat volume changes.

**Architecture:** Keep the existing native Win32 mixer and hotkey architecture. Make the button geometry change in the pure `MixerLayout`, and make repeat behavior an explicit hotkey-ID policy shared by `RegisterHotKey` and the CapsLock hook.

**Tech Stack:** Rust, `windows-sys`, native Win32 controls, Cargo unit tests.

## Global Constraints

- Preserve the existing 400x224 Signal Glass mixer and its value/focus-ring geometry.
- Preserve one-shot behavior for mute, reset, mixer, and menu shortcuts.
- Preserve configured `Ctrl`, `Alt`, `Ctrl+Alt`, and existing CapsLock routing.
- Do not add a new dependency or a separate middle-mouse action.
- Use conventional commits and keep changes limited to the mixer/hotkey implementation and tests.

---

### Task 1: Make mixer buttons fit without overlap

**Files:**
- Modify: `crates/volumectl/src/mixer.rs` in `MixerLayout::new`.
- Test: `crates/volumectl/src/mixer.rs` layout tests.

**Interfaces:**
- Consumes: existing `RectF`, `MixerLayout`, `WIN_W`, and `WIN_H`.
- Produces: a reset button rect from x=164 to the existing content right edge.

- [ ] **Step 1: Write the failing test**

Add `buttons_fit_inside_card_without_overlap` to the existing `mixer.rs`
tests. Construct `MixerLayout::new(WIN_W as f32, WIN_H as f32)` and assert:

```rust
assert!(layout.mute_rect.right <= WIN_W as f32 - 16.0);
assert!(layout.reset_rect.right <= WIN_W as f32 - 16.0);
assert!(layout.mute_rect.right <= layout.reset_rect.left);
assert!(layout.reset_rect.width() >= 200.0);
assert!(layout.value_rect.bottom + 8.0 <= layout.slider_rect.top);
```

- [ ] **Step 2: Run the focused test to verify it fails**

Run: `cargo test -q buttons_fit_inside_card_without_overlap`

Expected: FAIL because the current reset rectangle starts at x=228 and is
only 156 px wide.

- [ ] **Step 3: Write the minimal implementation**

Change the layout rectangles in `MixerLayout::new`:

```rust
value_rect: RectF::new(16.0, 60.0, content_right, 88.0),
reset_rect: RectF::new(164.0, buttons_top, content_right, buttons_bottom),
```

- [ ] **Step 4: Run the focused test to verify it passes**

Run: `cargo test -q buttons_fit_inside_card_without_overlap`

Expected: PASS.

- [ ] **Step 5: Commit the isolated layout change**

Run:

```text
git add crates/volumectl/src/mixer.rs
git commit -m "fix: keep mixer reset button text inside card"
```

### Task 2: Restore held volume hotkey repeat

**Files:**
- Modify: `crates/volumectl/src/hotkeys_win32.rs` in the hotkey registration and low-level keyboard hook.
- Test: `crates/volumectl/src/hotkeys_win32.rs` pure policy tests.

**Interfaces:**
- Consumes: existing `COMBOS`, action IDs, `hotkey_action`, and `LAST_COMBO_KEY`.
- Produces: `is_repeatable_hotkey_id(id: i32) -> bool`, used by both repeat paths.

- [ ] **Step 1: Write the failing tests**

Add tests asserting the four volume IDs are repeatable and the four command
IDs are not:

```rust
assert!(is_repeatable_hotkey_id(ID_VOL_UP));
assert!(is_repeatable_hotkey_id(ID_VOL_DOWN));
assert!(is_repeatable_hotkey_id(ID_VOL_UP_LARGE));
assert!(is_repeatable_hotkey_id(ID_VOL_DOWN_LARGE));
assert!(!is_repeatable_hotkey_id(ID_MUTE));
assert!(!is_repeatable_hotkey_id(ID_RESET));
assert!(!is_repeatable_hotkey_id(ID_MIXER));
assert!(!is_repeatable_hotkey_id(ID_SHOW_MENU));
```

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test -q is_repeatable_hotkey_id`

Expected: FAIL because the policy function does not exist.

- [ ] **Step 3: Implement the repeat policy and registration behavior**

Add the pure policy function returning true only for `ID_VOL_UP`,
`ID_VOL_DOWN`, `ID_VOL_UP_LARGE`, and `ID_VOL_DOWN_LARGE`. In `reg`, build
the modifier flags as `mods` plus `MOD_NOREPEAT` only when the policy returns
false.

- [ ] **Step 4: Implement CapsLock volume repeat**

In `key_proc`, compute the existing `repeat` value as before. Post the action
when the host exists and either the key is not a repeat or
`is_repeatable_hotkey_id(id)` is true. Keep swallowing the matched key.

- [ ] **Step 5: Run the focused tests to verify they pass**

Run: `cargo test -q is_repeatable_hotkey_id`

Expected: PASS.

- [ ] **Step 6: Commit the hotkey change**

Run:

```text
git add crates/volumectl/src/hotkeys_win32.rs
git commit -m "feat: repeat held volume hotkeys"
```

### Task 3: Full verification

**Files:**
- Verify: `crates/volumectl/src/mixer.rs`, `crates/volumectl/src/hotkeys_win32.rs`.

- [ ] **Step 1: Format and inspect the diff**

Run: `cargo fmt --all --check` and `git diff --check`.

- [ ] **Step 2: Run the full test suite**

Run: `cargo test -q`.

Expected: zero failures.

- [ ] **Step 3: Build the Windows release binary**

Run: `cargo build --release`.

Expected: exit code 0 and `target/release/volumectl.exe` exists and is non-empty.
