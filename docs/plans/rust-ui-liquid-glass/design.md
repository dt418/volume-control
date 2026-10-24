# Native Liquid Glass Design

**Status:** Approved 2026-10-23. All four sections reviewed and accepted.

This document specifies the native renderer architecture, data flow, material/motion system, error handling, and verification plan. It builds on Gate 1 requirements (`gate-1-requirements.md`) and research (`rust-native-liquid-glass-research.md`).

## D1 — Retained semantic scene architecture

### Ownership

`volumectl` (host) owns audio, configuration, hotkeys, tray, and persistence. `crates/liquid_glass` owns scene, material resolution, analytic motion, WGSL passes, blur cache, text integration boundary, and WGPU presentation. The renderer never mutates host state.

### Component boundary

```text
Host (audio/config/tray)
  │ confirmed AppState, ThemeTokens, UiCapabilities
  ▼
State bridge
  │ SceneDiff (dirty nodes, changed material roles, text runs)
  ▼
Retained semantic scene ──► AccessKit tree (same IDs)
  │ flatten: RenderPrimitive, text glyphs, material groups
  ▼
Render graph
  │ content texture → blur cache → SDF materials → text/icons/focus
  ▼
WGPU surface
  ▲
Winit input + IME → Interaction/focus → hit test (semantic bounds) → AppAction → Host
```

### Scene nodes

Each node carries:
- Stable semantic ID (shared with AccessKit, hit testing, focus manager).
- `MaterialRole` assignment.
- Layout bounds (used for hit testing — never deformed visual bounds).
- Z-order.
- Focus state, enabled/disabled.
- Content revision (drives blur cache invalidation).

### Platforms

- Windows: DX12, explicit Vulkan fallback. `winit` window.
- macOS: Metal. `winit` window.
- Linux: Vulkan. `winit` window. Narrow host-only layer-shell adapter for overlay bottom-right anchoring/stacking; no Wayland protocol in portable renderer.

### Text

Owned `TextSystem` wraps `glyphon 0.12`. Shaping via `cosmic-text`, glyph atlas cache, WGPU draw. Supports English and Vietnamese. Not a second UI renderer.

## D2 — Data flow

### Publish cycle

1. Host state changes (audio readback, config save, session update).
2. Host calls `NativeRenderer::publish(&mut self, state, tokens, caps)`.
3. Bridge diffs old/new `AppState` → produces `SceneDiff` marking dirty nodes.
4. Scene applies diff: updates node bounds, material roles, text runs.
5. Layout pass recomputes changed subtrees.
6. Flatten pass emits `RenderPrimitive { id, bounds, material_group, z, clip }` + text glyphs.

### Render frame

1. Resolve changed material roles + appearance → material tokens.
2. If `content_revision` changed: re-render content to offscreen texture, rebuild blur pyramid. Else reuse cached blur levels.
3. Draw scissored SDF material groups by z-order into WGPU target.
4. Draw text, icons, focus rings, and controls above glass.
5. Present frame.
6. Return `animating: bool`. Host requests redraw only while true or after state/input change.

### Intent dispatch

User input (pointer, keyboard, IME) → `winit` event → interaction/focus manager → hit test against semantic node bounds → emit `AppAction` → `HostHandle::enqueue(action)` → host routes, mutates, re-publishes.

### Host contract gaps (pre-cutover)

| Gap | Current state | Required extension |
|---|---|---|
| Session state | `host_core.sessions()` returns `Vec<AudioSessionInfo>` but `AppState` has no sessions field | Add `sessions: Vec<SessionState>` to `AppState`; host publishes on session changes. |
| Session actions | `host_core.set_session_volume/mute_session` exist but `AppAction` has no variants | Add `SetSessionVolume { id: String, percent: u8 }`, `ToggleSessionMute { id: String }`. |
| Settings draft | `ApplyConfig/CancelConfig/ResetConfig` only log | Wire host arms; add `SetAccent(AccentMode)`, `SetLanguage(Language)`. |
| Reduced transparency | `UiCapabilities` has no field | Add `reduced_transparency: bool`; `resolve_material` forces opaque. |
| Accent mode | `AppState` has no accent field | Add `accent: AccentMode`. |

## D3 — Material and motion system

### Material hierarchy

| MaterialRole | Usage | Blur | Tint | Saturation |
|---|---|---|---|---|
| Hud | Overlay volume indicator | High | Light | High |
| Panel | Mixer/Settings background | Medium | Medium | Medium |
| Control | Slider thumb, button surface | Low | Dense | Low |
| Accent | Volume bar, active toggle | None | Accent color | N/A |
| Tooltip | Hover tips | Low | Light | High |
| Menu | Tray context menu | Medium | Light | High |

