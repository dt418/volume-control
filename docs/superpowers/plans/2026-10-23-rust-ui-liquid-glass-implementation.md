# Native Liquid Glass Implementation Plan (D5)

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Document ID:** D5
**Path:** `docs/superpowers/plans/2026-10-23-rust-ui-liquid-glass-implementation.md`
**Parent Requirements:** [`docs/plans/rust-ui-liquid-glass/gate-1-requirements.md`](../../plans/rust-ui-liquid-glass/gate-1-requirements.md)
**Approved Design:** [`docs/superpowers/specs/2026-10-23-rust-ui-liquid-glass-design.md`](../specs/2026-10-23-rust-ui-liquid-glass-design.md)
**Product Specification:** [`docs/superpowers/specs/2026-10-23-rust-ui-liquid-glass-spec.md`](../specs/2026-10-23-rust-ui-liquid-glass-spec.md)
**Ticket Set:** [`docs/plans/rust-ui-liquid-glass/tickets/`](../../plans/rust-ui-liquid-glass/tickets/)

---

## 1. Goal

Implement the complete native Rust/WGPU UI renderer (`crates/liquid_glass`) and cut over all VolumeControl surfaces (Overlay, Mixer, Settings, Help, Tray) from hybrid webviews to native rendering with Apple-style Liquid Glass optics, achieving 0 heap allocations in steady state, < 50 ms cold launch, < 30 MB RAM footprint, and 100% WCAG AA accessibility parity.

---

## 2. Global Constraints & Invariants

1. **Host Authority**: `crates/volumectl` remains the single authority for audio hardware, session streams, `config.ini`, hotkeys, and event loop. The renderer never mutates audio or configuration directly.
2. **Single Renderer**: No second UI crate (egui, iced, dioxus) or webview engine enters release architecture.
3. **MSRV & Dependencies**: Workspace MSRV is Rust 1.92; pinned `wgpu = "30.0.1"`, `glyphon = "0.12"`, Windows `static-dxc`.
4. **Hit-Testing Invariant**: Hit testing always evaluates against static semantic control bounds, never visually stretched or deformed fluid geometry.
5. **A11y Priority**: High contrast or reduced transparency always forces opaque fallback (blur = 0, alpha = 1.0, 2px borders, contrast >= 4.5:1).
6. **No Release Shims**: Zero transitional webview code or compatibility aliases survive post-cutover.

---

## 3. SDLC Mapping & Execution Phases

| Phase | Tasks Included | Focus | Exit Gate |
|---|---|---|---|
| **Phase 1: Foundation** | Tasks 0–3 | Crate scaffold, WGPU lifecycle, TextSystem, Host contracts | `cargo check --workspace` passes; unit tests for GPU errors & text green |
| **Phase 2: Core Engine** | Tasks 4–9 | Scene graph, Material SDF, Blur pyramid, Springs, Input, A11y | Headless frame tests render glass, handle input, produce AccessKit tree |
| **Phase 3: Surfaces** | Tasks 10–15 | Overlay, Mixer, Settings, Help, Tray, Wayland adapter | Visual parity verified vs `mixer.png`, `settings_general.png`, `settings_appearance.png` |
| **Phase 4: Cutover** | Task 16 | Webview deletion, dependency purge, packaging | Final binary build with zero webview deps; smoke test on Windows/macOS/Linux |

---

## 4. Detailed Tasks & Subagent Loops

---

### Task 0: Workspace & Crate Scaffold (T00)
- [ ] Update root `Cargo.toml` with `rust-version = "1.92"` and member `"crates/liquid_glass"`.
- [ ] Add workspace dependencies: `wgpu = "30.0.1"`, `glyphon = "0.12"`, `bytemuck = "1.21"`, `glam = "0.29"`.
- [ ] Create `crates/liquid_glass/Cargo.toml` with `static-dxc` feature.
- [ ] Create initial `crates/liquid_glass/src/lib.rs` with public error types and module stubs.
- [ ] Verify: `cargo check -p liquid_glass --all-targets` and `cargo test -p liquid_glass`.

---

### Task 1: WGPU Surface Lifecycle & Multi-Backend Context (T01)
- [ ] Implement `GpuContext` in `crates/liquid_glass/src/gpu/context.rs`:
  - Windows: DX12 first, Vulkan fallback.
  - macOS: Metal only.
  - Linux: Vulkan only.
