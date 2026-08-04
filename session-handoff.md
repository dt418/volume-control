# Session Handoff

Handoff after Task 14 of the Signal Glass production UI plan (2026-08-03,
executed 2026-08-04).

## Where we are

- All plan tasks 1-14 complete: shared tokens/contracts (1-2), Windows
  primitives (3), Signal Rail (4), Windows overlay/mixer/Settings/Help/tray
  redesigns (5-9), Windows accessibility verification (10), macOS 26 AppKit
  renderer (11), Ubuntu 24.04 GTK4/libadwaita renderer (12), CI + release
  packaging (13), final evidence + tracking (14).
- Windows is the fully implemented and live-verified target (Session 008
  matrix). macOS/Linux renderers implement the same Signal Glass surface
  contract behind the shared `NativeRenderer` bridge; runtime/CI evidence is
  pending the first GitHub Actions run.
- vol-011 (area `adaptive-ui`, priority 11) is recorded in feature_list.json
  as **`in_progress`** — intentionally NOT `passing`. Human-only verification
  remains (see below).
- Unit suite: 220 passed / 0 failed. `cargo fmt --all --check` passes.
  Windows build clean (0 warnings). `cargo check --target
  x86_64-apple-darwin` and `cargo check --target x86_64-unknown-linux-gnu`
  (no features, via a pkg-config probe stub) both clean with 0 warnings.
- Recent commits (this worktree): `docs: record Signal Glass production
  verification` (this commit), `ci: add cross-platform UI build matrix and
  packaging`, `feat: add Ubuntu 24.04 Signal Glass renderer`, `feat: add
  macOS 26 Signal Glass renderer`.

## What still needs doing (follow-on)

1. **Push branch + open/merge the PR** so the new CI workflows run once.
   Watch the CI results, especially:
   - macOS job: AppKit smoke tests on a real macOS runner (first real
     runtime evidence for the renderer).
   - Ubuntu job: gtk-renderer build + `xvfb-run` smoke tests; layer-shell
     build depends on `libgtk-4-layer-shell-dev` being present in noble.
   - Windows job: full suite + release artifact validation.
2. **Human visual confirmation** (required before vol-011 can go `passing`):
   high-contrast mode, reduced-motion, 125%/150% DPI, taskbar/secondary-monitor
   work-area changes, backdrop/acrylic look, tray-menu clicks. All need an OS
   setting change + app relaunch (capabilities are snapshotted at startup).
3. **Host wiring for macOS/Linux** (hotkeys, audio backends, tray) is
   follow-on work; the renderers are surface scaffolding + smoke tests only.

## Verification commands (Windows)

```
PATH="/c/Users/Thanh/.cargo/bin:$PATH" cargo fmt --all --check
PATH="/c/Users/Thanh/.cargo/bin:$PATH" cargo build
PATH="/c/Users/Thanh/.cargo/bin:$PATH" cargo test -p volumectl
PATH="/c/Users/Thanh/.cargo/bin:$PATH" cargo check --target x86_64-apple-darwin -p volumectl
PATH="/c/Users/Thanh/.cargo/bin:$PATH" PKG_CONFIG=/tmp/rtk-stub-bin/pkg-config PKG_CONFIG_ALLOW_CROSS=1 \
  cargo check --target x86_64-unknown-linux-gnu -p volumectl
```

## Hygiene notes

- Do not stage or commit `.claude/settings.local.json`, `.superpowers/`,
  runtime config.json, scratch scripts, `target/`, or `dist/`.
- The pkg-config probe stub at `/tmp/rtk-stub-bin/pkg-config` is an
  environment shim for cross-checks from Windows, not a repo artifact.
- claude-progress.md Session 008 holds the live Windows verification matrix;
  Session 009 holds the renderer/CI evidence.
