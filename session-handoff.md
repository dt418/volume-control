# Session Handoff

Handoff after Session 033 (2026-08-11, Linux canvas gtk-rs 0.19 API-compat
fixes, commit `3e12b0b`). All 26 features are passing on Windows.

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
    Windows / 38 on Linux/macOS).
  - Mandatory ship flow (`scripts/ship.sh` + `scripts/ship.ps1`, 22 checks).
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
- Unit suite: **251 passed / 0 failed** (235 volumectl + 16 host-core).
  `cargo fmt --all --check` passes. Clippy `-D warnings` clean.
  Windows build clean (0 warnings). Cross-checks clean for macOS and Linux
  (GTK4/libadwaita) with the pkg-config stub env.

## What still needs doing (follow-on)

1. **Host wiring runtime verification** — audio backends compile and smoke-test
   on CI. **Remaining** (need native system services, out of scope for a
   Windows-hosted session): Linux/macOS **tray**, **global hotkeys**, and the
   **renderer host event loop** on real desktops (AppKit/GTK binders are
   implemented; runtime evidence needs a real machine).
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
| `test-format-lint.sh` | 39 Windows / 38 Linux-macOS | WSL-shim check is Windows-gated |
| `test-ship.sh` | 22 | |

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
  `.plans/`, runtime config.json, scratch scripts, `target/`, or `dist/`.
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
