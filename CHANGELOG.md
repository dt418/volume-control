# Changelog

All notable changes to VolumeControl are documented here.

## [Unreleased]

### Fixed

- Windows audio follows the current default playback device; switching output
  no longer leaves hotkeys controlling the endpoint captured at launch.
- A removed audio endpoint is re-acquired automatically instead of leaving the
  backend degraded until restart.
- The mixer lists per-app audio sessions again (`IAudioSessionManager2` is
  activated directly on the device; the old `IAudioClient::GetService` path
  returned `E_NOINTERFACE` on Windows 11).
- The mixer has clickable Settings, Help, and Close controls; Escape remains
  available as a keyboard shortcut.
- Windows runtime diagnostics are written to
  `%LOCALAPPDATA%\VolumeControl\logs\runtime.log`.

### Changed

- The mixer's system-output card is more compact, uses one threshold-colored
  slider, and exposes mute/reset as icon actions, leaving room for app sessions.
- Close controls, focus rings, and scrollbars are consistent across Mixer,
  Settings, and Help.

## [0.1.9] - 2026-10-21

### Fixed

- Clicking the desktop or Start menu icon now opens the mixer. Previously the
  app started tray-only (the icon lives in the overflow flyout), so a launch
  looked like nothing happened.
- Launching VolumeControl while it is already running now asks the existing
  instance to show the mixer instead of the second process exiting silently.
- The login auto-start entry launches tray-only (`--background`) and is
  migrated automatically for users whose existing Run entry predates the flag.
- The desktop/Start menu shortcut opens the mixer, including when another
  VolumeControl process is already running.
- Background launches (`--background`, including migrated login entries) no
  longer open a surface.
- Installer updates stop the running instance before replacing its executable.

## [0.1.8] - 2026-10-18

### Added

- Update check and notification banner in the Settings surface with direct links to release notes.
- One-shot CLI update check via `volumectl check-updates` reporting localized update status.
- Tauri IPC bridge command `check_for_updates` and `state://update` event.
- Winget deployment pipeline with REA static PE inspection (`deploy-winget.sh`, `deploy-winget.ps1`, `verify-installer-rea.mjs`).
- Live step-by-step progress feedback for installer scripts (`[1/3] downloading`, `[2/3] verifying`, `[3/3] installing`).
- Differential component updates preserving user configuration and skipping unchanged binaries.
- Ergonomic 1-line installation commands on Windows and Unix.
- Bilingual (English / Tiếng Việt) product landing site on GitHub Pages with responsive language toggle.
- The HUD overlay is click-through on Linux X11 in addition to Windows and macOS; on Wayland it stays interactive.

### Fixed

- The Windows bootstrap prints one result line per action — installed, updated, removed, or already installed — instead of finishing silently.
- A GUI install or update no longer blocks until the freshly launched app is closed: the bootstrap waits for the installer process only, not its descendants.

## [0.1.7] - 2026-10-17

### Added

- The app speaks English and Vietnamese: every surface (mixer, overlay,
  settings, help) resolves its strings through a locale catalog, the tray and
  settings expose the language switch, and the renderer smoke tests cover both
  languages.
- The tray menu gained a "Check for updates" action that reports the result
  inline, so users do not have to open the settings window to learn whether a
  newer release exists.

### Changed

- Startup stages are traced and the update check answers immediately, keeping
  the first window responsive on slower machines.

### Fixed

- The format-lint smoke test releases its concurrency lock on every exit path,
  so a refused run no longer blocks later invocations with "already running".
- The release-workflow harness resolves HEAD across shells (WSL, Git Bash,
  native Windows git) instead of failing inside a worktree.

### Tooling

- Record-file merge drivers (`feature_list.json`, `claude-progress.md`,
  `session-handoff.md`) merge parallel branches without conflict markers.
- The Ubuntu CI job builds the Linux bundles and the macOS job builds the dmg,
  so packaging failures surface on the pull request instead of at the tag.


