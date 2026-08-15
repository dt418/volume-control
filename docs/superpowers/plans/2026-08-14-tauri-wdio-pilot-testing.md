# Tauri WebDriver and Tauri Pilot Testing Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add real Tauri WebView E2E coverage with official WebdriverIO/Tauri tooling and debug-only Tauri Pilot workflows without weakening the production release surface.

**Architecture:** Keep the existing Rust/Vitest/Win32 test pyramid and add an `e2e/tauri` WebdriverIO package as the release-gated UI layer. Debug-only Cargo features and copied capability files enable the official WDIO plugins or Tauri Pilot for dedicated test builds; the production binary is built without either plugin. Pilot scenarios support exploration and replay, while stable release assertions live in WDIO specs or the existing verifier.

**Tech Stack:** Tauri v2, `@wdio/tauri-service`, WebdriverIO Mocha runner, `tauri-plugin-wdio`, `tauri-plugin-wdio-webdriver`, `tauri-plugin-pilot`, Rust 1.82 production toolchain, isolated Rust 1.95+ Pilot toolchain, TypeScript, PowerShell, GitHub Actions, existing Vitest/Cargo gates.

## Global Constraints

- Windows is the first E2E target; Linux follows under `xvfb-run`; macOS uses the embedded WebDriver provider.
- Production must not link `tauri-plugin-pilot`, expose `pilot:default`, expose test-only commands, or start a debug socket/named pipe.
- The production Rust toolchain remains 1.82. Pilot-enabled debug builds use an explicit Rust 1.95+ toolchain or fail clearly; they never silently fall back to production.
- WDIO is the pass/fail E2E release gate. Pilot is debug/dev inspection, recording, replay, and triage only.
- Do not use Pilot synthetic X11 shortcut presses as evidence for global-hotkey correctness; use the existing host probe/IPC route.
- Every E2E run uses one worker, a unique `VOLUMECTL_CONFIG_DIR`, condition-based waits, PID-scoped cleanup, and artifacts on failure.
- Existing Rust, frontend, surface, hotkey, records, and release gates remain mandatory.
- Test artifacts remain untracked under `output/`; no screenshots, logs, sockets, or driver binaries are committed.
- Every substantive code/script change stages `feature_list.json` and `claude-progress.md` together.

---

## File map

| File | Responsibility |
|---|---|
| `e2e/tauri/package.json`, `e2e/tauri/package-lock.json` | Isolated WebdriverIO dependencies and scripts |
| `e2e/tauri/wdio.conf.ts` | Tauri service/provider, binary path, artifacts, worker limits |
| `e2e/tauri/support/app-fixture.ts` | Debug build/start/cleanup and isolated config |
| `e2e/tauri/support/selectors.ts` | Stable accessible names and `data-testid` selectors |
| `e2e/tauri/support/artifacts.ts` | Screenshot, snapshot, browser/backend log, and timing persistence |
| `e2e/tauri/specs/*.e2e.ts` | Release-gated Mixer/Settings/Help/recovery/window scenarios |
| `e2e/pilot/scenarios/*.toml` | Debug-only Pilot record/replay scenarios |
| `e2e/pilot/README.md` | Local Pilot installation, socket selection, and safe cleanup |
| `src-tauri/src/lib.rs` | Feature-gated WDIO/Pilot plugin registration |
| `src-tauri/Cargo.toml` and `Cargo.lock` | Optional debug plugin dependencies and features |
| `e2e/tauri/capabilities/*.json` | Test permissions copied only into debug builds |
| `scripts/verify-tauri-e2e.ps1`, `scripts/verify-tauri-e2e.sh` | Cross-shell E2E orchestration and artifact checks |
| `.github/workflows/ci.yml` | Windows/Linux/macOS E2E jobs and artifact upload |
| `feature_list.json`, `claude-progress.md` | Exact commands, exit codes, and evidence paths |

---

### Task 1: Create the isolated WebdriverIO Tauri package

**Files:**
- Create: `e2e/tauri/package.json`
- Create: `e2e/tauri/wdio.conf.ts`
- Create: `e2e/tauri/package-lock.json`
- Create: `e2e/tauri/support/app-fixture.ts`
- Create: `e2e/tauri/support/selectors.ts`
- Create: `e2e/tauri/support/artifacts.ts`

