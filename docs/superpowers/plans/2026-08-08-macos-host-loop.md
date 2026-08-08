# macOS Native Host Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the existing macOS CoreAudio backend, rdev global hotkeys, shared UI action/state contracts, and AppKit renderer into a main-thread-owned native host loop without changing Linux or Windows behavior.

**Architecture:** Add a macOS-only `macos_app` host module that owns audio, config, hotkeys, renderer, state, and an action channel. The process main thread initializes `NSApplication`, creates `MacosRenderer`, and runs a non-busy 150 ms manual AppKit event-polling loop that drains hotkey/renderer actions and publishes confirmed state. The loop exits on `Exit` or listener failure after main-thread renderer cleanup. Keep `OpenTrayMenu` explicitly non-fatal and deferred; preserve CLI routing for explicit commands.

**Tech Stack:** Rust 2021, Cargo workspace, `objc2` 0.6, `objc2-app-kit` 0.3, `objc2-foundation` 0.3, existing `rdev` listener, CoreAudio adapter, shared `NativeRenderer` contract, harness-free macOS smoke binary.

## Global Constraints

- The host module and all AppKit imports must be gated by `#[cfg(target_os = "macos")]`.
- `MacosRenderer::create`, `publish`, `dispatch`, and `destroy` must run on the process main thread.
- The polling cadence is 150 ms and must not busy-loop.
- `OpenTrayMenu` logs that macOS tray/menu is unavailable and does not terminate the host.
- Explicit CLI arguments continue to route to `cli::run()`; only macOS no-argument launch changes.
- Do not add a tray/menu dependency in this feature.
- Do not refactor the working Linux GTK host or Windows host.
- Every substantive source change updates both `feature_list.json` and `claude-progress.md` in the same change set.
- Do not claim passing status without fresh verification output and an exit code.
- `init.sh` and the reset `init.ps1` task remain untouched.

---

## File map

- **Create:** `crates/volumectl/src/macos_app.rs` — macOS host context, capability detection, action translation/application, config reload, manual AppKit event-loop ownership, and shutdown.
- **Modify:** `crates/volumectl/src/lib.rs` — expose `macos_app` only on macOS.
- **Modify:** `crates/volumectl/src/main.rs` — route macOS no-argument launches to `macos_app::run()` while preserving explicit CLI and Linux branches.
- **Modify:** `crates/volumectl/Cargo.toml` — retain the existing AppKit/Foundation dependencies; no new runtime dependency is needed because the host uses `NSDate`-based manual event polling rather than `NSDate deadline polling` block APIs.
- **Create:** `crates/volumectl/tests/macos_host_smoke.rs` — harness-free macOS smoke binary that exercises the host's test seams on the main thread and exits deterministically.
- **Modify:** `feature_list.json` — add/update the vol-011 host-wiring entry with verification commands and evidence.
- **Modify:** `claude-progress.md` — append the implementation session, scope, verification output, and environmental limitations.

---

### Task 1: Define testable macOS host seams and action semantics

**Files:**
- Create: `crates/volumectl/src/macos_app.rs`
- Modify: `crates/volumectl/src/lib.rs`
- Test: `crates/volumectl/src/macos_app.rs` unit tests under `#[cfg(test)]`

**Interfaces:**
- Consumes: `AudioBackend`, `Config`, `HotkeyAction`, `RdevHotkeys`, `AppAction`, `AppState`, `HostHandle`, `UiCapabilities`, `WorkArea`, `tokens_for`, and `MacosRenderer`.
- Produces: `pub fn run() -> Result<(), String>` under `#[cfg(target_os = "macos")]`; private pure helpers `hotkey_to_action`, `apply_action`, `refresh_from_audio`, `config_mtime`, and `reload_config_if_changed` used by later event-loop code and tests.

- [ ] **Step 1: Add the macOS module declaration**

In `crates/volumectl/src/lib.rs`, add the module beside the existing platform modules:

```rust
#[cfg(target_os = "macos")]
pub mod macos_app;
```

Do not make the module visible on Windows or Linux.

- [ ] **Step 2: Write pure action-translation tests before implementation**

Add tests in `macos_app.rs` for the exact configured-step mapping:

```rust
#[test]
fn hotkeys_use_configured_small_and_large_steps() {
    let cfg = Config::default();
    assert_eq!(hotkey_to_action(HotkeyAction::VolumeUp, &cfg), AppAction::AdjustVolume { delta_percent: 2 });
    assert_eq!(hotkey_to_action(HotkeyAction::VolumeDownLarge, &cfg), AppAction::AdjustVolume { delta_percent: -10 });
    assert_eq!(hotkey_to_action(HotkeyAction::ToggleMute, &cfg), AppAction::ToggleMute);
    assert_eq!(hotkey_to_action(HotkeyAction::Reset50, &cfg), AppAction::ResetVolume);
    assert_eq!(hotkey_to_action(HotkeyAction::OpenMixer, &cfg), AppAction::ToggleSurface(SurfaceId::Mixer));
    assert_eq!(hotkey_to_action(HotkeyAction::OpenMenu, &cfg), AppAction::OpenTrayMenu);
}
```

