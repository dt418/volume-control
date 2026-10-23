# Session Handoff

## Session 131 (2026-10-23) - VolumeControl v0.1.11 version bump and release preparation

- Summary: Bumped version from 0.1.10 to 0.1.11 across all package manifests and lock files, updated CHANGELOG.md, verified version sync, and prepared tag v0.1.11 for release publishing.
- Changes:
  - `Cargo.toml`, `crates/volumectl/Cargo.toml`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `frontend/package.json`, `frontend/package-lock.json`, and `Cargo.lock`: bumped to 0.1.11.
  - `CHANGELOG.md`: added `[0.1.11] - 2026-10-23` release section.
- Verification:
  - `bash scripts/check-version-sync.sh v0.1.11`: confirmed version sync ok: 0.1.11 across all 7 targets.
  - `cargo check --workspace --no-default-features`: passed.
  - `npm --prefix frontend test`: 124 tests passed across 19 files.
- Records: `vol-113` registered and marked passing in `feature_list.json`.

## Session 129 (2026-10-23) - Reliable update notifications, background downloads, and transactional installation lifecycle

- Summary: Implemented host-owned UpdateManager coordinator, background streaming downloads with SHA-256 verification, user confirmation modal/panel in Settings UpdateCard, graceful process shutdown without force-kill (Local\VolumeControl.Exit and SIGTERM), and transactional installation lifecycle across Windows, Linux, and macOS.
- Changes:
  - `crates/volumectl/src/updates.rs`: `fetch_latest_release` with ReleaseInfo, strict semver validation, error propagation.
  - `src-tauri/src/update_manager.rs`: host-owned `UpdateManager` managing `UpdateState` with revision ordering, 15s delay, 6h periodic checks, request coalescing, and background download worker.
  - `src-tauri/src/lifecycle.rs`: `Local\VolumeControl.Exit` event, `signal-hook` SIGTERM on Unix, native notifications with deduplication in `update-state.json`, and offline prepared apply helper.
  - `scripts/Install-VolumeControl.ps1`, `packaging/install-linux.sh`, `packaging/install-macos.sh`: graceful shutdown, file locks, install receipts, transactional staging/rollback, and offline prepared apply.
  - `src-tauri/nsis/hooks.nsh`: eliminated force-kill and overrode `CheckIfAppIsRunning`.
  - `frontend/src/settings/UpdateCard.tsx` & `MixerSurface.tsx`: `UpdateStatePayload` contract, revision tracking, inline confirmation panel with version warning, Confirm and Later buttons.
  - `docs/index.html`: updated `tpl-ps`, `tpl-curl`, and `tpl-unix` snippets and graceful notes.
  - `scripts/verify-install-lifecycle.ps1` & `.sh`: lifecycle verification harnesses.
  - `.github/workflows/installer-lifecycle.yml` & `desktop-validation.yml`: native lifecycle validation workflow.
- Verification:
  - `cargo fmt --all --check` & `git diff --check`: clean.
  - `cargo clippy --workspace --all-targets --no-default-features -- -D warnings`: passed with 0 warnings.
  - `cargo test --workspace --no-default-features`: 380 tests passed across 8 suites.
  - `npm --prefix frontend test`: 128 tests passed across 20 files.
  - `scripts/test-install-volumecontrol.ps1` (PS 5.1 & pwsh 7) and `scripts/test-install-unix.sh` (23 checks): all passed.
  - `scripts/check-tauri-deadlock.sh`: 10/10 invariants passed.
  - `scripts/verify-install-lifecycle.sh`: verified in isolated WSL environment.
  - Records: `vol-111` registered and marked passing in `feature_list.json`.

## Session 123 (2026-10-21) - Winget 0.1.8 submission and desktop icon fix

- Winget: PR microsoft/winget-pkgs#454508 updated to 0.1.8 (fork branch
  `dt418.VolumeControl-0.1.5`, commit `8d38efec0`); installer SHA-256
  `618C5BF93FCCEF59A91B013939A5628F82CCA93688399569330EBF454306B0BB` matches the
  release `SHA256SUMS.txt`; `winget validate` passed. Repo `winget/` now tracks
  `0.1.8` (the unpublished 0.1.7 manifest dir was removed).
- Desktop icon: a no-argument launch opens the mixer, the login Run entry uses
  `--background` (with automatic migration for old entries), and a second launch
  signals the running primary through the named event
  `Local\VolumeControl.ShowMixer` instead of exiting silently.
- Verified: fmt, clippy `-D warnings`, workspace tests, live launch/handoff
  smokes on `target/debug/VolumeControl.exe`, autostart registry migration smoke.
- Note: installed v0.1.8 binaries keep the old behavior until the next release
  ships this fix.

## Session 121 (2026-10-18) - VolumeControl v0.1.8 release and bilingual landing page

Branch `release/0.1.8` (worktree `volume-control-v018-release`) prepares the
v0.1.8 release and completes the landing page bilingual localization:

- `docs/index.html`: added responsive English/Vietnamese language toggle (`#lang-btn`),
  complete dictionary covering hero, simulated overlay demo, remote installation
  snippets, feature cards, platform matrix, and trust boundaries, with browser
  detection and `localStorage` persistence.
