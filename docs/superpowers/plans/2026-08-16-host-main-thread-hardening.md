# Host Main-Thread Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the three Tauri surface commands timeout deterministically,
eliminate the `toggle()` TOCTOU, dismiss the Windows tray-overflow flyout on
every surface open, and log when the main-thread marshalling wait times out.

**Architecture:** All changes live in `src-tauri` (commands + WindowManager).
Commands keep the async `run_on_main_thread` + channel pattern and gain a
10-second `tokio::time::timeout`; `toggle` gains a single main-thread
`toggle_impl`; `open_impl` dismisses the overflow flyout before any branch;
`on_main` logs on timeout.

**Tech Stack:** Rust 2021, Tauri 2, wry, tokio 1 (time feature), Windows-only
win32 helper (`dismiss_tray_overflow`).

## Global Constraints

- `tauri` features MUST keep `custom-protocol` (see
  `.agents/skills/tauri-deadlock-guard/SKILL.md`).
- Surface commands MUST stay `async`; `scripts/check-tauri-deadlock.sh` must
  keep passing.
- No public API changes: `SurfaceId`, `WindowManager` method signatures used
  by `events_sink.rs` and `commands.rs` keep their names.
- Timeout value MUST be 10 seconds (mirrors `on_main`'s `recv_timeout`).
- `cargo clippy --workspace --all-targets --no-default-features -- -D
  warnings` and `cargo fmt --all --check` must pass after every task.
- Conventional commits; records guard requires `feature_list.json` +
  `claude-progress.md` updates in the final task.

---

### Task 1: Add tokio time and timeout the open_surface command

**Files:**
- Modify: `src-tauri/Cargo.toml` (`[dependencies]`)
- Modify: `src-tauri/src/commands.rs` (`open_surface` only)
- Test: `bash scripts/check-tauri-deadlock.sh` + `cargo build`

**Interfaces:**
- Consumes: existing `tauri::async_runtime::channel(1)` + `run_on_main_thread`
  pattern in `open_surface`.
- Produces: `tokio::time::timeout(Duration::from_secs(10), rx.recv())` used by
  all three surface commands (Task 2 reuses it).

- [ ] **Step 1: Add the tokio dependency**

In `src-tauri/Cargo.toml`, inside `[dependencies]` (alphabetical order):

```toml
tokio = { version = "1", features = ["time"] }
```

- [ ] **Step 2: Add imports to commands.rs**

At the top of `src-tauri/src/commands.rs`, add:

```rust
use std::time::Duration;
```

- [ ] **Step 3: Wrap open_surface's receive with a timeout**

In `open_surface`, replace:

```rust
rx.recv()
    .await
    .ok_or_else(|| "window manager closed".to_string())?
```

with:

```rust
match tokio::time::timeout(Duration::from_secs(10), rx.recv()).await {
    Ok(Some(result)) => result,
    Ok(None) => Err("window manager closed".to_string()),
    Err(_) => {
        log::warn!("surface command timed out waiting for the main thread: {surface:?}");
        Err("window manager operation timed out".to_string())
    }
}
```

- [ ] **Step 4: Build and verify**

Run: `cargo build -p volumecontrol-tauri --no-default-features`
Expected: compiles; no warnings.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/commands.rs
git commit -m "fix: timeout open_surface main-thread wait at 10s"
```

### Task 2: Timeout close_surface and surface_ready

**Files:**
- Modify: `src-tauri/src/commands.rs` (`close_surface`, `surface_ready`)

**Interfaces:**
- Consumes: the timeout pattern from Task 1.
- Produces: identical behavior for the other two surface commands.

- [ ] **Step 1: Apply the same timeout to close_surface**

In `close_surface`, replace:

```rust
rx.recv()
    .await
    .ok_or_else(|| "window manager closed".to_string())?
```

with:

```rust
match tokio::time::timeout(Duration::from_secs(10), rx.recv()).await {
    Ok(Some(result)) => result,
    Ok(None) => Err("window manager closed".to_string()),
    Err(_) => {
        log::warn!("surface command timed out waiting for the main thread: {surface:?}");
        Err("window manager operation timed out".to_string())
    }
}
```

- [ ] **Step 2: Apply the same timeout to surface_ready**

In `surface_ready`, replace:

```rust
rx.recv()
    .await
    .ok_or_else(|| "window manager closed".to_string())?
```

with:

```rust
match tokio::time::timeout(Duration::from_secs(10), rx.recv()).await {
    Ok(Some(result)) => result,
    Ok(None) => Err("window manager closed".to_string()),
    Err(_) => {
        log::warn!("surface command timed out waiting for the main thread: {surface:?}");
        Err("window manager operation timed out".to_string())
    }
}
```

- [ ] **Step 3: Build and lint**

Run: `cargo build -p volumecontrol-tauri --no-default-features`
Expected: compiles with no warnings.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "fix: timeout close_surface and surface_ready waits at 10s"
```

### Task 3: Toggle without TOCTOU

**Files:**
- Modify: `src-tauri/src/window_manager.rs` (`toggle`)
- Test: `cargo test -p volumecontrol-tauri --no-default-features` (compile)

**Interfaces:**
- Consumes: existing `on_main`, `is_open`, `open_impl`, `close_impl`.
- Produces: `fn toggle_impl(&self, surface: SurfaceId) -> Result<(), String>`
  (private), with `pub fn toggle` delegating through `on_main`.

- [ ] **Step 1: Add toggle_impl and re-route toggle**

In `src-tauri/src/window_manager.rs`, replace the existing `toggle`:

```rust
pub fn toggle(&self, surface: SurfaceId) -> Result<(), String> {
    self.on_main(move |manager| manager.toggle_impl(surface))?
}

fn toggle_impl(&self, surface: SurfaceId) -> Result<(), String> {
    if self.is_open(surface) {
        self.close_impl(surface)
    } else {
        self.open_impl(surface)
    }
}
```

- [ ] **Step 2: Build and test**

Run: `cargo test -p volumecontrol-tauri --no-default-features`
Expected: compiles; workspace unit tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/window_manager.rs
git commit -m "fix: decide surface toggle on the main thread"
```

### Task 4: Dismiss tray overflow on every open

**Files:**
- Modify: `src-tauri/src/window_manager.rs` (`open_impl`)

**Interfaces:**
- Consumes: existing `dismiss_tray_overflow()` (Windows-only).
- Produces: same behavior; the call moves to the top of `open_impl`.

- [ ] **Step 1: Move the dismiss call to the top of open_impl**

In `open_impl`, delete the call currently inside the "create webview" section:

```rust
#[cfg(target_os = "windows")]
dismiss_tray_overflow();
```

and insert the exact same two lines as the FIRST statements of `open_impl`,
before the `if self.is_open(surface)` branch. Keep the explanatory comment
above it.

- [ ] **Step 2: Build and lint**

Run: `cargo clippy --workspace --all-targets --no-default-features -- -D warnings`
Expected: passes.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/window_manager.rs
git commit -m "fix: dismiss tray overflow before every surface open"
```

### Task 5: Log on_main timeout

**Files:**
- Modify: `src-tauri/src/window_manager.rs` (`on_main`)

**Interfaces:**
- Consumes: existing `recv_timeout(10s)` result.
- Produces: a `log::warn!` before the existing error is returned.

- [ ] **Step 1: Replace the timeout mapping with a logged match**

In `on_main`, replace:

```rust
rx.recv_timeout(std::time::Duration::from_secs(10))
    .map_err(|_| "window manager operation timed out".to_string())
```

with:

```rust
match rx.recv_timeout(std::time::Duration::from_secs(10)) {
    Ok(result) => Ok(result),
    Err(_) => {
        log::warn!(
            "window manager operation timed out after 10s on {:?}; queued closure may still run",
            std::thread::current().id()
        );
        Err("window manager operation timed out".to_string())
    }
}
```

- [ ] **Step 2: Build and lint**

Run: `cargo clippy --workspace --all-targets --no-default-features -- -D warnings`
Expected: passes.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/window_manager.rs
git commit -m "fix: log diagnostic when main-thread marshal times out"
```

### Task 6: Full verification, records, and merge prep

**Files:**
- Modify: `claude-progress.md`, `feature_list.json`, `session-handoff.md`

**Interfaces:**
- Consumes: all of Tasks 1–5.
- Produces: a green CI-ready branch.

- [ ] **Step 1: Run the full local gate**

Run:

```bash
cargo fmt --all --check
git diff --check
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo test --workspace --no-default-features
bash scripts/check-tauri-deadlock.sh
```

Expected: all pass.

- [ ] **Step 2: Manual Windows release verification**

Build release (`cargo build --release -p volumecontrol-tauri
--no-default-features`), then verify:
- Tray → Help → footer "Settings" opens Settings; footer "Close" closes
  Help; app stays responsive.
- Tray "Open mixer" toggles the mixer open and closed.
- The tray overflow flyout closes both when Help is first opened and when an
  already-open mixer is re-shown.

- [ ] **Step 3: Run the E2E help regression**

Run: `E2E_DRIVER_PROVIDER=embedded ./scripts/verify-tauri-e2e.ps1 -Surface help`
Expected: 3 passing including "opens the Settings surface from the footer
without crashing the host".

- [ ] **Step 4: Update records**

Append a Session entry to `claude-progress.md` and a feature entry to
`feature_list.json` describing the four hardening changes and the verification
results; update `session-handoff.md` with the same summary.

- [ ] **Step 5: Commit and push branch**

```bash
git add claude-progress.md feature_list.json session-handoff.md
git commit -m "docs: record host main-thread hardening"
git push -u origin chore/host-main-thread-hardening
```

Open a PR to `main`; CI (checks + Windows/macOS/Ubuntu + release gate) must
be green before merge.