**Interfaces:**
- `npm run test:e2e --prefix e2e/tauri` invokes `wdio run wdio.conf.ts`.
- `createAppFixture()` returns `{ binary, configDir, outputDir, cleanup }`.
- `selectors.ts` exports stable selectors for `mixer`, `settings`, and `help` roots plus primary controls.
- `artifacts.ts` exports `saveE2eArtifacts(browser, outputDir, name)` and `recordTiming(name, ms)`.

- [ ] **Step 1: Resolve and pin official package versions**

Use Context7 for the current Tauri WebDriver/WebdriverIO documentation, then pin compatible versions of `@wdio/cli`, `@wdio/local-runner`, `@wdio/mocha-framework`, `@wdio/spec-reporter`, and `@wdio/tauri-service` in `e2e/tauri/package.json`. Do not add these packages to `frontend/package.json`.

- [ ] **Step 2: Write the package contract test**

Create a Node test or PowerShell fixture that verifies the package has the `test:e2e` script, the `wdio.conf.ts` entry point, and no production frontend dependency changes.

- [ ] **Step 3: Run the contract test RED**

Run: `npm run test:contract --prefix e2e/tauri`

Expected: fail because the package and script do not exist.

- [ ] **Step 4: Implement the package and fixture boundaries**

Configure one worker, Mocha timeout 60 seconds, deterministic output directories, and no hard-coded user paths. Make `cleanup()` idempotent and PID-scoped.

- [ ] **Step 5: Install and run the package contract GREEN**

Run: `npm install --prefix e2e/tauri` then `npm run test:contract --prefix e2e/tauri`.

Expected: the isolated lockfile is created and the contract passes.

- [ ] **Step 6: Commit the E2E package slice**

Run: `git add e2e/tauri feature_list.json claude-progress.md && git commit -m "test: scaffold isolated Tauri WebDriver suite"`

---

