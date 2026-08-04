# Signal Glass Production UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Implement the approved Signal Glass UI across Windows first, then add native macOS 26 and Ubuntu 24.04 renderer adapters, while preserving the host-owned audio/config/hotkey architecture.

**Architecture:** Shared `AppState`, `AppAction`, semantic tokens, capabilities, placement, and Signal Rail geometry remain platform-neutral. Native adapters consume confirmed state and emit actions; they never mutate audio/configuration directly. Windows keeps Win32 windows and native controls, adding Direct2D/DirectWrite-oriented custom chrome; macOS uses AppKit; Ubuntu uses GTK4/libadwaita with Wayland/X11 fallback.

**Tech Stack:** Rust 2021, Cargo workspace, `windows-sys`, Win32/native controls, Direct2D/DirectWrite/DWM where available, AppKit/objc2, GTK4/libadwaita/gtk4-layer-shell, Serde/JSON, existing WASAPI/tray/hotkey integrations.

## Status (2026-08-04) — all tasks complete and on master

Tasks 1–10 were committed directly to master (2026-08-03–04); Tasks 11–14
landed via PR #3 (merge `68baeac`; CI fixes `e79ca23`/`7301236` on the same
branch). Windows unit suite 220/220, all four CI jobs green on the first
merged run (`30899670095`). Evidence of record: `claude-progress.md`
(Session 003–009), `feature_list.json` (vol-011), `session-handoff.md`.

| # | Task | Commit(s) | Evidence |
|---|------|-----------|----------|
| 1 | Expand shared semantic tokens | `10b2447` | token tests, threshold colors unchanged |
| 2 | Renderer/host bridge contracts | `c85b364` | `NativeRenderer` + `HostHandle` tests |
| 3 | Windows drawing/DPI seams | `71be7ed`, `bc33590` | geometry tests 100/125/150%, negative origins |
| 4 | Shared Signal Rail | `099e021`, `2eb74f0` | rail tests 0/50/100%, muted marker, thresholds |
| 5 | Redesign Windows overlay | `38f6ed5` | live 336x88 placement, pixel evidence (Session 003/004) |
| 6 | Redesign Windows mixer | `c893dc6`, `a6a4a25` | live 400x224 + 16px gap, rail↔trackbar sync (Session 004) |
| 7 | Redesign Windows Settings | `e1855df` | live 760x620 rail/content, draft+preview (Session 005) |
| 8 | Redesign Windows Help | `5061d79` | live 520x500 card, conflict callout (Session 006) |
| 9 | Normalize tray experience | `510d8ba` | 12-entry menu dump = spec §9 (Session 007) |
| 10 | Windows accessibility verification | `85dbab0` | §11.2 UIA names live-dumped + matrix (Session 008); human-confirmation remainder open |
| 11 | macOS 26 AppKit renderer | `4598605`, `e79ca23`, `7301236` | `appkit smoke OK` on macos-15 arm64 (CI run 30899670095) |
| 12 | Ubuntu 24.04 GTK4 renderer | `a2b331c`, `e79ca23`, `7301236` | `gtk smoke OK` under `xvfb-run` (CI run 30899670095); layer-shell build skipped (lib absent from noble) |
| 13 | CI + release packaging | `31762ee` | 4-job matrix green; package.sh exercised with real artifacts |
| 14 | Final evidence + tracking | `1737692`, `332689a` | Session 009 evidence; vol-011 stays `in_progress` |

Open items (documented, not defects): human-confirmation remainder for
vol-011 (high-contrast, reduced-motion, 125/150% DPI, taskbar/secondary
monitors, acrylic look, tray-menu clicks) and macOS/Linux host wiring
(hotkeys, audio backends, tray).

## Global Constraints

- Keep Windows-only code behind `#[cfg(target_os = "windows")]`.
- Renderers consume confirmed `AppState` and emit `AppAction`; host owns audio, config, hotkey registration, and persistence.
- Preserve blacklist gating, beep behavior, config reload, external volume sync, tray behavior, click-through overlay, mixer/overlay separation, and clean exit.
- Keep native controls for keyboard and accessibility semantics.
- High contrast must use opaque surfaces and must not rely on color alone.
- Reduced and disabled motion must be deterministic and safe.
- Material fallback must always produce a complete usable opaque rendering.
- Preserve the VolumePro threshold colors: muted `#888888`, low `#27AE60`, medium `#0078D4`, high `#E05C00`.
- Do not weaken or remove tests or alter the non-Windows CLI fallback.
- Do not stage `.claude/settings.local.json`, `.superpowers/`, `target/`, runtime config, scratch files, or `.claude/worktrees/`.
- Update `claude-progress.md` and `feature_list.json` only from fresh verification evidence.

