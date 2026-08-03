# Progress Log

## Session 004 (2026-08-03) — Task 6: Signal Glass mixer redesign

- Goal: redesign the Windows mixer as the 400x224 Signal Glass precision card
  (spec §6): `VOLUME MIXER` eyebrow, `System output` caption, right-aligned
  28px live value, Signal Rail synchronized with the native trackbar, Mute /
  Reset to 50% buttons, 32px close target, two-layer focus ring, DPI scaling.
- Changed: `crates/volumectl/src/mixer.rs` (layout/paint/DPI/rail sync/focus
  ring), `crates/volumectl/src/app.rs` (thresholds seam), and
  `crates/volumectl/src/ui/platform/windows/primitives.rs` (canvas fix, below).

### Fresh build + test evidence
- `cargo fmt --all -- --check` → PASS.
- `cargo build` (via scripts/win-build.bat) → Finished dev profile, 0 warnings.
- `cargo test -p volumectl` → **173 passed; 0 failed; 0 ignored** (165 baseline
  + 8 new: DPI physical sizes 400x224/500x280/600x336, 16px physical gap at
  125/150%, spec layout rows, rail plan 0/50/100 + muted marker ≠ thumb, custom
  threshold boundaries, two-layer focus rings for every control, sync pushes
  state into the trackbar and keeps focus; plus the backdrop-canvas test).
- `git diff --check` → clean.

### Live-verified on Windows (100% DPI, dark system theme, probe-driven)
- Mixer rect [2140,1024,2540,1248] = **400x224**; overlay rect
  [2204,1264,2540,1352] = **336x88**; **vertical gap exactly 16px**, shared
  right edge (2540). (Mixer placement now consumes PHYSICAL sizes for both
  surfaces, so the gap holds at 125/150% by construction — unit-tested.)
- Mixer pixels at 50%: background 0x101319≈token 0x10131A, rail fill
  0x0078D4 (medium threshold, exact), thumb 0x171C24 (surface, exact), track
  0x344052 (border, exact); eyebrow/caption/value text rows all render.
- Rail ↔ trackbar mirror: posted VK_RIGHT to the native trackbar → `mixer
  hscroll: pos=51` → `mixer change: request=51%` → TBM_GETPOS 51 and the
  painted rail moved (pixel at (196,128) flipped thumb-cover 0x171C24 → fill
  0x0078D4). VK_HOME → 0, VK_END → 100, both via the host path, restored 50.
- Muted: rail fill 0x888888 (muted grey, exact), hollow diamond outline (24 px
  of 0xF5F7FA around an 0x888888 center — shape cue, not a filled copy),
  `Muted` grey label (98 glyph px), button flips to `Unmute`.
- Two-layer focus ring: Tab from the slider to Mute → 282 accent pixels +
  129 inner-contrast pixels in the ring band (both layers visible).
- Escape hides the mixer (visible=False) and the hotkey reopens it (True).
- Volume restored to its starting value after every probe run.

### Found + fixed (primitives canvas, live-verified)
- The mixer initially rendered a uniform grey surface: `ID2D1HwndRenderTarget`
  presents do NOT land in DWM-owned surfaces — the same class of problem the
  canvas already guarded against for layered windows. Extended the canvas's
  GDI gate to system-backdrop windows (`DWMWA_SYSTEMBACKDROP_TYPE` != NONE,
  new `backdrop_active`/`d2d_present_supported` helpers + test). After the
  fix, the acrylic mixer paints correctly via GDI (evidence above).

### Notes
- Native trackbar arrows move 1 tick (native behavior, unchanged from before);
  the rail mirrors the confirmed state exactly as the task requires.
- Button chrome renders light (0xF0F0F0) — A/B-verified identical on the
  pre-redesign binary (same `theme_controls`/DarkMode_Explorer path); not a
  regression from this task. Theming polish belongs to Task 10 verification.
