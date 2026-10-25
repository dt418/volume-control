# Native Liquid Glass — Traceable Specification

**Status:** Draft specification based on approved Gate 1 requirements and Gate 2 design.

This document maps design decisions to implementation tickets with dependency order, acceptance criteria, and traceability to requirements (R1–R10, N1–N6) and design sections (D1–D4).

## Dependency graph

```text
T0  Workspace + crate scaffold ─────────────────────┐
 │                                                    │
T1  WGPU surface lifecycle ──────────────────────────┤
 │                                                    │
T2  Text system (glyphon 0.12) ──────────────────────┤
 │                                                    │
T3  Host contract extensions ────────────────────────┤
 │     (sessions, settings draft, reduced transparency)│
 │                                                    │
 ├── T4  Retained semantic scene ────────────────────┤
 │        (needs T0, T1, T2)                          │
 │                                                    │
 ├── T5  Material resolution + SDF ──────────────────┤
 │        (needs T4)                                  │
 │                                                    │
 ├── T6  Blur pipeline ─────────────────────────────┤
 │        (needs T1, T4)                              │
 │                                                    │
 ├── T7  Analytic motion ───────────────────────────┤
 │        (needs T4, T5)                              │
 │                                                    │
 ├── T8  Input, focus, hit testing ─────────────────┤
 │        (needs T4)                                  │
 │                                                    │
 ├── T9  AccessKit integration ─────────────────────┤
 │        (needs T4, T8)                              │
 │                                                    │
 ├── T10 Overlay surface ───────────────────────────┤
 │        (needs T3, T4, T5, T6, T7, T8, T9)         │
 │                                                    │
 ├── T11 Mixer surface ─────────────────────────────┤
 │        (needs T3, T4, T5, T6, T7, T8, T9)         │
 │                                                    │
 ├── T12 Settings surface ──────────────────────────┤
 │        (needs T3, T4, T5, T6, T7, T8, T9)         │
 │                                                    │
 ├── T13 Help surface ──────────────────────────────┤
 │        (needs T4, T5, T6, T8, T9)                  │
 │                                                    │
 ├── T14 Tray integration ─────────────────────────┤
 │        (needs T3, T8)                              │
 │                                                    │
 ├── T15 Linux layer-shell adapter ─────────────────┤
 │        (needs T1, T10)                             │
 │                                                    │
 └── T16 Cutover + WebView removal ─────────────────┘
          (needs T10–T15, all surfaces at parity)
```

## Tickets

---

### T0 — Workspace and crate scaffold

**Traces:** N1, N2, D1.

**Scope:**
- Create `crates/liquid_glass/` with `Cargo.toml`: `wgpu = "30.0.1"`, `glyphon = "0.12"`, `bytemuck`, `glam`, `winit` (dev/app boundary).
- Set workspace `rust-version = "1.92"`.
- Add `static-dxc` feature for Windows.
- Verify `cargo check --all-targets` resolves cleanly.

**Acceptance:**
- [ ] `cargo check` passes on workspace with new crate.
- [ ] `wgpu 30.0.1` and `glyphon 0.12` in `Cargo.lock`.
- [ ] MSRV in workspace `Cargo.toml` is `1.92`.

**Depends on:** Nothing.

---

### T1 — WGPU surface lifecycle

**Traces:** N4, D1, D4.

**Scope:**
- `GpuContext` struct: instance, adapter, device, queue, surface configuration.
- Windows: DX12 first, Vulkan fallback. macOS: Metal. Linux: Vulkan.
- `CurrentSurfaceTexture` handling per N4: Success, Suboptimal, Timeout, Occluded, Outdated, Lost, Validation.
- Device loss: full rebuild (adapter, device, queue, surface, resources).
- OOM: clean teardown, no retry.

**Acceptance:**
- [ ] Surface creation and presentation on each target backend.
- [ ] Outdated → reconfigure path tested.
- [ ] Lost → surface recreation path tested.
- [ ] Device loss → full rebuild path tested (injectable error).
- [ ] OOM → clean exit, no retry loop.

