# Tauri UI Rendering Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore legacy placement and readable, crash-resistant rendering for the Tauri Mixer, Settings, and Help surfaces, with scripted Windows screenshots and process/geometry evidence.

**Architecture:** The Tauri `WindowManager` creates each webview hidden, applies final size and placement, and reveals it only after the React surface has applied its bootstrap appearance and signaled readiness. Each React surface becomes a bounded viewport shell with explicit header/body/footer regions and one intentional scroll container. A PowerShell verifier launches one real surface at a time through a diagnostic environment selector and records bounds, process health, logs, and PNG evidence.

**Tech Stack:** Tauri v2/Rust, React 19 + TypeScript + Tailwind v4, Vitest + Testing Library, PowerShell/Win32 diagnostic capture, existing Cargo and frontend quality gates.

## Global Constraints

- Preserve legacy geometry: Mixer `400x224` above the overlay, Settings `760x620` centered with `620x520` minimum, Help `520x500` bottom-right at `24/48` margins.
- Use monitor work areas and one logical-to-physical DPI conversion; retain the Mixer top clamp for short work areas.
- Keep Rust as the state source of truth and retain all current Mixer/Settings/Help functionality.
- Do not make the native HUD overlay, audio, hotkey, tray, or config schema changes.
- The frontend shell must keep visible controls inside `100dvh`; only the designated content region may scroll.
- A bootstrap error must render readable error content and still reveal the window; it must never leave an invisible webview that looks crashed.
- `scripts/verify-tauri-surfaces.ps1` must use condition-based waits, isolate config state, capture real windows, and preserve artifacts under `output/tauri-surface-evidence/` (untracked).
- Every substantive code/script change stages `feature_list.json` and `claude-progress.md` in the same change set.
- Before completion run the full repository format/lint/test gate, frontend build/tests, verifier, and records/self-tests. Never weaken `-D warnings`.

---

### Task 1: Diagnostic surface selector and verifier RED baseline

**Files:**
- Create: `scripts/verify-tauri-surfaces.ps1`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/lib.rs` unit tests (startup selector parser)

**Interfaces:**
- Produces `parse_verify_surface(value: &str) -> Result<Option<SurfaceId>, String>` for accepted labels `window-mixer`, `window-settings`, and `window-help`.
- Produces a startup-only `VOLUMECTL_VERIFY_SURFACE` path that calls the same `WindowManager::open` used by production after managed state is initialized.
- The verifier accepts `-Binary`, optional `-Surface`, `-TimeoutSeconds`, and `-OutputRoot`, and writes `<surface>.json`, `<surface>.png`, and `<surface>.log` beneath the selected evidence directory.

- [ ] **Step 1: Add the failing Rust parser tests**

```rust
#[test]
fn verify_surface_parser_accepts_only_webview_labels() {
    assert_eq!(parse_verify_surface("window-mixer").unwrap(), Some(SurfaceId::Mixer));
    assert_eq!(parse_verify_surface("window-settings").unwrap(), Some(SurfaceId::Settings));
    assert_eq!(parse_verify_surface("window-help").unwrap(), Some(SurfaceId::Help));
    assert_eq!(parse_verify_surface("window-overlay"), Err("unknown surface".into()));
    assert_eq!(parse_verify_surface(""), Ok(None));
}
```

- [ ] **Step 2: Run the focused test and verify the expected RED failure**

Run: `rtk cargo test -p volumecontrol-tauri verify_surface_parser`

Expected: compile/test failure because the parser does not exist yet. Fix test typos if the failure is unrelated; do not implement the production parser before observing this failure.

- [ ] **Step 3: Implement the diagnostic selector**

Add the parser beside the `run` setup code. After `app.manage(WindowManager::new(...))` and the existing `AppCore` state are installed, read `VOLUMECTL_VERIFY_SURFACE`; for a valid value call `app.state::<WindowManager>().open(surface)`, and return setup errors as `Box<dyn Error>` so invalid values fail loudly. No environment variable means normal startup behavior.

- [ ] **Step 4: Run the focused test and verify GREEN**

Run: `rtk cargo test -p volumecontrol-tauri verify_surface_parser`

Expected: all parser tests pass with exit code 0.

- [ ] **Step 5: Write the verifier script**

The script must:

```powershell
# Contract (implemented as parameters, not pseudocode at runtime)
param(
  [string]$Binary,
  [ValidateSet('window-mixer','window-settings','window-help')]
  [string[]]$Surface = @('window-mixer','window-settings','window-help'),
  [int]$TimeoutSeconds = 15,
  [string]$OutputRoot = 'output/tauri-surface-evidence/after'
)
```

For each surface create a unique temporary `VOLUMECTL_CONFIG_DIR`, start the binary with `VOLUMECTL_VERIFY_SURFACE`, poll for a top-level window whose title/label matches the selected surface, poll for nonzero client bounds and a responsive process, capture the window using the existing Win32 capture helper pattern from `scripts/verify-vol011.ps1`, and emit JSON containing PID, HWND, process exit code, client/outer bounds, monitor work area, DPI, and expected legacy bounds. Use `try/finally` to terminate only the child process and delete only the per-run temp config directory. Return nonzero on missing window, blank/failed capture, early process exit, or bounds mismatch.

- [ ] **Step 6: Run the verifier before UI fixes and preserve RED evidence**

Run against the existing release binary:

```powershell
pwsh -NoProfile -File scripts/verify-tauri-surfaces.ps1 `
  -Binary src-tauri/target/release/VolumeControl.exe `
  -OutputRoot output/tauri-surface-evidence/before
