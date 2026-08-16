# Changelog

All notable changes to VolumeControl are documented here.

## [Unreleased]

## [0.1.4] - 2026-08-17

### Fixed

- Windows Tauri E2E "No window could be found" flake: the mixer no longer
  auto-closes on focus loss under the debug E2E marker (production behavior
  unchanged), each E2E surface app gets an isolated WebView2 user-data
  folder so orphaned renderer processes from force-killed runs cannot break
  the next webview, and the E2E wrapper kills leftover app instances before
  the build, immediately before launch, and after the run — a crashed run
  can no longer hold the embedded WebDriver port or leave a stale HUD ghost
  on the desktop.

## [0.1.3] - 2026-08-16

### Added

- System tray on macOS and Linux: Tauri-managed tray with the same stable
  menu ids as the Windows native tray (mute, reset, mixer, settings, help,
  reload, open config, exit), a shared cross-platform `TrayCommand` mapping,
  and non-fatal tray creation so the host stays alive without a tray host.
- Overlay HUD on macOS and Linux: transparent always-on-top webview HUD
  (336×88, legacy bottom-right margins) with bootstrap-first rendering (the
  window never appears empty), a cancellable auto-hide plus a frontend
  self-hide guarantee, Rust-resolved theme shared with the mixer, and
  click-through on macOS. Windows keeps its native Win32 HUD.
- Linux per-app audio in the Mixer: PulseAudio sink-input enumeration with
  per-session volume and mute through the shared session commands; an
  unreachable Pulse server degrades to an empty list instantly (800 ms
  connect timeout + 10 s backoff) instead of stalling the UI. macOS per-app
  audio is documented as not supported (no public API).
- Platform verification scripts (`verify-platform.ps1` / `.sh`): one
  fail-closed command per platform runs the whole-app battery with per-step
  logs; E2E uses real audio by default and restores the device state.

### Changed

- FE–BE connection stability: the host poll threads no longer queue IPC
  commands behind audio probes (`try_lock`), and backend failures publish a
  one-shot `state://backend` Ready↔Degraded status so the Mixer shows a live
  notice instead of freezing silently.
- Webview performance: Pulse connect backoff eliminates multi-second mount
  stalls on machines without Pulse; Mixer session rows are memoized and
  session sorting is cached, so per-app rows only re-render when their data
  changes.
- The E2E debug runner defaults to the real OS audio endpoint (device state
  restored after the run); hosted CI opts into the deterministic virtual
  backend.
- Enforcement hardening from the three-domain pre-push review: CRLF-safe
  records `--check`, `core.quotepath=true` on all guard git captures,
  PowerShell/version parity in the format-lint gates, a fail-fast
  single-instance lock for the format-lint smoke suite, and the release
  workflow E2E gates pinned to the virtual backend.

## [0.1.2] - 2026-08-16

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
- Release packaging is now SHA-bound to the validated Windows, macOS, and
  Ubuntu desktop artifacts; the publish job verifies metadata and checksums
  instead of rebuilding an unvalidated tag binary.
- Manual-dispatch releases now resolve the requested tag and fail closed when
  it does not point to the selected workflow commit; annotated tags are
  dereferenced before comparison.
- Added the [cross-platform release evidence checklist](docs/testing/cross-platform-release-checklist.md)
  with exact Windows, Ubuntu/Xvfb, WSLg, macOS inspection, and release review
  commands. Hosted headless checks are explicitly bounded to deterministic
  surface/build evidence and do not claim native hotkeys, tray, hardware audio,
  Wayland, TCC/Accessibility, or multi-monitor behavior. E2E JUnit, manifest,
  and platform logs remain validation evidence reviewed separately from the
  publish-time metadata/checksum/package verifier.
- The current macOS package remains ad-hoc signed for validation. Public
  distribution is deferred to a protected Developer ID/notarization workflow;
  no signing secrets are stored in the repository.

### Verification

- Windows WebDriver matrix: 11/11 tests passed.
- Frontend Vitest: 84/84 tests passed; TypeScript/Vite build passed.
- Rust format, clippy (`-D warnings`), and workspace tests passed.
- E2E contract/support/Pilot contract and ship-flow checks passed.

Hosted cross-platform release evidence remains a required workflow outcome;
this documentation update records no new hosted run URL, run ID, or artifact
path. Local interactive verification remains Windows-first until hosted
artifacts are inspected under the checklist.