**Depends on:** T0.

---

### T2 — Text system

**Traces:** R5, D1.

**Scope:**
- `TextSystem` wraps `glyphon 0.12` + `cosmic-text`.
- Vietnamese shaping, fallback fonts, glyph atlas cache.
- WGPU text rendering pipeline.
- Long device-name layout handling.

**Acceptance:**
- [ ] English and Vietnamese text renders correctly.
- [ ] Glyph fallback for Vietnamese diacritics.
- [ ] Long device names truncate/wrap correctly.
- [ ] Atlas cache reuse verified (no per-frame allocation).

**Depends on:** T0.

---

### T3 — Host contract extensions

**Traces:** R1, R3, D2.

**Scope:**
- Add `sessions: Vec<SessionState>` to `AppState`. Host publishes on session changes.
- Add `SetSessionVolume { id: String, percent: u8 }`, `ToggleSessionMute { id: String }` to `AppAction`.
- Wire `ApplyConfig`, `CancelConfig`, `ResetConfig` in `host_core`.
- Add `SetAccent(AccentMode)`, `SetLanguage(Language)` to `AppAction`.
- Add `accent: AccentMode` to `AppState`.
- Add `reduced_transparency: bool` to `UiCapabilities`.
- Update `resolve_material` to force opaque when `reduced_transparency`.
- Per-platform detection: Windows `SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION)` or transparency registry; macOS `NSWorkspace.accessibilityDisplayShouldReduceTransparency`; Linux: GTK `gtk-enable-animations` or equivalent.

**Acceptance:**
- [ ] `AppState` round-trips with sessions through serde.
- [ ] Session volume/mute actions routed through host.
- [ ] Apply/Cancel/Reset settings draft arms functional.
- [ ] `resolve_material` returns Opaque when `reduced_transparency = true`.
- [ ] Existing capability tests still pass.

**Depends on:** Nothing (model-only; can parallel T0–T2).

---

### T4 — Retained semantic scene

**Traces:** D1, D2.

**Scope:**
- `Scene` with allocation-stable node storage (slotmap or similar).
- `SceneNode { id, material_role, bounds, z_order, focus_state, content_revision, children, a11y_data }`.
- `SceneDiff` from old/new `AppState` → dirty node set.
- Layout pass: recompute changed subtrees.
- Flatten pass: emit `RenderPrimitive { id, bounds, material_group, z, clip }` + text glyphs.

**Acceptance:**
- [ ] Scene nodes have stable IDs across state updates.
- [ ] Only dirty subtrees recompute layout.
- [ ] Flatten produces correct z-ordered primitives.
- [ ] No per-frame heap allocation in steady state.

**Depends on:** T0, T1, T2.

---

### T5 — Material resolution and SDF

**Traces:** R7, R8, D3.

**Scope:**
- `MaterialRole` enum: Hud, Panel, Control, Accent, Tooltip, Menu.
- `Appearance { color_scheme, reduce_transparency, increase_contrast, reduce_motion }`.
- Resolution chain: accessibility overrides → role + scheme → material tokens.
- SDF smooth union for related control groups.
- WGSL material shader.

**Acceptance:**
- [ ] High contrast → opaque tokens for all roles.
- [ ] Reduced transparency → opaque tokens for all roles.
- [ ] Role-specific blur/tint/saturation values applied correctly.
- [ ] SDF union visually merges related controls.
- [ ] Hit testing unaffected by SDF visual deformation.

**Depends on:** T4.

---

### T6 — Blur pipeline

**Traces:** R8, D2.

**Scope:**
- Render app content to offscreen readable texture.
- Multi-pass Gaussian downsampled blur pyramid.
- Cache by `content_revision`; reuse when content unchanged.
- Scissored blur sampling per material group.