Also add a pure deferred-menu test using a small `HostState` or action-result helper that proves `OpenTrayMenu` leaves `quit_requested` false. Keep audio calls out of pure tests; use the existing platform adapter only in the host smoke binary.

- [ ] **Step 3: Run the new unit tests to establish the initial failure**

Run:

```bash
rtk cargo test -p volumectl macos_app
```

Expected before implementation: on Windows/Linux the macOS-gated module tests are not compiled, so the command may report zero matching tests; the macOS cross-target compile/test path must fail until the module and helpers exist. Record the exact target limitation if the local host cannot execute macOS tests.

- [ ] **Step 4: Implement the host context and pure helpers**

Define a macOS-gated context with these fields:

```rust
struct HostCtx {
    audio: Box<dyn AudioBackend>,
    hotkeys: RdevHotkeys,
    renderer: MacosRenderer,
    config: Config,
    last_config_mtime: Option<SystemTime>,
    state: AppState,
    caps: UiCapabilities,
    quit_requested: bool,
}
```

Implement:

- `config_mtime()` from `config::config_path()` metadata.
- `detect_caps()` using AppKit primary-screen visible frame/backing scale when available, with `WorkArea::new(0, 0, 1600, 900)` and `dpi_scale: 1.0` fallback. Keep optional accessibility queries non-fatal; default `high_contrast` and `reduced_motion` to false when unavailable.
- `hotkey_to_action()` matching the exact mapping in the spec.
- `apply_action()` with clamped volume writes, mute/reset operations, surface visibility changes, appearance updates, `Exit`, deferred `OpenTrayMenu`, deferred `OpenConfigLocation`, and explicit logs for unsupported settings/blacklist actions.
- `refresh_from_audio()` that reads the backend, updates volume/mute/status, copies config appearance into `AppState`, computes `tokens_for(config.appearance.theme, caps.high_contrast, config.appearance.accent, || None)`, and calls `renderer.publish()` once.
- `reload_config_if_changed()` that loads config on mtime change, replaces appearance and modifier, calls `hotkeys.set_modifier()` when needed, and preserves current state if the load path cannot produce a new config.

- [ ] **Step 5: Run formatting and platform-neutral tests**

Run:

```bash
rtk cargo fmt --all --check
rtk cargo test -p volumectl
```

Expected: formatting passes; existing Windows/Linux tests remain green because the module is cfg-gated. Do not mark this task complete until both commands have fresh exit-0 output.

- [ ] **Step 6: Commit the host seam**

```bash
rtk git add crates/volumectl/src/macos_app.rs crates/volumectl/src/lib.rs
rtk git commit -m "feat: define macOS host action boundary"
```

The commit is allowed only after the repository record guard is satisfied; if the guard requires the feature/progress records at this intermediate commit, stage those records with the same change set.

---

### Task 2: Add the AppKit main-thread event loop and host lifecycle

**Files:**
- Modify: `crates/volumectl/src/macos_app.rs`
- Modify: `crates/volumectl/src/main.rs`

**Interfaces:**
- Consumes: `HostCtx`, `refresh_from_audio`, `apply_action`, `reload_config_if_changed`, `MacosRenderer`, `NSApplication`, `NSDate`, `NSEventMask`, `NSDefaultRunLoopMode`, and `objc2::MainThreadMarker`.
- Produces: `macos_app::run() -> Result<(), String>` that owns all AppKit work on the main thread and returns only after clean exit or a listener/startup failure.

- [ ] **Step 1: Add the macOS entry-point routing test/compile guard**

Update the no-argument non-Windows branch in `main.rs` so macOS is selected before the generic headless fallback:

```rust
#[cfg(target_os = "macos")]
{
    return match volumectl_lib::macos_app::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("volumectl: macOS host unavailable ({e})");
            std::process::ExitCode::FAILURE
        }
    };
}
```

Leave Linux GTK and Linux non-GTK paths unchanged. Explicit arguments must still reach `cli::run()`.

- [ ] **Step 2: Implement main-thread startup**

In `run()`:

1. Require `MainThreadMarker::new()` and return `Err("macOS host must run on the process main thread".into())` if absent.
2. Call `renderer::ensure_application()` before `MacosRenderer::create()`.
3. Load config, create `audio::default_backend()`, create `RdevHotkeys`, detect capabilities, create the `HostHandle` channel, construct the renderer, initialize `AppState`, and publish the first state.
4. Keep the receiver and context on the main thread; the `HostHandle` closure captures only `Sender<AppAction>`.

