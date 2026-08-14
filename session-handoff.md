# Session Handoff

Handoff after Session 040 (2026-08-13, global-hotkey migration + Hybrid Tauri UI:
`rdev` → `global-hotkey` 0.8.0 + 1% default step, commits `190d02b`..`HEAD`
on branch `feature/tauri-ui-hybrid`). All 30 features are passing on
Windows. **Wave 1 + Wave 2 of the ui-restoration-plan landed at `03cc22f`
(mixer SystemOutputRow + threshold SignalRail) and `392348a` (Settings full
config surface — six-section shell, draft lifecycle, blacklist/feedback/
thresholds/storage + legacy window geometry parity: mixer 400×224 bottom-right
above the overlay, settings 760×620 centered, help 520×500 bottom-right).**
Next: Wave 4 (verification + records: full battery + smoke — system row visible, settings all sections, blacklist add/remove end-to-end, help badges).
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
sh scripts/check-records.sh --branch origin/master

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