- Workspace bump: synchronized version `0.1.8` across workspace `Cargo.toml`,
  `crates/volumectl/Cargo.toml`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`,
  `frontend/package.json`, `frontend/package-lock.json`, and `Cargo.lock`.
- `CHANGELOG.md`: updated `[0.1.8]` release section documenting update notifications,
  CLI check-updates, Winget deployment pipeline, installer progress feedback,
  and landing page i18n.
- Record registered: `vol-102` in `feature_list.json` and session recorded in
  `claude-progress.md`. PR #65 opened on GitHub with automated multi-platform CI.
- Verified: `cargo fmt`, `git diff --check`, clippy `-D warnings`, workspace `cargo test`
  (355 tests pass), `npm test` (18 files / 108 tests pass), `check-tauri-deadlock.sh` (10/10),
  `test-release-workflow.sh` (44/44 pass), `test-check-records.sh` (35/35 pass).

## Session 120 (2026-10-18) - Update check and notification in Settings, CLI, and IPC bridge

PR #64 (`feat/update-notifications`) merged into `main` (`e732e11`):

- `crates/volumectl/src/updates.rs`: added `UpdateInfo` struct, `resolve_update_info`,
  and `check_update_info` with pure semver comparison and bounded timeouts.
- `crates/volumectl/src/cli.rs`: added `volumectl check-updates` CLI subcommand.
- `src-tauri/src/commands.rs`: added `check_for_updates` command emitting `state://update`.
- `frontend/src/settings/UpdateCard.tsx`: new Settings component with bilingual EN/VI
  strings, live loading state, and update alert banner linking to release notes.

## Session 110 (2026-10-17) - Merge main into the harness branch: record-id collision + WSL guard fix

