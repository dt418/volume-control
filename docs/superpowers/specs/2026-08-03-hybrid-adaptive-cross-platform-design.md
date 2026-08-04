# Hybrid Adaptive Cross-Platform UI Design

**Date:** 2026-08-03
**Status:** Implemented (2026-08-03–04); UI direction superseded by the Signal Glass production design
**Scope:** Full VolumeControl application UI, Windows-first delivery with later macOS/Linux expansion

## Goals

1. Give the complete app one coherent UX while allowing each operating system to retain its native visual language.
2. Make Windows the first fully polished target using the existing Rust + Win32 + `windows-sys` architecture.
3. Prepare macOS and Linux for native renderers without copying Windows APIs or making unsupported UI claims.
4. Provide polished glass/translucent styling with restrained droplet-inspired highlights, but always retain an opaque compatible fallback.
5. Preserve the current audio source-of-truth model, single host/application flow, live config behavior, tray actions, hotkeys, overlay, and mixer synchronization.
6. Keep the app lightweight: no WinUI 3, Windows App SDK, XAML, WebView, mandatory GPU shader pipeline, or second UI runtime.

## Design decisions

### Visual direction

The selected direction is **Hybrid adaptive**:

- Shared UX concepts, component roles, state/action model, design tokens, accessibility rules, and fallback policy.
- Platform-specific renderers, materials, typography defaults, window behavior, and native integration.
- Windows: Fluent/Mica-inspired surfaces, rounded cards, Windows accent colors, restrained motion.
- macOS: later AppKit renderer with Liquid Glass-inspired translucency, soft highlights, and droplet-like depth cues where supported.
- Linux: later GTK/libadwaita-inspired renderer, following the active desktop theme and compositor capabilities.

The visual profile is **A — polished but safe**:

- Use blur/translucency only when the platform and compositor support it.
- Prefer restrained gradients and soft highlights over expensive distortion.
- Fall back automatically to an opaque surface with equivalent contrast and hierarchy.
- Respect reduced-motion and high-contrast preferences.
- Do not claim to reproduce Apple's exact private or OS-specific UI. Droplet effects mean a subtle glass/highlight language, not a dependency on iOS/macOS-only APIs.

### Delivery order

1. Establish shared UI model, tokens, capabilities, and surface contracts.
2. Finish the Windows renderer and all Windows surfaces.
3. Freeze a verified Windows visual baseline.
4. Add macOS audio/UI adapters and native renderer.
5. Add Linux audio/UI adapters and native renderer.
6. Verify each target on native hardware/desktop environments before changing its feature status to passing.

## Shared architecture

The shared layer must not import Win32, AppKit, GTK, or platform-specific audio APIs.

Suggested module boundaries:

```text
crates/volumectl/src/ui/
├── model.rs          # AppState, AppAction, surface and preference enums
├── theme.rs          # design tokens and light/dark palettes
├── capabilities.rs   # compositor, DPI, accessibility, motion, work area
├── surface.rs        # surface lifecycle/state synchronization contracts
└── platform/
    ├── windows/      # first implementation
    │   ├── mixer.rs
    │   ├── overlay.rs
    │   ├── settings.rs
    │   └── primitives.rs
    ├── macos/        # later AppKit implementation
    └── linux/        # later GTK/libadwaita implementation
```

The exact file split may be adjusted during implementation if existing module boundaries are safer, but the dependency direction is fixed:

```text
                 ┌──────────────────────────┐
                 │ shared model/theme/      │
                 │ capabilities              │
                 └────────────┬─────────────┘
                              │
              ┌───────────────┴────────────────┐
              │                                │
┌─────────────▼─────────────┐    ┌────────────▼────────────┐
│ application host +        │    │ platform renderer +     │
│ audio/config/hotkey/tray  │    │ OS integration adapter  │
│ services                  │    │                          │
└─────────────┬─────────────┘    └────────────▲────────────┘
              │                               │
              └──── confirmed AppState ──────┘
                    AppAction → host
```

