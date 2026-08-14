# Windows-First Quality, Auto-Start, and INI Configuration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a fail-closed Windows release test pyramid, reliable Windows auto-start, and safe JSON-to-INI configuration migration with direct Settings editing, then verify the shared contracts on Linux and macOS.

**Architecture:** Keep Rust/AppCore as the source of truth. Add diagnostic-only host probes for real process/hotkey evidence, a platform adapter for auto-start, and an INI codec/store behind the existing `config::load`, `load_existing`, `save_validated`, and mtime-reload APIs. React Settings continues to own typed drafts and invokes typed Tauri commands; it never edits raw files as its primary workflow.

**Tech Stack:** Rust 2021, `windows-sys` Registry APIs, existing `global-hotkey` 0.8, Tauri v2, React 19/TypeScript, Vitest/Testing Library, PowerShell/Win32 `PrintWindow`, existing Cargo/frontend/records gates.

## Global Constraints

- Windows is the first runtime target; Linux/macOS work begins only after Windows gates pass.
- Release is fail-closed: fmt, diff-check, clippy `-D warnings`, workspace Rust tests, frontend tests/build, Tauri release build, surface verifier, hotkey probe, auto-start test, INI migration tests, and records guard must all exit 0.
- Never use `git commit --no-verify`, warning suppression, or a skipped release gate to make the release pass.
- Preserve Mixer `400x224`, Settings `760x620` (minimum `620x520`), and Help `520x500` geometry and the current surface readiness/error contract.
- Normal launches must not write diagnostic probe files; probe behavior is enabled only by explicit environment variables.
- All temporary config directories, registry values, child processes, and evidence artifacts are cleaned or restored in `finally` blocks.
- Existing JSON is never deleted automatically; the first successful migration leaves a validated JSON backup.
- Settings Save/Reset/Cancel remains draft-based and atomic; direct file editing is an advanced option, not a required workflow.
- Every substantive code/script change stages `feature_list.json` and `claude-progress.md` in the same change set.

---

## File map and ownership

| Area | Files | Responsibility |
|---|---|---|
| Probe and release evidence | `crates/volumectl/src/hotkey_probe.rs`, `src-tauri/src/lib.rs`, `scripts/verify-tauri-surfaces.ps1`, `scripts/verify-hotkeys.ps1` | Diagnostic timestamps, real-process hotkey smoke, latency summaries |
| Rust quality tests | `crates/volumectl/src/hotkeys_global.rs`, `crates/volumectl/src/host_core.rs`, `crates/volumectl/tests/host_core.rs` | Mapping, repeat, routing, p95 probe fixtures |
| Auto-start | `crates/volumectl/src/autostart.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/lib.rs` | Cross-platform contract and Windows HKCU Run adapter |
| Settings auto-start UI | `frontend/src/settings/AutoStartControl.tsx`, `frontend/src/settings/GeneralSection.tsx`, `frontend/src/settings/SettingsSurface.tsx`, related tests | Typed toggle, retry/error state, accessibility |
| INI codec/store | `crates/volumectl/src/config_ini.rs`, `crates/volumectl/src/config.rs`, `crates/volumectl/Cargo.toml` | Human-readable INI, migration, atomic persistence, recovery |
| Config IPC/UI | `src-tauri/src/commands.rs`, `frontend/src/settings/StorageSection.tsx`, `frontend/src/settings/SettingsSurface.test.tsx` | INI path, migration status, direct editing workflow |
| Release wiring | `scripts/ship.ps1`, `scripts/ship.sh`, `scripts/verify-release.ps1`, `feature_list.json`, `claude-progress.md` | Fail-closed orchestration and exact evidence records |

---

### Task 1: Lock the Windows release test contract

**Files:**
- Create: `scripts/verify-release.ps1`
- Create: `scripts/tests/verify-release.tests.ps1` only if the existing PowerShell self-test convention requires a separate test file
- Modify: `scripts/verify-tauri-surfaces.ps1`
- Modify: `feature_list.json`
- Modify: `claude-progress.md`

