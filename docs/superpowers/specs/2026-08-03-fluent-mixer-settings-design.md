# Fluent Mixer and Native Settings Design

**Date:** 2026-08-03
**Status:** Draft for review
**Scope:** Windows native UI improvements for the existing Rust VolumeControl app

## Goals

1. Make the Volume Mixer visually consistent with the Windows 11 flyout while keeping the project lightweight and compatible.
2. Add a native Settings window so users can configure the app without manually editing `config.json`.
3. Preserve the current Rust + Win32 + `windows-sys` architecture and MSVC build path.
4. Keep the original VolumePro hotkey model and make registration conflicts explicit and actionable.
5. Preserve the existing single message loop and audio source-of-truth model.

## Non-goals

- Do not introduce Windows App SDK, WinUI 3 runtime, XAML, C++/WinRT, MSIX packaging, or a second UI runtime.
- Do not replace the existing WASAPI backend, tray, overlay, or cross-platform scaffolds.
- Do not silently change the user's configured modifier when a hotkey is unavailable.
- Do not remove JSON persistence; the Settings window is a native editor for the same config model.
- Do not implement per-application volume control in this scope.

## Product behavior

### Hotkeys and conflicts

The default modifier remains `Ctrl+Alt`, matching the original repo:

- `Mod+Up/Down`: small volume step
- `Shift+Mod+Up/Down`: large volume step
- `Mod+WheelUp/Down`: small/large volume step according to Shift
- `Mod+M`: mute toggle
- `Mod+V`: mixer toggle
- `Mod+R`: reset to 50%
- `Shift+Mod+M`: tray menu
- Media volume keys remain native Windows keys and are not registered by the app.

Each registration is attempted independently. If another application owns a combination, the app continues running and records a structured status for that action. The UI shows the action, resolved combination, and an unavailable/conflict state with guidance to choose another modifier or use the blacklist. The configured modifier is never changed automatically.

The existing recommended blacklist presets remain available for `Alt` and `Ctrl`. `Ctrl+Alt` and `CapsLock` have an empty recommended blacklist, matching the original behavior.

### Mixer

The mixer remains a captionless, topmost, non-taskbar Win32 popup positioned at the bottom-right. It is restyled as a Fluent-like card:

- rounded DWM corners and a blue accent strip;
- Mica/system backdrop when supported, with a solid dark/light fallback;
- Windows theme-aware colors selected from `GetSysColor`/system theme state;
- Segoe UI/Segoe UI Variable font fallback;
- speaker icon and large percentage header;
- `System Volume` subtitle;
- custom-painted slider with selected track, thumb, hover/focus/pressed states;
- owner-drawn Mute/Unmute and Reset buttons with keyboard focus cues;
- muted icon/state and an `Unmute` action when muted;
- DPI-aware dimensions and placement.

Mixer state is synchronized before showing. The data flow remains one-way after user input:

```text
slider/button -> mixer window proc -> host WM_APP message -> WASAPI
              -> refresh_ui() -> mixer sync + overlay/tray update
```

Programmatic `TBM_SETPOS` updates never post a user-change message. Slider messages use the verified Win32 TBM constants and clamp values to 0..100. Audio readback remains authoritative.

### Settings

Add a native, modeless Settings window opened from the tray and from a new mixer/settings affordance. It uses the same Win32/GDI/DWM styling primitives as the mixer and is owned by the app's host window.

The first version exposes every current configuration field:

#### General

- small volume step (`1..50`)
- large volume step (`small step + 1..50`)
- overlay duration (`200..10000 ms`)
- modifier (`Ctrl+Alt`, `CapsLock`, `Alt`, `Ctrl`)

#### Hotkeys and conflicts

- read-only list of all actions and their resolved combinations;
- registration status: available, conflict, or hook-backed;
- conflict detail and recommendation to change modifier/blacklist;
- refresh status after modifier changes and live reload.

#### Mixer and overlay appearance

- theme: System, Dark, Light;
- accent color chosen from a small validated palette;
- overlay duration shortcut to the General setting;
- optional toggle for showing the custom overlay on app-triggered changes.

If new appearance fields are not yet part of the persisted `Config`, they must be added with serde defaults and validation rather than kept as UI-only state.

#### Blacklist

