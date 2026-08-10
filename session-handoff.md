# Session Handoff

Handoff after Session 020 (gate parser hardening + three-domain review,
commit `6eedc7b`). The enforcement stack is now adversarial-review-hardened
and all 17 features are recorded in feature_list.json.

## Where we are

- All plan tasks 1-14 complete: shared tokens/contracts (1-2), Windows
  primitives (3), Signal Rail (4), Windows overlay/mixer/Settings/Help/tray
  redesigns (5-9), Windows accessibility verification (10), macOS 26 AppKit
  renderer (11), Ubuntu 24.04 GTK4/libadwaita renderer (12), CI + release
  packaging (13), final evidence + tracking (14).
- Windows is the fully implemented and live-verified target (Session 008
  matrix). macOS/Linux renderers implement the same Signal Glass surface
  contract behind the shared `NativeRenderer` bridge.
- **Enforcement stack hardened** (Sessions 011-020):
  - Format-lint gate toolchain (v3 manifest, both parsers, 38 checks on
    Windows / 37 on Linux/macOS).
  - Mandatory ship flow (`scripts/ship.sh` + `scripts/ship.ps1`, 22 checks).
  - Three-domain pre-push review skill (guard core / gate chain / wiring).
  - Gate parser hardening (vol-017): numeric-only manifest version,
    case-sensitive forbidden paths, substantive `.claude/skills/*` JSON,
    fail-closed wiring assertions.
- **Native host loops** (Sessions 015-017): macOS CoreAudio + AppKit
  event loop, Linux PulseAudio + GTK/X11 host loop. Both compile and
  smoke-test on CI (macos-15 arm64 + Ubuntu 24.04 under xvfb-run).
- vol-011 (area `adaptive-ui`, priority 11) is **`in_progress`** — human-only
  verification remains (see below).
- Unit suite: **225 passed / 0 failed**. `cargo fmt --all --check` passes.
  Windows build clean (0 warnings). Cross-checks clean for macOS and Linux
  (GTK4/libadwaita).

## What still needs doing (follow-on)

1. **Human visual confirmation** (required before vol-011 can go `passing`):
   high-contrast mode, reduced-motion, 125%/150% DPI, taskbar/secondary-monitor
   work-area changes, backdrop/acrylic look, tray-menu clicks. All need an OS
   setting change + app relaunch (capabilities are snapshotted at startup).
2. **Host wiring runtime verification** — audio backends compile and smoke-test
   on CI. **Remaining** (need native system services, out of scope for a
   Windows-hosted session): Linux/macOS **tray**, **global hotkeys**, and the
   **renderer host event loop** on real desktops (AppKit/GTK binders are
   implemented; runtime evidence needs a real machine).
3. **Optional**: add `libgtk-4-layer-shell-dev` install to the Ubuntu CI job if
   it ever appears in noble repos, to get a real layer-shell compile+smoke
   (currently skipped by design).
4. **Hygiene follow-ups** (from pre-push review, non-blocking):
   - Renormalize `.githooks/pre-commit` worktree copy to LF (pre-existing
     CRLF from git autocrlf; committed blob is correct).
   - Mirror `.claude/skills/format-lint/scripts/format-lint.sh` into
     `.agents/skills/format-lint/scripts/` for full directory-level parity
     (pre-existing asymmetry, smoke test only checks ps1 pair + sh-vs-root).

## Enforcement stack self-test counts

| Self-test | Checks | Notes |
|---|---|---|
| `test-check-records.sh` | 30 | Windows; Linux/macOS same |
| `test-format-lint.sh` | 38 Windows / 37 Linux-macOS | WSL-shim check is Windows-gated |
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
  runtime config.json, scratch scripts, `target/`, or `dist/`.
- The pkg-config probe stub at `/tmp/rtk-stub-bin/pkg-config.cmd` is an
  environment shim for cross-checks from Windows, not a repo artifact.
- Run self-tests under Git Bash (`C:\Program Files\Git\bin\bash.exe`), not
  the WSL shim (`System32\bash.exe`). The WSL shim skips PowerShell checks.
- claude-progress.md Session 008 holds the live Windows verification matrix;
  Session 009 holds the renderer/CI evidence; Session 020 holds the
  enforcement hardening + three-domain review.