- feature_list.json left untouched: vol-011 stays `in_progress`; the Signal
  Glass plan's Task 14 owns final feature-state updates.

## Session 003 (2026-08-03) — Task 13: Windows verification of the adaptive UI

- Goal: run the adaptive cross-platform UI through its paces on Windows (live
  desktop session at DPI 100%, Win11 25H2 build 26200, 2560x1440 primary) and
  record FRESH evidence, honestly separated into live-verified / build-test /
  needs-human-visual-confirmation.
- Built (this plan): shared adaptive tokens (theme/high-contrast/accent/
  material/motion), capability detection (DPI, work area, compositor, HC,
  reduced motion), placement math (overlay + mixer above it, 16px gap),
  adaptive overlay + mixer + native Settings + Help, host action/state bridge,
  hotkey-status exposure, tray Settings command, macOS/Linux renderer seams.
- What was built is described in the plan; this entry records the verification.

### Fresh build + test evidence
- `PATH="/c/Users/Thanh/.cargo/bin:$PATH" /c/Users/Thanh/.cargo/bin/cargo.exe clean -p volumectl`
  then `... cargo.exe build` → `Finished dev profile` in 9.24s, 0 warnings.
- `... cargo.exe test -p volumectl` → **94 passed; 0 failed; 0 ignored**
  (includes mixer/settings/overlay/ui token/placement/surface/theme/hotkey tests).
- `git diff --check` → clean.

### Live-verified on Windows (evidence gathered this session)
- App starts cleanly (no hotkey conflicts logged this run), tray icon present
  (found via UIAutomation in the overflow flyout: name "VolumeControl",
  NotifyItemIcon), hidden host + mixer + overlay + settings + help windows all
  created once at startup, process stable for the whole session (handles 240,
  working set ~21MB, no panics, no repeated errors).
- Mixer slider → volume path end-to-end: TBM_SETPOS+WM_HSCROLL synchronously →
  `mixer hscroll: pos=42` → `mixer change: request=42%` → `publish: state=42%`;
  slider readback + label converged to 42% (and a programmatic TBM_SETPOS with
  no drag is correctly reverted by the 150ms poll — `mixer sync` log).
- Mute/Reset buttons via WM_APP_MIXER_MUTE / WM_APP_MIXER_RESET: state 42%→
  muted=true (button text flips Mute→Unmute) → reset to 50% muted=false.
- Mixer + overlay geometry: mixer rect [2180,1094,2540,1272], overlay rect
  [2220,1288,2540,1352]; vertical gap exactly 16px (1272+16=1288), shared right
  edge (2540), no overlap. Placement math verified against the live work area:
  overlay bottom = work_area.bottom − 40 (1352 = 1392−40), right = 2560−20 =
  2540; settings centered at [990,378,1570,1014] = ((2560−580)/2, (1392−636)/2).
- Keyboard nav (scripted via posted WM_KEYDOWN through the subclass):
  mixer Tab moves focus mute→reset→close (GetGUIThreadInfo focusClass=Button,
  hwnd matches each control), Escape hides mixer, Space on the focused close
  button hides the mixer (BN_CLICKED path). Settings: Tab moves volume_step→
  volume_step_large, Escape hides, Enter does not activate a non-default
  button (native Win32 behavior; Space activates).
- Settings window: all sections/controls present (General, Hotkeys, Appearance,
  Blacklist, Feedback, Storage + Apply/Reset/Cancel/Close). Apply with valid
  change persisted volume_step 2→5 to config.json, showed "Settings saved.",
  adopted live (VolumeUp applied +5); invalid values (step 30 / large 29) →
  inline error, config unchanged, window stays open, edits preserved; Reset
  reverts draft to baseline; Cancel hides without persisting.
- Config live reload: external edit (BOM-free UTF-8) volume_step→12 was picked
  up by the 150ms mtime watch (`config reloaded (step=12, step_large=13…)`),
  hotkey step applied +12 live, normalized step_large saved back; app did not
  crash. A BOM'd edit (my own error) hit the parse-error fallback → defaults,
  no crash.
