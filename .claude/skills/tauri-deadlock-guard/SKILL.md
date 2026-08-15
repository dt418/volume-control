---
name: tauri-deadlock-guard
description: Prevent Tauri webview deadlocks and unresponsive surfaces in volume-control. Use when touching src-tauri commands, window creation, surface open/close/ready flows, tauri.conf.json, or the tauri dependency features, and before adding any new WebViewWindowBuilder usage.
---

# Tauri Deadlock Guard

VolumeControl hit two production-class bugs that made every webview surface
appear dead:

1. Release builds resolved `WebviewUrl::App` to `http://localhost:1420`
   (`ERR_CONNECTION_REFUSED`) because the `tauri` dependency was missing the
   `custom-protocol` feature — `tauri/build.rs` keeps `cfg(dev)` active
   whenever that feature is absent, even in release profiles.
2. Calling `WebviewWindowBuilder::build()` from a synchronous Tauri command
   hung the main thread forever: wry needs a live event-loop message pump,
   and sync commands run inside an async task on the main thread.

This skill encodes the invariants that prevent both classes of bug.

## Invariants (do not violate)

1. `src-tauri/Cargo.toml` MUST keep `custom-protocol` in the `tauri` features
   list:
   `tauri = { version = "2", features = ["tray-icon", "image-png", "custom-protocol"] }`
   - Verify release webviews load the embedded assets (`Asset favicon.ico`
     debug logs, never a `localhost:1420` URL probe).
2. Window/webview creation and destruction MUST run on the Tauri main thread.
   - `WebviewWindowBuilder` may only be referenced in
     `src-tauri/src/window_manager.rs`.
   - Public surface operations (`open`, `close`, `surface_ready`) MUST marshal
     through `WindowManager::on_main`, which dispatches directly when already
     on the main thread and otherwise uses `run_on_main_thread` + a bounded
     channel with a timeout (see the existing 10s `recv_timeout`).
3. IPC surface commands MUST be `async`:
   - `pub async fn open_surface(...)`
   - `pub async fn close_surface(...)`
   - `pub async fn surface_ready(...)`
   Each marshals via `app.run_on_main_thread` and awaits an async channel;
   never block the main thread with `recv()` in a command.
4. Tray/poll-thread callers (menu events, hotkey/wheel threads) must go
   through the same `WindowManager` wrappers — never call window APIs
   directly from those threads.

## Workflow

1. Before changing surface code, run the guard script:
   - Windows PowerShell: `powershell -File scripts/check-tauri-deadlock.ps1`
   - POSIX/CI: `bash scripts/check-tauri-deadlock.sh`
2. After any change, run the full gate:
   `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
   --no-default-features -- -D warnings`, `cargo test --workspace
   --no-default-features`, frontend `npm test`.
3. Verify the runtime flow on Windows release:
   - Tray → Help → footer "Settings" opens Settings (window visible, not
     hidden); footer "Close" closes Help; app stays responsive.
   - No `window_manager operation timed out` in debug logs.
4. Run the E2E regression that covers this path:
   `E2E_DRIVER_PROVIDER=embedded ./scripts/verify-tauri-e2e.ps1 -Surface help`
   (includes "opens the Settings surface from the footer without crashing the
   host").

## References

- `src-tauri/src/window_manager.rs` — `on_main` marshalling pattern.
- `src-tauri/src/commands.rs` — async surface commands + channel pattern.
- `scripts/check-tauri-deadlock.sh` / `.ps1` — automated invariant checks.
- `claude-progress.md` Session 077 — full root-cause write-up.