### Shared model

The UI contract should represent:

- `AppState`: confirmed volume, mute state, active output device, surface visibility, error/status information.
- `AppAction`: volume changes, mute/reset, mixer toggle/close, settings/help open, theme/material/motion preference changes, config apply/cancel/reset, tray commands.
- `SurfaceId`: Overlay, Mixer, Settings, Help, Tray.
- `ThemeMode`: System, Light, Dark.
- `MotionMode`: Full, Reduced, Disabled.
- `MaterialMode`: Auto, Translucent, Opaque.
- Capability flags: compositor/transparency, blur/backdrop, high contrast, reduced motion, DPI scale, multi-monitor work area.

Audio state remains authoritative in the platform audio backend. A renderer may optimistically animate input, but it must settle from the confirmed `AppState` after the host applies and reads back the change.

### Shared design tokens

Tokens cover:

- light/dark background and surface colors;
- primary/secondary/disabled text colors;
- accent and volume threshold colors;
- focus and error indicators;
- spacing scale;
- corner radii;
- border and shadow/elevation levels;
- default typography roles;
- surface opacity and blur intent;
- animation durations and easing policy;
- minimum hit targets;
- contrast requirements.

Tokens describe intent, not fixed pixels. Platform renderers may map them to native metrics and controls while preserving hierarchy and accessibility.

## Windows renderer

Windows remains native Rust + Win32 + `windows-sys`, gated behind `#[cfg(target_os = "windows")]`. Existing WASAPI, RegisterHotKey, tray, overlay, and message-loop ownership are retained.

### Overlay

- Bottom-right, click-through, always-on-top, auto-hiding popup.
- Fluent/Mica-inspired surface when DWM/compositor capability permits.
- Accent-colored volume bar and clear percentage/mute state.
- Fade/slide animation only in Full motion mode.
- Opaque GDI fallback when backdrop/translucency is unavailable.
- Placement uses monitor work area and shared geometry so it never overlaps the mixer.

### Mixer

- Captionless, topmost, non-taskbar card.
- Header with title, current percentage, mute state, and visible close affordance.
- Native slider and action controls with theme-aware styling and keyboard focus cues.
- Fluent-like spacing, rounded corners, accent indicator, and soft shadow/backdrop where supported.
- Synchronizes before becoming visible and after every user or external volume change.
- Close, hotkey, and tray paths all use the same visibility state and toggle contract.

### Settings

A native modeless Win32 settings window will expose the existing configuration without requiring manual JSON edits:

- General: small/large volume steps, overlay duration, modifier.
- Hotkeys/conflicts: action list, availability, conflict details, guidance.
- Appearance: System/Light/Dark, material preference, motion preference, accent palette, overlay behavior.
- Blacklist: add/remove/clear entries and recommended presets.
- Feedback: beep enable, frequency, and duration values.
- Storage/actions: Apply, Save/Close, Cancel, Reset defaults, open config location.

Settings changes are validated and persisted safely. Successful Apply updates the host context directly and re-registers hotkeys as needed. Failed writes preserve the draft and display an actionable error.

### Help and Tray

- Help uses the same token hierarchy and theme state as Settings/Mixer.
- Tray menu exposes Mixer, Settings, Help, mute/reset, reload/config, blacklist assistance, and Exit.
- Menu actions dispatch through the same `AppAction`/host path rather than duplicating audio mutation logic.

## macOS and Linux expansion

These targets are explicitly later phases, not part of the Windows completion claim.

### macOS

- Use AppKit-native windows and controls for Mixer, Overlay, Settings, Help, and menu bar integration.
- Use CoreAudio for the audio backend and native global shortcut/accessibility mechanisms where available.
- Map shared tokens to macOS typography, spacing, materials, and vibrancy.
- Use Liquid Glass-inspired translucent cards, soft highlights, and restrained droplet-like depth only when the window server supports it.
- Provide opaque fallback and reduced-motion behavior for older macOS versions or accessibility settings.

