# Session Handoff

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
