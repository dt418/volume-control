# Startup & Maintainability Cleanup Design

**Status:** Approved (Plan D of the review-findings decomposition)

**Goal:** Keep the Tauri main thread free of slow startup work and remove a
duplicated command-handler list that can drift silently.

**Background:** `get_bootstrap` is a synchronous command that runs on the
main thread and calls `AppCore::bootstrap()`, which enumerates WASAPI audio
sessions — blocking the event loop for tens of milliseconds every time a
surface opens. Separately, `pub fn builder()` in `lib.rs` duplicates the
19-command `generate_handler!` list that `run()` already builds, and nothing
calls `builder()`.

## Scope

`src-tauri/src/commands.rs` (one signature change) and
`src-tauri/src/lib.rs` (remove dead `builder()` + unused imports). No
frontend or interface changes.

## Requirements

### R1 — `get_bootstrap` runs off the main thread

- `commands.rs`: change `pub fn get_bootstrap(...)` to
  `pub async fn get_bootstrap(...)`.
- Keep the parameter `core: State<'_, Arc<Mutex<AppCore>>>` and the exact
  `Result<BootstrapPayload, String>` return type; keep the E2E bootstrap
  failure env check and the poison-recovery `unwrap_or_else`.
- Do NOT marshal via `run_on_main_thread`: the command only locks the core
  and reads state, so running on a tokio worker is safe and is the point of
  the change.

### R2 — Remove the duplicate handler list

- `lib.rs`: delete `pub fn builder()` (including its `install_menu_event_handler`
  + `invoke_handler` chain) and any imports that become unused.
- `run()` keeps its single `generate_handler![...]` — the authoritative list.
- `install_menu_event_handler` stays as-is and is still applied in `run()`.

## Testing

- `cargo build -p volumecontrol-tauri --no-default-features` compiles
  (removing `builder()` fails the build if anything still references it).
- `cargo clippy --workspace --all-targets --no-default-features --
  -D warnings` and `cargo fmt --all --check` pass.
- `bash scripts/check-tauri-deadlock.sh` still passes (surface commands
  remain async).
- E2E `verify-tauri-e2e.ps1 -Surface all` stays green — every surface loads
  through the async bootstrap path.

## Acceptance Criteria

- `get_bootstrap` no longer executes on the main thread.
- Exactly one `generate_handler!` list exists in `src-tauri`.
- No behavior change for the frontend payload or timing guarantees beyond
  the bootstrap moving off the event loop.

## Residual Risks

- Async command scheduling can reorder relative to other commands; the
  frontend already handles the bootstrap/event race via the
  `volumeEventRevision` guard in the mixer.