Branch `fix/release-workflow-git-path-resolution` (worktree
`volume-control-windows-release-bootstrap`) merged `origin/main` (`1ba66e5`,
App i18n #46) as `0c798cc`. The record merge drivers landed the three record
files without conflict markers; the merged ledger and the guard then needed
two repairs:

- `vol-089` id collision: this branch had rewritten main's macOS
  `AppAction::CheckUpdates` record into the format-lint lock record, so the
  driver unioned two different features under one id and main's macOS
  title/behavior text dropped out. `vol-089` is now restored verbatim from
  main, the format-lint record moved to `vol-095`, and `vol-096` records the
  guard fix below. Ledger: 63 features (48 passing / 15 in_progress), ids
  unique, `priority == id` for all.
- `scripts/check-records.sh` resolves the repository the way vol-088 does for
  the release harness: walk up to the `.git` marker, convert a drive-letter
  `gitdir:` with `wslpath -u` (WSL) or `cygpath -u` (MSYS), export
  `GIT_DIR`/`GIT_WORK_TREE` - only after plain `git rev-parse --git-dir`
  already failed. Without it the WSL Linux gate (`scripts/format-lint.sh` runs
  `sh scripts/check-records.sh --staged`) ended in
  `FAIL - git diff --cached failed (exit 128)` in a worktree created by native
  Windows git.

Verified in this worktree: WSL bash `bash scripts/check-records.sh --staged`
exit 0 (before: exit 1, bare `fatal: not a git repository`), staged-scratch
negative reports the records failure with the real trigger path, MSYS `sh`
path unchanged, `scripts/test-check-records.sh` 35 `ok`,
`scripts/test-records-merge.sh` 9 `ok`, `scripts/test-format-lint.sh` all
checks passed (including the refused-run lock check), `bash -n` clean. Next:
push, PR CI, merge.

## Session 108 (2026-10-17) - Native Linux and macOS packages

Branch `chore/release-0.1.5` adds native packages to the release contract:
`dmg` on macOS and `deb`/`rpm`/`AppImage` on Ubuntu, staged by `package.sh`
under canonical names, required by the verifier (macOS 3 artifacts, Ubuntu
5), covered by fixtures, asserted in the release harness, and documented in
the READMEs + CHANGELOG. Next: PR, CI (macOS/Ubuntu bundle builds are the
proof), merge, tag v0.1.7.

## Session 107 (2026-10-17) - macOS/Linux installers + cp1252-safe bootstrap

Branch `chore/release-0.1.5` (worktree `volume-control-release-015`)
continues with the macOS/Linux remote-install feature and the fix for the
bootstrap asset discovered during live verification:

- `packaging/install-macos.sh` + `packaging/install-linux.sh` (verified
  against a fixture release by `scripts/test-install-unix.sh`, 23/23) ship
  as release assets next to each package; `package.sh`, the release verifier
  (two artifacts per macOS/Ubuntu set), the fixture matrix, and CI
  (macOS + Ubuntu jobs run the suite) all cover them.
- The PowerShell bootstrap lost its BOM and became pure ASCII: VS text is
  `$([char]0xXXXX)` escapes, which survive the cp1252 decode Windows
  PowerShell 5.1 applies to `irm` responses. Regression tests decode the
  script as cp1252 and assert it parses and renders VI correctly.
- Next: PR, CI, merge, tag v0.1.6, then re-run the live one-liner checks
  against the published assets.
- Release 0.1.6 prep is included in the same branch: version bump (workspace,
  volumectl, Tauri, tauri.conf.json, frontend), `[0.1.6]` CHANGELOG, and
  winget-pkgs 1.12 manifests validated with `winget validate --manifest`.
  The installer keeps the `-setup.exe` name (user decision); v0.1.5 release
  notes now carry an asset legend. After v0.1.6 publishes: submit the winget
  manifest PR, then add native Linux bundles (deb/rpm/AppImage) and a macOS
  dmg for v0.1.7.

## Session 106 (2026-10-17) - Release v0.1.5 preparation

Branch `chore/release-0.1.5` (worktree `volume-control-release-015`, forked
from `main` at 8f366cc) bumps every product version to 0.1.5 and writes the
`[0.1.5]` CHANGELOG section. The tag `v0.1.5` is pushed only after the PR
CI is green, so the release workflow publishes the verified asset set
(bootstrap script, NSIS installer, three packages, canonical SHA256SUMS.txt)
that the product page and the in-app update check depend on.

## Session 104 (2026-10-17) - Bilingual UI (EN/VI) + tray update check

Branch `feat/app-i18n` (worktree `volume-control-app-i18n`, forked from
`main` at 54e7e5a) implements the user's all-in i18n request plus the tray
update check:

- `config.ini [general] language = en|vi`; `Language` enum, strict INI
  parse/write, `SettingsPatch.language` validation; new `crate::locale`
  tables drive the Windows tray, overlay labels/toasts, Win32 mixer
  (title/UIA labels, relabeled on show), Win32 settings header/nav/sections,
  and GTK/macOS mixer controls. `update_settings` relabels trays immediately.
- `crate::updates` (ureq, GitHub `releases/latest`, pure `is_newer`) plus
  `TrayCommand::CheckUpdates` in both hosts; Tauri fetches off the core lock
  and toasts the localized result.
- Frontend `lib/i18n.tsx` (typed dictionaries, provider, `useT`) localizes
  Mixer, all Settings sections + shell, Help, and Overlay; General carries
  the language selector. tsc clean; vitest 17 files/105 tests.
- PowerShell bootstrap carries a 23-ID EN/VI message table with
  `VOLUMECONTROL_LANG`/culture selection (EN byte-identical, BOM added for
  PS 5.1 decoding). Suite PASS under EN and VI.
- Note: the two `scripts/*.ps1` files do not exist on `main` yet; they were
  copied from the unmerged `feat/windows-release-bootstrap` branch so the
  bootstrap i18n lands with this feature. Merge order matters: PR #42
  (release bootstrap) first, then this branch.
- Verified: cargo tests, clippy `-D warnings`, fmt, tsc, vitest, PS suite,
  and a real-network run of the update check (EN + VI output).
- REA/performance: `VOLUMECTL_STARTUP_TRACE=1` stage trace (ready 59-116 ms
  over 6 debug runs; stage table + re-measure recipe in
  `docs/performance.md`), immediate "Checking for updates…" overlay card
  before the worker fetch, and REA artifact evidence (rust-auditable +
  PE-resource lanes; decompiler/layout/objdump lanes unavailable on this
  host).
- CI follow-up: the Linux/macOS smoke binaries now pass `Language::En` to the
  new renderer signatures and the macOS host handles `AppAction::CheckUpdates`
  (both cfg-gated to their platforms; CI is their compile check).

## Session 102 (2026-10-17) - Windows release bootstrap Task 4

Task 4 (uncommitted): both READMEs gained a Windows install section with the
safe local download-and-verify sequence (separate `Invoke-WebRequest`
statements, `Get-FileHash` check against `SHA256SUMS.txt` before execution),
`-Action install` / `-Action update` / `-Action update -Version` /
`-Action uninstall` / `-Silent` examples, and explicit trust limits
(unsigned releases, checksum is not code signing, possible SmartScreen
warning, `%LOCALAPPDATA%\VolumeControl` root, no forced update termination,
config preserved at `%APPDATA%\volume-control\config.ini`). Six doc-contract
harness checks were red before the docs and green after. Remote-command
surface added: `docs/index.html` (irm/curl/winget × install/update/uninstall,
no pipe-into-shell anywhere) and `scripts/generate-winget-manifest.sh`
(hash-true winget-pkgs trio for `dt418.VolumeControl`). Current counts: 54
recorded features (12 in_progress), harness reports 36 `ok` checks, bootstrap
PowerShell suite exits `PASS`. Full pre-commit gate green: fmt,
`git diff --check`, clippy `-D warnings`, `cargo test --workspace
--no-default-features`, `test-check-records`, `test-format-lint`, `test-ship`
all exit 0. Enable GitHub Pages for this repo with source **Deploy from a
branch → `/docs`** to publish the page at
https://dt418.github.io/volume-control/.

## Session 101 (2026-10-17) - Windows release bootstrap Task 3

Task 3 (uncommitted): `.github/workflows/desktop-validation.yml` builds
`--bundles nsis` on windows (else `--no-bundle`) and writes `artifacts[]`
metadata from the staged `dist/SHA256SUMS.txt` manifest instead of a
`find -print -quit` scalar. `.github/workflows/release.yml` verifies each
downloaded validation artifact, stages only manifest-declared files into a
flat `release-assets/` rejecting duplicate basenames, generates canonical
sorted `release-assets/SHA256SUMS.txt` (two-space, no CI paths, no overwrite
flag), and uploads all staged assets with `gh release create`/`gh release
upload`. Five new static harness assertions were red before the workflow
edits and green after.

Parent verification green (Git Bash):

```bash
bash scripts/test-release-workflow.sh
```

Also required for the pre-commit hook (pre-existing on main, clippy 1.98):
added `clippy.toml` `msrv = "1.82"` matching `Cargo.toml` so the
`chunks_exact_to_as_chunks` lint honors MSRV (project keeps `chunks_exact`;
`as_chunks` is 1.88+) and replaced two `is_multiple_of(2)` (1.87 API) calls
in `crates/volumectl/src/autostart.rs` with `byte_len % 2 != 0`. fmt and
clippy `-D warnings` both exit 0 after the fix.

## Session 100 (2026-10-17) - Windows release bootstrap Task 2

Task 2 (uncommitted): `scripts/package.sh` stages the Windows NSIS installer
plus `Install-VolumeControl.ps1` into `dist/` and emits a sorted canonical
basename-only `SHA256SUMS.txt`; `scripts/verify-release-metadata.sh` parses
strict `artifacts: [{name, sha256}]` metadata in both Python and Node paths
and fail-closes on non-array, duplicate, path, hidden, missing, malformed
digest, duplicate manifest, or digest-mismatch inputs with exact platform sets
(macOS one zip, Ubuntu one tarball, Windows zip plus setup exe plus bootstrap).
Five new fixtures under `scripts/test-fixtures/` plus harness assertions cover
the windows array pass, ubuntu array pass, missing NSIS, duplicate checksum,
and bootstrap digest mismatch cases.

Parent verification green (Git Bash):

```bash
bash scripts/test-release-workflow.sh
```

## Session 099 (2026-10-17) - Windows release bootstrap Task 1

Task 1 files added, uncommitted: `scripts/Install-VolumeControl.ps1` and
`scripts/test-install-volumecontrol.ps1`. Bootstrap permits only
`%LOCALAPPDATA%\VolumeControl`; resolves stable or exact stable `vX.Y.Z`
GitHub releases; requires one versioned NSIS installer and one
`SHA256SUMS.txt`; accepts canonical basename-only SHA-256 records; verifies
downloaded installers before launch; waits for child success; removes temporary
downloads only after success; and refuses update while `VolumeControl` runs
without force-terminating it. Tests use an isolated temp root and copied
`cmd.exe` setup fixture for checksum, validation, child-argument, and
running-process behavior.

Parent verification green (uncommitted changes):

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/test-install-volumecontrol.ps1
```

## Session 072 (2026-08-15) - Volume and mute regression coverage

`crates/volumectl/tests/host_core.rs` now covers the complete system action
matrix: +/-1%, +/-10% large steps, boundary clamping, reset 50%, and two-way
mute transitions. The fake backend was corrected to toggle its mute bit so the
test exercises the same contract as Windows WASAPI/PulseAudio/CoreAudio.

The Tauri `toggle_mute` command now calls a fallible `AppCore::toggle_mute`
method, publishes the backend-returned state directly, and returns endpoint
errors to the frontend instead of silently swallowing them.

Windows WASAPI operations now initialize COM on the thread that performs each
read/write; the backend no longer stores a startup-thread guard that could be
moved and dropped on a different thread.

AppCore now enforces mute as an audible-zero operation (native mute flag plus
scalar volume 0) and restores the saved scalar on unmute. Direct positive
volume and reset actions explicitly clear the mute fallback.

The focused mixer E2E asserts the scalar slider sequence `50 -> 0 -> 50`, not
just the frontend “Muted” label.

The frontend now registers volume/session listeners before bootstrap and uses
an event revision to prevent an older bootstrap response from overwriting a
newer mute result. `MixerSurface.test.tsx` covers this race explicitly.

Settings E2E now waits for the actual General inputs before editing, avoiding
the separate shell-ready/form-ready timing race observed in the full matrix.

`e2e/tauri/specs/mixer.e2e.ts` now waits for the actual `state://volume` result
and asserts Mute -> Unmute -> Mute accessibility state. The full Windows ship
gate passed before this assertion was added; a focused rerun later hit an
environmental WebDriver startup refusal on port 4445 and did not reach the
application, so hosted CI must provide the final E2E evidence.

## Session 073 (2026-08-15) - Hosted E2E audio fallback

The failed PR run was investigated from the Ubuntu and Windows job logs. Both
runners lacked a default audio endpoint, so the intentional `UnavailableAudio`
fallback returned an error from `toggle_mute`; the frontend therefore kept the
truthful unmuted label. This was an environment limitation, not a regression
in the real backend path.

`volumectl::audio::E2eAudio` is now a debug-only in-memory backend with a unit
test for the scalar/mute round trip. `src-tauri` selects it only when the debug
marker and `VOLUMECTL_E2E_AUDIO=virtual` are both set. The Tauri debug E2E
wrapper supplies that marker, making hosted Linux/Windows IPC/event/UI checks
deterministic without weakening production behavior or release contents.

## Session 074 (2026-08-15) - FE/BE connection diagnosis

The FE→BE bridge is wired correctly: `ipc.ts` delegates to Tauri core invoke
and event APIs, `commands.rs` registers `toggle_mute`/`set_volume`/reset, and
`TauriSink` emits `state://volume`. The observed “connection lost” symptom was
the audio backend returning an endpoint/init error; `SystemOutputRow` caught it
with an empty handler, so users saw no explanation and no state change.

`MixerSurface` now passes an error callback into the system output row. Mute
and reset rejections render an accessible status notice while retaining the
last confirmed backend state. The new React regression test proves that a
failed mute does not fabricate a Muted label.

## Session 075 (2026-08-15) - WDIO debug single-instance guard

The ship gate also exposed a separate Windows harness failure: the Tauri app
exited with code 0 before the embedded WebDriver endpoint was ready because
the production single-instance mutex rejected a debug WDIO process. The guard
now bypasses that mutex only when `debug_assertions` and
`VOLUMECTL_E2E_DEBUG=1` are both true. Release builds and ordinary launches
retain the original single-instance protection.

## Session 076 (2026-08-15) - Tray menu commands + tray-host lifecycle

Two independent bugs made every tray menu item appear dead:

1. Event routing: Tauri's runtime installs its own global `muda` menu-event
   handler, which by design stops sending events to
   `MenuEvent::receiver()`. The old 150 ms poll drained an empty channel.
   The host now registers `Builder::on_menu_event` and dispatches the menu id
   through `TrayCommand::from_menu_id` into `AppCore::handle_action`.
2. Lifecycle: Tauri exits when the last webview surface closes. Opening
   mixer/settings/help and then closing the window killed the whole app, so
   the tray vanished. `RunEvent::ExitRequested` is now prevented unless the
   tray Exit command set `EXIT_REQUESTED` first.

Also: `AppCore` publishes its initial confirmed state at construction so the
tray label shows the real volume immediately, and a unit test covers the
startup publish. Verified on the Windows release build via UI Automation:
Mute/Reset update the label, Open mixer/Settings/Help open and the host stays
alive after each close, Reload keeps the host alive, Open config file opens
the editor dialog, and Exit terminates the process.

## Session 077 (2026-08-15) - Webview surfaces unresponsive in release

Two root causes made Help/Settings appear completely dead in release builds:

1. Missing `custom-protocol` feature. `tauri/build.rs` sets `cfg(dev)` when
   the feature is absent, so even release binaries resolved every
   `WebviewUrl::App` to `http://localhost:1420`. With no dev server running,
   webviews showed ERR_CONNECTION_REFUSED, frontend JS never ran, and
   `surface_ready` never showed the window. Fix: `tauri` features now include
   `custom-protocol`.
2. Main-thread deadlock. Surface commands were sync; Tauri ran them inside an
   async task on the main thread, and `WebviewWindowBuilder::build()` (wry)
   needs the event loop message pump — so creating Settings from the Help
   footer hung forever and the whole app became laggy/unresponsive. Fix:
   `open_surface`/`close_surface`/`surface_ready` are async and marshal the
   window work through `run_on_main_thread` with an async channel;
   `WindowManager::on_main` does the same for tray/poll-thread callers with a
   10s timeout.

Also fixed: PowerShell single-surface E2E evidence (scalar `switch` splatted
as characters) and added an E2E regression clicking Help footer Settings.
Verified on Windows release: Help search filters, Help footer Settings opens
the Settings surface, Edit config opens Notepad, Close closes Help, and the
host stays alive; `verify-tauri-e2e.ps1 -Surface help` passes 3/3 including
evidence.

## Session 078 (2026-08-15) - Deadlock guard skill + post-merge review

PR #27 (custom-protocol + async surface commands) merged to main with every
GitHub check green. Follow-ups:

- New skill `tauri-deadlock-guard` (mirrored to `.claude/skills/`) documents
  the invariants and verification workflow; new guard scripts
  `scripts/check-tauri-deadlock.sh` / `.ps1` enforce them automatically and
  run as a CI step in the `checks` job. AGENTS.md/CLAUDE.md reference both.
- `WindowManager::open` recreates a surface when the active set says it is
  open but the native window is gone (was a silent no-op).
- Windows tray overflow flyout is dismissed before showing any surface;
  previously it stayed open after a menu click and swallowed clicks over the
  bottom-right Help/Mixer area. Verified via `WindowFromPoint` + real clicks.

Review notes (not yet changed): WASAPI `get_state` runs every 150 ms on the
slow poll thread (COM STA init per call) — a callback/event-driven audio
notification would cut idle wakeups; `get_bootstrap` enumerates sessions on
the main-thread IPC path; frontend mixer bundle (~157 kB JS) is dominated by
framer-motion/lucide. These are tracked as follow-ups, not regressions.

## Session 071 (2026-08-15) — Project-scoped Claude safe-flow hook

`.claude/settings.json` now wires `.claude/hooks/agent-safe-flow.sh` as a Bash
`PreToolUse` hook. It blocks direct pushes, destructive git operations,
verification bypasses, and admin merge bypasses. The supported deletion paths
are `git branch -d <merged-local-branch>` and
`gh pr merge <PR> --merge --delete-branch`; `main` remains protected by the
strict `Release gate (required)` check.

The contract runner `scripts/test-agent-safe-flow.sh` passes blocked/allowed
cases, malformed input, documented git global-option/force-flag variants,
restore/checkout path forms, short `-n`, executable-bit, and exact settings-wiring checks under
Git Bash, including false-positive checks for safe checkout and refs named
`push`. CI invokes this contract in the shared checks and Windows jobs, and
`test-ship.sh` also covers it; hook/settings diffs select the full platform
scope.

## Session 070 (2026-08-15) — Main branch protection

GitHub `main` is now protected with strict required check `Release gate
(required)`, admin enforcement, and force-push/deletion disabled. The gate
requires checks and Windows for every change, plus Linux/macOS whenever the
scope classifier selects them; release/tag validation remains a full matrix.

## Session 069 (2026-08-15) — Cost-balanced CI and Node runtime warning cleanup

PR #23 is merged, so the reusable `workflow-warning-auditor` skill is now on
`main`. Its official-action runtime baseline is `setup-node@v7+` and
`setup-python@v7+`; all live workflows use Node 24-capable majors and the
strict audit is clean.

`ci.yml` now classifies pull-request diffs. Documentation/tooling-only PRs
retain the lower-cost Windows/checks path, while runtime, UI, E2E, workflow,
release, and uncertain diffs run Linux/macOS too. Pushes to `main` always run
the full matrix. An always-running `release-gate` fails closed if checks or
Windows fail, or if selected Linux/macOS jobs fail; tag/manual release
validation remains a three-platform reusable workflow.

`scripts/test-ship.sh` covers the scope classifier and release-gate wiring.
Local audit/contract checks pass; the new workflow must still complete a
hosted PR/main run before its release-readiness claim is promoted.

## Session 068 (2026-08-15) — macOS config INI test normalization

The full `main` CI run exposed five macOS-only config INI assertions that used
Windows-style `.exe` expectations. Production parsing intentionally applies
`config::normalize_blacklist_entry`, which maps these entries to `.app` on
macOS. The tests now compare round-trip/save/migration/order results against
that public normalization contract.

## Session 067 (2026-08-15) — Reusable workflow warning auditor skill

Added `.agents/skills/workflow-warning-auditor` and whitelisted it as a
project-authored skill. It audits workflow YAML with PyYAML, reports actionable
Node runtime, mutable-ref, permissions, branch-trigger, and dangerous-trigger
findings, and supports strict JSON/text output. The skill and bundled script
validate successfully; the current workflows correctly report the setup-node
v4/setup-python v5 Node-runtime warnings for planned upgrades.

## Session 066 (2026-08-15) — Cross-platform E2E evidence path fix

CI on `main` showed Ubuntu and macOS WDIO specs passing but evidence validation
failing with a missing JUnit under `output/...` because the Bash wrapper changed
cwd to `e2e/tauri` while retaining a relative output root. The wrapper now
normalizes relative roots to absolute repository paths before the cwd change;
the ship smoke test asserts this contract.

## Session 065 (2026-08-15) — Default branch renamed to main

PR #21 merged successfully at `9cb0a66` after green format/lint, Windows E2E,
release-artifact, and security checks. GitHub now uses `main` as the default
branch; local `main` tracks `origin/main`, and the old `master` ref is gone.
Operational guard, ship, CI, skill-mirror, and handoff references now use
`origin/main`.

## Session 064 (2026-08-15) — Full verification and pre-push hardening

Fresh verification found and fixed a process-global config-test race and a
records-guard deletion bypass. The current branch has:

- shared poison-tolerant `CONFIG_DIR_LOCK` for all tests mutating
  `VOLUMECTL_CONFIG_DIR`;
- fail-closed staged/branch deletion detection for both audit records plus
  regression fixtures;
- canonical INI documentation in `CLAUDE.md` and current ship-test counts.

Evidence: Rust workspace test groups all pass (333 total: 280 library + 12
Tauri + 4 session + 21 host integration + 16 host suite), frontend 89/89 +
production build, fmt/diff/clippy, Linux/macOS `volumectl` target checks
(Linux GTK feature included), records/format-lint/ship/release/Codex/E2E
contracts, and the real Windows auto-start registry restoration verifier all
pass. A clean `tauri build --no-bundle --ci` produced
`target/release/VolumeControl.exe`. Three-domain pre-push review is clean
after the fixes. Hosted desktop/release matrix still remains the authoritative
cross-platform artifact gate.

PR CI follow-up: removed a tracked `.superpowers/` report rejected by the diff
policy and fixed both E2E wrappers to run their evidence assertion from
`e2e/tauri`, where the isolated `tsx` dependency is installed.

## Session 063 (2026-08-15) — Auto-start and canonical INI integration

The active feature branch now contains the current Tauri-path auto-start and
INI work on top of merge commit `b740a28c`:

- Windows HKCU Run adapter plus `get_autostart`/`set_autostart` commands and
  an accessible immediate Settings switch with read-back and retry behavior.
- Typed canonical `config.ini` persistence with strict parsing, atomic writes,
  legacy JSON backup migration/recovery, malformed-edit preservation, and
  visible Storage notices; Settings remains the primary typed editor.
- `scripts/verify-autostart.ps1` has a native Windows run recorded: enable,
  read-back, disable cleanup, restoration, and unrelated Run-value checks
  pass without launching the binary. Linux/macOS target checks remain hosted-CI
  work because this Windows host lacks their native system dependencies.

Local Windows verification: Rust fmt/diff/clippy/workspace tests pass;
frontend Vitest 15 files / 89 tests and the production build pass. Before
publishing, run the Windows registry restoration verifier and hosted
Windows/Linux/macOS release matrix, then inspect SHA-bound artifacts.

## Session 061 (2026-08-15) — Cross-platform testing and release-safety plan

Plan `docs/superpowers/plans/2026-08-15-cross-platform-testing-release-safety.md`
is complete through local Task 7 review. The branch now has reproducible
coordinator depth/config contracts, fail-closed WDIO evidence with explicit
providers, balanced Windows/Linux/macOS CI scheduling, SHA-bound reusable
desktop release artifacts, tag-to-`github.sha` preflight binding (including
annotated tags), and fail-closed JUnit/manifest/timings/platform-log assembly.
The cross-platform checklist documents Windows interactive hotkey latency,
Ubuntu/Xvfb, WSLg, hosted macOS, package inspection, and ad-hoc versus future
Developer ID/notarization evidence boundaries.

Local verification on Windows:

- `cargo fmt --all --check`, `git diff --check`, clippy with `-D warnings`, and
  `cargo test --workspace --no-default-features` pass (308 tests across the
  workspace crates and host suites).
- `npm test --prefix frontend` passes 15 files / 84 tests; frontend build passes.
- E2E support/type/contract/production-exclusion/Pilot contracts pass; the
  official `npm run test:e2e:debug --prefix e2e/tauri -- --surface all` passes
  all 6 surfaces / 11 tests with runtime evidence. The raw `test:e2e` script
  is a low-level WDIO command and requires the debug wrapper lifecycle.
- `bash scripts/test-ship.sh`, `bash scripts/test-release-workflow.sh`,
  `python scripts/test-codex-config.py`, and the staged records guard pass.
- `scripts/verify-hotkey-latency.ps1 -?` is usage-only verified; real native
  latency evidence remains a Windows interactive/manual release step.

Hosted Ubuntu/macOS/Windows workflow runs, artifact inspection, and a
disposable release rehearsal remain required before marking cross-platform
features as passing or publishing a release. Do not claim native Linux/macOS
hotkey, tray, hardware-audio, Wayland, TCC/Accessibility, Retina/multi-monitor,
or Gatekeeper coverage from the local Windows run.

## Session 051 (2026-08-15)

README.md and README.vi.md now describe the native-first Tauri hybrid UI,
Settings-first configuration, per-action shortcut recording, platform status,
and the safe frontend/E2E commands. Added CHANGELOG.md for the unreleased
feature set. CI/Release workflows now explicitly install/build the frontend
and use `tauri build --no-bundle --ci` for FE+BE artifacts. The final full local verification is being rerun before pushing
the branch to a PR targeting the repository default branch `main`; hosted
Linux/macOS jobs remain the cross-platform release evidence.

## Session 050 (2026-08-15)

Mixer resilience and readability pass completed. `get_bootstrap` and the
native fast/slow poll loops recover poisoned mutex guards instead of leaving a
backend error stuck; the webview now shows an actionable connection alert with
Retry rather than `Backend unavailable`. Mixer glass/card/control alpha,
borders, labels, and focus rings were strengthened for readable contrast.
Frontend 15/84, Rust workspace, build, recovery, Mixer, and full Windows E2E
11/11 evidence pass. Hosted Linux/macOS CI remains outstanding.

## Session 049 (2026-08-15)

Implemented configurable global shortcut recording. Settings now renders one
Record/Clear row per action, captures portable modifier + key combinations,
supports disabling a shortcut, and keeps preset layouts for Ctrl+Alt/Alt/Ctrl.
The backend persists `Config.hotkeys`, validates malformed/duplicate bindings,
registers native shortcuts safely, and publishes `Disabled`/conflict status to
Help and Settings. Rust/frontend tests and the Windows WebDriver matrix pass;
hosted Linux/macOS CI remains the final cross-platform evidence.

## Session 048 (2026-08-15)

The Tauri WebDriver/Pilot integration now has Windows-first live evidence and
cross-platform CI/ship wiring. CI runs the WDIO matrix on Windows, Ubuntu under
Xvfb, and macOS; `scripts/ship.sh` runs the same fail-closed WDIO wrapper after
the frontend build and before the release Tauri build. Pilot remains local
diagnostic tooling only. Current local commits include `faea6e2` (Pilot), with
the CI/ship slice pending its next checkpoint commit.

Handoff after Session 040 (2026-08-13, global-hotkey migration + Hybrid Tauri UI:
`rdev` → `global-hotkey` 0.8.0 + 1% default step, commits `190d02b`..`HEAD`
on branch `feature/tauri-ui-hybrid`). All 30 features are passing on
Windows. **Wave 1 + Wave 2 of the ui-restoration-plan landed at `03cc22f`
(mixer SystemOutputRow + threshold SignalRail) and `392348a` (Settings full
config surface — six-section shell, draft lifecycle, blacklist/feedback/
thresholds/storage + legacy window geometry parity: mixer 400×224 bottom-right
above the overlay, settings 760×620 centered, help 520×500 bottom-right).**
Next: Wave 4 (verification + records: full battery + smoke — system row visible, settings all sections, blacklist add/remove end-to-end, help badges). **All four waves of the ui-restoration-plan are now complete (Wave 1 mixer SignalRail at `03cc22f`, Wave 2 settings full config + legacy geometry at `392348a`, Wave 3 help parity at `6777e91`, Wave 4 verification at `8e9a986`).** Outstanding follow-on items: tray-driven live open of Settings/Help on a real interactive desktop (this session had no Shell_TrayWnd); Linux/macOS host runtime verification on real machines (pre-existing); push the feature branch when ready.
on branch `feature/tauri-ui-hybrid`). All 30 features are passing on
Windows.