**Acceptance:**
- [ ] Blur visually correct at multiple radii.
- [ ] Cache hit: no blur recomputation when content unchanged.
- [ ] Cache miss: blur rebuilds on content change.
- [ ] Blur respects material group clip bounds.

**Depends on:** T1, T4.

---

### T7 — Analytic motion

**Traces:** R9, D3.

**Scope:**
- Closed-form spring: `x(t) = A·e^(-ζωt)·cos(ωd·t + φ) + target`.
- Retarget preserves velocity.
- `settled(t)` detection → stop redraw.
- Pointer stretch, trailing, ripple, gravity bias.
- `reduce_motion` disables all fluid effects; immediate transitions remain.

**Acceptance:**
- [ ] Spring evaluation matches closed-form for known parameters.
- [ ] Retarget mid-animation preserves velocity continuity.
- [ ] Settled detection stops requesting redraws.
- [ ] All fluid effects disabled under reduce_motion.
- [ ] No per-frame allocation.

**Depends on:** T4, T5.

---

### T8 — Input, focus, and hit testing

**Traces:** R4, R5, D1, D2.

**Scope:**
- Winit event → interaction manager.
- Hit testing against semantic node bounds (not visual deformation).
- Focus traversal: Tab/Shift+Tab, arrow keys, Escape.
- Pointer capture for sliders/drag.
- IME: `WindowEvent::Ime` → focused editor, cursor rectangle update.
- Click-through: capability-gated (macOS/X11); Wayland HUD stays interactive.

**Acceptance:**
- [ ] Hit test returns correct node under pointer during stretch animation.
- [ ] Tab traversal visits all focusable controls in order.
- [ ] IME preedit/commit works for Vietnamese.
- [ ] Pointer capture for slider drag.
- [ ] Click-through on supported platforms; interactive on Wayland.

**Depends on:** T4.

---

### T9 — AccessKit integration

**Traces:** R6, D1.

**Scope:**
- `accesskit_winit` adapter.
- Tree built from scene nodes with same IDs.
- Names, roles, ranges (sliders), checked state (mute toggles), actions.
- Tree updates on state change.

**Acceptance:**
- [ ] UI Automation (Windows), VoiceOver (macOS), AT-SPI (Linux) read tree.
- [ ] Screen reader action → AppAction → state change → tree update.
- [ ] Slider ranges reported correctly.
- [ ] Mute toggle checked state correct.

**Depends on:** T4, T8.

---

### T10 — Overlay surface

**Traces:** R1, R2, R4, R8, R9.

**Scope:**
- Bottom-right positioned glass HUD showing volume percentage and mute state.
- Auto-hide after timeout.
- Glass material with Hud role.
- Fluid feedback on volume change.
- Click-through where supported.

**Acceptance:**
- [ ] Volume change → overlay appears with correct percentage.
- [ ] Auto-hide after configured timeout.
- [ ] Glass blur and material visually correct.
- [ ] Reduced motion → no spring animation.
- [ ] Screen reader announces volume change.

**Depends on:** T3, T4, T5, T6, T7, T8, T9.

---

### T11 — Mixer surface

**Traces:** R1, R2, R4, R5.

**Scope:**
- Session list with per-session volume slider and mute toggle.
- System output row.
- Device name display.
- Keyboard navigation.

**Acceptance:**
- [ ] All active sessions displayed with correct volume/mute.
- [ ] Session volume/mute changes dispatch through host and update UI.
- [ ] Long device names handled.
- [ ] Tab/arrow navigation through all controls.
- [ ] Vietnamese session names render correctly.

**Depends on:** T3, T4, T5, T6, T7, T8, T9.

---

### T12 — Settings surface

**Traces:** R1, R2, R3, R5.

**Scope:**
- Theme, material, motion, accent, language selection.
- Draft/preview mode: changes visible immediately.
- Apply commits through host; Cancel restores confirmed state.
- Blacklist management.
- Hotkey display and configuration feedback.

