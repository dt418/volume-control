# Linux Native Host Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire one GTK-owned Linux host for X11 and Wayland-capable sessions, with X11 hotkeys, honest Wayland degradation, authoritative audio/state publication, correct geometry, clean shutdown, and truthful runtime evidence.

**Architecture:** Keep a pure, testable Linux host core responsible for action application, audio readback, configuration reload, capability status, and shutdown. Keep GTK/GLib default-context source installation and renderer construction as a thin main-thread integration layer. Use one host for both display backends; represent X11 hotkeys, Wayland hotkey unavailability, audio availability, and layer-shell availability as explicit capabilities rather than separate hosts.

**Tech Stack:** Rust 2021, Cargo workspace, GTK4, libadwaita, GLib default main context, optional gtk4-layer-shell, `AudioBackend`, `RdevHotkeys`, `LinuxRenderer`, Xvfb smoke, optional real/nested Wayland compositor.

## Global Constraints

- Keep `init.sh` unchanged and do not restore or reopen the abandoned `init.ps1` task.
- Keep Windows behavior and macOS AppKit behavior unchanged.
- Preserve the macOS physical-geometry-to-AppKit-point conversion and its backing-scale-2.0 regression test.
- Defer Linux tray/AppIndicator/StatusNotifier integration; `OpenTrayMenu` must be non-fatal and explicit.
- Do not enable `rdev` evdev grabbing or claim Wayland global-hotkey support.
- Explicit CLI arguments retain existing CLI behavior; no-argument native-host startup errors are reported honestly rather than hidden by a one-shot CLI fallback.
- Missing mandatory dependencies are build failures or reasoned skips, never passes; missing optional compositor/layer-shell environments are recorded as explicit skips.
- Every substantive change updates both `feature_list.json` and `claude-progress.md` in the same final change set.
- Follow TDD: each production behavior begins with a failing test or a harness regression that demonstrably fails before the implementation.

---

### Task 1: Establish Linux host-core seams and test fixtures

**Files:**
- Modify: `crates/volumectl/src/linux_app.rs`
- Test: `crates/volumectl/src/linux_app.rs` module tests or a new `crates/volumectl/tests/linux_host_core.rs` test target, following the existing crate test conventions.

**Interfaces:**
- Consumes: `AudioBackend`, `AppAction`, `AppState`, `Config`, `UiCapabilities`, `HostHandle`, `RdevHotkeys`.
- Produces: a Linux host context that stores `Option<Box<dyn AudioBackend>>`, optional hotkeys, renderer state, config/mtime, capabilities, shutdown, and explicit degraded status; test helpers can construct it with fake audio and without GTK.

- [ ] **Step 1: Write a failing test for fake-audio host construction.**

Add a fake `AudioBackend` in the test module and a test that constructs the host core without GTK or a real PulseAudio server:

```rust
#[test]
fn host_core_accepts_injected_audio_backend() {
    let audio = Box::new(FakeAudio::new(0.5, false));
    let host = HostCore::for_test(audio);
    assert_eq!(host.confirmed_state().volume_percent, 50);
    assert!(!host.quit_requested());
}
```

The test must fail because the current concrete `LinuxAudio` context has no injectable host-core constructor.

- [ ] **Step 2: Run the focused test and verify the expected failure.**

Run:

```bash
rtk cargo test -p volumectl --lib host_core_accepts_injected_audio_backend --no-default-features
```

Expected: FAIL because `HostCore`/`for_test` is not yet defined, not because of a test typo.

- [ ] **Step 3: Implement the minimal host-core seam.**

Extract the non-GTK-owned fields from the current `HostCtx` into a testable `HostCore` and store audio as `Option<Box<dyn AudioBackend>>`. Add only the constructor/accessors required by the test; keep `LinuxRenderer` and GTK source installation outside this core. Preserve the existing `AppState` and `UiCapabilities` types.

- [ ] **Step 4: Run the focused test and verify it passes.**

Run the same command. Expected: PASS, with the existing Linux-disabled/default-feature test suite still compiling.

- [ ] **Step 5: Commit the isolated seam.**