## Where we are

- All plan tasks 1-14 complete: shared tokens/contracts (1-2), Windows
  primitives (3), Signal Rail (4), Windows overlay/mixer/Settings/Help/tray
  redesigns (5-9), Windows accessibility verification (10), macOS 26 AppKit
  renderer (11), Ubuntu 24.04 GTK4/libadwaita renderer (12), CI + release
  packaging (13), final evidence + tracking (14).
- Windows is the fully implemented and live-verified target (Session 008
  matrix). macOS/Linux renderers implement the same Signal Glass surface
  contract behind the shared `NativeRenderer` bridge.
- **Enforcement stack hardened** (Sessions 011-033):
  - Format-lint gate toolchain (v3 manifest, both parsers, 40 checks on
    Windows / 39 on Linux/macOS with PowerShell; 26 without).
  - Mandatory ship flow (`scripts/ship.sh` + `scripts/ship.ps1`, 25 checks).
  - Three-domain pre-push review skill (guard core / gate chain / wiring).
  - Gate parser hardening (vol-017): numeric-only manifest version,
    case-sensitive forbidden paths, substantive `.claude/skills/*` JSON,
    fail-closed wiring assertions.
  - Records guard self-test hardened (Session 033): Windows-safe tmpdir
    (`mktemp` + `cygpath -w`), unclassified-path fail-closed case, `--staged`
    git-failure coverage; bash-gate version sed now full-line anchored.