- [ ] **Step 3: Implement the 150 ms manual AppKit event-polling loop**

Use `NSDate::dateWithTimeIntervalSinceNow(POLL_INTERVAL_SECONDS)` as the deadline for `NSApplication::nextEventMatchingMask_untilDate_inMode_dequeue`. This keeps the process responsive to AppKit events without busy-looping and avoids a new block/timer runtime dependency. All mutable host context remains on the process main thread; the rdev listener and renderer callback send only `AppAction` values through channels.

Each poll must:

```rust
let deadline = NSDate::dateWithTimeIntervalSinceNow(POLL_INTERVAL_SECONDS);
if let Some(event) = app.nextEventMatchingMask_untilDate_inMode_dequeue(
    NSEventMask::Any,
    Some(&deadline),
    unsafe { objc2_foundation::NSDefaultRunLoopMode },
    true,
) {
    app.sendEvent(&event);
    app.updateWindows();
}
listener_error = drain_host_actions(&mut ctx, &receiver);
refresh_from_audio(&mut ctx);
```

The loop condition is `while !ctx.quit_requested`; `drain_host_actions` sets that flag on `Exit` or listener failure. Do not send `MacosRenderer`, `NSPanel`, or other AppKit values to the rdev thread.

- [ ] **Step 4: Implement clean shutdown and error propagation**

After the event loop exits, call `ctx.renderer.destroy()` on the main thread before dropping the context. When `Exit` is received, return `Ok(())`; when the listener fails, preserve and return `Err(format!("global keyboard listener stopped: {error}"))` after cleanup.

- [ ] **Step 5: Verify cross-target compilation**

Run:

```bash
rtk cargo fmt --all --check
rtk cargo check -p volumectl --target x86_64-apple-darwin --all-targets
rtk cargo build
rtk cargo test -p volumectl
```

Expected: macOS cross-check finishes without warnings; Windows build/tests remain green. If the target cannot link on Windows, use `cargo check` for the macOS target and record the exact linker limitation rather than claiming a native runtime pass.

- [ ] **Step 6: Commit the lifecycle wiring**

```bash
rtk git add crates/volumectl/src/macos_app.rs crates/volumectl/src/main.rs
rtk git commit -m "feat: wire macOS AppKit host loop"
```

---

### Task 3: Add deterministic macOS host smoke coverage

**Files:**
- Create: `crates/volumectl/tests/macos_host_smoke.rs`
- Modify: `crates/volumectl/Cargo.toml`
- Modify: `crates/volumectl/src/macos_app.rs` only if a narrowly scoped `#[doc(hidden)]` smoke seam is required

**Interfaces:**
- Consumes: `macos_app` test seams, `MacosRenderer`, `HostHandle`, `AppAction`, `AppState`, and AppKit main-thread initialization.
- Produces: a harness-free `macos_host_smoke` binary whose `main()` runs on the macOS process main thread and prints `macos host smoke OK` on success; it is a no-op on non-macOS targets.

- [ ] **Step 1: Write the failing harness-free smoke binary**

Add this target to `crates/volumectl/Cargo.toml`:

```toml
[[test]]
name = "macos_host_smoke"
path = "tests/macos_host_smoke.rs"
harness = false
```

The smoke binary must be gated inside `main()`:

```rust
fn main() {
    #[cfg(target_os = "macos")]
    run_smoke();
}
```

The first version should assert the host seams: AppKit initialization succeeds on the main thread, a `HostHandle` sends a representative `AppAction` into a receiver, deferred `OpenTrayMenu` does not set the quit flag, and the renderer can be created, published once, and destroyed.

- [ ] **Step 2: Run the smoke target before implementation is complete**

Run:

```bash
rtk cargo test -p volumectl --test macos_host_smoke
```

Expected on macOS before the seam is complete: compilation or assertion failure. On Windows/Linux it should compile as a no-op when the macOS-gated imports are absent.

- [ ] **Step 3: Expose only the minimal test seam**

If private host fields cannot be exercised without broadening the public API, add a `#[cfg(any(test, feature = "..."))]` helper or a `#[doc(hidden)]` function that:

- creates a channel and `HostHandle`;
- enqueues `AppAction::ToggleSurface(SurfaceId::Mixer)`;
- proves the receiver gets the exact action;
- calls the deferred-menu action helper and checks `quit_requested == false`;
- initializes `MacosRenderer` on the main thread, publishes a known `AppState`, and calls `destroy()`.

Do not add a production feature solely to run the CI smoke unless Cargo's integration-test cfg rules require it; prefer a small `pub(crate)`/test-support module arrangement that does not expose mutable audio state.

- [ ] **Step 4: Run the smoke binary on the macOS runner**