**Acceptance:**
- [ ] Draft changes preview immediately.
- [ ] Apply → host saves; Cancel → reverts to confirmed.
- [ ] All existing settings controls present and functional.
- [ ] IME works in text input fields (Vietnamese).
- [ ] Focus traversal covers all controls.

**Depends on:** T3, T4, T5, T6, T7, T8, T9.

---

### T13 — Help surface

**Traces:** R2, R5.

**Scope:**
- Keyboard shortcuts display.
- Scrollable content.
- Bilingual (EN/VI).

**Acceptance:**
- [ ] All shortcuts displayed correctly.
- [ ] Content scrolls smoothly.
- [ ] Language switch works.
- [ ] Screen reader reads help content.

**Depends on:** T4, T5, T6, T8, T9.

---

### T14 — Tray integration

**Traces:** R2.

**Scope:**
- System tray icon.
- Right-click context menu with surface toggles, settings access, exit.
- Menu items as AppActions.

**Acceptance:**
- [ ] Tray icon visible on all platforms.
- [ ] Context menu items dispatch correct actions.
- [ ] Menu accessible by keyboard.

**Depends on:** T3, T8.

---

### T15 — Linux layer-shell adapter

**Traces:** R4, R10, D1.

**Scope:**
- Host-only adapter (not in `liquid_glass` crate).
- Bottom-right anchoring for overlay.
- Layer stacking (overlay layer).
- Keyboard interactivity mode for Wayland HUD.

**Acceptance:**
- [ ] Overlay anchored bottom-right on Wayland.
- [ ] Overlay on correct layer.
- [ ] HUD interactive on Wayland (no input-shape).
- [ ] No layer-shell code in `crates/liquid_glass`.

**Depends on:** T1, T10.

---

### T16 — Cutover and WebView removal

**Traces:** N6, R2.

**Scope:**
- All surfaces at native parity (T10–T15 complete).
- Delete WebView UI paths, Tauri frontend, React dependencies.
- Delete compatibility aliases, transitional scaffolding.
- Final dependency audit.

**Acceptance:**
- [ ] No WebView, React, or Tauri frontend code in release.
- [ ] All surfaces functional natively.
- [ ] No release shims or aliases.
- [ ] Clean `cargo build --release` on all targets.

**Depends on:** T10, T11, T12, T13, T14, T15.

---

## Implementation phases

### Phase 1 — Foundation (T0, T1, T2, T3)
Parallel work. No rendering visible yet. Establishes crate, GPU lifecycle, text, and host contracts.

### Phase 2 — Core renderer (T4, T5, T6, T7, T8, T9)
Scene, materials, blur, motion, input, accessibility. T4 first; T5–T9 partially parallel after T4.

### Phase 3 — Surfaces (T10, T11, T12, T13, T14, T15)
Each surface is an independent ticket after Phase 2. T10–T13 parallel. T14 parallel. T15 after T10.

### Phase 4 — Cutover (T16)
Only after all Phase 3 surfaces reach parity.

## Traceability matrix

| Requirement | Tickets |
|---|---|
| R1 (host authority) | T3, T10, T11, T12, T14 |
| R2 (replace WebView) | T10, T11, T12, T13, T14, T16 |
| R3 (config.ini, settings draft) | T3, T12 |
| R4 (surface geometry, click-through) | T8, T10, T15 |
| R5 (EN/VI, IME, focus, keyboard) | T2, T8, T11, T12, T13 |
| R6 (accessibility tree) | T9, T10, T11, T12, T13 |
| R7 (theme/material/motion/accent) | T5, T7 |
| R8 (glass rendering) | T5, T6, T10, T11, T12, T13 |
| R9 (bounded fluid) | T7, T10 |
| R10 (platform backends) | T1, T15 |
| N1 (one renderer) | T0, T16 |
| N2 (MSRV, dependencies) | T0 |
| N3 (allocation-stable, cache) | T4, T6 |
| N4 (surface acquisition) | T1 |
| N5 (accessibility priority) | T5, T9 |
| N6 (no release shims) | T16 |