- External sync: OS media-volume key (keybd_event VK_VOLUME_UP) changed system
  volume 69→72; app poll logged `ext change: 72% muted=false` and synced.
- Theme rendering (screenshot pixel evidence): dark system theme → mixer body
  RGB(20,20,24) = token 0x141418, settings accent bar RGB(0,103,192) =
  accent 0x0067C0, overlay bar fill RGB(0,120,212) = volume medium threshold,
  track RGB(56,56,68) = border token. Light theme override → mixer body
  RGB(255,255,255). Renders correctly under both Opaque and Blurred (Auto)
  materials (overlay visible + blue bar in-process capture).
- Clean exit: PostMessage(WM_QUIT) to host → message loop returns → process
  exits cleanly (same path as tray Exit → PostQuitMessage); verified twice.

### Needs human visual confirmation (NOT claimed passing)
- High-contrast mode, reduced-motion, and 125%/150% DPI require OS setting
  changes (and, for HC/motion, an app restart since capabilities are snapshotted
  at startup). Steps: Settings > Accessibility > Contrast themes → pick a HC
  theme; Settings > Accessibility > Visual effects → turn off animation;
  Settings > Display > Scale → 125%/150% (logoff may be required). Relaunch the
  app, open mixer/overlay/settings, confirm.
- Taskbar height / secondary-monitor work-area changes.
- Backdrop/acrylic appearance and the perceptual color deltas (e.g. the small
  track-color shift) — pixel values are verified (above) but the on-screen look
  needs a human.
- The tray menu interactions (tray-origin Exit/Settings) use Windows 11 tray
  virtualization; the underlying actions were verified via the posted-message
  path, but the actual tray menu click needs a human or UIA.
- Help surface: window exists (480x420) and is created at startup; it is opened
  only from the tray menu, which was not automated — needs human click.

### Concerns / notes
- One screenshot-timing lesson: the overlay auto-hides after 1800 ms; a first
  naive capture (delayed by PowerShell Add-Type startup) made the overlay appear
  "invisible". In-process trigger+capture disproved this — overlay renders
  correctly under all materials. No overlay defect.
- Config file at %APPDATA%\volume-control\config.json was returned to the
  original values after testing (volume_step 2, step_large 10, overlay 1800,
  modifier CtrlAlt, blacklist empty, appearance System/Auto/Full/System); the
  `appearance` and `beep` sections are now written explicitly (equivalent to
  the prior implicit defaults).
- System volume left at 50% (its state at the start of the session).

## Current Verified State

- Repository root: D:\Projects\volume-control
- Standard startup path: `scripts/win-build.bat run` or `cargo run` (workspace default member = crates/volumectl) — MUST run through vcvars (MSVC env)
- Standard verification path: `scripts/win-build.bat build` then `scripts/win-build.bat test`
- Current active feature: **vol-011 (hybrid adaptive cross-platform UI)** — status
  `in_progress`, NOT `passing`. It is implemented and verified live on Windows
  (see Session 003), but the plan sets `passing` only after all required checks
  pass, and the human-confirmation remainder is unverified.
- Current blocker: none on Windows. macOS/Linux are **unverified follow-on work**:
  the renderer seams compile by construction (cfg-gated) but no native macOS/
  Linux UI is implemented or verified in this plan; needs a mac/Linux host or CI.
- PASSING with recorded evidence: vol-001 (workspace), vol-002 (audio), vol-003 (hotkeys), vol-004 (overlay), vol-005 (tray), vol-006 (config reload+sync), vol-007 (mac/Linux scaffolds+docs), vol-008 (release E2E), vol-009 (mixer/overlay placement fix), vol-010 (mixer close button + system theme)
- Task 13 (Windows verification of the adaptive UI): 94/94 unit tests pass; live
  verification recorded in Session 003 (mixer slider/buttons, geometry/gap, DPI
  100% work-area placement, Settings Apply/Cancel/Reset/error, config reload,
  external sync, keyboard nav, dark/light rendering, tray presence, clean exit).
  Remaining: HC mode, reduced motion, 125/150% DPI, taskbar/secondary-monitor
  changes, acrylic look, and tray-menu clicks need human visual confirmation.