```bash
rtk git add crates/volumectl/src/linux_app.rs crates/volumectl/tests/linux_host_core.rs
rtk git commit -m "refactor: isolate Linux host core"
```

---

### Task 2: Add action reducer tests and authoritative audio behavior

**Files:**
- Modify: `crates/volumectl/src/linux_app.rs`
- Test: `crates/volumectl/tests/linux_host_core.rs` or the host-core test module.

**Interfaces:**
- Consumes: `HostCore` from Task 1 and `AppAction` variants.
- Produces: `apply_action`, audio mutation helpers, error-retaining state behavior, surface/appearance behavior, and explicit deferred-action handling.

- [ ] **Step 1: Write failing tests for audio actions and failure retention.**

Cover one behavior per test:

```rust
#[test]
fn set_volume_clamps_and_uses_backend() { /* 125 -> 100 */ }

#[test]
fn adjust_volume_reads_confirmed_value_before_mutating() { /* 50 + 10 -> 60 */ }

#[test]
fn failed_audio_mutation_keeps_previous_confirmed_state() { /* backend error */ }

#[test]
fn tray_action_is_deferred_without_shutdown() { /* OpenTrayMenu */ }

#[test]
fn exit_requests_shutdown() { /* Exit */ }
```

Use a fake backend that records calls and can return a controlled `AudioError`. Assert on confirmed state and shutdown, not only call counts.

- [ ] **Step 2: Run the focused tests and verify they fail for missing behavior.**

```bash
rtk cargo test -p volumectl --test linux_host_core --no-default-features
```

Expected: FAIL on missing reducer/seam or current behavior that silently drops/degrades the action.

- [ ] **Step 3: Implement the minimal reducer.**

Move current Linux action handling into `HostCore::apply_action`. Clamp values using existing core helpers. For each mutation, call the backend and retain the prior state on error; log the concrete error and mark audio unavailable/degraded. Keep surface visibility and appearance updates host-owned. Log explicit deferred messages for tray/config/blacklist actions.

- [ ] **Step 4: Run the tests and the existing library suite.**

```bash
rtk cargo test -p volumectl --test linux_host_core --no-default-features
rtk cargo test -p volumectl --lib --no-default-features
```

Expected: all focused tests and the existing suite pass.

- [ ] **Step 5: Commit the reducer behavior.**

```bash
rtk git add crates/volumectl/src/linux_app.rs crates/volumectl/tests/linux_host_core.rs
rtk git commit -m "feat: add Linux host action reducer"
```

---

### Task 3: Add hotkey/provider capability and configuration reload coverage

**Files:**
- Modify: `crates/volumectl/src/linux_app.rs`
- Modify if required: `crates/volumectl/src/hotkeys_rdev.rs` (public behavior unchanged)
- Test: `crates/volumectl/tests/linux_host_core.rs`

**Interfaces:**
- Consumes: `RdevHotkeys::{try_recv, listener_failure, set_modifier}`, `Config`, `config_path`, `load_existing`.
- Produces: X11 hotkey translation, explicit Wayland unavailable state, listener-failure reporting, safe config reload, modifier synchronization.

- [ ] **Step 1: Write failing tests for hotkey and reload behavior.**

Add tests that use queued actions or a narrow host-core input seam:

```rust
#[test]
fn open_mixer_hotkey_toggles_mixer_surface() { /* OpenMixer */ }

#[test]
fn wayland_hotkeys_are_reported_unavailable_without_evdev() { /* capability */ }

#[test]
fn listener_failure_marks_hotkeys_degraded_but_keeps_host_alive() { /* failure */ }

#[test]
fn malformed_config_reload_preserves_previous_config() { /* invalid file */ }
```

The malformed-config test must use a temporary config path or injected loader so it cannot modify the user’s real configuration.

- [ ] **Step 2: Run the focused tests and verify they fail.**

```bash
rtk cargo test -p volumectl --test linux_host_core hotkey --no-default-features
rtk cargo test -p volumectl --test linux_host_core config --no-default-features
```

Expected: FAIL because Linux currently does not check listener failure, reload configuration, or represent Wayland hotkey unavailability.