Run:

```bash
rtk cargo test -p volumectl --test macos_host_smoke
```

Expected on the macOS CI runner: `macos host smoke OK`, exit 0, with no AppKit main-thread panic. On the Windows worktree, run the target to verify it compiles/no-ops and record that it cannot provide macOS runtime evidence locally.

- [ ] **Step 5: Commit smoke coverage**

```bash
rtk git add crates/volumectl/tests/macos_host_smoke.rs crates/volumectl/Cargo.toml crates/volumectl/src/macos_app.rs
rtk git commit -m "test: smoke macOS host lifecycle"
```

---

### Task 4: Update required records and run the full verification battery

**Files:**
- Modify: `feature_list.json`
- Modify: `claude-progress.md`

**Interfaces:**
- Consumes: fresh outputs from Tasks 1–3 and the repository's guard/ship scripts.
- Produces: honest vol-011 evidence that distinguishes code/CI evidence from unavailable real-desktop checks; records are staged in the same change set as all substantive source changes.

- [ ] **Step 1: Add the vol-011 host-wiring evidence entry/update**

Update the existing vol-011 feature rather than creating a duplicate. Add verification items for:

- `cargo fmt --all --check`.
- Windows `cargo build` and `cargo test`.
- macOS target `cargo check --target x86_64-apple-darwin --all-targets`.
- macOS CI harness-free host smoke.
- explicit CLI routing preservation.
- deferred `OpenTrayMenu` behavior.
- records guard and full battery.

Keep status `in_progress` unless the complete vol-011 acceptance list, including the remaining real-desktop and tray requirements, is genuinely satisfied. This macOS host slice alone must not mark the parent feature `passing`.

- [ ] **Step 2: Append a progress session entry**

Record:

- the macOS host-loop scope;
- files changed;
- `OpenTrayMenu` deferral;
- main-thread/manual event-polling architecture;
- exact fresh verification commands and pass/fail output;
- macOS runtime evidence source (CI runner if local Windows cannot run it);
- remaining Linux tray/Wayland runtime and Windows human checks.

- [ ] **Step 3: Run the repository verification battery**

Run each command separately and capture exit status:

```bash
rtk bash scripts/check-records.sh --branch origin/master
rtk bash scripts/format-lint.sh
rtk bash scripts/test-check-records.sh
rtk bash scripts/test-format-lint.sh
rtk bash scripts/test-ship.sh
rtk cargo build
rtk cargo test
```

Then stage all source and record changes and run:

```bash
rtk bash scripts/check-records.sh --staged
```

Do not use `--no-verify` or a skip flag. If a gate fails, stop, fix only the root cause, and rerun the failed command plus the full battery.

- [ ] **Step 4: Run the mandatory three-domain review**

Invoke the repository's `pre-push-review` skill against the changed enforcement/wiring/records paths. Any confirmed finding requires a minimal fix and fresh negative verification before rerunning the full battery. Record the review result in `feature_list.json` and `claude-progress.md`.

- [ ] **Step 5: Run the supported ship dry run**

Use the supported flow without pushing:

```bash
rtk bash scripts/ship.sh --dry-run
```

On Windows, if the PowerShell bridge is needed, also run the equivalent `scripts/ship.ps1 -DryRun`. Report the actual result; do not claim a clean ship if either path is unavailable or fails.

- [ ] **Step 6: Commit the complete feature slice**

After all checks pass and both records are staged:

```bash
rtk git add crates/volumectl/src/macos_app.rs crates/volumectl/src/lib.rs crates/volumectl/src/main.rs crates/volumectl/Cargo.toml crates/volumectl/tests/macos_host_smoke.rs feature_list.json claude-progress.md
rtk git commit -m "feat: add macOS native host loop"
```

Use the repository-required co-author trailer if the commit is created through the agent workflow. Do not push without the user's explicit request and the mandatory review evidence.

---

## Plan self-review

- **Spec coverage:** Startup, main-thread ownership, capability fallback, 150 ms polling, hotkey mapping, action semantics, config reload, deferred menu behavior, entry routing, smoke coverage, records, and verification are covered by Tasks 1–4.
- **No placeholders:** Every task specifies concrete files, signatures, commands, expected outcomes, and commit actions. No `TODO`, `TBD`, or vague “add appropriate handling” step remains.
- **Type consistency:** `macos_app::run()`, `HostCtx`, `hotkey_to_action`, `apply_action`, `refresh_from_audio`, and `reload_config_if_changed` are named consistently across tasks. `MacosRenderer` is imported from `crate::ui::platform::macos::renderer` as defined by the current module tree.
- **Scope:** Tray/menu, Linux refactor, Windows behavior, and visual redesign are explicitly excluded. The plan leaves vol-011 `in_progress` unless its broader acceptance criteria are independently proven.
