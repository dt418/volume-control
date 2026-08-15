# Task 2 report: fail-closed WDIO evidence

## Scope

Completed only the Task 2 WDIO evidence and wrapper changes. The unrelated
cross-platform plan file and work outside the Task 2 file list were left
untouched.

## Delivered

- Pinned `@wdio/junit-reporter` `9.30.1` and configured JUnit output under the
  requested E2E output root while retaining the spec reporter.
- Added manifest writing/validation, deterministic timing output with optional
  p95 budgets, and fail-closed checks for JUnit, manifest, timings, and each
  requested spec result.
- Added frontend/backend runtime error collection with the documented
  unavailable-audio/degraded-hotkey allowlist and a runtime-spec assertion.
- Updated PowerShell and Bash wrappers to preserve WDIO failures, validate
  evidence after successful runs, and report temporary capability/bridge
  cleanup failures.
- Changed Windows, macOS, and Linux CI E2E artifact uploads to run always and
  fail when required artifacts are absent.
- Updated `feature_list.json` and `claude-progress.md` records.

## Focused verification

All commands were run from `D:\Projects\volume-control`:

- `npm test --prefix e2e/tauri` — 11 passed, 0 failed.
- `npm run typecheck --prefix e2e/tauri` — passed.
- `npm run test:contract --prefix e2e/tauri` — 1 passed, 0 failed.
- `npm run test:production-exclusion --prefix e2e/tauri` — passed.

## Concerns and limits

- No real desktop WDIO E2E run was started in this handoff; the debug binary,
  desktop driver, and platform GUI/audio environment were not exercised.
- Hosted Linux/macOS behavior and CI artifact upload behavior remain platform
  checks for the parent release-safety workflow.
- The repository-wide Rust quality gate is outside this focused Task 2 handoff
  and remains the parent agent's responsibility.

## Review fix round 1

- Fixed stale evidence acceptance: PowerShell and Bash wrappers now create a
  unique `run-...` output directory per invocation, pass its run ID through
  WDIO, and require the manifest run ID to match before accepting JUnit and
  timings artifacts. CI upload paths follow the unique run directory.
- Fixed backend evidence discovery: runtime diagnostics now scan both the
  configured service log directory and the WDIO output root (including nested
  `.log`/`.txt` files), and only explicit backend ERROR/SEVERE/PANIC records
  are treated as failures. The support test uses the concrete
  `[Tauri:Backend:0]` service-log format and confirms warning text containing
  the word "error" is not misclassified.
- Added an exact full-surface manifest contract covering Mixer, Runtime,
  Windows, Recovery, Settings, and Help, plus parse validation for timings
  JSON. The debug-run usage text now lists every supported surface.
- Fix-round verification: `npm test --prefix e2e/tauri` (13/13), `npm run
  typecheck --prefix e2e/tauri`, `npm run test:contract --prefix e2e/tauri`,
  `npm run test:production-exclusion --prefix e2e/tauri`, `bash -n
  scripts/verify-tauri-e2e.sh`, PowerShell parse validation, `git diff
  --check`, and `sh scripts/check-records.sh --staged` all pass. No real
  desktop WDIO run was started in this fix round.