- **Native host loops** (Sessions 015-017): macOS CoreAudio + AppKit
  event loop, Linux PulseAudio + GTK/X11 host loop. Both compile and
  smoke-test on CI (macos-15 arm64 + Ubuntu 24.04 under xvfb-run).
- **vol-011** is **`passing`** — automated verification script
  (`scripts/verify-vol011.ps1`) confirms all 6 checks: high-contrast,
  reduced-motion, DPI scaling, work-area placement, backdrop/acrylic, tray
  menu. PrintWindow-based screenshot capture for layered/transparent windows
  added in Session 022.
- **vol-018** is **`passing`** — verify script tooling with PrintWindow
  screenshot capture.
- Unit suite: **285 passed / 0 failed** (249 volumectl + 12 host_core + 16 linux_host_core + 4 window_manager + 4 commands).
  `cargo fmt --all --check` passes. Clippy `-D warnings` clean.
  Windows build clean (0 warnings). Cross-checks clean for macOS and Linux
  (GTK4/libadwaita) with the pkg-config stub env.

## What still needs doing (follow-on)

1. **Host wiring runtime verification** — audio backends compile and smoke-test
   on CI. **Remaining** (need native system services, out of scope for a
   Windows-hosted session): Linux/macOS **tray** and the **renderer host event
   loop** on real desktops (AppKit/GTK binders are implemented; runtime
   evidence needs a real machine). Global hotkeys were migrated to native
   registration (`global-hotkey`: RegisterHotKey / Carbon / X11) in Session
   039; Windows hotkeys are runtime-verified by the existing host, Linux/macOS
   hotkey runtime verification on real desktops is still outstanding.
