# Hybrid Adaptive Cross-Platform UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish a shared adaptive UI contract, complete the Windows Fluent-inspired renderer and native Settings surface, and leave clean platform seams for later AppKit and GTK/libadwaita renderers.

**Architecture:** Keep the existing single Win32 host window responsible for audio, configuration, hotkeys, tray commands, and confirmed state. Add platform-neutral UI model, theme, capability, and placement modules; refactor existing Win32 surfaces to consume those contracts without introducing WinUI 3, XAML, WebView, or another UI runtime.

**Tech Stack:** Rust 2021, Cargo workspace, `windows-sys`, Win32/GDI/DWM/common controls, WASAPI, `tray-icon`, `muda`, Serde/JSON, native AppKit/CoreAudio and GTK/libadwaita seams for later phases.

## Status (2026-08-04) — fully executed

All 14 tasks completed on 2026-08-03 (commits `021f745` → `ce5fff1`,
`66079df`, `e917b39`), committed directly to master. Evidence of record:
`claude-progress.md` Session 003–004 (94/94 tests, live Windows matrix) and
`feature_list.json` vol-011. The UI direction was then superseded by the
Signal Glass production design (docs/superpowers/specs/
2026-08-03-signal-glass-production-ui-design.md), whose plan executed the
Windows redesigns, native macOS/Linux renderers, CI, and release packaging.

## Global Constraints

- Windows-only code remains behind `#[cfg(target_os = "windows")]`.
- WASAPI remains authoritative for volume and mute state.
- Renderers dispatch actions to the application host; they never mutate audio or write config directly.
- Every UI surface settles from confirmed state after host readback.
- Preserve RegisterHotKey conflict behavior, mixer/overlay separation, click-through overlay behavior, tray behavior, live config reload, and current hotkey behavior.
- Do not add WinUI 3, Windows App SDK, XAML, WebView, mandatory GPU shaders, or a second UI runtime.
- Translucency, blur, animation, and droplet-inspired highlights are optional; opaque rendering remains fully usable.
- Respect dark mode, high contrast, reduced motion, keyboard navigation, DPI scaling, and multi-monitor work areas.
- Use one active feature in `feature_list.json`; do not mark work passing without fresh evidence in `claude-progress.md`.

## File Map

### Create

- `crates/volumectl/src/ui/mod.rs` — shared UI exports.
- `crates/volumectl/src/ui/model.rs` — `AppState`, `AppAction`, surface and preference enums.
- `crates/volumectl/src/ui/theme.rs` — light/dark/high-contrast design tokens.
- `crates/volumectl/src/ui/capabilities.rs` — compositor, accessibility, material fallback, DPI, and work-area contracts.
- `crates/volumectl/src/ui/surface.rs` — surface geometry and placement math.
- `crates/volumectl/src/ui/settings.rs` — platform-neutral Settings draft state machine.
- `crates/volumectl/src/ui/platform/mod.rs` — renderer boundary.
- `crates/volumectl/src/ui/platform/windows/mod.rs` — Windows renderer exports.
- `crates/volumectl/src/ui/platform/windows/primitives.rs` — Win32 capability/theme/DPI/work-area helpers.
- `crates/volumectl/src/ui/platform/macos/mod.rs` — compile-safe later AppKit seam.
- `crates/volumectl/src/ui/platform/linux/mod.rs` — compile-safe later GTK seam.
- `crates/volumectl/src/settings.rs` — native Windows modeless Settings window.

### Modify

- `crates/volumectl/src/lib.rs`, `app.rs`, `config.rs`, `mixer.rs`, `overlay.rs`, `help.rs`, `tray.rs`, `hotkeys_win32.rs`.
- `Cargo.toml` only if required Windows feature flags are missing.
- `README.md`, `README.vi.md`, `feature_list.json`, `claude-progress.md`.

## Task 0: Establish baseline

- [x] Confirm repository root, read `claude-progress.md`, `feature_list.json`, and `git log --oneline -5`.
- [x] Run `./scripts/win-build.bat build` and `./scripts/win-build.bat test`.
- [x] Record baseline failures before implementation; do not overwrite unrelated changes.

## Task 1: Shared UI state and action contract

**Files:** create `ui/mod.rs`, `ui/model.rs`; modify `lib.rs`.

