# Host Main-Thread Hardening Design

**Status:** Approved (Plan A of the review-findings decomposition; Plans B–D
are separate specs)

**Goal:** Make every Tauri surface-window operation fail loudly and
deterministically instead of hanging or silently no-op'ing, by hardening the
main-thread marshalling layer discovered during the Session 077 deadlock
investigation.

**Background:** Release builds previously deadlocked when surface commands
ran on the main thread without a live message pump. The fix (async commands +
`WindowManager::on_main`) landed, but the follow-up review found four
hardening gaps in the same layer: async commands await their marshalled
result without a timeout, `toggle()` decides open-vs-close off the main
thread (TOCTOU), the tray-overflow flyout is only dismissed when a webview is
created (not when an existing surface is re-shown), and `on_main` timeout has
no diagnostic log.

## Scope

This spec covers `src-tauri` only:

1. Async surface commands (`open_surface`, `close_surface`, `surface_ready`)
   timeout after 10 seconds instead of awaiting forever.
2. `WindowManager::toggle` decides open-vs-close inside a single main-thread
   marshalled operation.
3. `dismiss_tray_overflow()` runs before every surface open/re-show on
   Windows, not just webview creation.
4. `on_main` logs a warning when its 10-second wait times out.

Out of scope: frontend error feedback (Plan B), audio write-path cleanup
(Plan C), startup/maintainability cleanup (Plan D), poll/COM performance
(possible Plan E).

## Requirements

### R1 — Command timeout

- `src-tauri/Cargo.toml`: add `tokio = { version = "1", features = ["time"] }`
  to `[dependencies]`.
- `src-tauri/src/commands.rs`: in `open_surface`, `close_surface`, and
  `surface_ready`, replace the bare `rx.recv().await` with
  `tokio::time::timeout(Duration::from_secs(10), rx.recv()).await`.
  - On `Elapsed`: `log::warn!("surface command timed out waiting for the main
    thread: {surface:?}")` and return
    `Err("window manager operation timed out".to_string())`.
  - On `Ok(None)`: keep the existing `"window manager closed"` error.
  - On `Ok(Some(result))`: return `result` unchanged.
- The 10-second value mirrors `WindowManager::on_main`'s existing
  `recv_timeout`.

### R2 — Toggle without TOCTOU

- `src-tauri/src/window_manager.rs`: add
  `fn toggle_impl(&self, surface: SurfaceId) -> Result<(), String>` that reads
  `is_open` and calls `close_impl` or `open_impl` on the main thread.
- Change `pub fn toggle` to:

  ```rust
  pub fn toggle(&self, surface: SurfaceId) -> Result<(), String> {
      self.on_main(move |manager| manager.toggle_impl(surface))?
  }
  ```

- No public signature changes; `toggle_surface` in `events_sink.rs` is
  unchanged.

### R3 — Overflow dismissal on every open

- In `open_impl`, move the
  `#[cfg(target_os = "windows")] dismiss_tray_overflow();` call to the top of
  the function, before the `is_open` branch, so re-showing an existing
  bottom-right surface also dismisses the flyout.

### R4 — Timeout diagnostics

- In `on_main`, when `recv_timeout` returns `Err`:

  ```rust
  log::warn!(
      "window manager operation timed out after 10s on {:?}; queued closure may still run",
      std::thread::current().id()
  );
  ```

  then return the existing `"window manager operation timed out"` error.

## Testing

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
  --no-default-features -- -D warnings`, `cargo test --workspace
  --no-default-features` must pass.
- `bash scripts/check-tauri-deadlock.sh` must still pass (it asserts the
  surface commands remain `async` and marshalled).
- Manual Windows release verification:
  - Tray → Help → footer "Settings" opens Settings; footer "Close" closes
    Help; app stays responsive.
  - Tray "Open mixer" toggles the mixer open and closed (toggle path).
  - Overflow flyout is dismissed both when opening Help first time and when
    re-showing an already-open mixer.
- E2E: `E2E_DRIVER_PROVIDER=embedded ./scripts/verify-tauri-e2e.ps1 -Surface
  help` passes (includes the Help-footer Settings regression).

## Acceptance Criteria

- All four requirements implemented with no public API changes.
- No surface command can block the webview caller indefinitely: every
  marshalled result arrives within 10s or returns a string error.
- `toggle()` decisions happen on the main thread.
- Timeouts produce a diagnostic warning in `RUST_LOG=debug`/`warn` output.
- CI (Windows/macOS/Ubuntu + release gate) is green on the implementing PR.

## Residual Risks

- A timed-out closure still runs later on the main thread; the warning makes
  this observable, and the follow-up operation is idempotent (open/close
  re-check state).
- `toggle` races are reduced but not eliminated at the audio layer (two
  toggles queued back-to-back still net to the second toggle's intent).