## Task 1: Expand shared semantic tokens

**Files:** Modify `crates/volumectl/src/ui/theme.rs`, `crates/volumectl/src/ui/mod.rs`; test `theme.rs`.

Add semantic fields for subtle/strong surfaces and borders, accent hover/pressed, selected/disabled states, muted/status colors, and explicit focus/error states. Preserve current public token fields and threshold colors. Ensure high contrast is opaque and has strong borders/focus.

Tests must cover light/dark/high-contrast opacity, contrast of primary/secondary/focus text, hover/pressed distinction, and unchanged threshold colors. Run `cargo test -p volumectl`; commit `feat: expand Signal Glass semantic tokens`.

## Task 2: Define renderer and host bridge contracts

**Files:** Create/modify `crates/volumectl/src/ui/renderer.rs`, `ui/model.rs`, `ui/mod.rs`; tests in shared UI modules.

Add the normative `NativeRenderer` contract with `create(host, capabilities)`, `publish(state, tokens, capabilities)`, `dispatch(action)`, and `destroy`. Define an opaque `HostHandle` that can enqueue `AppAction` without exposing host internals. Keep action normalization and confirmed-state semantics explicit.

Test serialization, action normalization, and state immutability. Run crate tests; commit `feat: add native renderer host contract`.

## Task 3: Add Windows native drawing and DPI seams

**Files:** Modify Windows Cargo feature declarations and `ui/platform/windows/primitives.rs`; create focused `d2d.rs` and `text.rs` if compatible with existing dependency strategy; update platform exports.

Add Windows-gated resource-safe helpers for DPI metrics, logical-to-device conversion, rounded surfaces, borders, focus rings, text layout, and Signal Rail drawing. Initialization failure must select the existing complete opaque path. Do not add WinUI, XAML, WebView, mandatory GPU shaders, or a second UI runtime.

Test pure geometry at 100/125/150% DPI and negative monitor origins. Verify Windows build/test; commit `feat: add Windows native drawing primitives`.

## Task 4: Implement shared Signal Rail geometry

**Files:** Create `crates/volumectl/src/ui/signal_rail.rs`; export from `ui/mod.rs`; tests in the module.

Define platform-neutral rail state and geometry with threshold fill, thumb marker, and explicit muted diamond/outline marker. Add clamping, minimum target sizing, Home/End mapping, and configured step adjustments. Renderers use geometry only; platform code chooses paint API.

Test 0/50/100%, muted marker distinction, bounds, thresholds, keyboard mapping, and high-contrast outline requirements. Commit `feat: add shared Signal Rail geometry`.

## Task 5: Redesign Windows overlay

**Files:** Modify `crates/volumectl/src/overlay.rs` and app/primitives integration; tests in overlay module.

Use 336x88 logical size, title `Volume`, `System output`, right-aligned value, shared Signal Rail, muted marker, and structured toast. Preserve bottom-right work-area placement, topmost/no-activate/click-through styles, timer, threshold colors, and state publication. Apply reduced/disabled motion policy.

Test appearance resolution, placement, toast/volume modes, fallback, motion behavior, and auto-hide. Verify build/test and interactive geometry/theme behavior before commit `feat: redesign Windows Signal Glass overlay`.

## Task 6: Redesign Windows mixer

**Files:** Modify `crates/volumectl/src/mixer.rs`, app/primitives integration; tests in mixer module.

Use 400x224 logical size with `VOLUME MIXER`, `System output`, right-aligned 28px value, Signal Rail synchronized to native trackbar, Mute, Reset to 50%, and 32px close target. Preserve native trackbar semantics, keyboard navigation, Escape, focus stability, host routing, and safe teardown order. Maintain exact 16px gap above overlay.

Test slider/action synchronization, Home/End/arrows, focus retention, close/Escape, target sizing, and teardown ordering. Verify live rectangles, DPI, keyboard, and accessibility; commit `feat: redesign Windows Signal Glass mixer`.

## Task 7: Redesign Windows Settings

**Files:** Modify `crates/volumectl/src/settings.rs`; reuse `ui/settings.rs`; tests in Settings modules.