### Task 2: Gate debug-only WDIO and Pilot plugins

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`
- Modify: `Cargo.lock`
- Create: `e2e/tauri/capabilities/e2e-wdio.json`
- Create: `e2e/tauri/capabilities/e2e-pilot.json`
- Create: `e2e/tauri/prepare-debug-capabilities.mjs`
- Test: `src-tauri/src/lib.rs` feature/registration tests when pure registration helpers are extracted

**Interfaces:**
- Cargo features are exactly `e2e-wdio` and `e2e-pilot`.
- `register_debug_plugins(builder)` adds only the feature-selected plugin and is called by both `builder()` and `run()` paths.
- `prepare-debug-capabilities.mjs` copies only the requested capability into `src-tauri/capabilities` and restores the directory in `finally`.

- [ ] **Step 1: Inspect plugin permissions before wiring**

Resolve the current official WDIO plugin setup and permission names from Tauri documentation; inspect the installed plugin permission files after dependency resolution. Record exact versions and permission identifiers in the E2E README.

- [ ] **Step 2: Write a production-exclusion test**

Add a script fixture that builds the normal production dependency graph and asserts `tauri-plugin-pilot`, `pilot:default`, and debug capability files are absent from the production build input.

- [ ] **Step 3: Run the exclusion test RED**

Run: `node e2e/tauri/test-production-exclusion.mjs`

Expected: fail because debug feature/capability wiring is absent.

- [ ] **Step 4: Add optional dependencies and features**

Add optional `tauri-plugin-wdio` and `tauri-plugin-wdio-webdriver` dependencies under the selected debug feature. Add `tauri-plugin-pilot` as an optional dependency behind `e2e-pilot`. Keep Pilot out of default features and use a separate Rust 1.95+ command path for Pilot builds.

- [ ] **Step 5: Register plugins only under explicit features and marker**

Guard plugin registration with both the Cargo feature and an explicit `VOLUMECTL_E2E_DEBUG=1` check. A production binary ignores the marker and starts without a test socket. The plugin setup must not change normal WindowManager/AppCore startup.

- [ ] **Step 6: Implement capability copy/restore**

Keep debug capability JSON outside the normal capability directory; copy it immediately before the debug build and restore the directory in `finally`. Include `pilot:default` only in the Pilot capability and the exact WDIO permissions only in the WDIO capability.

- [ ] **Step 7: Run debug and production compile checks**

Run:

```text
cargo check -p volumecontrol-tauri --no-default-features
node e2e/tauri/prepare-debug-capabilities.mjs --provider wdio --check-only
```

Expected: normal compile succeeds and the capability script leaves no tracked file changes. Pilot compile is run later with the explicit Rust 1.95+ toolchain.

- [ ] **Step 8: Run the exclusion test GREEN and commit**

Run: `node e2e/tauri/test-production-exclusion.mjs`.

Then: `git add src-tauri e2e/tauri Cargo.lock feature_list.json claude-progress.md && git commit -m "test: isolate Tauri E2E plugins from production"`.

---

### Task 3: Implement WDIO startup, provider, and artifact helpers

**Files:**
- Modify: `e2e/tauri/wdio.conf.ts`
- Modify: `e2e/tauri/support/app-fixture.ts`
- Modify: `e2e/tauri/support/artifacts.ts`
- Create: `e2e/tauri/support/commands.ts`
- Test: `e2e/tauri/support/*.test.ts`

**Interfaces:**
- `startE2eApp(provider: 'embedded' | 'tauri-driver')` returns a connected WebDriver session.
- `waitForSurface(browser, surface: 'mixer' | 'settings' | 'help')` waits for a visible `data-surface` root and `surface_ready` completion.
- `invokeForTest(browser, command, args)` uses `browser.tauri.execute()` only for deterministic setup/inspection.
- `saveE2eArtifacts` always captures screenshot, accessibility snapshot, URL/title/state, and `logs --level error` equivalent output on failure.

- [ ] **Step 1: Add failing helper tests**

Test output path sanitization, unique config directories, cleanup idempotence, surface selector mapping, and timing p50/p95 calculation.

- [ ] **Step 2: Run helper tests RED**

Run: `npm test --prefix e2e/tauri -- --run support`

Expected: failures for missing helpers.

- [ ] **Step 3: Configure the embedded provider**

Set `appBinaryPath` to the debug Tauri binary, `driverProvider: 'embedded'` by default, `maxInstances: 1`, and a 60-second Mocha timeout. Allow `E2E_DRIVER_PROVIDER=tauri-driver` for Windows/Linux CI fallback.

- [ ] **Step 4: Implement condition-based waits and cleanup**

Wait for the surface root and title; never use fixed sleeps for readiness. Capture child PID and kill only that process in `after`/signal cleanup.

- [ ] **Step 5: Implement artifact capture**

On every failed spec, write PNG, JSON snapshot, title/url/state, frontend console errors, backend logs, and timing summary beneath `output/tauri-e2e/<run-id>/<spec>/`.

- [ ] **Step 6: Run helper tests GREEN**

Run: `npm test --prefix e2e/tauri -- --run support`.

Expected: all helper tests pass without creating artifacts outside `output/`.

- [ ] **Step 7: Commit the provider slice**

Run: `git add e2e/tauri feature_list.json claude-progress.md && git commit -m "test: configure Tauri WebDriver provider and artifacts"`.

---

### Task 4: Add release-gated Mixer, Settings, and Help E2E specs

**Files:**
- Create: `e2e/tauri/specs/mixer.e2e.ts`
- Create: `e2e/tauri/specs/settings.e2e.ts`
- Create: `e2e/tauri/specs/help.e2e.ts`
- Modify: `e2e/tauri/support/selectors.ts`

**Interfaces:**
- Specs use accessible names and selectors from `selectors.ts`; no brittle generated class names.
- Every spec calls `waitForSurface` and records timing/artifacts through shared helpers.

- [ ] **Step 1: Write failing Mixer spec**

Assert the root/header/content/footer contract, System Output, search filtering, system mute/reset, keyboard slider interaction, empty/error state, and Escape close.

- [ ] **Step 2: Run the Mixer spec RED**

Run: `npm run test:e2e --prefix e2e/tauri -- --spec specs/mixer.e2e.ts`.

Expected: fail until the debug binary/provider is started and selectors are wired.

- [ ] **Step 3: Write failing Settings spec**

Assert six sections, focus order, draft-only edits, validation alert, Save/Reset/Cancel, retryable error, config path, and direct Settings editing.

- [ ] **Step 4: Write failing Help spec**

Assert shortcut cards, status badges, conflict callout, search, Settings CTA, footer actions, Escape, screenshot, and no console errors.

- [ ] **Step 5: Run all surface specs and fix only product/test defects**

Run: `npm run test:e2e --prefix e2e/tauri -- --spec specs/mixer.e2e.ts --spec specs/settings.e2e.ts --spec specs/help.e2e.ts`.

Expected: stable pass with no fixed-sleep retries, duplicate-key warnings, or uncaught promise errors.

- [ ] **Step 6: Commit the surface spec slice**

Run: `git add e2e/tauri/specs e2e/tauri/support/selectors.ts feature_list.json claude-progress.md && git commit -m "test: cover Tauri Mixer Settings and Help E2E flows"`.

---

### Task 5: Add recovery, multi-window, IPC, and performance specs

**Files:**
- Create: `e2e/tauri/specs/recovery.e2e.ts`
- Create: `e2e/tauri/specs/windows.e2e.ts`
- Modify: `e2e/tauri/support/commands.ts`
- Modify: `scripts/verify-tauri-e2e.ps1`
- Create: `scripts/verify-tauri-e2e.sh`

**Interfaces:**
- `mockBootstrapFailure(browser)` rejects `get_bootstrap` for a single test and restores the mock afterward.
- `listOwnedWindows(browser)` returns labels/titles for only the test process.
- `timingReport()` emits p50/p95 JSON and returns nonzero when approved deterministic budgets fail.

- [ ] **Step 1: Write recovery and multi-window specs**

Mock bootstrap rejection and assert readable `role=alert`, readiness, and process health. Open all three diagnostic surfaces, enumerate labels/titles, verify no default-position flash through evidence, and close only owned windows.

- [ ] **Step 2: Add performance assertions**

Measure bootstrap-to-ready, command round-trip, state-event-to-render, screenshot, and snapshot durations using monotonic browser timestamps plus host timestamps. Assert only the approved deterministic limits; record other metrics for comparison.

- [ ] **Step 3: Run focused specs RED**

Run: `npm run test:e2e --prefix e2e/tauri -- --spec specs/recovery.e2e.ts --spec specs/windows.e2e.ts`.

Expected: failures until command mocking, multi-window enumeration, and timing reports are implemented.

- [ ] **Step 4: Implement the PowerShell/Git Bash wrappers**

Wrappers build the debug binary, prepare capabilities, start WDIO, pass `VOLUMECTL_CONFIG_DIR`, preserve JUnit/artifacts, and restore copied capability files in `finally`.

- [ ] **Step 5: Run focused specs and wrapper tests GREEN**

Run both wrappers with a deliberate missing-binary fixture and a real debug binary. The missing fixture must exit nonzero without touching unrelated processes.

- [ ] **Step 6: Commit the recovery/performance slice**

Run: `git add e2e/tauri scripts/verify-tauri-e2e.* feature_list.json claude-progress.md && git commit -m "test: add Tauri E2E recovery and performance evidence"`.

---

### Task 6: Add Tauri Pilot debug scenarios and safe local workflow

**Files:**
- Create: `e2e/pilot/scenarios/mixer.toml`
- Create: `e2e/pilot/scenarios/settings.toml`
- Create: `e2e/pilot/scenarios/help.toml`
- Create: `e2e/pilot/scenarios/recovery.toml`
- Create: `e2e/pilot/README.md`
- Modify: `e2e/tauri/prepare-debug-capabilities.mjs`

**Interfaces:**
- `npm run test:pilot --prefix e2e/tauri -- scenario=<name>` starts a Pilot-enabled debug app with a unique socket/window selector and runs `tauri-pilot run ... --junit`.
- Scenarios use Pilot `snapshot`, `click`, `fill`, `assert`, `wait`, `screenshot`, and `logs`; global shortcut tests call IPC/host probe instead of synthetic X11 keypress.

- [ ] **Step 1: Verify the Pilot CLI/toolchain requirement**

Install `tauri-pilot-cli` in the documented developer environment and check `tauri-pilot --version`. If the active Rust toolchain is below 1.95, run Pilot builds with `cargo +<Rust 1.95+ toolchain>` and document the exact toolchain in `e2e/pilot/README.md`.

- [ ] **Step 2: Write the Mixer/Settings/Help TOML scenarios**

Each scenario starts with `ping`/`windows`, snapshots interactive controls, performs one meaningful interaction, asserts visible text, captures a screenshot, and checks `logs --level error`.

- [ ] **Step 3: Write the recovery scenario**

Use the debug IPC/mock route to force bootstrap failure; assert the readable alert and record a JUnit failure when the surface is blank or the process exits.

- [ ] **Step 4: Run Pilot scenarios RED or explicit unsupported**

Run: `npm run test:pilot --prefix e2e/tauri -- scenario=settings`.

Expected: pass on a prepared Rust 1.95+ debug environment, or a clear nonzero diagnostic stating the missing toolchain/plugin/capability; never a silent skip.

- [ ] **Step 5: Add the MCP instructions**

Document `tauri-pilot mcp` with an explicit `--socket` and `--window` selector. State that Pilot MCP is local debug tooling and may not attach to production binaries.

- [ ] **Step 6: Commit the Pilot slice**

Run: `git add e2e/pilot e2e/tauri/prepare-debug-capabilities.mjs feature_list.json claude-progress.md && git commit -m "test: add debug-only Tauri Pilot scenarios"`.

---

### Task 7: Wire Windows-first CI and artifacts

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `e2e/tauri/package.json`
- Modify: `scripts/verify-tauri-e2e.ps1`
- Modify: `scripts/verify-tauri-e2e.sh`

- [ ] **Step 1: Add a Windows E2E job**

Install Node/Rust dependencies, build the debug feature binary, run WDIO specs, upload JUnit/snapshots/screenshots/logs, then build production and run the existing surface verifier. The job must fail on missing artifacts or uncaught logs.

- [ ] **Step 2: Add the Linux E2E job**

Install WebKitGTK/WebDriver/Xvfb dependencies, run `xvfb-run` with the embedded or `tauri-driver` provider, upload artifacts, and keep X11 global-hotkey probe coverage separate from Pilot keypresses.

- [ ] **Step 3: Add the macOS E2E job**

Use the embedded provider, run renderer/host smoke before WDIO, and upload the same structured artifacts. Do not claim native driver parity that macOS does not provide.

- [ ] **Step 4: Add artifact retention and cleanup assertions**

Upload artifacts on failure and on explicit diagnostic runs; assert no test sockets, temporary config folders, or child processes remain after the job.

- [ ] **Step 5: Run CI workflow self-tests**

Run: `bash scripts/test-format-lint.sh`, `bash scripts/test-check-records.sh`, `bash scripts/test-ship.sh`, and the local E2E wrapper contract tests.

- [ ] **Step 6: Commit CI wiring**

Run: `git add .github/workflows/ci.yml e2e scripts feature_list.json claude-progress.md && git commit -m "ci: run Tauri WebDriver E2E on desktop targets"`.

---

### Task 8: Make the release gate fail closed and review the integration

**Files:**
- Modify: `scripts/ship.ps1`
- Modify: `scripts/ship.sh`
- Modify: `feature_list.json`
- Modify: `claude-progress.md`
- Modify: `session-handoff.md`

- [ ] **Step 1: Add the E2E phase to ship scripts**

Run the WDIO Windows-first gate after frontend build and before packaging. Keep Pilot outside the production ship path; expose a separate diagnostic command that never changes the release exit code from success to failure based on Pilot availability.

- [ ] **Step 2: Add deliberate-failure fixtures**

Verify nonzero exits for missing debug binary, missing driver, blank screenshot, uncaught browser error, missing JUnit, stale test socket, and orphaned child process.

- [ ] **Step 3: Run the complete local battery**

Run:

```text
npm test --prefix frontend
npm run build --prefix frontend
npm run test:e2e --prefix e2e/tauri
cargo fmt --all --check
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo test --workspace --no-default-features
frontend/node_modules/.bin/tauri build --no-bundle
pwsh -NoProfile -File scripts/verify-tauri-e2e.ps1 -Binary target/release/VolumeControl.exe -OutputRoot output/tauri-e2e/release
sh scripts/check-records.sh --staged
bash scripts/test-check-records.sh
bash scripts/test-format-lint.sh
bash scripts/test-ship.sh
```

- [ ] **Step 4: Inspect all evidence**

Open every failure/pass screenshot, inspect accessibility snapshots for all primary controls, review JUnit/timing/error logs, and verify release binaries contain no Pilot plugin or capability.

- [ ] **Step 5: Run the required review workflow**

Run the repository's three-domain pre-push review for guard core, gate chain, and records wiring. Fix every genuine finding and rerun the full battery.

- [ ] **Step 6: Record the final result**

Update `feature_list.json`, `claude-progress.md`, and `session-handoff.md` with exact commands, exit codes, platform matrix, artifact paths, Pilot toolchain limitations, and any environment-specific limitations.

- [ ] **Step 7: Commit the release-gate slice**

Run: `git add scripts e2e .github feature_list.json claude-progress.md session-handoff.md && git commit -m "release: enforce Tauri WebDriver E2E quality gate"`.