### Task 14 (2026-08-03) — Tracking and final repository verification

- Added vol-011 (area `adaptive-ui`, priority 11) to feature_list.json as
  `in_progress` with evidence for what IS verified (94/94 tests, live-verified
  scriptable paths, geometry/theme pixel evidence) and the human-confirmation
  remainder + macOS/Linux-unverified follow-on recorded in `notes`. It is NOT
  marked `passing` (required human checks remain unverified).
- Whole-workspace `cargo fmt --all` normalization: the repo had ~13 pre-existing
  rustfmt diffs (mostly missing trailing newlines in files earlier tasks did not
  touch). Ran `cargo fmt --all`; reviewed the diff — it is formatting-only
  (line re-wrapping, trailing-comma insertion, import sorting; strip-whitespace
  comparison confirmed no semantic token changes). 17 source files under
  crates/volumectl/src were normalized.
- Final verification checks (all recorded, all PASS):
  1. `cargo fmt --all -- --check` → PASS (exit 0).
  2. `cargo build` (Windows) → Finished dev profile, 0 warnings.
  3. `cargo test -p volumectl` → 94 passed; 0 failed; 0 ignored.
  4. `git diff --check` → clean.
  5. `git status --short` → only intended files staged (fmt-normalized source +
     tracking files); `.claude/settings.local.json`, `.superpowers/`, runtime
     config, scratch scripts, and target/ were NOT staged.
- Commits: `style: rustfmt whole workspace` (formatting-only) + `docs: record
  adaptive UI milestone and final verification` (feature_list.json +
  claude-progress.md).

### Session 002 (2026-08-03)

- Goal: Add an explicit mixer close/toggle affordance and system dark-mode support without introducing WinUI 3 or another UI runtime.
- Completed:
  - Added a visible top-right `×` button. It routes through the existing `WM_CLOSE` hide path; hotkey/tray toggles still reopen the mixer.
  - Added Windows system theme detection using `AppsUseLightTheme`, with a light fallback when the registry value is unavailable.
  - Applied theme-aware DWM dark-mode state, background brush, static-label colors, and dark common-control theme for buttons/slider.
  - Preserved the existing native Rust/Win32 architecture, WASAPI synchronization, and mixer-above-overlay placement.
- Verification:
  - `scripts/win-build.bat build` — succeeded.
  - `scripts/win-build.bat test` — 3 passed, 0 failed.
- Remaining:
  - Full interactive Windows screenshot/UIAutomation verification of the close button and both light/dark variants is still pending; build and unit-test evidence is complete.

## Session Log

### Session 001 (2026-08-03)