- [x] Add platform-neutral `SurfaceId`, `ThemeMode`, `MaterialMode`, `MotionMode`, `SurfaceVisibility`, `AppState`, `UiStatus`, and `AppAction`.
- [x] Include confirmed volume/mute/device state, surface visibility, appearance preferences, and host-directed actions for volume, mute, reset, surfaces, config apply/cancel/reset, appearance, config location, blacklist, and exit.
- [x] Add constructors, surface visibility helpers, normalization at action boundaries, and model-only unit tests.
- [x] Run `cargo test -p volumectl`; commit `feat: add shared UI state and action contract`.

## Task 2: Configuration appearance preferences and validation

**Files:** modify `config.rs`, `ui/model.rs`; add config tests.

- [x] Add serde-defaulted `AppearanceConfig` containing System/Light/Dark, Auto/Translucent/Opaque, Full/Reduced/Disabled, and System/Blue/Green/Purple/Orange accent choices.
- [x] Preserve old JSON compatibility and existing bounds.
- [x] Expose normalization and strict field-specific validation; add `save_validated` that validates, normalizes, persists, and returns the saved config.
- [x] Preserve stable mtime reload behavior and add compatibility/invalid-config tests.
- [x] Run focused config tests; commit `feat: add persisted adaptive UI preferences`.

## Task 3: Shared design tokens

**Files:** create `ui/theme.rs`; modify `ui/mod.rs`.

- [x] Add `Rgba`, `ThemeTokens`, spacing/radius/animation tokens, typography roles, hit target, focus/error, volume threshold, surface, border, and elevation tokens.
- [x] Implement light/dark/high-contrast token selection and accent selection.
- [x] Ensure high contrast uses opaque surfaces; add palette tests.
- [x] Run focused theme tests; commit `feat: add adaptive UI design tokens`.

## Task 4: Capability fallback and placement contracts

**Files:** create `ui/capabilities.rs`, `ui/surface.rs`; modify `ui/mod.rs`.

- [x] Add `WorkArea`, `SurfaceSize`, `SurfaceRect`, `UiCapabilities`, `ResolvedMaterial`, fallback resolution, and placement functions.
- [x] Implement fallback order: blurred/translucent, translucent, opaque, with high contrast forcing opaque.
- [x] Place against monitor work areas, including negative-origin monitors; preserve 16px mixer/overlay gap.
- [x] Add tests covering known 2560x1400 geometry and material resolution.
- [x] Run focused capability tests; commit `feat: add UI capabilities and monitor placement`.

## Task 5: Windows primitives

**Files:** create `ui/platform/mod.rs`, `ui/platform/windows/mod.rs`, `ui/platform/windows/primitives.rs`; modify `Cargo.toml`, `lib.rs`.

- [x] Move registry theme lookup into shared Windows helper.
- [x] Detect DWM/composition, high contrast, reduced motion, DPI, and monitor work area using verified `windows-sys` APIs.
- [x] Add `colorref`, `scale_px`, `detect_capabilities`, `system_theme`, `work_area_for`, and surface styling helpers.
- [x] Keep all Windows imports gated; add only necessary feature flags.
- [x] Run Windows build/test; commit `feat: add Windows adaptive rendering primitives`.

## Task 6: Host state/action bridge

**Files:** modify `app.rs`, `ui/model.rs`, `mixer.rs`, `tray.rs`.

- [x] Add `AppState` and capabilities to `AppContext`.
- [x] Centralize confirmed-state publication so `last_state`, shared state, overlay, tray, mixer, Settings, and Help synchronize together.
- [x] Convert mixer messages, hotkeys, and tray commands into `AppAction` at the host boundary while keeping audio/config/hotkey mutation in the host.
- [x] Preserve blacklist/beep behavior and add action mapping tests.
- [x] Run Windows build/test; commit `refactor: centralize confirmed UI state and actions`.

## Task 7: Adaptive overlay

**Files:** modify `overlay.rs`, `app.rs`, Windows primitives.

- [x] Replace local colors with shared tokens.
- [x] Preserve bottom-right, click-through, topmost/no-activate, threshold bar, toast, and auto-hide behavior.
- [x] Use monitor work areas and shared placement; apply material and motion fallback policy.
- [x] Keep geometry constants as the single source consumed by mixer placement.
- [x] Run Windows build/test; commit `refactor: apply adaptive tokens to volume overlay`.

## Task 8: Adaptive mixer