**Interfaces:**
- `verify-release.ps1 -Binary target/release/VolumeControl.exe -OutputRoot output/tauri-surface-evidence/release -TimeoutSeconds 15` orchestrates surface, hotkey, auto-start, and process-soak checks.
- The script returns nonzero on any missing artifact, process crash, geometry mismatch, blank capture, latency budget violation, or cleanup failure.
- Evidence is written to `output/tauri-surface-evidence/release/<UTC timestamp>/` with JSON, PNG, and log files.

- [ ] **Step 1: Write failing PowerShell contract checks**

Assert that the release verifier rejects a missing binary, creates a unique run directory, forwards the binary path to each child verifier, and exits nonzero when a child verifier returns nonzero.

- [ ] **Step 2: Run the contract checks and observe RED**

Run: `pwsh -NoProfile -File scripts/tests/verify-release.tests.ps1`

Expected: failure because `verify-release.ps1` does not exist.

- [ ] **Step 3: Implement the fail-closed orchestrator**

Use `try/finally`; invoke each verifier with `&`; collect exit codes and artifact paths; write `release-summary.json`; never use fixed sleeps for readiness; terminate only the PID returned by the child verifier.

- [ ] **Step 4: Run the contract checks GREEN**

Run: `pwsh -NoProfile -File scripts/tests/verify-release.tests.ps1`

Expected: all contract checks pass, including deliberate missing-binary and child-failure fixtures.

- [ ] **Step 5: Commit the release-contract slice**

Run: `git add scripts/verify-release.ps1 scripts/tests feature_list.json claude-progress.md && git commit -m "test: add fail-closed Windows release verifier"`

---

### Task 2: Add diagnostic hotkey timestamps without changing normal behavior