2. **Optional**: add `libgtk-4-layer-shell-dev` install to the Ubuntu CI job if
   it ever appears in noble repos, to get a real layer-shell compile+smoke
   (currently skipped by design).
3. **Session handoff hygiene**: refresh this document after each substantive
   landing (Session 033: Linux canvas gtk-rs 0.19 API-compat fixes +
   libpulse-sys direct dep + enforcement-stack hardening).

## Enforcement stack self-test counts

| Self-test | Checks | Notes |
|---|---|---|
| `test-check-records.sh` | 33 | Windows; Linux/macOS same |
| `test-format-lint.sh` | 40 Windows / 39 Linux-macOS | WSL-shim check is Windows-gated |
| `test-ship.sh` | 25 | |

## Verification commands (Windows host)

```

# Full format-lint gate (includes tests)
bash scripts/format-lint.sh

# PowerShell gate
powershell -ExecutionPolicy Bypass -File .agents/skills/format-lint/scripts/format-lint.ps1

# Self-tests (run under Git Bash, not WSL shim)
bash scripts/test-check-records.sh
bash scripts/test-format-lint.sh
bash scripts/test-ship.sh

# Records guard
sh scripts/check-records.sh --branch origin/main

# Ship flow (dry run)
bash scripts/ship.sh --dry-run

# vol-011 verification
powershell -ExecutionPolicy Bypass -File scripts/verify-vol011.ps1 -Release
```

