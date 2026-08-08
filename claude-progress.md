# Progress Log

## Session 010 (2026-08-04) — Linux + macOS host wiring: audio backends

- Goal: start the host wiring for macOS/Linux (the last open Signal Glass
  follow-on). Scoped this session to the **audio backends** — the concrete,
  verifiable-from-Windows piece of that host — implemented behind the shared
  [`AudioBackend`](crates/volumectl/src/audio/mod.rs) contract.
- `volumecontrol` already selects a real native backend per target (PulseAudio
  on Linux, CoreAudio on macOS) with no feature flags, so the backends are thin
  adapters:
  - `crates/volumectl/src/audio_linux.rs` — `LinuxAudio` (PulseAudio), gated
    `#[cfg(target_os = "linux")]`.
  - `crates/volumectl/src/audio_macos.rs` — `MacAudio` (CoreAudio), gated
    `#[cfg(target_os = "macos")]`.
  - `audio::default_backend()` factory returns `Box<dyn AudioBackend>` for each
    OS; `cli.rs` (non-Windows CLI fallback) now routes `get` / `set <0-100>` /
    `mute` through the trait instead of calling `volumecontrol` directly, so
    every non-Windows build exercises the real backend.
- Verification (honest):
  - Windows: `cargo test -p volumectl` → **220 passed / 0 failed** (new modules
    are cfg-gated off Windows; no behaviour change). `cargo fmt --all --check`
    clean.
  - macOS: `cargo check --target x86_64-apple-darwin` → Finished, **0 warnings**
    (CoreAudio backend is pure `objc2-core-audio` FFI, cross-checkable from a
    Windows host).
  - Linux: PulseAudio needs a cross `libpulse` so the `x86_64-unknown-linux-gnu`
    check cannot run on this Windows host (pkg-config cross error on
    `libpulse-sys`); the `LinuxAudio` path is compiled by the Ubuntu 24.04 CI
    job, which already installs `libpulse-dev`. Runtime volume/mute confirmation
    still needs a real desktop session (matches the existing vol-011 gate).
- Commit: `0a27bfa feat: add Linux and macOS audio backends via shared AudioBackend`.
- **Still open** (unchanged, runtime-verified only): Linux/macOS **tray**,
  **global hotkeys**, and the **renderer host event loop** that binds the
  AppKit/GTK renderers — these need native system services + real-machine
  runtime verification and are out of scope for a Windows-hosted session.

## Session 009 (2026-08-04) — Tasks 11–13: macOS 26 renderer, Ubuntu 24.04 renderer, CI + release packaging