**Files:**
- Create: `crates/volumectl/src/hotkey_probe.rs`
- Modify: `crates/volumectl/src/lib.rs`
- Modify: `crates/volumectl/src/host_core.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `crates/volumectl/src/hotkey_probe.rs` and `crates/volumectl/tests/host_core.rs`

**Interfaces:**
- `HotkeyProbe::from_env() -> Option<HotkeyProbe>` is enabled only when `VOLUMECTL_HOTKEY_PROBE` is set.
- `HotkeyProbe::record(stage: ProbeStage, action: Option<HotkeyAction>)` appends JSONL with a monotonic nanosecond timestamp.
- `ProbeStage` is exactly `Registered`, `EventReceived`, `AppCoreHandled`, and `StatePublished`.
- `HotkeyProbe::summary(path: &Path) -> Result<HotkeyLatencySummary, String>` returns count, median, p95, and repeat intervals.

- [ ] **Step 1: Write failing probe unit tests**

Test that disabled probes perform no I/O, enabled probes emit valid JSONL, stage ordering is preserved, and a fixture with timestamps `[0, 10ms, 20ms, 60ms]` produces the expected median/p95 and repeat interval.

- [ ] **Step 2: Run focused tests RED**

Run: `cargo test -p volumectl hotkey_probe --no-default-features`

Expected: compile failure because the module and types are missing.

- [ ] **Step 3: Implement the probe and wire stages**

Instantiate the probe during `AppCore::new` only from the environment; record registration after `GlobalHotkeys::new`, record event receipt in `poll_hotkeys`, record handling after `apply_hotkey`, and record publication at the EventSink state boundary. Keep all writes best-effort and log a warning only when probe mode is explicitly enabled.

- [ ] **Step 4: Add hotkey probe integration assertions**

Use the injected audio backend in `host_core` tests to verify each of the eight actions reaches the shared pipeline and that command actions remain one-shot under repeated OS events.

- [ ] **Step 5: Run focused GREEN checks**

Run: `cargo test -p volumectl hotkey_probe host_core hotkeys_global --no-default-features`

Expected: probe, mapping, routing, duplicate suppression, and 50 ms repeat tests pass.

- [ ] **Step 6: Commit the probe slice**

Run: `git add crates/volumectl/src/hotkey_probe.rs crates/volumectl/src/lib.rs crates/volumectl/src/host_core.rs src-tauri/src/lib.rs crates/volumectl/tests/host_core.rs feature_list.json claude-progress.md && git commit -m "test: add diagnostic hotkey latency probe"`

---

### Task 3: Exercise all Windows hotkeys and process stability

**Files:**
- Create: `scripts/verify-hotkeys.ps1`
- Modify: `scripts/win32_pinvoke.cs` only when a missing `SendInput`/window-query declaration is required
- Modify: `scripts/verify-tauri-surfaces.ps1`
- Test: `scripts/tests/verify-hotkeys.tests.ps1`

**Interfaces:**
- `verify-hotkeys.ps1 -Binary target/release/VolumeControl.exe -OutputRoot output/tauri-surface-evidence/release-hotkeys -TimeoutSeconds 15` launches one isolated process and sends the eight action matrix.
- The script writes `registration.json`, `latency.json`, `process-soak.json`, `process.log`, and `failure.txt` on failure.
- The hard budget is 8/8 registration and p95 <= 75 ms; synthetic Shift limitations are recorded explicitly rather than converted to a pass.

- [ ] **Step 1: Write fixture tests for missing process, malformed probe output, and budget failure**

Use fixture JSONL files to verify nonzero exits for missing actions, p95 `76ms`, and an early process exit; verify success for a valid 8-action/p95 `40ms` fixture.

- [ ] **Step 2: Run fixture tests RED**

Run: `pwsh -NoProfile -File scripts/tests/verify-hotkeys.tests.ps1`

Expected: failure because the verifier and budget parser are absent.

- [ ] **Step 3: Implement condition-based runtime waits**

Launch with `VOLUMECTL_CONFIG_DIR`, `VOLUMECTL_HOTKEY_PROBE`, and `VOLUMECTL_HOTKEY_PROBE_OUTPUT`; wait for the process and registered status instead of sleeping; send Ctrl+Alt combinations with Win32 `SendInput`; capture the current volume/state after each action; hold a volume key to measure repeat cadence; observe process health for 30 seconds.

- [ ] **Step 4: Implement cleanup and evidence validation**

Capture the child PID before launch, kill only that PID in `finally`, verify the probe file is non-empty and parseable, and keep all artifacts when any assertion fails.

- [ ] **Step 5: Run fixture tests GREEN**

Run: `pwsh -NoProfile -File scripts/tests/verify-hotkeys.tests.ps1`

Expected: all deliberate failure fixtures fail and the valid fixture passes.

- [ ] **Step 6: Run the real release hotkey smoke**

Run: `pwsh -NoProfile -File scripts/verify-hotkeys.ps1 -Binary target/release/VolumeControl.exe -OutputRoot output/tauri-surface-evidence/release-hotkeys`

Expected: 8/8 registration in a clean desktop, p95 <= 75 ms, repeat intervals near 50 ms, and no crash during soak. If Shift injection is unavailable, the result contains an explicit physical-keyboard follow-up and release remains blocked for the missing action.

- [ ] **Step 7: Commit the hotkey smoke slice**

Run: `git add scripts/verify-hotkeys.ps1 scripts/tests scripts/verify-tauri-surfaces.ps1 feature_list.json claude-progress.md && git commit -m "test: verify Windows hotkey latency and stability"`

---

### Task 4: Expand the React UI/UX contract suite

**Files:**
- Modify: `frontend/src/mixer/MixerSurface.test.tsx`
- Modify: `frontend/src/settings/SettingsSurface.test.tsx`
- Modify: `frontend/src/help/HelpSurface.test.tsx`
- Modify: `frontend/src/styles.test.ts`
- Create: `frontend/src/accessibility.test.tsx` when shared axe-style assertions cannot fit existing surface tests

**Interfaces:**
- Every surface test asserts `data-surface`, header/content/footer siblings, one scroll region, non-empty error states, and accessible names for all primary controls.
- Mixer tests cover system volume, mute, reset, search, stale-session removal, empty state, Esc, blur close, and duplicate session keys.
- Settings tests cover all six sections, every draft field, validation errors, Save/Reset/Cancel, retryable bootstrap error, and auto-start control once Task 6 lands.
- Help tests cover all cards, search, status badges, conflict callout, Settings CTA, footer actions, Escape, and error state.

- [ ] **Step 1: Add failing cases for missing interactions**

Add tests for keyboard activation of every button, `aria-live` status changes, focus order, disabled Save while clean, error retry, and no footer inside a scroll container.

- [ ] **Step 2: Run the focused frontend suite RED**

Run: `npm test --prefix frontend -- --run src/mixer/MixerSurface.test.tsx src/settings/SettingsSurface.test.tsx src/help/HelpSurface.test.tsx`

Expected: new assertions fail only for the missing contracts.

- [ ] **Step 3: Implement the smallest UI fixes**

Preserve the approved shells; add stable labels, focus-visible states, retry buttons, and compact-layout spacing. Do not move application state into components or bypass the Rust source of truth.

- [ ] **Step 4: Run frontend tests and build GREEN**

Run: `npm test --prefix frontend` and `npm run build --prefix frontend`

Expected: all tests pass with no React key warnings, no unhandled promise rejection, and a clean TypeScript/Vite build.

- [ ] **Step 5: Commit the UI contract slice**

Run: `git add frontend feature_list.json claude-progress.md && git commit -m "test: cover complete Tauri surface UI and UX contracts"`

---

### Task 5: Implement the Windows auto-start adapter

**Files:**
- Create: `crates/volumectl/src/autostart.rs`
- Modify: `crates/volumectl/src/lib.rs`
- Modify: `crates/volumectl/Cargo.toml` only if a Windows feature is needed; use existing `windows-sys` workspace dependency first
- Test: `crates/volumectl/src/autostart.rs`

**Interfaces:**
- `pub struct AutostartStatus { pub enabled: bool, pub command: Option<String> }` derives `Serialize`.
- `pub fn status() -> Result<AutostartStatus, String>` reads the current platform state.
- `pub fn set_enabled(enabled: bool) -> Result<AutostartStatus, String>` writes or removes the current-user entry and returns read-back state.
- `pub fn quote_command_path(path: &Path) -> String` is pure and tested on spaces, quotes, and trailing separators.

- [ ] **Step 1: Write pure tests RED**

Test command quoting, missing value → disabled, matching executable → enabled, and unrelated Run values are untouched.

- [ ] **Step 2: Run focused tests RED**

Run: `cargo test -p volumectl autostart --no-default-features`

Expected: compile failure because the module and interfaces are absent.

- [ ] **Step 3: Implement the platform-neutral contract**

On Windows, use `RegOpenKeyExW`, `RegQueryValueExW`, `RegSetValueExW`, and `RegDeleteValueW` against `HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Run`; return an explicit unsupported error on Linux/macOS for this wave. Never panic on access or malformed registry data.

- [ ] **Step 4: Run focused tests GREEN**

Run: `cargo test -p volumectl autostart --no-default-features`

Expected: pure helper and platform tests pass.

- [ ] **Step 5: Commit the adapter slice**

Run: `git add crates/volumectl/src/autostart.rs crates/volumectl/src/lib.rs crates/volumectl/Cargo.toml feature_list.json claude-progress.md && git commit -m "feat: add Windows current-user auto-start adapter"`

---

### Task 6: Expose auto-start through Tauri and Settings

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `frontend/src/settings/AutoStartControl.tsx`
- Modify: `frontend/src/settings/GeneralSection.tsx`
- Modify: `frontend/src/settings/GeneralSection.test.tsx` or `SettingsSurface.test.tsx`

**Interfaces:**
- Tauri command `get_autostart() -> Result<AutostartStatus, String>`.
- Tauri command `set_autostart(enabled: bool) -> Result<AutostartStatus, String>`.
- `AutoStartControl` accepts `{ enabled: boolean; disabled?: boolean; onChange(enabled): Promise<void> }` and exposes an accessible switch plus inline status/error.

- [ ] **Step 1: Add failing command/UI tests**

Mock `get_autostart` and `set_autostart`; assert initial state comes from the command, toggling calls the command once, successful read-back updates the switch, and rejected writes leave the previous state with an alert and retry button.

- [ ] **Step 2: Run focused tests RED**

Run: `npm test --prefix frontend -- --run src/settings/SettingsSurface.test.tsx`

Expected: failures for the missing control and commands.

- [ ] **Step 3: Register the commands and render the control**

Add both commands to `builder()` and `run()` invoke handlers. Load auto-start state after bootstrap; keep it separate from the config draft because the registry is an immediate side effect. Put the control in General and preserve Save/Reset behavior for config fields.

- [ ] **Step 4: Run Rust/frontend checks GREEN**

Run: `cargo test -p volumecontrol-tauri --no-default-features` and `npm test --prefix frontend -- --run src/settings/SettingsSurface.test.tsx`.

Expected: command registration and Settings control tests pass.

- [ ] **Step 5: Add Windows registry integration with restoration**

Run: `pwsh -NoProfile -File scripts/verify-autostart.ps1 -Binary target/release/VolumeControl.exe -OutputRoot output/tauri-surface-evidence/autostart`

The script records original value, enables, reads back, disables, verifies removal, and restores the original value in `finally`; it fails if the app opens a webview surface during startup.

- [ ] **Step 6: Commit the Settings slice**

Run: `git add src-tauri frontend/src/settings scripts/verify-autostart.ps1 feature_list.json claude-progress.md && git commit -m "feat: expose Windows auto-start in Settings"`

---

### Task 7: Add the INI codec and atomic persistence

**Files:**
- Create: `crates/volumectl/src/config_ini.rs`
- Modify: `crates/volumectl/src/lib.rs`
- Modify: `crates/volumectl/Cargo.toml`
- Modify: `Cargo.toml` only for a pinned INI parser dependency
- Test: `crates/volumectl/src/config_ini.rs`

**Interfaces:**
- `pub fn parse_ini(text: &str) -> Result<Config, ConfigError>`.
- `pub fn serialize_ini(config: &Config) -> Result<String, ConfigError>`.
- `pub fn load_ini(path: &Path) -> Result<Config, ConfigError>`.
- `pub fn save_ini_atomic(config: &Config, path: &Path) -> Result<(), ConfigError>`.
- `pub fn migrate_json_to_ini(json_path: &Path, ini_path: &Path) -> Result<MigrationOutcome, ConfigError>`.
- `MigrationOutcome` is exactly `Migrated`, `AlreadyPresent`, or `NoLegacyFile`.

- [ ] **Step 1: Select and pin the parser dependency**

Resolve the current official documentation for the selected crate through Context7, prefer a small parser with deterministic key/section access, and record the version in `Cargo.toml`/`Cargo.lock`. Do not introduce a parser that silently coerces invalid numbers or booleans.

- [ ] **Step 2: Write failing codec tests**

Cover the full schema, defaults for omitted optional appearance/feedback fields, repeated `blacklist.item.N` keys sorted numerically, unknown keys ignored with a warning, invalid integer/bool/enum values rejected, and serialization that parses back to an equal typed config.

- [ ] **Step 3: Run codec tests RED**

Run: `cargo test -p volumectl config_ini --no-default-features`

Expected: compile failure because the codec is absent.

- [ ] **Step 4: Implement parse/serialize with explicit field mapping**

Map `[general]`, `[appearance]`, `[feedback]`, `[color_thresholds]`, and `[blacklist]` to existing `Config` fields. Use the current validation functions after parsing; never let parser defaults bypass validation.

- [ ] **Step 5: Implement atomic writes**

Write to a sibling temporary file with a unique suffix, flush and `sync_all`, rename over the destination, and remove the temporary file on failure. Preserve existing destination contents when the destination is a directory or a write fails.

- [ ] **Step 6: Implement JSON migration**

When INI is absent and JSON exists, parse/validate JSON, write INI atomically, and leave JSON untouched as backup. Return `Migrated`; do not overwrite a present INI.

- [ ] **Step 7: Run codec tests GREEN**

Run: `cargo test -p volumectl config_ini config --no-default-features`

Expected: round-trip, malformed input, migration, atomic failure, and existing JSON config tests pass.

- [ ] **Step 8: Commit the codec slice**

Run: `git add Cargo.toml Cargo.lock crates/volumectl/Cargo.toml crates/volumectl/src/config_ini.rs crates/volumectl/src/lib.rs feature_list.json claude-progress.md && git commit -m "feat: add typed INI config codec and migration"`

---

### Task 8: Switch AppCore and config reload to canonical INI

**Files:**
- Modify: `crates/volumectl/src/config.rs`
- Modify: `crates/volumectl/src/host_core.rs`
- Modify: `crates/volumectl/tests/host_core.rs`
- Modify: `crates/volumectl/src/config_ini.rs`

**Interfaces:**
- `config::config_path()` returns `$env:VOLUMECTL_CONFIG_DIR/config.ini` when the override is set, otherwise the platform config directory with the `config.ini` filename.
- `config::load_existing()` returns a validated INI config, migrating JSON when needed and falling back to the valid backup/defaults with a warning state.
- `AppCore::reload_config_if_changed()` watches the INI mtime and preserves the previous valid config on malformed external edits.

- [ ] **Step 1: Add failing path/migration/reload tests**

Use a locked temporary `VOLUMECTL_CONFIG_DIR` to assert the path ends in `config.ini`, first load migrates JSON, malformed INI preserves the previous config, valid INI edits reload once, and a second poll with unchanged mtime is stable.

- [ ] **Step 2: Run tests RED**

Run: `cargo test -p volumectl config host_core --no-default-features`

Expected: failures for the JSON path assumption and missing INI reload behavior.

- [ ] **Step 3: Route existing public config APIs through INI**

Keep callers on `load`, `load_existing`, `save_validated`, and `config_path`; replace only their storage implementation. Preserve exact validation/error strings used by Settings and existing tests.

- [ ] **Step 4: Add recovery warning state**

Expose a non-panicking load warning through a typed `ConfigLoadNotice` consumed by bootstrap; keep `BootstrapPayload.config` compatible and add an optional notice field so older frontends can ignore it.

- [ ] **Step 5: Run Rust tests GREEN**

Run: `cargo test --workspace --no-default-features`.

Expected: all existing tests plus migration/reload/recovery tests pass.

- [ ] **Step 6: Commit AppCore integration**

Run: `git add crates/volumectl/src/config.rs crates/volumectl/src/host_core.rs crates/volumectl/src/config_ini.rs crates/volumectl/tests/host_core.rs feature_list.json claude-progress.md && git commit -m "feat: make INI the canonical AppCore config store"`

---

### Task 9: Make Settings the primary INI editing workflow

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `frontend/src/settings/StorageSection.tsx`
- Modify: `frontend/src/settings/SettingsSurface.tsx`
- Modify: `frontend/src/settings/SettingsSurface.test.tsx`
- Modify: `frontend/src/settings/StorageSection.test.tsx`

**Interfaces:**
- `config_path` returns the INI path and an optional migration/recovery notice is rendered as a visible `role="status"` or `role="alert"`.
- `update_settings` continues to send one typed patch; no frontend code writes INI text.
- `open_config_location` remains available as an advanced action and is labeled accordingly.

- [ ] **Step 1: Add failing Settings migration/recovery tests**

Assert the Storage section displays `config.ini`, shows a migration warning when bootstrap includes one, keeps Save enabled only for draft changes, and labels raw-file opening as advanced.

- [ ] **Step 2: Run focused tests RED**

Run: `npm test --prefix frontend -- --run src/settings/StorageSection.test.tsx src/settings/SettingsSurface.test.tsx`

Expected: failures for the INI path and notice copy.

- [ ] **Step 3: Implement the Settings notice and labels**

Render the notice with readable contrast and retry-safe wording; preserve the six-section shell and all existing direct fields. Do not make the user open a file to change supported settings.

- [ ] **Step 4: Run frontend tests/build GREEN**

Run: `npm test --prefix frontend` and `npm run build --prefix frontend`.

Expected: all frontend tests pass and the multi-entry build is clean.

- [ ] **Step 5: Commit the Settings/config UX slice**

Run: `git add src-tauri/src/commands.rs frontend/src/settings feature_list.json claude-progress.md && git commit -m "feat: make Settings the primary INI config editor"`

---

### Task 10: Add Linux/macOS compile and host-independent parity checks

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `scripts/format-lint.sh`
- Modify: `scripts/format-lint-steps.json` only when a new check is required
- Modify: `crates/volumectl/src/autostart.rs`
- Test: `crates/volumectl/src/config_ini.rs`, `crates/volumectl/tests/host_core.rs`

**Interfaces:**
- INI parser, migration, validation, Settings patch, and autostart unsupported contract compile under Linux/macOS targets.
- Linux/macOS auto-start adapters are not claimed as complete until native desktop evidence exists; unsupported results are explicit and tested.

- [ ] **Step 1: Add cross-target tests for parser and unsupported auto-start**

Assert Linux/macOS can parse/serialize/migrate the same INI fixtures and `set_enabled` returns the documented unsupported result without touching files.

- [ ] **Step 2: Run cross-target checks RED if platform cfgs are incomplete**

Run the repository's pkg-config-stub Linux check and macOS check exactly as documented in `session-handoff.md`.

Expected: any missing cfg import or platform-specific dependency is exposed before implementation is called complete.

- [ ] **Step 3: Implement cfg-safe adapters**

Keep Windows Registry imports under `cfg(windows)` and keep parser/migration code platform-neutral. Do not add Windows-only types to shared `Config` or frontend payloads.

- [ ] **Step 4: Run cross-target GREEN checks**

Run: `cargo check --target x86_64-unknown-linux-gnu -p volumectl --tests --no-default-features` and `cargo check --target x86_64-apple-darwin -p volumectl --tests --no-default-features`.

Expected: both compile cleanly with no warnings.

- [ ] **Step 5: Commit parity checks**

Run: `git add .github/workflows/ci.yml scripts crates/volumectl feature_list.json claude-progress.md && git commit -m "test: add cross-platform INI and autostart parity checks"`

---

### Task 11: Run the complete release battery and record evidence

**Files:**
- Modify: `scripts/ship.ps1`
- Modify: `scripts/ship.sh`
- Modify: `feature_list.json`
- Modify: `claude-progress.md`
- Modify: `session-handoff.md`

- [ ] **Step 1: Run all frontend checks**

Run: `npm test --prefix frontend` and `npm run build --prefix frontend`.

- [ ] **Step 2: Run all Rust checks**

Run: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --no-default-features -- -D warnings`, and `cargo test --workspace --no-default-features`.