## Verification commands (macOS/Linux native)

```
cargo fmt --all --check
cargo build
cargo test --workspace --no-default-features

# macOS cross-check from Windows:
cargo check --target x86_64-apple-darwin -p volumectl --tests

# Linux cross-check from Windows (needs libpulse-dev on CI):
cargo check --target x86_64-unknown-linux-gnu -p volumectl --tests
```

## Hygiene notes

- Do not stage or commit `.claude/settings.local.json`, `.superpowers/`,
  `.plans/`, `.pi/`, runtime config.json, scratch scripts, `target/`, or
  `dist/`.
- The pkg-config probe stub at `%TEMP%\rtk-stub-bin\pkg-config.cmd` is an
  environment shim for cross-checks from Windows, not a repo artifact. Set
  `PKG_CONFIG` to it and `PKG_CONFIG_ALLOW_CROSS=1` for cross-target checks.
- Run self-tests under Git Bash (`C:\Program Files\Git\bin\bash.exe`), not
  the WSL shim (`System32\bash.exe`). The WSL shim skips PowerShell checks.
- claude-progress.md Session 008 holds the live Windows verification matrix;
  Session 009 holds the renderer/CI evidence; Session 022 holds the
  verification script tooling + screenshot capture; Session 033 holds the
  Linux canvas gtk-rs 0.19 fixes and enforcement-stack hardening.