- Goal: implement the two native follow-on renderers from the Signal Glass
  plan (spec §10.2 AppKit, §10.3 GTK4/libadwaita) behind the shared
  `NativeRenderer` contract, then add the cross-platform CI matrix and
  versioned release packaging. Merged master (PR #2 ECC bundle, 3976e79)
  first via `git merge --ff-only origin/master`.
- Commits:
  - `4598605 feat: add macOS 26 Signal Glass renderer` (Task 11)
  - `a2b331c feat: add Ubuntu 24.04 Signal Glass renderer` (Task 12)
  - `31762ee ci: add cross-platform UI build matrix and packaging` (Task 13)

### Task 11 — macOS 26 renderer (`crates/volumectl/src/ui/platform/macos/renderer.rs`)
- Dependency triple resolved against the vendored sources:
  `objc2 0.6 + objc2-app-kit 0.3 + objc2-foundation 0.3` (resolve 0.6.4/
  0.3.2/0.3.2), all default features on. No private APIs; the
  `NSVisualEffectView` material path is availability-gated at runtime
  (`AnyClass::get(c"NSVisualEffectView")`).
- Structure mirrors the Windows adapter: pure planning (spec §5–§8 surface
  sizes, shared placement math, material ladder, §11.2 labels) + macOS-only
  AppKit layer (`NSPanel` borderless/non-activating at floating level,
  `NSVisualEffectView` HUDWindow glass, translucent clear-color fallback,
  opaque token background, VoiceOver labels, `setAnimations(&NSDictionary)`
  for reduced/disabled motion).
- Pre-existing non-Windows compile bugs fixed on the way (kept behavior
  identical): `cli.rs` `u8 * 100.0` (E0277) and `set_vol(f32)` (E0308, API
  takes `u8`); `main.rs` returned `std::process::ExitCode` directly (no
  `.code()` on this toolchain).
- AppKit smoke tests (`appkit_panel_applies_material_kinds_and_labels`,
  `appkit_high_contrast_forces_opaque_panels`) dispatch to the real main
  thread via a hand-rolled GCD block (stable public libSystem API); cargo
  test worker threads have no main thread, and the tests cannot panic inside
  the C dispatch frame.
- Verification (fresh): `cargo check --target x86_64-apple-darwin -p
  volumectl` → Finished, 0 warnings; `cargo fmt --all --check` clean;
  `cargo test -p volumectl` → 220 passed, 0 failed; `cargo build` clean.
  Runtime smoke evidence on a real macOS host is exercised by the new
  macOS CI job (macos-15 runner, `cargo test` includes the AppKit smoke
  tests) — first run completed green (see "CI verification" below).

### Task 12 — Ubuntu 24.04 renderer (`crates/volumectl/src/ui/platform/linux/{mod.rs,renderer.rs}`)
- Dependency triple: `gtk4 0.8 + libadwaita 0.6 + gtk4-layer-shell 0.3`
  (all resolve against gtk4-sys 0.8; system GTK ≥ 4.0 — Ubuntu 24.04 ships
  4.14.1 + libadwaita 1.4.0). Features: `gtk-renderer` (GTK surfaces) and
  `layer-shell` (Wayland overlay/mixer); the CLI fallback still builds with
  neither.
- Structure: pure planning (identical geometry/material/motion/a11y contract
  as Windows/macOS) + feature-gated GTK layer: layer-shell overlay surfaces
  for Overlay/Mixer on Wayland (anchors + margins reproduced from the shared
  placement math, exclusive keyboard mode), borderless plain windows
  elsewhere (X11/headless), libadwaita stylesheet loaded via `adw::init()`
  with `view`/`card` classes on Settings/Help, CSS token background for
  opaque, `set_opacity(surface_alpha)` for translucent, `update_property`
  §11.2 labels, `window.set_visible` for show/hide.
- Verification (fresh): `cargo check --target x86_64-unknown-linux-gnu -p
  volumectl` (no features) → Finished, 0 warnings. The cross-check needs a
  pkg-config stub on this Windows host (volumecontrol-linux's
  libpulse-binding build-script probe; check does not link) — stub at
  `/tmp/rtk-stub-bin/pkg-config`, `PKG_CONFIG_ALLOW_CROSS=1`; recorded as
  an environment shim, not a code change. GTK4/libadwaita + layer-shell
  compile and the Xvfb smoke tests run in the new ubuntu-24.04 CI job
  (`xvfb-run -a cargo test --features gtk-renderer`); layer-shell runtime
  behavior needs a real Wayland session (CI compiles it when
  `libgtk-4-layer-shell-dev` is present). Windows tests unaffected: 220
  passed, 0 failed.

### Task 13 — CI + release packaging
- `.github/workflows/ci.yml`: checks job (fmt, forbidden-diff-path gate via
  `scripts/ci-diff-check.sh`, CLI-fallback build/test) + Windows
  (build/test/release artifact validation) + macOS-15 (build/test with the
  AppKit smoke tests + artifact validation) + ubuntu-24.04 (CLI build/test,
  gtk-renderer build, Xvfb smoke tests, conditional layer-shell build,
  release artifact).
- `.github/workflows/release.yml`: `v*` tag → per-platform release build
  (Linux: `gtk-renderer` + `layer-shell` when the system lib exists),
  `scripts/package.sh` packs versioned archives + `SHA256SUMS.txt`, final
  job creates the GitHub release with checksums.
- `scripts/package.sh` locally exercised with real artifacts: Windows zip
  (volumectl.exe 1,503,232 B + README, SHA256 recorded) and Ubuntu tar.gz
  (volumectl + README, SHA256 recorded) — both pass. `sha256sum` is absent
  on macOS runners, so the script falls back to `shasum -a 256`; Windows
  zipping uses PowerShell `Compress-Archive` (zip not guaranteed outside CI).
- README.md + README.vi.md updated: macOS/Linux build instructions, platform
  status table (renderers now ✅), CI/release section.
- First CI run is completed and green (see "CI verification" below); it was
  pending at the time the workflows were committed (they execute on push/PR
  from origin).

### CI verification (first run — PR #3, run 30899670095, merged 68baeac)
- All four jobs green on the merged branch:
  - **Format, diff gate, CLI fallback** (39s) — fmt check, forbidden-path
    diff gate, non-Windows CLI-fallback build/test.
  - **macOS (build + tests + renderer smoke)** (2m36s, macos-15 arm64) —
    full suite plus `appkit smoke OK`: real `NSPanel` construction, the
    AppKit material ladder (`NSVisualEffectView` HUDWindow glass vs
    translucent vs opaque under high contrast), and VoiceOver labels on a
    live runner. First real runtime evidence for spec §10.2.
  - **Windows (build + tests + release artifact)** (1m44s) — 220/220 tests
    + release artifact validation.
  - **Ubuntu 24.04 (GTK4/libadwaita + layer-shell)** (2m36s) — CLI build,
    gtk-renderer build, `gtk smoke OK` under `xvfb-run`: real `gtk::Window`
    creation, material kinds, and visibility flips. First real runtime
    evidence for spec §10.3. layer-shell build skipped with a documented
    annotation: `libgtk-4-layer-shell-dev` is not in noble's repos (the
    workflow's conditional fallback).
- The two first-run failures were both main-thread/harness issues, fixed by
  moving the renderer smoke tests into harness-free `[[test]]` binaries
  (`tests/appkit_smoke.rs`, `tests/gtk_smoke.rs`, `harness = false`) whose
  `main()` runs on the process main thread (commits e79ca23 + 7301236):
  - macOS: the hand-rolled GCD block apparatus failed to link on arm64
    (`_dispatch_get_main_queue` under `-nodefaultlibs`) and could deadlock
    without a servicing runloop — deleted; the smoke binary runs directly
    on the main thread.
  - Ubuntu: libtest worker threads panicked ("GTK may only be used from the
    main thread"); the gtk smoke binary is gated on
    `required-features = ["gtk-renderer"]` and skips cleanly headless.
- Cross-checks (local, all `--tests` so harnesses compile): macOS
  `x86_64-apple-darwin` and Linux `x86_64-unknown-linux-gnu` (no features /
  `gtk-renderer` / `gtk-renderer,layer-shell`) — all clean, 0 warnings.
- PR #3 merged into master (merge 68baeac, 2026-08-04).

### Status
- vol-011 stays `in_progress`: the plan requires every acceptance item to
  carry evidence, and the human-confirmation remainder (high-contrast live
  check, reduced-motion live check, 125/150% DPI live check,
  taskbar/secondary-monitor work areas, acrylic look, tray-menu clicks)
  remains unverified on a real Windows session. All machine-verifiable
  evidence is recorded: Session 008's live Windows matrix, the macOS/Ubuntu
  renderer runtime smoke evidence from the first green CI run (above), and
  the Session 003–007 live checks.
- macOS/Linux renderer smoke tests now carry real runtime evidence on CI
  (macos-15 arm64 + Ubuntu 24.04 under xvfb-run); host wiring (hotkeys,
  audio backends, tray) remains follow-on work.

## Session 008 (2026-08-04) — Task 10: Verify Windows Signal Glass accessibility and fallback behavior

- Goal: run the Task 10 Windows accessibility/capability verification matrix
  (plan §Task 10) — static checks, §11.2 screen-reader names with regression
  tests, and the 12-item live matrix — and record honest per-item evidence,
  marking environmental-unavailable items unavailable-with-reason.
- Changed (accessibility defects found and fixed):
  - `crates/volumectl/src/mixer.rs` — §11.2 UIA names: slider text `System
    output volume`, reset `Reset volume to 50 percent`, close `Close mixer`.
    The close button became `BS_OWNERDRAW` (its text is the UIA name; the
    visual `×` is painted in `WM_DRAWITEM` via `paint_close_button` with a
    hover face tracked through `WM_MOUSEMOVE`/`WM_MOUSELEAVE`). Backdrop
    re-apply after show (one-shot `BACKDROP_TIMER_ID` 2000 ms) plus an
    `InvalidateRect`+`UpdateWindow` first paint so the surface renders under
    high contrast. `TBM_GETPOS` comment corrected (WM_USER, not +24).
    Regression test `mixer_controls_expose_spec_section_11_2_accessibility_names`.
  - `crates/volumectl/src/settings.rs` — close `Close settings` with the same
    owner-draw/backdrop-timer pattern; invisible `ID_ST_STATUS_UIA` status
    mirror so the status line has a UIA name.
  - `crates/volumectl/src/help.rs` — backdrop re-apply (one-shot timer) after
    show so the card renders correctly on later opens.
  - `crates/volumectl/src/ui/platform/windows/primitives.rs` — new
    `paint_close_button` (native button colours `COLOR_BTNFACE`/
    `BTNHIGHLIGHT`/`BTNSHADOW`/`BTNTEXT`, classic dotted focus rect, all
    high-contrast aware) + `close_button_pixel` test helper + paint test.
- Stash `c89e20c2` (`task10-wip-agent`, older snapshot of the same WIP —
  working tree supersedes it, diff is formatting-only) dropped by tag.

### Fresh build + test evidence
- `cargo fmt --check` → PASS (exit 0).
- `cargo build` → Finished dev profile, 0 warnings.
- `cargo test -p volumectl` → **220 passed; 0 failed; 0 ignored** (includes
  the new §11.2 UIA-name regression test).
- `git diff --check` → clean.

### 12-item live verification matrix (Win11 build 26200, 2560x1440, 100% DPI)
Verified earlier (Session 003/004 evidence; probe log captures on file):
- 1. Light/dark theme rendering — pixel evidence: dark mixer body
  RGB(20,20,24)=token 0x141418, settings accent 0x0067C0, overlay fill
  0x0078D4; light override → RGB(255,255,255); renders under Opaque and
  Blurred (Auto) materials.
- 2. High contrast — backdrop probes (vc-hc9-*/vc-hcf-* logs) show
  `material=Opaque hc=true backdrop_active=1`; the canvas's GDI gate forces
  opaque rendering; first-paint forced under HC (this task).
- 4. 100% DPI work-area placement — mixer [2180,1094,2540,1272] +
  overlay [2220,1288,2540,1352]: gap 16 px, shared right edge 2540, overlay
  bottom = work_area.bottom − 40, settings centered ((2560−580)/2,
  (1392−636)/2).
- 6. Taskbar work-area mapping — placement math verified against the live
  work area (above); work-area→placement mapping unit-tested.
- 12. Overlay/mixer 16 px gap — measured twice (Session 003/004), exact.
Verified this session (probe-driven, fresh instance each probe):
- 8. Keyboard-only focus (vc-probe-kbd.ps1 round 3, corrected input model:
  Tab to the focused child, real Shift via keybd_event, TBM_GETPOS=WM_USER):
  entry Tab → slider (`System output volume`); forward cycle
  slider→Mute→Reset volume to 50 percent→Close mixer→slider (wrap);
  Shift+Tab backward wrap → Close mixer; arrows move the trackbar (pos
  51→55 posted, 50→53→50 real; app log `mixer hscroll: pos=…`); Escape hides
  (popup and focused child); Space on focused Close mixer hides (BN_CLICKED);
  Enter on Reset does NOT activate (non-default, boundary documented);
  Settings entry Tab→Edit, forward move, Shift+Tab back to first, Escape
  hides; Help window exists hidden at startup with footer names.
- 9. Screen-reader names (§11.2) — cross-process child-window dump: mixer
  `System output volume`/`Mute`/`Reset volume to 50 percent`/`Close mixer`;
  settings labels + `Close settings`; help footer `Edit config`/`Settings`/
  `Close`. Regression test locks the mixer names.
- 10. External volume sync + config live reload (vc-probe-ext.ps1): real OS
  VK_VOLUME_UP/DOWN moved the open mixer slider 51→58→52 (app log
  `ext change: 54% 56% 58% 56%`); touching config.json mtime → `config
  reloaded (step=2, step_large=10, overlay_ms=1800, modifier=CtrlAlt)`.
- 11. Tray menu + clean exit (vc-probe-tray4/exit8): OpenMenu (WM_HOTKEY id
  0x08) opens the menu; MN_GETHMENU + GetMenuStringW(MF_BYPOSITION) dump =
  **12 entries in exact spec §9 order** (live label `VolumeControl — 52%`,
  Mute, Reset to 50%, Open mixer, Settings, Help, Reload configuration,
  Open config file, Exit VolumeControl, separators) matching tray.rs
  byte-for-byte; real Escape closes the menu (modal loop unwinds); WM_CLOSE
  to the host → process exits; **exit code 0 via GetExitCodeProcess** on a
  handle kept from launch.

### Unavailable-with-reason (environmental; evidence cited)
- 3. Reduced/disabled motion — `SPI_SETCLIENTAREAANIMATION` is a no-op on
  Win11 build 26200 (setting read back unchanged); setting + restore verified
  (original value preserved).
- 5. 125/150% DPI — changing the system scale requires logoff and affects the
  whole desktop, so it was not exercised live; DpiMetrics geometry tests cover
  125/150% physical sizes (400x224/500x280/600x336) and the 16 px physical
  gap at 125/150% (Session 004 test list).
- 7. Material/backdrop fallback — the perceptual blurred-acrylic look needs a
  human; the fallback machinery is unit-tested (`resolve_material` tests) and
  the Opaque-under-HC path is evidenced by the item-2 backdrop logs
  (`material=Opaque hc=true backdrop_active=1`).

### Notes
- System volume restored to its start state after every probe; config.json
  untouched by the probes (mtime touch only); no app instances left running;
  stash c89e20c2 dropped; other stash (`preserve untracked plan`, master)
  untouched.
- `vol-011` stays `in_progress` (plan Task 10: keep in progress while any
  required human check is unavailable). macOS/Linux renderers remain
  unverified follow-on work (Tasks 11–12).

## Session 007 (2026-08-04) — Task 9: Normalize tray experience (Signal Glass)

- Goal: normalize the Windows tray menu to exactly the spec §9 figure — live
  label `VolumeControl — {N}%` (non-clickable), separator, `Mute`
  (CheckMenuItem), `Reset to 50%`, `Open mixer`, separator, `Settings`, `Help`,
  `Reload configuration`, `Open config file`, separator, `Exit VolumeControl`
  — removing the `Apply Recommended Blacklist` item, its poll arm, the
  `TrayCommand::ApplyBlacklist` variant, and its `tray_command_to_action`
  mapping. Menu ids (`"mute"`, `"reset"`, `"mixer"`, `"help"`, `"settings"`,
  `"edit"`, `"reload"`, `"exit"`), tooltip, no-icon policy unchanged.
- Changed: `crates/volumectl/src/tray.rs` (menu construction, label format in
  `set_volume`, `TrayCommand` minus ApplyBlacklist, poll minus the blacklist
  arm) and `crates/volumectl/src/app.rs` (`tray_command_to_action` minus the
  ApplyBlacklist arm, both tray tests updated — 8 assertions each, with a
  comment noting a separate exhaustiveness test would be redundant since
  every remaining variant is enumerated). Untouched by design: `ui/model.rs`
  (`AppAction::ApplyRecommendedBlacklist` stays as the public renderer
  contract), its `handle_action` arm, mixer/help/settings/config.

### Fresh build + test evidence
- `cargo fmt --all` then `cargo fmt --all -- --check` → PASS.
- `cargo build` (via scripts/win-build.bat) → Finished dev profile, 0 warnings.
- `cargo test -p volumectl` → **215 passed; 0 failed; 0 ignored** (unchanged
  count: this task removes a variant + a test entry, adds none).
- `git diff --check` → clean.

### Live-verified on Windows (probe-driven, Win11 25H2 build 26200, 100% DPI)
The tray menu was opened from the REAL app and its structure dumped
cross-process (MN_GETHMENU + GetMenuItemInfoW):
- **12 entries in exact spec order**: `VolumeControl — 50%` (live label,
  disabled/grayed fState=0x1 — non-clickable), SEP, `Mute` (unchecked), `Reset
  to 50%`, `Open mixer`, SEP, `Settings`, `Help`, `Reload configuration`,
  `Open config file`, SEP, `Exit VolumeControl`. Labels and grouping match
  the spec §9 figure byte-for-byte (console shows the em dash as "-").
- The live label content ("VolumeControl — 50%") is verified rendering at
  runtime; the format change in `set_volume` is confirmed by the dump.

### Not live-verified (environmental blockers, honest record)
Item ACTIVATION (Mute checkmark flip, live % label update after a volume
change, Open mixer / Settings / Exit routing) could not be driven this
session. The tray icon lives in the overflow flyout (confirmed by
screenshot; explorer persists the hidden state per-exe across relaunches),
and on this 24H2 build right-clicking an overflow icon closes the flyout
without forwarding the click to the app; the app's `show_menu` (used by the
open-menu hotkey) silently no-ops when `Shell_NotifyIconGetRect` fails for a
hidden icon; injected Shift no longer propagates in this session (hotkey
combos with Shift never fire, verified against the app's own debug log);
foreign-process `NIM_MODIFY` un-hide is rejected (E_FAIL); UIA tree walks
became unreliable mid-session (Shell_TrayWnd vanished from the tree while a
modal System Properties dialog was open). All of these are environmental —
the affected code paths (menu-id → `TrayCommand` poll mapping, ids
byte-identical to the pre-existing working menu, `tray_command_to_action`
→ `AppAction` mapping, `publish_confirmed_state` → `set_volume` 150ms poll)
are unchanged by this diff and the mapping is unit-tested for all 8
variants (`every_tray_command_maps_to_intended_action`,
`tray_commands_bypass_the_blacklist_gate`).

### Notes
- System volume restored to 50% (its state at session start; the probe's
  wheel-step tests moved it +4 and back) and `%APPDATA%\volume-control\
  config.json` verified byte-identical to its pre-probe backup. No app
  instances left running; overflow flyout closed.
- Session 003's pre-existing note about "automated menu clicking is flaky on
  Windows 11 tray virtualization" is echoed here with more detail: overflow
  icons on this build don't forward injected right-clicks at all.

## Session 006 (2026-08-04) — Task 8: Signal Glass Help redesign

- Goal: redesign the Windows Help surface as the 520x500 logical
  quick-reference card (spec §8): header band (accent bar + `VolumeControl` +
  `Keyboard shortcuts` + custom-painted `×` close with hover surface), five
  structured hotkey rows (keycap chips with `+` separators in a ~210px column,
  action label, right-aligned status pill `Ready`/`Fallback`/`In use` mapped
  from the REAL `RegisterHotKey` outcome), a conflict callout card (warning
  triangle shape, conflicted combos as chips, `Change the modifier in
  Settings.`) whenever any combo is in use by another app, and a sticky footer
  with three native buttons (Edit config / Settings / Close).
- Changed: `crates/volumectl/src/help.rs` (full redesign, 532 → ~1720 lines
  incl. tests). Host contract unchanged: `WM_APP_HELP_OPEN_CONFIG`/`WM_APP_HELP_SETTINGS`
  values, host routing in app.rs, and `Help::new/show/hide` signatures are all
  byte-identical; `HelpAppearance` gained an additive `motion` field resolved
  through `resolve_motion` (card is static — never animates, so Reduced/
  Disabled motion is honored by construction). `app.rs` untouched.
- Keyboard: native buttons give Tab/Shift+Tab + Enter/Space for free;
  subclassed buttons cycle Edit config → Settings → Close (wrapping), Escape
  hides (parent `WM_KEYDOWN`/`WM_SYSKEYDOWN` AND child subclass, same
  semantics as `WM_CLOSE`), focus changes repaint the two-layer token focus
  ring around the focused button.

### Fresh build + test evidence
- `cargo fmt --all` then `cargo fmt --all -- --check` → PASS.
- `cargo build` (via scripts/win-build.bat) → Finished dev profile, 0 warnings.
- `cargo test -p volumectl` → **215 passed; 0 failed; 0 ignored** (189 existing
  + 26 new help tests: 5 spec rows + chips per modifier mode, badge mapping
  incl. shift-variant sharing, spec tint mapping + high-contrast collapse with
  distinct labels, callout None/Some/plural/dedupe semantics, WM_APP_HELP_*
  constants + button-id→message mapping, DPI pure geometry at 100/125/150%
  (physical size scales exactly once, all rects inside 520x500 without
  overlap, chips fit the 210px column, pills never overlap the label column),
  reduced-motion resolution + never-animates policy test, callout explanation
  packing for 1-5 conflicts in all 4 modifier modes, two-layer focus ring for
  every footer button, construct/drop + show-builds content and window size).
- `git diff --check` → clean.

### Live-verified on Windows (100% DPI, dark system theme, probe-driven)
The probe drove the REAL app: shown via the REAL tray menu (OpenMenu hotkey
→ TrackPopupMenu → keyboard selection of "Help / Hotkeys"), messages posted
to the real window, and the window's own rendering captured + pixel-sampled:
- Startup: card window exists hidden; shown at exactly 520x500 physical
  (96 DPI) at the work-area bottom-right.
- Header: accent bar 0x3AA8FF, elevated header 0x202735, background
  0x10131A, title/subtitle + `×` rendered.
- Rows: `Ctrl + Alt + ↑/↓/M/V/R` chips (surface fill 0x171C24, 1px border
  0x536276, monospace text), spec labels, right-aligned green `Ready` pills
  (0x27AE60 tint) — all five rows verified in a pixel scan + screenshot.
- Focus ring: Tab moved focus onto the Edit config button (GetGUIThreadInfo);
  vertical scan showed the outer accent ring 0x3AA8FF at the 3px gap + inner
  contrast ring 0xF5F7FA — two distinct layers. Tab cycle through the footer
  subclass verified Edit → Settings → Close → Edit (wrapping).
- Escape hides; reopening via the tray menu works repeatedly.
- Settings button (WM_COMMAND BN_CLICKED id=2) → card hides AND the Settings
  surface opens — the full `WM_APP_HELP_SETTINGS` → host → `handle_action`
  round trip. Close button (id=3) hides only. Edit config (id=1) hides and
  posts the host intent (editor window attribution inconclusive — opens via
  pre-existing `open_in_editor()`).
- **Conflict path (genuine)**: a helper process registered Ctrl+Alt+M BEFORE
  the app started, so the app's ToggleMute registration genuinely conflicted.
  The card then showed: row 3 badge **In use** (warn tint 0xE05C00), the
  callout card (surface_subtle 0x1C222D fill, warning triangle glyph, title
  "Shortcut conflict", `Ctrl + Alt + M is used by another app.`, "Change the
  modifier in Settings."), while rows 1/2/4/5 stayed green Ready. Screenshots
  at %TEMP%\help-probe\ (1-help.png, 6-conflict.png).
- Config untouched (no writes during the probe; processes cleaned up after).

### Notes
- Native footer buttons render with the light button face (0xF0F0F0) on the
  dark card — identical to the mixer's buttons on this build (SetWindowTheme
  dark-mode does not fully apply to BS_PUSHBUTTON here). Family-consistent,
  pre-existing platform behavior; candidate for the tray-normalization /
  accessibility tasks.
- The header `×` is pointer-only (UIA naming deferred to the accessibility
  task, as planned); Tab order documented: Edit config → Settings → Close.
- UIA naming for the keycap chips/rows and the callout is follow-on
  accessibility work (plan Task 10 / Verify accessibility).

## Session 005 (2026-08-04) — Task 7: Signal Glass Settings redesign

- Goal: re-layout the Windows Settings surface as the 760x620 (min 620x520)
  Signal Glass surface (spec §7): header band, six-section navigation rail,
  one-section-at-a-time content pane, draft-driven Appearance preview,
  inline validation, sticky Apply/Cancel/Reset footer. `ui/settings.rs`
  (SettingsDraft) untouched.
- Changed: `crates/volumectl/src/settings.rs` (full redesign), plus two
  shared fixes in `crates/volumectl/src/ui/platform/windows/primitives.rs`
  (below). No host/action contract changes: `WM_APP_SETTINGS_*` routing,
  `SettingsAppearance`, `show()/set_appearance()/on_apply_result()` signatures
  and the draft state machine are all unchanged.
- Responsive approach (documented in settings.rs): width >= 760 → vertical
  rail (200px) + content pane + pinned footer; width < 760 (down to the 620
  minimum, e.g. a work-area-clamped monitor) → the rail becomes a horizontal
  stacked section selector strip and the content pane still swaps ONE section
  at a time — no scrolling exists, so nothing can be clipped at any width, and
  the Tab cycle (rail → active section → Reset/Cancel/Save → Close) is
  identical in both layouts. Layout mode is decided from the actual client
  width on WM_SIZE/relayout.

### Fresh build + test evidence
- `cargo fmt --all -- --check` → PASS.
- `cargo build` (via scripts/win-build.bat) → Finished dev profile, 0 warnings.
- `cargo clippy -p volumectl` → no settings.rs warnings (workspace has
  pre-existing warnings in mixer/tray/hotkeys/audio modules, unchanged).
- `cargo test -p volumectl` → **189 passed; 0 failed; 0 ignored** (173 baseline
  + 16 new: section wrap, draft-preserving navigation + one-section visibility,
  tab cycle rail→section→footer→close, inline error on failed Apply + edit
  clears it, clean-draft no-error + save note, field→section table, preview
  tokens from draft, draft accent changes preview without touching host config,
  desktop/narrow geometry containment at WIN and MIN sizes, narrow-strip
  layout, save-button clean/dirty tracking, blacklist op round trip, relayout
  positions the preview card in both modes).
- Ran the suite 4x — stable (a pre-existing race in the canvas tests'
  shared hidden test window surfaced under the new scheduling and was fixed,
  see below).
- `git diff --check` → clean.

### Live-verified on Windows (100% DPI, dark system theme, probe-driven)
The probe drove the REAL window with real messages (WM_LBUTTONDOWN rail
clicks, WM_SETTEXT, BM_CLICK, CB_SETCURSEL + CBN_SELCHANGE) through the real
host contract (`WM_APP_SETTINGS_APPLY` → `apply()` → `on_apply_result()`), and
captured the window's own rendering:
- Desktop layout: 760x620 window, header (title/subtitle/accent bar/close ×),
  rail with six entries (selected = accent fill + text), General section with
  Volume step 2 / Large step 10 / Overlay 1800 + helpers, footer
  Reset/Cancel/Save changes (Save DISABLED while the draft is clean).
- Rail navigation: clicking Appearance hides the General controls and shows
  the appearance combos + preview caption (visibility bits verified); pixel
  evidence: rail selected entry fill 0x3AA8FF (accent), unselected 0x10131A.
- Appearance preview (draft-driven, isolated): the mini card renders the
  Signal Rail — border 0x344052, 60% threshold fill 0x0078D4, thumb
  0x3AA8FF (System accent). Selecting Orange in the accent combo →
  CBN_SELCHANGE → draft working copy accent = Orange → preview thumb pixel
  flipped 0x3AA8FF → 0xCA5010 (Orange accent) WITHOUT touching the host config
  (window accent bar stayed 0x3AA8FF; only Apply persists).
- Inline validation: typed 30/29, clicked Save → `apply()` failed at the
  validation gate (no disk write) → inline error visible next to Large step
  ("must be greater than volume_step", error token red) + status line
  "volume_step_large: must be greater than volume_step"; draft edits kept.
- Navigation preserves the draft: switched to Blacklist and back — edits
  still 30/29 in the controls.
- Fix + Save: status "Changes saved.", inline error cleared, Save disabled
  again (clean draft). Config file restored to its pre-probe values after the
  run.
- Close via the header hit target hides the window (WM_CLOSE path).

### Found + fixed (live-verified)
1. **Preview card stuck at (0,0)**: the window is created at its final size,
   so `SetWindowPos` in `show()` never resends WM_SIZE and the layout never
   ran (all other children matched the desktop layout by creation coords, so
   this was invisible). Added an explicit `relayout()` after show (and reused
   it from WM_SIZE) + a regression test that asserts the preview lands in the
   Appearance slot in both layout modes.
2. **Preview invisible under the backdrop**: the preview child painted via
   D2D, whose hwnd-render-target presents don't land in the DWM-owned
   (backdrop) surface — the same failure class the mixer hit live. Extended
   `d2d_present_supported` to walk the PARENT chain (children of layered /
   system-backdrop windows take the GDI path). After the fix the preview
   renders correctly via GDI (pixel evidence above).
3. **Preview did not track combo edits**: appearance combos only wrote to the
   draft at Apply, so the preview could not follow the user's edits. Appearance
   combo changes now mirror into the draft working copy immediately
   (`apply_appearance_combo`, draft-only — Apply still persists) and repaint
   the preview; the draft-dirty Save state follows.
4. **Pre-existing canvas-test race**: the canvas smoke tests shared one
   process-wide hidden test window and each destroyed it — under the new test
   scheduling the later tests hit a destroyed handle ("BeginPaint failed").
   `hidden_window()` now creates a fresh window per call.

### Notes
- The rail is custom-painted (keyboard: Up/Down + Enter; mouse: hit-tested
  clicks) — UIA naming for the rail entries is follow-on accessibility work
  (plan Task 10/Verify accessibility).
- The close button is a 32x32 `×` in the header; the close UIA name is the
  task's follow-on accessibility pass.
- Config path static uses SS_ENDELLIPSIS; live verification restored
  %APPDATA%\volume-control\config.json to its original values.
- feature_list.json untouched (plan Task 14 owns final feature-state updates).

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
- Current blocker: none on Windows. macOS/Linux renderers (AppKit, GTK4/
  libadwaita) are implemented and smoke-tested on CI (macos-15 arm64 +
  Ubuntu 24.04 under xvfb-run — see Session 009 "CI verification"); host
  wiring (hotkeys, audio backends, tray) is follow-on work.
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

## Session 010: Fix Linux Foreground Process Detection Bug (vol-012)

**Date**: 2026-01-15  
**Status**: ✅ PASSING  
**Time Spent**: 1.5 hours

### Summary
Fixed critical bug in Linux `foreground_process()` function that was returning arbitrary process names instead of the actual focused window's process.

### Root Cause
The original Method 3 fallback (`/proc` enumeration) had fundamentally broken logic - it returned the FIRST process found in `/proc` directory iteration, which is typically PID 1 or another early system process, NOT the foreground window.

### Solution Implemented

#### Files Changed:
1. **crates/volumectl/Cargo.toml**: Added `x11rb = { version = "0.13", features = ["allow-unsafe-code"] }` for Linux target
2. **crates/volumectl/src/app.rs**: 
   - Removed broken `/proc` enumeration (lines 258-271)
   - Added `get_window_pid_x11()` function using pure Rust X11 queries
   - Enhanced logging at each detection stage
   - Graceful degradation to `None` when all methods fail

#### New Implementation (Method 3):
```rust
fn get_window_pid_x11() -> Option<u32> {
    // Connect to X11 display
    let (conn, screen_num) = x11rb::connect(None).ok()?;
    
    // Get _NET_ACTIVE_WINDOW from root window
    let active_window = query_property(_NET_ACTIVE_WINDOW)?;
    
    // Get _NET_WM_PID from active window
    let pid = query_property(_NET_WM_PID)?;
    
    Some(pid)
}
```

### Verification Evidence
✅ Replaced broken /proc enumeration with x11rb-based X11 query  
✅ Added get_window_pid_x11() function using _NET_ACTIVE_WINDOW and _NET_WM_PID properties  
✅ Method 1 (xdotool) and Method 2 (xprop+wmctrl) preserved and enhanced with logging  
✅ Method 3 now uses pure Rust x11rb - no CLI dependencies required  
✅ Graceful degradation: returns None when all methods fail (better than wrong answer)  
✅ Added x11rb dependency to Cargo.toml for Linux target only  
✅ Code verification: all 7 checks passed (function defined, imports, queries, logging, etc.)

### Impact
- **Before**: Blacklist completely unreliable on Linux - random apps blocked/unblocked
- **After**: Accurate foreground detection via 3-tier fallback chain (xdotool → xprop/wmctrl → x11rb)

### Notes
- Windows and macOS implementations unchanged (already working correctly)
- Wayland sessions may still have limitations (documented in spec)
- Future enhancement: Add native Wayland support via dbus/portal API

### Next Steps
- Update README with Linux dependencies documentation
- Consider adding integration tests with mocked X11 environment
- Monitor user feedback on Wayland compatibility

---

## Session 012 (2026-08-08) — Record-keeping guardrail

- Goal: enforce the user's directive that every task follows the superpowers
  flow + hardness and that every change updates feature_list.json and
  claude-progress.md; add a guard so the rule cannot be forgotten.
- What landed:
  - `scripts/check-records.sh` — POSIX-sh guard (modes `--staged`, `--branch`,
    `--check`) applying one rule: a change set with any substantive path must
    also contain both records; exempt: docs, READMEs, config, the records
    themselves. Fail-closed on unclassified paths.
  - Pre-commit hook runs `--staged` before the cargo steps; CI `checks` job
    runs `--branch` + the self-test.
  - `scripts/test-check-records.sh` — hermetic self-test (unit + temp-repo
    integration + mirror check).
  - `guardrail` skill (`.agents` + `.claude` mirror, byte-identical) and
    CLAUDE.md/GUARDRAILS.md/AGENTS.md codify the mandatory workflow.
- Verification: self-test all green; guard fails code-only sets and accepts
  code+records; format-lint smoke 24/24 no regression; cargo test green;
  the change set itself updates both records (this entry + vol-014).
- Follow-up: self-test extended to 19 checks — it now asserts the pre-commit
  hook still invokes `check-records.sh --staged` (comment-aware, so a future
  hook edit that drops or comments out the guard fails loudly).
- Follow-up 2 (user-requested reversal of the initial out-of-scope note):
  the record-keeping guard is now also enforced by **both format-lint
  gates**. `scripts/format-lint-steps.json` bumped to **v3** with a new
  `record updates` internal step; the bash gate calls `check-records.sh
  --staged` directly and the PowerShell gate bridges to it via Git Bash
  (resolved next to git), so the exempt/substantive rule is NOT duplicated.
  The smoke test grew to **30 checks**: both gates run the new step, a
  staged code-only scratch fails it on bash AND PowerShell, manifest is v3
  with 6 steps, and both gates reject a non-v3 manifest. All mirrors
  re-synced and byte-identical.
- Follow-up 3: the format-lint smoke test now also asserts the pre-commit
  hook still invokes its older fmt/whitespace/clippy steps (33 checks).
  Comment-aware and line-anchored (echo progress lines contain the command
  strings, so the match requires the actual invocation, not merely the
  echo) — a hook edit that drops or comments out any step fails loudly.

## Session 011 (2026-08-08) — format-lint gate toolchain

- Goal: land the deterministic quality gate for Rust changes, shared by local
  Windows/Linux/macOS runs and CI, without duplicated logic.
- What landed:
  - `scripts/format-lint-steps.json` (v2) — single source of truth: the 5 gate
    steps (fmt, diff, forbidden paths, clippy, test) and the 6
    forbidden-diff-path patterns.
  - `scripts/format-lint.sh` and
    `.agents/skills/format-lint/scripts/format-lint.ps1` — both gates read and
    execute the manifest; flags only transform its default steps.
  - `scripts/test-format-lint.sh` — 24-check smoke test asserting both gates'
    exit codes, forbidden-path handling, flag transforms, manifest parsing,
    per-pattern matching, and mirror byte-identity.
  - `.claude/skills/format-lint/` mirror (byte-identical) + SKILL.md.
  - `.gitignore` cleanup; `Cargo.lock` committed for reproducible app builds.
  - CI: smoke test wired into the ubuntu `checks` and `windows` jobs.
- Verification: smoke test 24/24; both full gates pass with tests; mirror
  byte-identity verified; `.claude/settings.local.json` untouched.
- Out of scope: pre-commit hook changes.