### Appearance

```rust
struct Appearance {
    color_scheme: ColorScheme,    // Light / Dark
    reduce_transparency: bool,   // from UiCapabilities
    increase_contrast: bool,     // from UiCapabilities
    reduce_motion: bool,         // from UiCapabilities
}
```

### Resolution chain

1. `reduce_transparency || high_contrast` → all roles → opaque tokens. Blur = 0, alpha = 1.0.
2. `increase_contrast` → increase edge/rim width, foreground separation.
3. Role + scheme → base blur radius, tint color, saturation, opacity, edge, shadow, rim values.
4. Motion tokens: `reduce_motion` → spring amplitude = 0, ripple = 0, stretch = 0. Immediate state feedback (opacity, color transitions) preserved.

### Bounded fluid motion

| Effect | Input | Implementation | Reduce-motion behavior |
|---|---|---|---|
| Pointer stretch | Velocity × position | Analytic spring, interruptible | Disabled |
| Trailing | Velocity lag | Analytic decay | Disabled |
| Ripple | Pointer down/up | Radial analytic wave | Disabled |
| Gravity bias | Constant downward | Light offset + shadow bias | Disabled |
| State transition | Value change | Analytic spring or immediate | Immediate (no spring) |
| Focus ring | Focus change | Opacity spring | Immediate opacity |

### Analytic spring

Closed-form evaluation: `x(t) = A·e^(-ζωt)·cos(ωd·t + φ) + target`.

- No per-frame integration; evaluate at any `t`.
- Retarget preserves velocity for interruptible animation.
- `settled(t)` when `|x(t) - target| < ε` → stop requesting redraw.
- `without_bounce` variant for reduced-motion fallback if needed.

### SDF union

- Related controls (slider track + thumb) → smooth-union SDF → single blur sample region.
- Card groups → union → shared material boundary and edge treatment.
- Hit testing uses semantic rectangle bounds, never SDF-deformed visual geometry.

## D4 — Error handling and verification

### GPU error recovery

| Condition | Action | Escalation |
|---|---|---|
| `Success` | Render, submit, present. | — |
| `Suboptimal` | Render, submit, present, schedule reconfiguration. | — |
| `Timeout` / `Occluded` | Skip frame, await next redraw/exposure event. | — |
| `Outdated` | Reconfigure surface from latest nonzero size. | — |
| `Lost` (surface) | `Instance::create_surface()` → configure → resume. | — |
| `Lost` (device) | Rebuild adapter, device, queue, all surfaces, all GPU resources. | — |
| `Validation` | Hold error state, log, await valid configuration. | — |
| OOM | Log, clean teardown, no retry loop. | Clean exit |
| No adapter | Degrade to opaque fallback or clean exit. | — |

### Accessibility verification matrix

| Condition | Material | Motion | Text | Controls |
|---|---|---|---|---|
| Normal | Blurred glass | Full spring | Normal | Normal |
| High contrast | Opaque | Full spring | HC foreground/background | HC borders |
| Reduced transparency | Opaque | Full spring | Normal | Normal |
| Reduced motion | Glass | Immediate | Normal | Normal |
| VM/RDP (no compositor) | Opaque | Reduced | Normal | Normal |
| Screen reader active | Glass (visual) | Full | Normal + AccessKit tree | Keyboard + a11y actions |

### Surface verification plan

| Surface | Smoke proof | Accessibility proof |
|---|---|---|
| Overlay | Volume change → glass HUD appears bottom-right, auto-hides. | Screen reader announces volume; reduced motion = no spring. |
| Mixer | Show → session list, sliders, mute toggles work. | Tab/arrow navigation; per-session a11y names and ranges. |
| Settings | Draft edit → preview; Apply commits; Cancel restores. | Focus traversal all controls; IME Vietnamese in text fields. |
| Help | Show → keyboard shortcuts visible, scrollable. | Screen reader reads content; links navigable. |
| Tray | Right-click → context menu with actions. | Menu items accessible by keyboard. |

### Build targets

| Target | Backend | Required proof |
|---|---|---|
| Windows x64 | DX12, Vulkan fallback | Surface create/present, opaque fallback. |
| macOS arm64 | Metal | Surface create/present, Retina scale. |
| Linux x64 | Vulkan | Surface create/present, layer-shell overlay anchor. |

## Gate 2 approval

Approval means D1–D4 are binding for specification and implementation. It authorizes detailed specification work, ticket creation, and implementation planning. It does **not** authorize product-code changes, dependency changes, or removal of current UI paths.

**Approved 2026-10-23.** User approved Gate 2 in this session.