- [ ] **Step 3: Implement provider classification and reload.**

Detect the display backend from GTK/GDK environment/session information. Create `RdevHotkeys` only for X11. On Wayland store no listener and an explicit reason. Add host-core methods to drain hotkeys, map actions using the current config, inspect listener failure, reload only through `load_existing`, preserve the previous config on failure, and call `set_modifier` when an existing listener’s modifier changes.

- [ ] **Step 4: Run focused and existing tests.**

```bash
rtk cargo test -p volumectl --test linux_host_core --no-default-features
rtk cargo test -p volumectl --lib --no-default-features
```

Expected: PASS.

- [ ] **Step 5: Commit the hotkey/reload behavior.**

```bash
rtk git add crates/volumectl/src/linux_app.rs crates/volumectl/src/hotkeys_rdev.rs crates/volumectl/tests/linux_host_core.rs
rtk git commit -m "feat: report Linux host capabilities"
```

---

### Task 4: Correct Linux capability detection and geometry policy

**Files:**
- Modify: `crates/volumectl/src/linux_app.rs`
- Modify: `crates/volumectl/src/ui/platform/linux/renderer.rs`
- Test: `crates/volumectl/tests/linux_host_core.rs` and existing renderer tests.

**Interfaces:**
- Consumes: GDK monitor/display APIs, `UiCapabilities`, `WorkArea::{right,bottom}`, `plan_surfaces`.
- Produces: detected display scale/work area where available, deterministic fallback, correct layer-shell margins for arbitrary origins, and keyboard policy split.

- [ ] **Step 1: Write failing geometry tests.**

Add tests for a work area with a nonzero and negative origin asserting layer-shell margins are computed from absolute edges:

```rust
#[test]
fn layer_shell_margins_use_absolute_work_area_edges() {
    let work_area = WorkArea::new(-1920, -40, 1920, 1040);
    let rect = SurfaceRect::new(-420, 876, -84, 964);
    assert_eq!(right_margin(work_area, rect), 0);
    assert_eq!(bottom_margin(work_area, rect), 36);
}
```

Add scale tests that assert logical conversion occurs once at the Linux renderer boundary. Keep the existing macOS Retina 2x test untouched and include it in the verification command.

- [ ] **Step 2: Run the focused tests and verify the expected failure.**

```bash
rtk cargo test -p volumectl --lib layer_shell_margins --no-default-features
```

Expected: FAIL because current layer-shell code subtracts from width/height and does not expose the absolute-edge helper.

- [ ] **Step 3: Implement the smallest geometry and capability changes.**

Use `work_area.right() - rect.right` and `work_area.bottom() - rect.bottom`. Keep shared `WorkArea` and physical placement unchanged. Detect monitor geometry/scale through the active GDK display; use the existing safe fallback only when unavailable. Do not add accessibility dependencies. Make overlay noninteractive/nonexclusive and mixer interactive only when visible/on demand.

- [ ] **Step 4: Run renderer and shared geometry tests.**

```bash
rtk cargo test -p volumectl --lib surface --no-default-features
rtk cargo test -p volumectl --lib renderer --no-default-features
rtk cargo test -p volumectl --lib retina_appkit_frame_converts_physical_pixels_to_points_once --no-default-features
```

Expected: PASS, including the macOS 2x regression.

- [ ] **Step 5: Commit the capability/geometry changes.**

```bash
rtk git add crates/volumectl/src/linux_app.rs crates/volumectl/src/ui/platform/linux/renderer.rs
rtk git commit -m "fix: align Linux surface capabilities"
```

---

### Task 5: Wire GTK default-context polling and clean lifecycle

**Files:**
- Modify: `crates/volumectl/src/linux_app.rs`
- Modify: `crates/volumectl/src/main.rs`
- Test: `crates/volumectl/tests/gtk_smoke.rs` or a new harness-free `crates/volumectl/tests/linux_host_smoke.rs`.