### Linux

- Prefer GTK/libadwaita-compatible native surfaces and desktop theme integration.
- Support PulseAudio/PipeWire through a platform audio adapter selected during implementation.
- Use compositor-aware transparency only when available; otherwise render a clean opaque card.
- Respect desktop dark/light theme, high contrast, reduced motion, scale factor, and work area.
- Tray/global shortcut behavior must be treated as desktop-environment capabilities, not assumed universally.

## Capability and fallback policy

Each renderer chooses the strongest safe presentation at runtime:

1. Translucent surface with blur/backdrop when supported.
2. Translucent surface without blur when alpha composition is supported.
3. Opaque surface using the same token hierarchy when composition is unavailable or expensive.
4. High-contrast palette when requested by the platform.

Motion policy:

- Full: short fade/slide transitions and subtle surface response.
- Reduced: opacity/state transitions only; no droplet movement or spring effects.
- Disabled: immediate state changes.

No visual effect may block input, reduce text contrast below accessibility requirements, or become the only indication of state.

## Data flow

```text
OS audio/event source
        ↓
platform audio adapter
        ↓
confirmed AppState
        ↓
platform surface renderer
        ↓
user input → AppAction
        ↓
application host
        ↓
audio/config/hotkey/tray services
        ↓
readback → confirmed AppState → all open surfaces
```

The single host/application owner remains responsible for audio mutation, config persistence, hotkey registration, and cross-surface synchronization. Renderers do not write config files or audio state directly.

## Error handling

- Unsupported DWM/AppKit/GTK material APIs are non-fatal; use opaque rendering.
- Audio write/read errors preserve the last confirmed state and show an actionable status.
- Config validation errors stay in the Settings draft.
- Config write errors do not discard edits.
- Hotkey conflicts remain explicit and never trigger silent modifier changes.
- Missing tray/global shortcut/compositor support is reported as capability status, not a crash.
- Surface creation/destruction must release native resources exactly once.

## Verification plan

### Shared automated tests

- AppState/AppAction conversion and serialization.
- Theme token selection for System/Light/Dark.
- Capability fallback ordering.
- Reduced-motion and high-contrast policy.
- Monitor/work-area placement math.
- Minimum hit targets and range validation.
- No feedback-loop behavior when syncing programmatic slider changes.

### Windows verification

- `scripts/win-build.bat build`.
- `scripts/win-build.bat test`.
- Mixer, Overlay, Settings, Help, and Tray use matching light/dark tokens.
- Mixer and Overlay rectangles remain separated on the primary and secondary work areas.
- Slider, mute, reset, hotkey, tray, and external volume changes converge to the same WASAPI state.
- Close/toggle works from visible button, hotkey, and tray.
- Test 100%, 125%, and 150% DPI; multiple monitors; taskbar/work-area changes.
- Test keyboard-only navigation, focus cues, high contrast, reduced motion, and compositor-disabled fallback.
- Confirm no timer, GDI brush/font, window, or message-loop leaks.

### macOS/Linux verification

- Do not mark platform UI features passing without native builds and runtime checks on representative systems.
- Verify audio backend, tray/menu integration, shortcut capability, theme detection, compositor fallback, DPI/scale, and accessibility behavior per desktop environment.

## Acceptance criteria

- Windows has a consistent Fluent/adaptive visual system across every surface.
- Glass/droplet-inspired effects are polished but never required for usability.
- The app remains lightweight and native; no mandatory second UI runtime is added.
- Shared state/action/theme contracts are independent of platform UI APIs.
- Windows is fully verified before macOS/Linux renderer work is declared complete.
- macOS/Linux expansion has clear native integration seams and honest verification status.
- Existing audio, hotkey, tray, overlay, config, and synchronization behavior remains intact.
