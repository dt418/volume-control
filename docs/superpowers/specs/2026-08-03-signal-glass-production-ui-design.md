# Signal Glass Production UI Design

> Approved design specification for the next VolumeControl UI implementation phase.

**Date:** 2026-08-03
**Status:** Implemented — Windows live-verified, macOS/Ubuntu renderers CI-smoke-tested (2026-08-04)
**Product:** VolumeControl
**Target audience:** People who need fast, reliable system-volume control without opening a full audio mixer.

> **Implementation status (2026-08-04):** All 14 tasks of the implementation
> plan are complete and merged to master (PR #3, merge `68baeac`). Windows is
> the fully implemented, live-verified target (Session 003–008 matrix in
> claude-progress.md); the macOS 26 AppKit and Ubuntu 24.04 GTK4/libadwaita
> renderers implement the same surface contract and their smoke tests pass on
> real CI runners (first green run `30899670095`: `appkit smoke OK` on
> macos-15 arm64, `gtk smoke OK` on ubuntu-24.04 under `xvfb-run`).
> `feature_list.json` keeps vol-011 at `in_progress` until the
> human-confirmation remainder is verified live (see §14 note below).

## 1. Goal

Deliver a production-ready native UI system that feels intentionally designed on each supported platform while preserving VolumeControl's existing host-owned audio/configuration architecture:

- Fluent-inspired native surfaces on Windows 11.
- Native glass/material surfaces on macOS 26 where public AppKit APIs support them.
- Glass-inspired, compositor-aware surfaces on Ubuntu 24.04 LTS with Wayland and X11 fallback.
- A shared visual language, state model, action contract, accessibility semantics, and opaque fallback across all renderers.

The UI must be beautiful when material effects are available and equally usable and coherent when they are not.

## 2. Design direction: Signal Glass

VolumeControl is treated as a signal instrument rather than a generic settings form. The interface communicates three things with minimal visual noise:

1. Current output volume.
2. Current mute/output state.
3. The next available control action.

The distinctive product signature is the **Signal Rail**: a shared volume visualization used by the overlay and mixer. It combines threshold-aware fill, a precise thumb/marker, and an explicit muted state. The rail is data-bearing, not decorative.

Visual restraint is deliberate:

- No large hero gradients, ambient glow, or perpetual animation.
- Material effects are subtle and never required for comprehension.
- Volume threshold colors remain authoritative from the existing VolumePro palette.
- Every color-only state also has text, shape, or icon support.

## 3. Architecture

### 3.1 Shared layer

The existing platform-neutral modules remain the source of truth:

```text
crates/volumectl/src/ui/model.rs          AppState, AppAction, SurfaceId
crates/volumectl/src/ui/theme.rs          semantic tokens and visual scales
crates/volumectl/src/ui/capabilities.rs   DPI, contrast, compositor, fallback
crates/volumectl/src/ui/surface.rs        work-area placement and geometry
crates/volumectl/src/ui/settings.rs       draft, validation, commit state machine
crates/volumectl/src/app.rs               host action bridge and confirmed state
```

Shared code contains no Windows, AppKit, GTK, CoreAudio, PulseAudio, or PipeWire imports.

### 3.2 Native renderer adapters

Each platform owns lifecycle, event translation, painting, and accessibility integration through a native renderer adapter:

```rust
pub trait NativeRenderer {
    type Error;

    fn create(
        host: HostHandle,
        capabilities: UiCapabilities,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized;

    fn publish(
        &mut self,
        state: &AppState,
        tokens: &ThemeTokens,
        capabilities: &UiCapabilities,
    );

    fn dispatch(&mut self, action: AppAction);
    fn destroy(&mut self);
}
```

The exact platform handle types may remain target-gated. The contract is normative:

- Renderers consume confirmed `AppState`.
- User intent becomes `AppAction`.
- Renderers never mutate audio, configuration, or hotkey registration directly.
- The host reads authoritative state after a mutation and republishes it.
- Surface visibility and status are settled from confirmed state.

### 3.3 Platform baselines

| Platform | Baseline | Native stack | Material policy |
|---|---|---|---|
| Windows | Windows 11 | Win32, Direct2D, DirectWrite, DWM, native controls | Mica/Acrylic-like when available; translucent or opaque fallback |
| macOS | macOS 26 | AppKit, `objc2`, CoreAudio, native controls | Public native material/glass APIs; translucent or opaque fallback |
| Linux | Ubuntu 24.04 LTS | GTK4, libadwaita, layer-shell where available, PipeWire/PulseAudio | Compositor-aware glass-inspired treatment; opaque fallback |

Windows remains the first implementation target. macOS and Linux must be built as explicit follow-on renderer work, not represented as passing solely because shared seams compile.

## 4. Global visual system

### 4.1 Spacing

Use a 4px logical grid. Renderers scale logical pixels exactly once using the platform DPI/scale API.

| Token | Value | Use |
|---|---:|---|
| `xs` | 4px | icon/text gap, small alignment adjustment |
| `sm` | 8px | label/control gap, compact row gap |
| `md` | 12px | control row spacing |
| `lg` | 16px | card padding, section spacing |
| `xl` | 24px | surface padding, major grouping |
| `xxl` | 32px | window-level separation |

### 4.2 Radius

| Token | Value | Use |
|---|---:|---|
| `control` | 4px | buttons, fields, combo boxes |
| `card` | 8px | nested groups |
| `surface` | 12px | overlay, mixer, settings, Help surface |
| `pill` | 999px | status badges and compact indicators |

### 4.3 Typography

Use native system UI typography with deliberate hierarchy:

| Role | Preferred face | Size | Weight | Use |
|---|---|---:|---:|---|
| `display_value` | Segoe UI Variable / native system UI | 28px | 600 | live volume value |
| `surface_title` | Segoe UI Variable / native system UI | 17px | 600 | surface title |
| `section_title` | Segoe UI Variable / native system UI | 15px | 600 | Settings/Help group title |
| `body` | Segoe UI Variable / native system UI | 13px | 400 | primary content |
| `label` | Segoe UI Variable / native system UI | 12px | 600 | field labels, eyebrow |
| `caption` | Segoe UI Variable / native system UI | 11px | 400 | helper text |
| `keycap` | Cascadia Mono / platform monospace | 12px | 600 | hotkey combinations |

Volume values use tabular or fixed-width numerals where the native text API supports them. Fallbacks are Segoe UI, Segoe UI Variable, native system UI, and platform monospace equivalents.

### 4.4 Semantic color tokens

Existing threshold colors remain unchanged:

```text
muted  #888888
low    #27AE60
medium #0078D4
high   #E05C00
```

The new surface palette is:

| Token | Light | Dark |
|---|---|---|
| `background` | `#F7F9FC` | `#10131A` |
| `surface` | `#FFFFFF` | `#171C24` |
| `surface_elevated` | `#FFFFFF` | `#202735` |
| `surface_subtle` | `#F1F4F8` | `#1C222D` |
| `border` | `#D7DEE8` | `#344052` |
| `border_strong` | `#AEB9C8` | `#536276` |
| `text_primary` | `#17202B` | `#F5F7FA` |
| `text_secondary` | `#526071` | `#AAB4C3` |
| `text_disabled` | `#8995A3` | `#758192` |
| `accent` | `#0067C0` | `#3AA8FF` |
| `accent_hover` | `#005AAB` | `#62B8FF` |
| `accent_pressed` | `#004A8D` | `#198FEA` |

High-contrast mode uses opaque surfaces and strong borders. Secondary/disabled information must not rely on tint alone. Focus uses a visible two-layer ring.

### 4.5 Semantic states

Every interactive surface/control defines these states:

- Normal.
- Hover.
- Pressed.
- Focused.
- Disabled.
- Selected.
- Invalid.
- Muted where applicable.

The state must remain distinguishable under light, dark, high contrast, and color-vision limitations.

## 5. Overlay specification

### 5.1 Purpose and geometry

The overlay is a status capsule that answers current volume, output identity, and mute state at a glance.

Logical design size:

```text
Width: 336px
Height: 88px
```

The existing bottom-right work-area placement and click-through/topmost behavior remain. Mixer placement continues to guarantee an exact 16px vertical gap above the overlay.

### 5.2 Layout

```text
┌────────────────────────────────────┐
│  Volume                       72%  │
│  System output                     │
│  ━━━━━━━━━━━━━━━●────────────     │
└────────────────────────────────────┘
```

- Surface padding: 16px.
- Title left-aligned; value right-aligned.
- Output identity uses secondary text.
- Signal Rail spans the content width.
- Rail height: 8px.
- Thumb/marker diameter: 12px.
- Surface has a 1px border in opaque mode.
- Elevation/shadow is optional and must have an opaque equivalent.

### 5.3 States

Normal:

```text
Volume                         72%
System output
━━━━━━━━━━━━━━━●────────────
```

Muted:

```text
Volume                       Muted
System output
━━━━━━━━━━━━━━━◇────────────
```

The muted state uses the `Muted` label and an outline/diamond marker. It never relies only on gray.

Text toast:

```text
┌────────────────────────────────────┐
│  ✓  Settings saved                 │
└────────────────────────────────────┘
```

Toasts use a status glyph/shape and text; they do not show an irrelevant volume rail.

### 5.4 Motion

- Full motion: opacity plus 4px vertical translation, 120ms.
- Reduced motion: opacity only, 120ms.
- Disabled motion: immediate presentation.
- Existing auto-hide duration remains authoritative.

## 6. Mixer specification

### 6.1 Purpose and geometry

The mixer is the precision control card, not an enlarged overlay.

Logical design size:

```text
Width: 400px
Height: 224px
```

Placement invariant remains:

```text
mixer.bottom + 16px == overlay.top
```

### 6.2 Layout

```text
┌────────────────────────────────────────┐
│  VOLUME MIXER                     ×    │
│  System output                         │
│                                        │
│                            72%         │
│  ━━━━━━━━━━━━━━━●────────────          │
│                                        │
│  [  Mute  ]       [ Reset to 50% ]     │
└────────────────────────────────────────┘
```

- Eyebrow: `VOLUME MIXER`, 11px semibold.
- Close control: 32px minimum hit target.
- Output label: secondary text.
- Live value: 28px, right-aligned.
- Signal Rail: custom-drawn visual synchronized with a native trackbar used for keyboard and accessibility semantics.
- Buttons: Mute as secondary, Reset as quiet/tertiary.
- Button height: minimum 32px, preferred 36px.
- Minimum button gap: 8px.

### 6.3 Interaction

Tab order is:

```text
Slider → Mute → Reset → Close
```

Required behavior:

- Arrow Left/Right adjusts by the configured small step.
- Shift plus Arrow adjusts by the configured large step where supported by the native event path.
- Home sets 0%; End sets 100%.
- Space/Enter activate the focused button.
- Escape hides the mixer.
- Focus remains stable after confirmed-state publication.
- Slider changes always go through the host action bridge and audio readback.

Muted state changes button text to `Unmute`, adds a text status cue, and changes the rail marker shape.

## 7. Settings specification

### 7.1 Purpose and geometry

Settings explains behavior in user terms and avoids exposing implementation details unnecessarily.

Desktop logical size:

```text
Width: 760px
Height: 620px
```

Minimum logical size:

```text
Width: 620px
Height: 520px
```

If the available work area or scale makes the desktop layout too narrow, the navigation rail becomes a stacked section selector without removing content.

### 7.2 Desktop layout

```text
┌────────────────────────────────────────────────────────────┐
│ Settings                                              ×    │
│ Configure how VolumeControl behaves                        │
├────────────────────┬───────────────────────────────────────┤
│ General            │ General                               │
│ Hotkeys            │ Adjust how volume changes feel        │
│ Appearance         │                                       │
│ Blacklist          │ Volume step                           │
│ Feedback           │ [  2                              ]   │
│ Storage            │ Small change applied by ↑ / ↓         │
│                    │                                       │
│                    │ Large step                            │
│                    │ [ 10                             ]   │
│                    │ Shift + ↑ / ↓                        │
│                    │                                       │
│                    │                       [Save changes]  │
│                    │ [Reset]                 [Cancel]      │
└────────────────────┴───────────────────────────────────────┘
```

Header:

- Title: `Settings`.
- Subtitle: `Configure how VolumeControl behaves`.
- Close hit target is at least 32px.
- A small accent rail/signal indicator identifies the surface without becoming decoration.

Navigation sections:

- General.
- Hotkeys.
- Appearance.
- Blacklist.
- Feedback.
- Storage.

Selected navigation item uses an elevated surface and 3px accent rail. Text remains visible in high contrast.

### 7.3 Section content

Every section uses the same structure: title, one-line description, grouped controls, helper text, and inline errors.

**General**

- Volume step.
- Large volume step.
- Overlay duration.
- Helper text explains the behavior in plain terms.

**Hotkeys**

- Modifier selector.
- Per-hotkey registration status.
- Conflict callout with the real registration status and a direct path to the modifier control.

**Appearance**

- Theme: System, Light, Dark.
- Material: Auto, Translucent, Opaque.
- Motion: Full, Reduced, Disabled.
- Accent: System, Blue, Green, Purple, Orange.
- Small Signal Rail preview using the resolved tokens.

**Blacklist**

- Add entry.
- List entries.
- Remove, Clear, and Recommended actions.
- Empty state copy:

```text
No blocked applications
VolumeControl will respond to shortcuts everywhere.
[Add application]
```

**Feedback**

- Beep enabled.
- Blocked beep frequency/duration.
- Limit beep frequency/duration.
- Helper text uses user-facing language rather than implementation terms.

**Storage**

- Config path.
- Open config file.
- Reload configuration.
- Current storage status.
- Storage actions remain separate from reset/destructive actions.

### 7.4 Footer and draft behavior

Actions:

- Primary: `Save changes`.
- Secondary: `Cancel`.
- Quiet: `Reset`.

Save is disabled when the draft is clean. On success, the draft baseline updates, the current section remains visible, and the window reports `Changes saved`. On validation failure, the first invalid field receives focus, the inline error explains the correction, and all edits remain. On persistence failure, the window remains open and edits remain intact.

The existing `SettingsDraft` state machine and host-owned persistence rules are retained.

## 8. Help specification

### 8.1 Purpose and geometry

Help is a scannable quick-reference card, not a whitespace-aligned text dump.

Logical design size:

```text
Width: 520px
Height: 500px
```

### 8.2 Layout

```text
┌──────────────────────────────────────────────┐
│ VolumeControl                           ×    │
│ Keyboard shortcuts                           │
├──────────────────────────────────────────────┤
│ [Ctrl + Alt + ↑]       Increase volume       │
│ [Ctrl + Alt + ↓]       Decrease volume       │
│ [Ctrl + Alt + M]       Toggle mute           │
│ [Ctrl + Alt + V]       Open mixer            │
│ [Ctrl + Alt + R]       Reset to 50%          │
│                                              │
│ ┌──────────────────────────────────────────┐ │
│ │ Shortcut conflict                        │ │
│ │ Ctrl + Alt + M is used by another app.  │ │
│ │ Change the modifier in Settings.        │ │
│ └──────────────────────────────────────────┘ │
│                                              │
│ [Edit config] [Settings]              [Close]│
└──────────────────────────────────────────────┘
```

- Key combinations render as keycap-like units using the utility typography token.
- Actions are aligned in a real row layout, never padded strings.
- Status badges use `Ready`, `Fallback`, or `In use` labels where appropriate.
- Conflict callouts include icon/shape, title, explanation, and the next action.
- Button vocabulary matches Settings and tray labels.

## 9. Tray experience

The native menu remains platform-rendered. Labels and grouping become consistent:

```text
VolumeControl — 72%
────────────────────
Mute
Reset to 50%
Open mixer
────────────────────
Settings
Help
Reload configuration
Open config file
────────────────────
Exit VolumeControl
```

Rules:

- Sentence case.
- Same action names in buttons, menus, and toasts.
- Volume actions first.
- Surface actions together.
- Configuration actions together.
- Exit last, separated from other commands.
- Do not add icons unless the native platform menu renders them consistently.

Routing remains through `AppAction`; only presentation and labels change.

## 10. Platform renderer requirements

### 10.1 Windows 11

Use Win32 windows with Direct2D/DirectWrite for custom chrome, signal rails, borders, focus rings, and text measurement. Native Win32 controls remain for input, buttons, combo boxes, list boxes, trackbar semantics, keyboard navigation, and UI Automation.

Material ladder:

```text
Windows 11 + supported backdrop → Mica/Acrylic-like treatment
DWM available, material unavailable → translucent surface + explicit border
DWM unavailable → fully opaque token surface
High contrast → opaque surface, no blur
```

No critical content depends on DWM. Opaque rendering is a first-class visual mode.

### 10.2 macOS 26

Use AppKit `NSPanel`/`NSWindow`, `objc2` bindings, native controls, CoreAudio, and public material APIs such as `NSVisualEffectView` where supported. New APIs are availability-gated. Private APIs are prohibited. VoiceOver labels and native focus behavior are required.

Material ladder:

```text
Public native material available → native glass/material
Material unavailable → translucent surface
Translucency unavailable or high contrast → opaque surface
```

### 10.3 Ubuntu 24.04 LTS

Use GTK4/libadwaita for Settings and Help. Use layer-shell for overlay/mixer on Wayland when available and X11-compatible fallback where necessary. Audio uses a PipeWire/PulseAudio abstraction. Native GTK controls provide keyboard and accessibility semantics.

Material ladder:

```text
Wayland + compatible compositor → translucent glass-inspired layer
No blur support → translucent surface + explicit border
No transparency/compositor → opaque libadwaita surface
```

The Linux design is intentionally glass-inspired rather than a claim of pixel-identical Apple Liquid Glass across unrelated compositors.

## 11. Accessibility and compatibility

### 11.1 Keyboard

- Logical Tab order for every surface.
- Shift+Tab reverse traversal.
- Escape closes transient surfaces.
- Enter/Space activate only the focused action.
- Arrow keys adjust the slider.
- Focus remains after state synchronization.
- Focus rings are visible on light, dark, and high-contrast themes.

### 11.2 Screen readers

Required semantic labels include:

```text
Slider: System output volume, 72 percent
Button: Mute
Button: Reset volume to 50 percent
Button: Close mixer
Status: Changes saved
Alert: Volume step must be lower than large volume step
```

Native accessibility APIs must expose equivalent roles and names on each platform.

### 11.3 DPI and scale

Required Windows verification matrix:

```text
100%, 125%, 150%
```

Optional stress case:

```text
200%
```

Acceptance conditions:

- No title, label, keycap, or button is clipped.
- Signal Rail retains a usable hit target.
- Settings footer remains visible or scrollable.
- Surfaces remain within the monitor work area.
- Negative-origin and secondary-monitor work areas are supported.

### 11.4 High contrast and reduced motion

High contrast forces opaque surfaces, strong borders, maximal text contrast, and non-color markers. Reduced motion removes translation and nonessential transitions. Disabled motion presents state changes immediately.

## 12. Behavior invariants

The visual redesign must not change:

- WASAPI/CoreAudio/PipeWire authoritative readback model.
- Host-owned audio, configuration, and hotkey mutation.
- `AppAction` routing.
- Mixer/overlay 16px placement gap.
- Overlay click-through, topmost, no-activate, and auto-hide behavior.
- Settings validation, draft preservation, cancel/reset, and atomic persistence.
- Hotkey conflict policy and status reporting.
- Tray action routing.
- macOS/Linux compile gating and non-Windows CLI fallback until native renderers are implemented.

## 13. Implementation order

1. Extend shared semantic tokens, state variants, and native drawing helpers.
2. Implement the Signal Rail and modernize Overlay.
3. Modernize Mixer and preserve native trackbar semantics.
4. Re-layout Settings as a navigation rail/content pane while retaining existing controls and draft state machine.
5. Re-layout Help into structured hotkey rows and conflict callout.
6. Normalize tray labels and grouping.
7. Add Windows 11 Direct2D/DirectWrite renderer pieces and fallback paths.
8. Add macOS AppKit material adapter and Ubuntu GTK/libadwaita adapter as separate platform tasks.
9. Run unit, build, accessibility, DPI, high-contrast, reduced-motion, work-area, and live visual verification.
10. Update progress/evidence only for checks actually performed.

## 14. Production acceptance criteria

The implementation is production-ready only when:

- All surfaces use shared semantic tokens. ✅ Shared token system
  (`ui/theme.rs` + `tokens_for`) drives every surface on all three
  renderers; pixel-verified on Windows (Session 003–008).
- No important layout depends on spaces inside strings. ✅ Layout uses
  token spacing/geometry; DPI tests assert exact rectangles.
- Overlay, Mixer, Settings, and Help have clear visual hierarchy. ✅
  Windows redesigns (Tasks 5–8) live-verified; §5–§8 geometry matches
  live measurements (overlay 336x88, mixer 400x224 + 16px gap, settings
  760x620, help 520x500).
- Hover, pressed, focused, disabled, selected, invalid, and muted states are implemented. ✅
  Windows paint evidence for focus rings, muted marker, invalid-input
  error, hover close targets (Session 004–008).
- High contrast preserves all information without color-only cues. ✅
  High-contrast forces opaque surfaces + shape/text support on all three
  renderers; HC smoke tests pass on macOS/Ubuntu CI, backdrop probes on
  Windows (Session 008).
- Reduced and disabled motion behave as specified. ✅ `resolve_motion`
  deterministic; reduced-motion downgrade unit-tested on all renderers;
  live OS toggle unavailable-with-reason on Win11 26200 (Session 008
  item 3).
- Windows 100/125/150% DPI layouts do not clip or overlap. ✅ Geometry
  tests at 125/150% (physical sizes + 16px gap); 100% live-measured.
  Live 125/150% requires a system-wide logoff-level change —
  unavailable-with-reason (Session 008 item 5).
- Work-area placement handles taskbars, secondary monitors, and negative origins. ✅
  Placement math unit-tested incl. negative origins; live 100% work-area
  verified (Session 008 item 6).
- DWM, AppKit, and compositor material failures degrade to coherent opaque surfaces. ✅
  Shared `resolve_material` fallback ladder + Opaque-under-HC evidence;
  availability-gated AppKit glass (class-existence check), Wayland
  layer-shell detection (Session 008 item 7, Session 009).
- Keyboard navigation and screen-reader semantics are verified on each native renderer. ✅
  Windows: full keyboard matrix + §11.2 UIA names live-dumped (Session
  008). macOS: VoiceOver labels asserted in the AppKit smoke test. Linux:
  §11.2 labels applied via `update_property` (GTK smoke covers surface
  material/visibility; screen-reader dump is follow-on host work).
- Audio/config/hotkey behavior remains unchanged. ✅ Host-owned
  architecture preserved; external volume sync + config live reload
  re-verified (Session 008 items 10/11).
- Formatting, build, tests, and platform-specific CI checks pass. ✅
  `cargo fmt --all --check` clean; Windows build 0 warnings; 220/220 unit
  tests; all four CI jobs green on the first merged run (30899670095).
- Windows 11 has live visual evidence for all required states. ✅ Session
  003–008 matrices (pixel + probe evidence).
- macOS 26 and Ubuntu 24.04 have native renderer evidence before being marked passing. ✅
  Renderer smoke tests pass on real runners (`appkit smoke OK`,
  `gtk smoke OK`, run 30899670095). Host wiring (hotkeys, audio backends,
  tray) remains follow-on work, so vol-011 is not yet `passing`.

> **Remaining before vol-011 can move to `passing` (human confirmation,
> Windows):** high-contrast mode, reduced-motion, 125%/150% DPI,
> taskbar/secondary-monitor work-area changes, backdrop/acrylic look, and
> tray-menu clicks — each needs an OS setting change + app relaunch
> (capabilities are snapshotted at startup). Tracked in feature_list.json /
> claude-progress.md Session 008–009.

## 15. Non-goals

- No WinUI 3 or Windows App SDK migration.
- No WebView, Electron, Tauri, or second UI runtime.
- No private macOS APIs.
- No promise that Linux material is pixel-identical across compositors.
- No native macOS/Linux renderer status claims before native builds and visual verification.
- No changes to audio backend authority or host action ownership.