- Do not rewrite `claude-progress.md` with PowerShell `Set-Content -Encoding
  UTF8` — it double-encodes em-dashes and adds a BOM. Prefer the edit tool.

## Model dispatch policy (Session 040)

- **Deep reasoning / investigation / review**: `openai-codex/gpt-5.6-luna` (xhigh) → escalate `gpt-5.6-terra` (high) → `gpt-5.6-sol` (medium) if the previous cannot solve it. ALWAYS `context: 'fresh'` for codex models (their context window overflows with a forked long session).
- **Implementation / mechanical fixes**: `opencode-go/deepseek-v4-flash` (fast/cheap).
- **Workflow**: investigate + plan with the capable model first, implement with flash.
- **Web research**: `pi-web-access` extension (web_search/fetch_content/get_search_content) + `@upstash/context7-pi` (resolve-library-id/query-docs) — enable via `extensions: ['npm:pi-web-access']` + `tools: [...]` on the subagent launch.
- Extensions installed this session: `npm:pi-web-access`, `npm:@upstash/context7-pi`.

## Session 108 (2026-10-17) - Stale main reconciled + smoke lock release

Branch `fix/format-lint-lock-release` (worktree
`volume-control-format-lint-lock`, forked from `main` at ac26346).

- The scheduled resume run found its `volume-control` checkout 6 commits
  behind `origin/main` with superseded uncommitted work: a tray "Check for
  updates" item that opened the releases page (the feature is on `main` in a
  fuller form - ureq fetch against `releases/latest` plus a localized overlay
  result, PR #43 / vol-090) and an untracked 2026-10-16
  windows-release-bootstrap plan/spec (implemented by PR #42, vol-087..vol-092).
  That change set is preserved in `git stash@{0}`; `main` is fast-forwarded to
  ac26346 and the workspace gate is green again
  (`cargo test --workspace --no-default-features`).
- `scripts/test-format-lint.sh` released its concurrency lock only after its
  hermetic preconditions, so a refused run wedged every later invocation into
  "already running" until the lock directory was removed by hand. The release
  trap is installed with the lock now, and check 10 of the suite proves a
  refused run releases it.
- Next: open the PR for `fix/format-lint-lock-release`; PR #45 is redundant
  after that (same trap fix as `e6a67e6`, and its HEAD-resolution hunk is
  already on `main` through #44's squash).
- Running this suite needs Git Bash and a built `frontend/dist`: the PATH
  `bash` on this host is the WSL shim, which cannot read a Windows-form
  worktree gitdir, and the gate's clippy step compiles
  `tauri::generate_context!`.