- editable list of executable names;
- add, remove, and clear actions;
- apply recommended blacklist for the selected modifier;
- normalization to lowercase `name.exe` values;
- explanatory text describing that blacklisting suppresses custom hotkeys only while that process is foreground.

#### Sound feedback

- master enable toggle;
- blocked-hotkey frequency and duration;
- volume-limit frequency and duration;
- numeric ranges match current validation.

#### Storage and actions

- Apply: validate and persist atomically, then apply live changes;
- Save/Close: persist valid changes and close;
- Cancel: discard uncommitted edits;
- Reset to defaults: confirm before replacing current form values;
- Open config folder/path for troubleshooting.

The existing mtime reload remains a compatibility path for external edits. Settings saves should update the in-memory context directly after a successful atomic write to avoid a visible reload race. On write failure, keep the form open, preserve edits, and show an actionable error.

## Architecture

Add a `settings` Windows module with a `Settings` owner object and a `SettingsData` window state stored through `GWLP_USERDATA`, following the existing mixer/help pattern. Keep controls and model conversion separate:

- `SettingsDraft`: editable form values and validation errors;
- `Settings::show(config, hotkey_status)`: populate controls and display;
- settings window proc: translates control events to host messages;
- app host: validates, persists, updates `AppContext`, re-registers hotkeys, and refreshes UI.

Introduce a shared hotkey registration-status model rather than deriving status from log text. `Win32Hotkeys::register` should retain per-action outcomes while preserving the current non-fatal behavior. The app passes a snapshot to Help/Settings.

Use host messages for settings actions (`Apply`, `Cancel`, `Reset`, `OpenConfigFolder`) so audio/config mutation stays on the app's existing message-loop owner thread.

## Persistence and validation

Extend `Config` only where appearance/settings behavior requires it, with serde defaults so existing files remain valid. Save via a temporary file in the same directory followed by replacement/rename; if replacement is unavailable on the platform, return an error rather than partially writing the config.

Validation rules:

- small step: 1..50;
- large step: strictly greater than small step and <=50;
- overlay duration: 200..10000 ms;
- blacklist: trim, lowercase, retain only `.exe` entries;
- beep frequencies/durations: retain existing validated ranges;
- theme/accent: enum/palette values only.

## Error handling

- Mixer child/control creation failure aborts mixer creation with a clear error.
- DWM styling failures are non-fatal and fall back to solid painting.
- Hotkey conflicts are non-fatal, visible in Settings/Help, and logged with action + combination.
- Settings validation errors stay local to the form and identify the invalid field.
- Config write errors do not discard the draft; show the path and OS error.
- Audio write/read errors log and leave the last confirmed UI state authoritative.
- Window close, Escape, and app shutdown must release all GDI brushes/fonts and child state exactly once.

## Verification plan

### Automated/build

- `scripts/win-build.bat build`
- `scripts/win-build.bat test`
- non-Windows compile remains gated and unaffected where possible;
- unit tests for config draft validation, normalization, theme defaults, and hotkey status mapping.

### Windows behavioral checks

1. Start with default `Ctrl+Alt`; confirm all available registrations and report known external conflicts without terminating.
2. Change modifier from Settings to `CapsLock`, `Alt`, and `Ctrl`; confirm live re-registration and status updates.
3. Add/remove blacklist entries and confirm foreground-app suppression.
4. Open mixer from tray and hotkey; confirm it opens with current WASAPI percentage before becoming visible.
5. Drag slider across low/mid/high values; confirm slider, label, WASAPI state, overlay, and tray agree.
6. Toggle mute/unmute and reset; confirm icon, button label, slider, and audio state agree.
7. Exercise light/dark/system appearance and fallback when DWM backdrop is unavailable.
8. Open Settings, edit each field, Apply, restart, and confirm persistence.
9. Cancel and Reset-to-defaults behavior; confirm no unintended writes.
10. Verify no feedback-loop burst after slider changes and no stale 1%/50% display.

## Acceptance criteria

- Existing app build/tests continue to pass.
- Mixer matches the provided Windows 11-style reference substantially better than the current plain controls.
- Users can configure all current settings from the Settings window without editing JSON.
- Default hotkeys remain compatible with the original repo and conflicts are explicit rather than silently remapped.
- Mixer and Settings remain native, lightweight, and buildable through the existing MSVC wrapper.
- Verification evidence is recorded in `claude-progress.md` and the relevant feature state is updated only after runnable checks pass.