```

Expected: the script reaches the current rendering/visibility failure for at least one surface and preserves a failure log/PNG or an explicit missing-window diagnostic. This is the baseline artifact; do not delete it.

---

### Task 2: Tauri place-before-show lifecycle and geometry RED/GREEN

**Files:**
- Modify: `src-tauri/src/window_manager.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/window_manager.rs` unit tests and command registration tests

**Interfaces:**
- Add `WindowManager::surface_ready(surface: SurfaceId) -> Result<(), String>`.
- Add Tauri command `surface_ready(window: tauri::WebviewWindow, window_manager: State<'_, WindowManager>) -> Result<(), String>`; infer the `SurfaceId` from `window.label()` and delegate to the manager.
- Add `WindowManager::parse/placement` helpers only where they remain pure and testable; do not expose mutable internals to the frontend.

- [ ] **Step 1: Add failing pure geometry tests for the legacy Mixer clamp**

Add a short-work-area case equivalent to the native helper: when the computed Mixer top is negative, the returned top is the work-area top and the size remains the scaled Mixer size.

- [ ] **Step 2: Run the geometry test and verify RED**

Run: `rtk cargo test -p volumecontrol-tauri mixer_clamps_to_work_area`

Expected: failure showing the current negative top coordinate.

- [ ] **Step 3: Add failing lifecycle tests around the pure readiness state**

Extract a small private `SurfaceState` transition helper and test that `opened -> ready` is idempotent, unknown labels are rejected, and an already-open surface takes the reposition/focus path instead of returning before placement.

- [ ] **Step 4: Implement minimal lifecycle changes**

In `open`:

1. If an existing window is active, reapply placement, show, and focus it as appropriate; only return after that succeeds.
2. Add `.visible(false)` to the builder.
3. Build the window, call `apply_placement` with `set_size` before `set_position`, install the existing focus/destroy handlers, and mark it active but not ready.
4. In `surface_ready`, reapply placement, call `show`, focus the window, and mark ready. Repeated calls must return `Ok(())`.

In `place_surface`, clamp the Mixer top to `wa_y.max(computed_top)`. Keep Settings centered and Help bottom-right exactly as the locked contract requires. Register `surface_ready` in both `builder()` and `run()` invoke handlers.

- [ ] **Step 5: Run focused Rust tests and compile**

Run: `rtk cargo test -p volumecontrol-tauri window_manager` and `rtk cargo check -p volumecontrol-tauri --no-default-features`.

Expected: all targeted tests pass and the command compiles without warnings.

---

### Task 3: Shared frontend readiness/error contract

**Files:**
- Create: `frontend/src/lib/surface.ts`
- Modify: `frontend/src/lib/ipc.ts` only if a typed helper is needed
- Modify: `frontend/src/mixer/sessionStore.ts`
- Modify: `frontend/src/mixer/MixerSurface.tsx`
- Modify: `frontend/src/settings/SettingsSurface.tsx`
- Modify: `frontend/src/help/HelpSurface.tsx`
- Test: `frontend/src/lib/surface.test.ts`, surface tests

**Interfaces:**
- Add `markSurfaceReady(): Promise<void>` calling `invoke('surface_ready')`.
- Add a shared `SurfaceError` rendering contract: a `role="alert"` message with a retry-safe readable fallback.

- [ ] **Step 1: Write failing frontend tests**

Add tests that:

1. `markSurfaceReady` invokes `surface_ready` exactly once per mounted surface.
2. A rejected `get_bootstrap` renders a non-empty `role="alert"` and still invokes `surface_ready`.
3. Successful bootstrap applies appearance before readiness is sent.

- [ ] **Step 2: Run the focused Vitest tests and verify RED**

Run: `npm test --prefix frontend -- src/lib/surface.test.ts`

Expected: failures for the missing helper and missing error/ready behavior.

- [ ] **Step 3: Implement the shared helper and bootstrap finally paths**

Use one guarded effect per entry. Start bootstrap and listeners in the effect, call `applyAppearance` before setting loaded state, catch into a readable error state, and call `markSurfaceReady()` in `finally` exactly once. Keep the current fail-soft behavior for Help/Mixer data, but make failure visible rather than silently rendering an empty surface.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run: `npm test --prefix frontend -- src/lib/surface.test.ts frontend/src/mixer/MixerSurface.test.ts frontend/src/settings/SettingsSurface.test.ts frontend/src/help/HelpSurface.test.ts`

Expected: all focused tests pass with no React key warnings or unhandled rejection output.

---

### Task 4: Legacy-bounded React shells and readability

**Files:**
- Modify: `frontend/src/styles.css`
- Modify: `frontend/src/mixer/MixerSurface.tsx`
- Modify: `frontend/src/settings/SettingsSurface.tsx`
- Modify: `frontend/src/settings/SectionNav.tsx`
- Modify: `frontend/src/help/HelpSurface.tsx`
- Modify: `frontend/src/help/HelpFooter.tsx`
- Test: `frontend/src/mixer/MixerSurface.test.tsx`, `frontend/src/settings/SettingsSurface.test.tsx`, `frontend/src/help/HelpSurface.test.tsx`, `frontend/src/styles.test.ts`

**Interfaces:**
- Each surface root exposes `data-surface="mixer|settings|help"`.
- Each shell exposes `data-testid="surface-header"`, `data-testid="surface-content"`, and `data-testid="surface-footer"`.
- Content regions use explicit `overflow-y-auto`; roots use `h-dvh min-h-0 overflow-hidden`.

- [ ] **Step 1: Add failing shell contract tests**

For each surface assert that the root is viewport-bounded, header/content/footer are all present after bootstrap, and the footer is a sibling of (not a child inside) the scroll region. Settings additionally asserts a title, subtitle, close button, and six-section nav; Help asserts title/subtitle/close/footer; Mixer asserts System Output remains outside the app-list scroll container.

- [ ] **Step 2: Run shell tests and verify RED**

Run: `npm test --prefix frontend -- src/mixer/MixerSurface.test.tsx src/settings/SettingsSurface.test.tsx src/help/HelpSurface.test.tsx`

Expected: failures for missing shell markers, Settings header, or bounded structure.

- [ ] **Step 3: Implement the shared CSS and shells**

In `styles.css`, add a reusable `.surface-shell` and `.surface-scroll` layer with `height: 100dvh`, `min-height: 0`, `overflow: hidden`, readable fallback backgrounds, and consistent border/text contrast. Avoid relying on transparency alone.

In Mixer, make the root a shell, keep the header/search/System Output fixed, and put only the session list/empty state in the scroll region. Preserve search, mute, slider, stale-row, and Esc behavior.

In Settings, add a compact legacy header with title/subtitle/close, arrange nav and pane in a flex/grid body, make the section pane the only scroll container, and keep the status/action footer visible. Preserve the 760px wide vertical rail and the below-760px horizontal selector using the existing `min-[760px]` breakpoint.

In Help, make header, shortcut body, and `HelpFooter` siblings under the shell. Put cards/callouts in the body scroll region and keep footer buttons visible at `520x500`.

- [ ] **Step 4: Run focused shell tests and frontend build**

Run: `npm test --prefix frontend -- src/mixer/MixerSurface.test.tsx src/settings/SettingsSurface.test.tsx src/help/HelpSurface.test.tsx` and `npm run build --prefix frontend`.

Expected: all surface tests pass and TypeScript/Vite build exits 0.

---

### Task 5: Capture after evidence and deliberate-failure checks

**Files:**
- Modify: `scripts/verify-tauri-surfaces.ps1`
- Create: `scripts/tests/verify-tauri-surfaces.tests.ps1` only if the repository’s existing PowerShell self-test convention requires a separate test file
- Test: verifier against the built binary and negative fixtures

- [ ] **Step 1: Build the real packaged binary**

Run: `npm run build --prefix frontend` then `rtk npx tauri build --no-bundle` from the repository root.

Expected: `src-tauri/target/release/VolumeControl.exe` exists and the frontend build is included in the embedded assets.

- [ ] **Step 2: Run the verifier for all three surfaces**

```powershell
pwsh -NoProfile -File scripts/verify-tauri-surfaces.ps1 `
  -Binary src-tauri/target/release/VolumeControl.exe `
  -OutputRoot output/tauri-surface-evidence/after
```

Expected: exit 0; three JSON records and three non-blank PNGs; every process remains alive through the observation interval; recorded bounds match the expected legacy contract within the script’s explicit one-pixel rounding tolerance.

- [ ] **Step 3: Run negative verifier cases**

Run the script with an invalid binary path, a deliberately wrong expected geometry fixture, and a capture timeout. Each must exit nonzero and write a diagnostic log without touching unrelated processes or tracked files.

- [ ] **Step 4: Inspect evidence visually**

Open all six before/after PNGs with the image viewer. Confirm readable text, non-blank surfaces, visible headers/footers, no clipping of primary controls, no early wrong-position flash in the after captures, and acceptable fallback contrast in both cached light/dark themes. Record the observations in `claude-progress.md`.

---

### Task 6: Full verification, records, and review

**Files:**
- Modify: `feature_list.json`
- Modify: `claude-progress.md`
- Modify: `session-handoff.md` only if the final counts/evidence paths require refresh

- [ ] **Step 1: Run the frontend and targeted Rust gates**

Run: `npm test --prefix frontend`, `npm run build --prefix frontend`, and `rtk cargo test -p volumecontrol-tauri window_manager`.

- [ ] **Step 2: Run the repository quality gate**

Run the full Windows gate from `.agents/skills/format-lint/scripts/format-lint.ps1`, then `rtk cargo test --workspace --no-default-features` if the gate’s test step was skipped. Treat every warning/error as a failure.

- [ ] **Step 3: Run records and guard self-tests**

Run `bash scripts/check-records.sh --staged`, `bash scripts/test-check-records.sh`, `bash scripts/test-format-lint.sh`, and `bash scripts/test-ship.sh` from Git Bash. Stage records only after their contents include exact commands, exit codes, and evidence paths.

- [ ] **Step 4: Review the diff and evidence**

Check `rtk git diff --check`, `rtk git status --short`, the complete staged diff, all verifier JSON, and all PNGs. Confirm no `target/`, `output/`, logs, or local config files are tracked.

- [ ] **Step 5: Record the final result**

Update `feature_list.json`’s Tauri UI feature with the exact passing commands and `output/tauri-surface-evidence/before|after` paths. Add a dated session entry to `claude-progress.md` with the root cause, files changed, tests, script exit code, process-crash checks, and visual findings.

- [ ] **Step 6: Run the required review workflow before any commit/push**

Use the repository’s pre-push review workflow for guard core, gate chain, and records wiring. Fix every genuine finding, rerun the full battery, and only then present the clean handoff.