- [ ] **Step 3: Build the release binary**

Run: `frontend/node_modules/.bin/tauri build --no-bundle`.

Expected: `target/release/VolumeControl.exe` exists and includes the current frontend assets.

- [ ] **Step 4: Run real Windows evidence**

Run `verify-release.ps1` against the release binary. Confirm all three surface JSON/PNG/log sets, hotkey latency summary, auto-start restoration, and process-soak results.

- [ ] **Step 5: Run records and self-tests**

Run: `sh scripts/check-records.sh --staged`, `bash scripts/test-check-records.sh`, `bash scripts/test-format-lint.sh`, and `bash scripts/test-ship.sh` from Git Bash.

- [ ] **Step 6: Review all artifacts**

Inspect all PNGs with the image viewer; confirm no blank UI, clipped primary controls, unreadable contrast, or wrong geometry. Review JSON p95/soak/registry cleanup fields and verify unrelated processes were not terminated.

- [ ] **Step 7: Update records and handoff**

Record exact commands, exit codes, artifact paths, latency summary, migration result, auto-start restoration, and any physical-keyboard/environment limitation in `feature_list.json`, `claude-progress.md`, and `session-handoff.md`.

- [ ] **Step 8: Run final diff and release review**

Run: `git diff --check`, `git status --short`, `git diff --cached`, and the repository pre-push review workflow. Fix every genuine finding, rerun the full battery, and only then publish the release artifact.