- [ ] Implement `SurfaceManager` in `crates/liquid_glass/src/gpu/surface.rs`:
  - Handle `CurrentSurfaceTexture` variants: `Success`, `Suboptimal`, `Timeout`, `Occluded`, `Outdated`, `Lost`, `Validation`.
  - Reconfigure on `Outdated`; recreate surface on `Lost` (surface); full GPU rebuild on `Lost` (device).
- [ ] Write injectable error unit tests in `crates/liquid_glass/src/gpu/tests.rs`.
- [ ] Verify: `cargo test -p liquid_glass --lib gpu::tests`.

---

### Task 2: Native Text System (T02)
- [ ] Implement `TextSystem` in `crates/liquid_glass/src/text/system.rs` wrapping `glyphon 0.12`.
- [ ] Configure `cosmic-text` font fallback chains: Segoe UI, SF Pro, Cantarell with full Vietnamese coverage.
- [ ] Implement text layout, line wrapping, and ellipsis truncation for long audio device labels.
- [ ] Write unit tests verifying Vietnamese diacritic rendering and glyph atlas caching.
- [ ] Verify: `cargo test -p liquid_glass --lib text::tests`.

---

### Task 3: Host Contract Extensions (T03)
- [ ] Add `pub sessions: Vec<AudioSessionState>` and `pub accent: AccentMode` to `AppState` in `crates/volumectl/src/ui/model.rs`.
- [ ] Add `SetSessionVolume`, `ToggleSessionMute`, `SetAccent`, `SetLanguage` variants to `AppAction`.
- [ ] Add `pub reduced_transparency: bool` to `UiCapabilities` in `crates/volumectl/src/ui/capabilities.rs`.
- [ ] Update `resolve_material` to force `ResolvedMaterial::Opaque` when `reduced_transparency == true`.
- [ ] Wire `ApplyConfig`, `CancelConfig`, `ResetConfig`, `SetSessionVolume`, `ToggleSessionMute` in `crates/volumectl/src/host_core.rs`.
- [ ] Verify: `cargo test -p volumectl --lib ui::model::tests` and `cargo test -p volumectl --lib ui::capabilities::tests`.

---

### Task 4: Retained Semantic Scene & Node Graph (T04)
- [ ] Implement `SemanticId`, `SceneNode`, and `Scene` arena in `crates/liquid_glass/src/scene/`.
- [ ] Implement `SceneDiff` generator calculating dirty subtrees from consecutive `AppState` publications.
- [ ] Implement z-ordered flattening producing `RenderPrimitive` streams.
- [ ] Write tests ensuring node IDs persist across updates and zero allocations occur during steady-state diffing.
- [ ] Verify: `cargo test -p liquid_glass --lib scene::tests`.

---

### Task 5: Material Resolution & SDF Shading Pipeline (T05)
- [ ] Implement `MaterialRole` mapping to optical properties (`tokens.rs`).
- [ ] Implement accessibility resolution: `high_contrast || reduced_transparency` forces opaque tokens.
- [ ] Write `crates/liquid_glass/src/material/shaders/glass.wgsl`:
  - Signed-distance evaluation for rounded rectangles.
  - Smooth polynomial union for organic control merging.
  - Dual-layer specular edge highlights and luminance tinting.
- [ ] Implement dynamic uniform upload and pipeline binding.
- [ ] Verify: `cargo test -p liquid_glass --lib material::tests`.

---

### Task 6: Multi-Pass Blur Pipeline & Pyramid Caching (T06)
- [ ] Implement offscreen backdrop composition pass rendering app-owned content to readable texture.
- [ ] Write separable Gaussian blur compute/render shader (`crates/liquid_glass/src/blur/shaders/blur.wgsl`).
- [ ] Implement downsampled ping-pong blur pyramid with cache invalidation gated by `content_revision`.
- [ ] Verify: `cargo test -p liquid_glass --lib blur::tests`.

---

### Task 7: Closed-Form Analytic Spring Motion (T07)
- [ ] Implement `AnalyticSpring` second-order harmonic oscillator in `crates/liquid_glass/src/motion/spring.rs`.
- [ ] Ensure velocity continuity during mid-flight retargeting.
- [ ] Implement `is_settled()` detection to halt redraw loops when animation settles.
- [ ] Implement pointer stretch, trailing, and ripple waves with reduced-motion bypass.
- [ ] Verify: `cargo test -p liquid_glass --lib motion::tests`.

---