**Interfaces:**
- Consumes: `HostCore`, `LinuxRenderer`, `HostHandle`, GTK default `MainContext`, `MainLoop`.
- Produces: no-argument Linux native startup, fast/slow GLib sources, renderer action channel delivery, state publication, explicit destruction, and honest fatal-startup behavior.

- [ ] **Step 1: Write a failing harness-free smoke assertion.**

Extend or create a main-thread GTK smoke binary that asserts:

```rust
assert!(renderer_action_reaches_host);
assert!(surface_visibility_changes);
assert!(exit_requests_shutdown);
assert!(renderer_destroyed_before_return);
```

The smoke must run only when `target_os = "linux"` and `gtk-renderer` is enabled; otherwise its `main` remains a no-op like existing platform smoke binaries. First run it against the current host to demonstrate the missing action/lifecycle behavior.

- [ ] **Step 2: Run the smoke and verify the expected failure.**

Under an available X11 test display:

```bash
xvfb-run -a cargo test -p volumectl --features gtk-renderer --test linux_host_smoke -- --nocapture
```

Expected: FAIL on at least one new host-routing or lifecycle assertion before implementation.

- [ ] **Step 3: Replace the custom GLib context with default-context source installation.**

Initialize GTK on the main thread, create the host channel and renderer, then install:

```rust
gtk::glib::timeout_add_local(Duration::from_millis(15), ...poll_fast...);
gtk::glib::timeout_add_local(Duration::from_millis(150), ...poll_slow...);
```

Capture the `MainLoop` weak/clone handles safely so `Exit` removes/quits both sources. Call `renderer.destroy()` exactly once on shutdown. Keep the core reducer outside the closures. Change no-argument Linux startup to report native-host startup errors honestly; retain explicit-argument CLI routing.

- [ ] **Step 4: Run the smoke and Linux feature tests.**

```bash
xvfb-run -a cargo test -p volumectl --features gtk-renderer --test linux_host_smoke -- --nocapture
rtk cargo test -p volumectl --features gtk-renderer --lib
```

Expected: PASS when GTK/Xvfb dependencies are installed. If mandatory GTK packages are unavailable, record the exact build skip/failure reason rather than calling it pass.

- [ ] **Step 5: Commit the GTK lifecycle wiring.**

```bash
rtk git add crates/volumectl/src/linux_app.rs crates/volumectl/src/main.rs crates/volumectl/tests/linux_host_smoke.rs
rtk git commit -m "feat: wire Linux GTK host loop"
```

---

### Task 6: Complete X11 and Wayland verification paths

**Files:**
- Modify: `crates/volumectl/tests/gtk_smoke.rs`
- Modify/create: `crates/volumectl/tests/linux_host_smoke.rs`
- Modify: `.github/workflows/ci.yml`
- Modify: `README.md`
- Modify: `README.vi.md`

**Interfaces:**
- Consumes: the host and renderer from Tasks 1–5.
- Produces: reproducible X11 smoke, conditional Wayland/layer-shell smoke, dependency installation policy, and truthful evidence language.

- [ ] **Step 1: Add negative/runtime classification checks before changing CI.**

Add shell/CI assertions that distinguish:

- Xvfb X11 renderer smoke from Wayland proof.
- missing `libgtk-4-layer-shell-dev` from a layer-shell pass.
- absent Wayland compositor from a runtime pass.
- absent PulseAudio/PipeWire server from an audio pass.

Run the checks against a deliberately unavailable optional environment and verify they print `SKIP` plus a reason, not `PASS`.

- [ ] **Step 2: Implement the X11 smoke path.**

Make the smoke verify GTK initialization, LinuxRenderer creation, initial publication, representative renderer-to-host action delivery, a surface state transition, `OpenTrayMenu` non-fatal behavior, and clean renderer destruction under Xvfb. Keep it explicit that Xvfb proves X11/GTK only.

- [ ] **Step 3: Add conditional real/nested Wayland smoke.**

When a compositor and layer-shell dependency exist, run the Wayland smoke for layer-shell mapping, absolute-origin margins, noninteractive overlay, interactive mixer, clean exit, and explicit Wayland hotkey degradation. When unavailable, emit and record the exact skip reason. Never use `unstable_grab`.