## [0.1.6] - 2026-10-17

### Added

- Native packages in every release: the macOS job builds a `.dmg` and the
  Ubuntu job builds `.deb`, `.rpm`, and `.AppImage` bundles (Tauri bundlers)
  alongside the zip and tarball; `package.sh` stages them under canonical
  names, `verify-release-metadata.sh` requires them per platform, and the
  fixture matrix covers the new sets.
- macOS/Linux one-line installers: `install-macos.sh` and `install-linux.sh`
  ship next to each package in the release, verify SHA-256 against the
  release `SHA256SUMS.txt` before unpacking, and cover install, update, and
  uninstall in user scope with EN/VI messages (`VOLUMECTL_LANG`,
  `VOLUMECTL_VERSION`, `VOLUMECTL_INSTALL_ROOT` overrides). The release
  verifier, the fixture matrix (including a new macOS fixture), and the
  macOS/Ubuntu CI jobs now exercise the per-platform artifact sets.
- `scripts/generate-winget-manifest.sh` emits winget-pkgs **1.12** manifests
  that pass `winget validate --manifest`.

### Fixed

- The published PowerShell bootstrap is now pure ASCII with `[char]` escapes
  for Vietnamese text. Windows PowerShell 5.1 decodes `irm` text/plain
  responses as cp1252, so the previous UTF-8 BOM arrived as `i»¿` and the
  documented `[scriptblock]::Create((irm ...))` one-liner failed to parse;
  the test suite now decodes the script as cp1252 and asserts that it parses
  and renders the Vietnamese messages correctly.

### Repository

- Added the MIT `LICENSE` file the README already claimed (also required by
  the winget manifest's `LicenseUrl`).

## [0.1.5] - 2026-10-17

### Added

- Verified Windows install bootstrap: `scripts/Install-VolumeControl.ps1`
  installs, updates, and uninstalls strictly under
  `%LOCALAPPDATA%\VolumeControl`, resolves stable or exact `vX.Y.Z` GitHub
  releases, verifies the NSIS installer against the release
  `SHA256SUMS.txt`, never force-terminates a running VolumeControl during
  update, and keeps `%APPDATA%\volume-control\config.ini` across update and
  uninstall.
- Multi-language UI (English/Vietnamese): `[general] language = en|vi` in
  `config.ini` with a Settings selector; the tray, overlay, mixer, settings
  window, the GTK/macOS renderer controls, and every webview surface render
  localized labels. The PowerShell bootstrap ships the same EN/VI message
  table (`VOLUMECONTROL_LANG` overrides the Windows UI culture).
- Tray "Check for updates": queries the latest stable GitHub release, shows
  an immediate in-progress card in the active language, and reports the
  result (up to date / version available / check failed) on the overlay.
- Release tooling: multi-asset packaging (`dist/` NSIS + installer script +
  canonical `SHA256SUMS.txt`), a fail-closed release-metadata verifier, a
  Windows NSIS bundle step in desktop validation, immutable release
  publishing (`release-assets/` with no overwrite flag), a product page
  (`docs/index.html`) with `irm`/`curl`/`winget` install commands, and
  `scripts/generate-winget-manifest.sh` for winget-pkgs submissions.

### Changed

- Startup now logs per-stage timings behind `VOLUMECTL_STARTUP_TRACE=1`
  (zero-cost when off); `docs/performance.md` records the measured budget.
- Clippy is pinned to the project MSRV (`clippy.toml`), so
  `chunks_exact` stays valid on Rust 1.82.

### Fixed

- `bash scripts/test-release-workflow.sh` resolves `HEAD` across Windows
  git worktrees and WSL/MSYS shells instead of exiting 128 mid-run.
- macOS host handles the new `AppAction::CheckUpdates`, and the Linux/macOS
  renderer smoke binaries pass the language argument through.

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