- Date: 2026-08-03
- Goal: Scaffold the Cargo workspace for the VolumeControl Rust app; set up the project harness (skills, plugins, templates).
- Completed:
  - Cargo workspace scaffolded (root Cargo.toml + crates/volumectl with lib + bin).
  - Core modules drafted: config.rs, core.rs, audio/mod.rs (trait), audio_windows.rs (WASAPI), hotkeys/mod.rs, hotkeys_win32.rs (RegisterHotKey), cli.rs, main.rs, app.rs (Win32 message loop shell).
  - Harness setup COMPLETE:
    - superpowers plugin 6.2.0 installed (project scope, enabled) — 14 skills (brainstorming, writing-plans, executing-plans, test-driven-development, subagent-driven-development, systematic-debugging, verification-before-completion, etc.) + SessionStart hook. Skills on disk at ~/.claude/plugins/cache/superpowers-marketplace/superpowers/6.2.0/skills/. NOTE: SessionStart hook activates only on a fresh Claude Code session.
    - rtk 0.44.2 installed (prebuilt x86_64-pc-windows-msvc binary at ~/.cargo/bin/rtk.exe — cargo install from git failed on icu_normalizer_data build script). Project scope: CLAUDE.md instructions + .rtk/filters.toml. Global PreToolUse hook registered in ~/.claude/settings.json (backup at settings.json.bak), verified auto-rewriting `git status` → `rtk git status`. Uninstall: `rtk init -g --uninstall` + remove CLAUDE.md block.
    - learn-harness-engineering templates applied: CLAUDE.md (harness version), feature_list.json (8 features vol-001..vol-008), claude-progress.md, init.sh (cargo-adapted).
    - caveman NOT installed: research showed it is a caveman-speak communication-style skill (not a technical harness), ~1-1.5k input tokens/turn overhead vs modest output savings (net-negative on terse workloads). Reversible toggle later via `claude plugin marketplace add JuliusBrussee/caveman && claude plugin install caveman@caveman` if desired.
  - Git repo initialized at D:\Projects\volume-control (needed by superpowers worktrees).
- Verification run: `cargo build` green (0 errors, 0 warnings) via scripts/win-build.bat (vcvars64 wrapper); `cargo test` 0 failed.
- Evidence captured: end-to-end hotkey test — AHK SendInput Ctrl+Alt+Up/Down → WM_HOTKEY (ids 1/2) → apply() → WASAPI set_volume; system volume 98% → 100% confirmed by get_state.
- Commits: initial checkpoint commit pending (working tree has the full scaffold + verified core).
- Files or artifacts updated: see "Completed" above; also scripts/win-build.bat, target/debug/volumectl.exe.
- Known risk or unresolved issue:
  - MSVC toolchain setup: this machine had NO C linker — installed MSVC Build Tools 17.14 + Windows SDK 10.0.26100. Builds MUST run through scripts/win-build.bat (sets PATH/LIB/INCLUDE via vcvars64.bat).
  - Ctrl+Alt+M/R/V conflict with the running VolumePro AHK script (same default modifier) — handled gracefully (logged + skipped); user can change modifier in config.json.
  - overlay.rs COMPLETE + verified (vol-004 passing): GDI-painted Win32 popup, bottom-right, threshold colors, click-through (WS_EX_LAYERED|TRANSPARENT), auto-hide timer. Verified via EnumWindows visibility transitions + screenshot.
  - tray.rs COMPLETE + verified (vol-005 passing): tray-icon + muda menu (Volume % live label, Mute check, Reset 50%, separator, Exit). Tray icon found via UIA; menu captured in screenshot (Reset to 50% / Exit items); clean exit verified via WM_QUIT (same path as menu Exit). Added Ctrl+Alt+Shift+M OpenMenu hotkey (reachable even when icon is in the overflow flyout). NOTE: automated menu clicking is flaky on Windows 11 tray virtualization — items confirmed visually instead.
  - Config live reload COMPLETE + verified (vol-006 passing): mtime watch in the 150ms timer; volume_step 2->10 mid-run produced 10%/press deltas (88->86->76); modifier change re-registers hotkeys; load() save-if-changed avoids reload loops.
  - README.md + README.vi.md written (vol-007): bilingual docs, platform status table, build steps, config paths.
  - Release E2E verified (vol-008): 1.2MB optimized binary; all 6 checks passed on release build (run, hotkeys x3, overlay present+auto-hide, tray icon via UIA, config live reload, clean exit).
  - superpowers plugin SessionStart hook + rtk PreToolUse hook activate on a fresh Claude Code session.
- Next best step: optional future work — macOS CoreAudio backend, Linux PulseAudio/PipeWire backend, OpenMixer GUI (vol-003/005 mention), per-app volume via IAudioSessionManager, startup-on-boot shortcut.