- [ ] **Step 4: Update CI package and evidence wording.**

Install the mandatory GTK4/libadwaita/PulseAudio/X11 packages by default in the supported Linux host job. Keep layer-shell/compositor installation conditional and report a reasoned skip when unavailable. Add separate job summaries for X11 pass, Wayland pass/skip, and audio runtime availability. Do not call a skipped optional environment green.

- [ ] **Step 5: Update user-facing dependency documentation.**

Document mandatory Linux native packages, optional layer-shell/compositor packages, X11 `DISPLAY` requirements, Wayland global-hotkey degradation, and the exact meaning of `PASS`, `FAIL`, and `SKIP` evidence. Do not alter `init.sh`.

- [ ] **Step 6: Run the available runtime paths.**

```bash
xvfb-run -a cargo test -p volumectl --features gtk-renderer --test linux_host_smoke -- --nocapture
cargo test -p volumectl --features gtk-renderer --test gtk_smoke -- --nocapture
```

Run the Wayland command only if a real/nested compositor is present; otherwise capture the exact skip. Confirm no Xvfb output is used as Wayland evidence.

- [ ] **Step 7: Commit the runtime verification wiring.**

```bash
rtk git add crates/volumectl/tests crates/volumectl/.github 2>/dev/null || true
rtk git add crates/volumectl/tests .github/workflows/ci.yml README.md README.vi.md
rtk git commit -m "test: verify Linux host runtimes honestly"
```

---

### Task 7: Update records and run the mandatory verification battery

**Files:**
- Modify: `feature_list.json`
- Modify: `claude-progress.md`
- Review: `scripts/check-records.sh`, `scripts/format-lint.sh`, `.agents/skills/pre-push-review/SKILL.md`, `.claude/skills/pre-push-review/SKILL.md`

**Interfaces:**
- Consumes: implementation and runtime evidence from Tasks 1–6.
- Produces: truthful vol-011 evidence, clean staged records, and review-ready repository state.

- [ ] **Step 1: Update the records with only observed evidence.**

Keep `vol-011` as `in_progress`. Add the Linux host slice verification commands and their actual results. Record separately:

- X11/GTK smoke result.
- Audio runtime result or exact unavailable-server reason.
- Wayland/layer-shell result or exact skip reason.
- macOS Retina and existing macOS host evidence.
- Remaining unverified desktop checks.

Do not mark the feature `passing` unless all acceptance evidence is actually available.

- [ ] **Step 2: Run formatting and diff checks.**

```bash
rtk cargo fmt --all --check
rtk git diff --check HEAD
```

Expected: PASS.

- [ ] **Step 3: Run the full repository verification battery.**

```bash
bash scripts/format-lint.sh
bash scripts/test-check-records.sh
bash scripts/test-format-lint.sh
bash scripts/test-ship.sh
rtk cargo test --workspace
rtk cargo clippy --workspace --all-targets --all-features -- -D warnings
```

On Windows/macOS/Linux commands that cannot run in the current environment, capture the actual skip/failure and do not replace it with a pass claim.

- [ ] **Step 4: Run branch and staged records guards.**

```bash
sh scripts/check-records.sh --branch origin/master
rtk git add feature_list.json claude-progress.md crates/volumectl/src crates/volumectl/tests .github README.md README.vi.md docs/superpowers/specs/2026-08-08-linux-host-loop-design.md docs/superpowers/plans/2026-08-08-linux-host-loop.md
sh scripts/check-records.sh --staged
```

Expected: both records checks pass.

- [ ] **Step 5: Run the mandatory three-domain pre-push review.**

Dispatch one reviewer each for guard core, gate chain, and wiring/records consistency. Fix only verified genuine findings, add a live negative verification for each fix, rerun the relevant tests, and update the records with the review evidence.

- [ ] **Step 6: Perform final status review and commit only after all required checks pass.**

```bash
rtk git status --short
rtk git diff --cached --check
rtk git diff --cached --stat
```

Commit through the repository’s supported ship flow only after the verification battery and review are green. Do not push without explicit user authorization.
