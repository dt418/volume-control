# Session Handoff

Handoff after Task 14 of the Signal Glass production UI plan (2026-08-03,
executed 2026-08-04), refreshed after the CI wrap-up (PR #3 merged) and then
again after Session 010 (Linux + macOS audio backends, commit `0a27bfa`).

## Where we are

- All plan tasks 1-14 complete: shared tokens/contracts (1-2), Windows
  primitives (3), Signal Rail (4), Windows overlay/mixer/Settings/Help/tray
  redesigns (5-9), Windows accessibility verification (10), macOS 26 AppKit
  renderer (11), Ubuntu 24.04 GTK4/libadwaita renderer (12), CI + release
  packaging (13), final evidence + tracking (14).
- Windows is the fully implemented and live-verified target (Session 008
  matrix). macOS/Linux renderers implement the same Signal Glass surface
  contract behind the shared `NativeRenderer` bridge.
- **First CI run green and merged**: PR #3 (branch
  worktree-signal-glass-production-ui) merged into master as 68baeac on
  2026-08-04. All four jobs passed — Format/diff gate, macOS (build+tests+
  `appkit smoke OK` on macos-15 arm64), Windows (220/220 + release
  artifact), Ubuntu 24.04 (`gtk smoke OK` under xvfb-run). layer-shell
  build skipped with a documented annotation (`libgtk-4-layer-shell-dev`
  not in noble repos).
- The two first-run CI failures were main-thread/harness issues, fixed by
  moving the renderer smoke tests into harness-free `[[test]]` binaries
  (`tests/appkit_smoke.rs`, `tests/gtk_smoke.rs`, `harness = false`; commit
  7301236) whose `main()` runs on the process main thread. The GCD
  block-ABI apparatus (which failed to link on arm64) was deleted.
- vol-011 (area `adaptive-ui`, priority 11) is recorded in feature_list.json
  as **`in_progress`** — intentionally NOT `passing`. Human-only verification
  remains (see below).
- Unit suite: 220 passed / 0 failed. `cargo fmt --all --check` passes.
  Windows build clean (0 warnings). `cargo check --target
  x86_64-apple-darwin -p volumectl --tests` and all three Linux combos
  (`x86_64-unknown-linux-gnu`, no features / `gtk-renderer` /
  `gtk-renderer,layer-shell`, all with `--tests`) are clean with 0 warnings.
- Recent commits (this worktree, all merged): `7301236 fix: run AppKit/GTK
  smoke tests as harness-free main-thread binaries`, `e79ca23 fix: compile
  macOS/Ubuntu renderer test harnesses on CI`, `docs: record Signal Glass
  production verification`, `ci: add cross-platform UI build matrix and
  packaging`, `feat: add Ubuntu 24.04 Signal Glass renderer`, `feat: add
  macOS 26 Signal Glass renderer`.

## What still needs doing (follow-on)

1. **Human visual confirmation** (required before vol-011 can go `passing`):
   high-contrast mode, reduced-motion, 125%/150% DPI, taskbar/secondary-monitor
   work-area changes, backdrop/acrylic look, tray-menu clicks. All need an OS
   setting change + app relaunch (capabilities are snapshotted at startup).
2. **Host wiring for macOS/Linux** — the **audio backends** are done (Session
   010, commit `0a27bfa`): `audio_linux.rs` (PulseAudio), `audio_macos.rs`
   (CoreAudio), both behind the shared `AudioBackend` trait via
   `audio::default_backend()`, and `cli.rs` now routes `get`/`set`/`mute`
   through the trait. macOS cross-checks clean on Windows; Linux compiles on the
   Ubuntu CI job (`libpulse-dev`); runtime volume/mute still needs a real
   desktop. **Remaining** in host wiring (runtime-verified only, need native
   system services — out of scope for a Windows-hosted session): Linux/macOS
   **tray**, **global hotkeys**, and the **renderer host event loop** that binds
   the AppKit/GTK renderers.
3. **Optional**: add `libgtk-4-layer-shell-dev` install to the Ubuntu job if
   it ever appears in noble repos, to get a real layer-shell compile+smoke
   on CI (currently skipped by design).

## Verification commands (Windows host)

```
PATH="/c/Users/Thanh/.cargo/bin:$PATH" cargo fmt --all --check
PATH="/c/Users/Thanh/.cargo/bin:$PATH" cargo build
PATH="/c/Users/Thanh/.cargo/bin:$PATH" cargo test -p volumectl
PATH="/c/Users/Thanh/.cargo/bin:$PATH" cargo check --target x86_64-apple-darwin -p volumectl --tests
PATH="/c/Users/Thanh/.cargo/bin:$PATH" PKG_CONFIG=/tmp/rtk-stub-bin/pkg-config PKG_CONFIG_ALLOW_CROSS=1 \
  cargo check --target x86_64-unknown-linux-gnu -p volumectl --tests
# + the same Linux line with --features gtk-renderer and
#   --features gtk-renderer,layer-shell (all must be 0 warnings)
```

## Hygiene notes

- Do not stage or commit `.claude/settings.local.json`, `.superpowers/`,
  runtime config.json, scratch scripts, `target/`, or `dist/`.
- The pkg-config probe stub at `/tmp/rtk-stub-bin/pkg-config.cmd` is an
  environment shim for cross-checks from Windows, not a repo artifact.
- claude-progress.md Session 008 holds the live Windows verification matrix;
  Session 009 holds the renderer/CI evidence (including the green first
  CI run).
