# Changelog

All notable changes to VolumeControl are documented here.

## [Unreleased]

### Added

- Tauri v2 Mixer, Settings, and Help webview surfaces with lazy window
  creation, live state events, bounded layouts, and accessible controls.
- Per-action global shortcut recording in Settings. Users can record, clear,
  restore modifier presets, and see duplicate/conflict validation before Save.
- Persisted `hotkeys` bindings with migration from the legacy modifier-only
  configuration and a `Disabled` registration state for cleared actions.
- Isolated WebdriverIO/Tauri desktop E2E coverage for Mixer, Settings, Help,
  recovery, runtime bridge, and owned-window timing; debug-only Tauri Pilot
  scenarios are available for exploratory replay.

### Changed

- Settings is now the primary configuration workflow; JSON remains compatible
  for automation and advanced editing, with live reload retained.
- Mixer surfaces use more opaque cards, stronger borders/text, visible focus
  rings, and an actionable Retry state instead of the ambiguous
  `Backend unavailable` footer.
- Bootstrap and native poll loops recover poisoned state locks so a worker
  failure does not permanently take the mixer offline.
- CI and ship checks run the frontend build and fail-closed Tauri WebDriver
  matrix before release packaging.
- CI and Release workflows now use the Tauri CLI for release artifacts after
  an explicit frontend install/build, so packaged binaries contain both the
  webview assets and Rust backend rather than only a Cargo binary.
- Tauri dev/build hooks now use the CLI object form with `cwd: ../frontend`,
  preventing directory-dependent `package.json` lookup failures during local
  or CI builds.
- Audio backend initialization is now fail-soft when no default output device
  exists, keeping the host alive so the mixer can present its recovery state.
- Mixer E2E coverage accepts the intentional Windows-only session empty state
  on Linux and macOS while retaining the Windows no-session assertions.
- Global hotkey initialization now degrades to explicit unavailable statuses
  when the native input service cannot be created, keeping the host alive in
  headless sessions and CI.
- Help E2E no longer treats WebKit's expected transport close race as an app
  failure; it still verifies the filtered surface's accessible Close control.
- Cross-platform host-core tests now assert macOS `.app` blacklist normalization
  alongside Windows `.exe` and Linux bare-name behavior.

### Verification

- Windows WebDriver matrix: 11/11 tests passed.
- Frontend Vitest: 84/84 tests passed; TypeScript/Vite build passed.
- Rust format, clippy (`-D warnings`), and workspace tests passed.
- E2E contract/support/Pilot contract and ship-flow checks passed.

Cross-platform release evidence is produced by the hosted Linux/macOS CI jobs;
the local interactive verification in this development session is Windows-first.