**Files:** modify `mixer.rs`, `app.rs`, Windows primitives.

- [x] Replace local theme with shared tokens and Windows helpers.
- [x] Preserve canonical trackbar IDs/range, no feedback loop, synchronization, visible `×` close button, and host routing.
- [x] Use work-area placement and shared mixer-above-overlay math.
- [x] Add Tab/Enter/Space/Escape behavior and visible focus cues.
- [x] Run Windows build/test; commit `refactor: apply adaptive Fluent styling to mixer`.

## Task 9: Settings draft state machine

**Files:** create `ui/settings.rs`; modify `ui/mod.rs`.

- [x] Add `SettingsDraft { original, current, dirty, error }` with `new`, `replace`, `reset`, `validate`, `commit`, and `cancel`.
- [x] Keep edits in memory until host persistence succeeds; add cancel/reset/validation tests.
- [x] Run focused Settings tests; commit `feat: add Settings draft and apply state machine`.

## Task 10: Native Windows Settings

**Files:** create `settings.rs`; modify `app.rs`, `lib.rs`, Windows primitives.

- [x] Build a modeless native Win32 Settings window using standard controls only.
- [x] Expose General, Hotkeys/conflicts, Appearance, Blacklist, Feedback, and Storage/actions sections.
- [x] Add controls for all existing config fields and appearance preferences, Apply/Save and Close/Cancel/Reset/Open Config actions.
- [x] Read controls into a draft; keep invalid drafts visible; persist only after validation succeeds; preserve edits on write failure.
- [x] Re-register hotkeys only after a successful modifier change; route all mutation through host; use shared theme/accessibility tokens.
- [x] Run Windows build/test; commit `feat: add native Windows Settings window`.

## Task 11: Hotkey status, Help, and Tray unification

**Files:** modify `hotkeys_win32.rs`, `help.rs`, `tray.rs`, `app.rs`.

- [x] Expose per-action registration status and Win32 error codes without changing conflict policy.
- [x] Add Tray Settings command and route Mixer, Settings, Help, mute, reset, reload, blacklist, and Exit through host actions.
- [x] Refactor Help to shared light/dark/high-contrast tokens and add Settings affordance.
- [x] Run Windows build/test; commit `feat: expose hotkey status and unify surface actions`.

## Task 12: macOS/Linux renderer seams

**Files:** create target-gated `ui/platform/macos/mod.rs` and `ui/platform/linux/mod.rs`; modify platform exports and bilingual README files.

- [x] Define a renderer boundary consuming only shared model/tokens/capabilities; do not import Windows APIs into shared code.
- [x] Keep current non-Windows CLI fallback and avoid adding unneeded runtime dependencies.
- [x] Document Windows as the verified renderer and macOS/Linux as follow-on native AppKit/CoreAudio and GTK/libadwaita/PulseAudio/PipeWire work.
- [x] Run available non-Windows compile check honestly; rerun Windows build/test; commit `feat: add macOS and Linux UI renderer seams`.

## Task 13: Windows verification

- [x] Run fresh Windows build and test.
- [x] Verify light/dark/high-contrast/reduced-motion behavior, keyboard-only navigation, 100/125/150% DPI, primary/secondary work areas, taskbar changes, material fallback, and mixer/overlay separation.
- [x] Verify slider/readback convergence, external sync, close/toggle from button/hotkey/tray/Escape, Settings Apply/Cancel/Reset/error behavior, and resource/message-loop stability.
- [x] Record fresh evidence in `claude-progress.md`; do not reuse stale evidence for changed code.

## Task 14: Tracking and final repository verification

**Files:** modify `feature_list.json`, `claude-progress.md`, and `session-handoff.md` if needed.

- [x] Add the adaptive UI feature as `in_progress`; only set `passing` after all required checks pass.
- [x] Record macOS/Linux as unverified follow-on work.
- [x] Run `cargo fmt --all -- --check`, Windows build/test, `git diff --check`, and `git status --short`.
- [x] Ensure `.superpowers/`, runtime config, and unrelated files are not staged.
- [x] Commit the verified milestone and leave a clean restart path.

## Follow-on plans

Native macOS AppKit/CoreAudio and Linux GTK/libadwaita/PulseAudio/PipeWire implementation and verification are separate plans after the Windows visual baseline is frozen. No macOS/Linux UI feature is passing in this plan.