### Task 8: Input Handling, Focus Traversal & Semantic Hit Testing (T08)
- [ ] Implement semantic hit testing checking static un-deformed layout bounds (`crates/liquid_glass/src/input/hit_test.rs`).
- [ ] Implement `FocusManager` handling Tab/Shift+Tab, Arrow keys, and Escape dismiss.
- [ ] Implement Vietnamese IME composition rectangle tracking via `winit::WindowEvent::Ime`.
- [ ] Implement gesture translation to `AppAction` and queueing via `HostHandle`.
- [ ] Verify: `cargo test -p liquid_glass --lib input::tests`.

---

### Task 9: AccessKit Assistive-Technology Tree Integration (T09)
- [ ] Integrate `accesskit` and `accesskit_winit` in `crates/liquid_glass/src/a11y/`.
- [ ] Build accessibility tree mirroring scene nodes using identical `SemanticId`s.
- [ ] Expose Sliders, Checkboxes, Buttons, Lists with proper values and states.
- [ ] Handle `accesskit::ActionRequest` (SetValue, Default) and route to `AppAction`.
- [ ] Verify: `cargo test -p liquid_glass --lib a11y::tests`.

---

### Task 10: Native Overlay Surface (T10)
- [ ] Implement 336×88 logical px bottom-right HUD in `crates/liquid_glass/src/surfaces/overlay.rs`.
- [ ] Wire liquid volume fill gauge, device label, mute icon, and auto-hide timer.
- [ ] Configure capability-gated click-through on macOS (mouse events ignored) and Windows (`WS_EX_TRANSPARENT`).
- [ ] Verify: `cargo test -p liquid_glass --lib surfaces::overlay::tests`.

---

### Task 11: Native Volume Mixer Surface (T11)
- [ ] Implement 400×224 logical px floating card in `crates/liquid_glass/src/surfaces/mixer.rs`.
- [ ] Render master rail, search box, and per-app audio session rails matching `mixer.png`.
- [ ] Wire real-time slider drags and mute toggles to `AppAction`.
- [ ] Verify: `cargo test -p liquid_glass --lib surfaces::mixer::tests`.

---

### Task 12: Native Settings Surface (T12)
- [ ] Implement 760×620 window in `crates/liquid_glass/src/surfaces/settings.rs` matching `settings_general.png` and `settings_appearance.png`.
- [ ] Implement sidebar navigation, general controls, and appearance pickers.
- [ ] Enforce draft/commit lifecycle: live preview during draft; Save commits to `config.ini`; Cancel reverts.
- [ ] Verify: `cargo test -p liquid_glass --lib surfaces::settings::tests`.

---

### Task 13: Native Help Surface (T13)
- [ ] Implement 520×500 logical px shortcut reference card in `crates/liquid_glass/src/surfaces/help.rs`.
- [ ] Display bilingual shortcut list dynamically reacting to current language.
- [ ] Verify: `cargo test -p liquid_glass --lib surfaces::help::tests`.

---

### Task 14: Native System Tray Integration (T14)
- [ ] Implement platform tray adapters in `crates/volumectl/src/ui/tray.rs`.
- [ ] Provide menu actions: Open Mixer, Open Settings, Open Help, Open Config Location, Check Updates, Exit.
- [ ] Verify: `cargo test -p volumectl --lib ui::tray::tests`.

---

### Task 15: Linux Wayland Layer-Shell Placement Adapter (T15)
- [ ] Implement host-only layer-shell adapter in `crates/volumectl/src/ui/platform/linux/layer_shell.rs`.
- [ ] Configure bottom-right anchoring and `Layer::Overlay` stacking.
- [ ] Ensure 0 Wayland code enters `crates/liquid_glass`.
- [ ] Verify: `cargo test -p volumectl --lib ui::platform::linux::tests`.

---

### Task 16: Release Cutover & Webview Removal (T16)
- [ ] Delete `frontend/` directory completely.
- [ ] Remove `tauri` webview dependencies from `src-tauri/Cargo.toml` and root `Cargo.toml`.
- [ ] Purge transitional webview bridges and shims.
- [ ] Update packaging scripts (NSIS, DMG, tar.gz) for self-contained native executable.
- [ ] Execute full workspace verification:
  ```bash
  cargo build --release --workspace
  cargo test --workspace --all-targets
  cargo clippy --workspace --all-targets -- -D warnings
  cargo fmt --all --check
  ```