Use 760x620 desktop size, 620x520 minimum, navigation rail for General/Hotkeys/Appearance/Blacklist/Feedback/Storage, grouped content, appearance preview, inline validation, sticky Apply/Cancel/Reset footer, and stacked narrow fallback. Retain native Edit/ComboBox/CheckBox/ListBox/Button controls and existing WM_APP host messages.

Test draft dirty/apply/cancel/reset/invalid persistence, navigation state retention, preview isolation, narrow layout, and deterministic keyboard focus. Commit `feat: redesign Signal Glass settings surface`.

## Task 8: Redesign Windows Help

**Files:** Modify `crates/volumectl/src/help.rs`; tests in Help module.

Use 520x500 logical size and structured hotkey rows with keycap visuals, registration/conflict/unavailable badges, conflict callout, and consistent footer controls. Preserve existing host messages and close behavior. Keep status intelligible without color alone.

Test row data, accessible descriptions, conflict semantics, action routing, DPI layout, and reduced motion. Commit `feat: redesign Signal Glass help surface`.

## Task 9: Normalize tray experience

**Files:** Modify `crates/volumectl/src/tray.rs` and only required `app.rs` mapping; tests for command mapping.

Use the approved labels/grouping: current percentage, Mute, Reset to 50%, Open mixer, Settings, Help, Reload configuration, Open config file, Exit VolumeControl. Preserve tray-icon/muda routing, current state updates, and clean exit. Commit `feat: normalize Signal Glass tray menu`.

## Task 10: Windows accessibility and capability verification

**Files:** Modify only defects found in Windows UI/primitives; update progress/feature tracking only with fresh evidence.

Run format, build, crate tests, and diff checks. Live verify light/dark/high contrast, reduced/disabled motion, 100/125/150% DPI, taskbar/secondary monitor work areas, material fallback, keyboard-only focus, screen-reader names, overlay/mixer 16px gap, external sync/reload, tray, and clean exit. Keep `vol-011` in progress if any required human check is unavailable. Commit `test: verify Windows Signal Glass accessibility and fallback behavior`.

## Task 11: Implement macOS 26 renderer

**Files:** Target-gated Cargo dependencies and `crates/volumectl/src/ui/platform/macos/{mod.rs,renderer.rs}` plus focused surface modules.

Add AppKit NSPanel/NSWindow surfaces, native controls/VoiceOver semantics, public material APIs with availability gates, Signal Rail semantics, host bridge, and opaque fallback. Do not use private APIs or alter CLI fallback. Compile/test on macOS 26 and record native evidence. Commit `feat: add macOS 26 Signal Glass renderer`.

## Task 12: Implement Ubuntu 24.04 renderer

**Files:** Target-gated dependencies and `crates/volumectl/src/ui/platform/linux/{mod.rs,renderer.rs}` plus layer-shell/X11 modules.

Add GTK4/libadwaita surfaces, gtk4-layer-shell Wayland overlay where available, X11 fallback, accessible Signal Rail, theme/motion propagation, compositor-aware material, and opaque fallback. Preserve CLI fallback and host bridge. Compile/test on Ubuntu 24.04 and record Wayland/X11 evidence. Commit `feat: add Ubuntu 24.04 Signal Glass renderer`.

## Task 13: Add CI and release packaging

**Files:** Create `.github/workflows/ci.yml`, `.github/workflows/release.yml`; add packaging scripts and update README docs.

CI must cover Windows build/test/release, macOS compile/test/renderer smoke, Ubuntu 24.04 GTK/libadwaita compile/test, formatting, diff checks, and artifact validation. Release artifacts should include Windows portable/native installer, then macOS/Linux packages only when their renderers exist and package commands succeed. Use versioned names and checksums.

Commit `ci: add cross-platform UI build matrix and packaging`.

## Task 14: Final evidence and tracking

**Files:** Modify `claude-progress.md`, `feature_list.json`, optional `session-handoff.md`.

Run all fresh platform-appropriate verification. Record exact commands, counts, rectangles, DPI/theme/material/accessibility evidence, native platform evidence, and blockers. Set `vol-011` to passing only after every required acceptance item has evidence; otherwise retain in_progress. Commit `docs: record Signal Glass production verification`.

Execution order: Tasks 1–4 shared layer; Tasks 5–9 Windows surfaces; Task 10 Windows gate; Tasks 11–12 native follow-ons; Task 13 CI/release; Task 14 final evidence.
