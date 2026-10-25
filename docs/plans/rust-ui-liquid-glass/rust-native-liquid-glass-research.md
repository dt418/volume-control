# Rust-native Liquid Glass Research

**Scope.** Primary-source and artifact study of primary conceptual reference [`ziuus/liquid-glass`](https://github.com/ziuus/liquid-glass), secondary pattern evidence [`al1h3n/liquid-rust`](https://github.com/al1h3n/liquid-rust), and published `@glass-sdk/liquid-glass`, for a full native VolumeControl UI replacement on Linux, macOS, and Windows. Neither Rust repository is an import or source dependency. Source state reviewed 2026-10-23. This report does not modify product code.

## Evidence convention

- **Observed** means source code or repository-owned README explicitly shows it.
- **Inference** means an integration conclusion from observed facts.
- **Product decision** records requirements supplied for this work. It is not a claim about Apple or either library.
## REA record

- **Observed.** REA 6.7.0 statically analyzed NPM artifact `@glass-sdk/liquid-glass@0.0.2`: registry integrity matched downloaded tarball; tarball SHA-256 is `995719ca2b9bd65bd18394e89168bacf332aefa98da9d9815301a161b8bfcd1e`.
- **Observed.** Evidence `ev_b7af4fd1d956e60f83f210b956592b86f47c4a53866bad656466f08d8c411b1a` retained analysis of `package.json`, `dist/core.js`, `dist/gpu.js`, and `dist/dom.js`. It parsed three JavaScript files, read 38,273 bytes, retained 4,861 semantic nodes and 5,005 relations, and reported no parse failures.
- **Limit.** Analysis selected four of 126 inventoried files, semantic coverage is partial, and REA did not execute code. Complete local Evidence record: `C:\Users\Thanh\AppData\Local\Temp\liquid-glass-research-ju0xmyeb\rea-published-sdk-evidence-input.json`.
- **Observed.** Published SDK uses `vgpu`, `@lisse/core`, `@base-ui/react`, and React. It produces WebGPU material maps then reads them back for DOM/SVG filtering; it is not a native Rust renderer. [`package/package.json`](https://registry.npmjs.org/@glass-sdk%2fliquid-glass/latest) and published `dist/gpu.js` (artifact identity above).


## Decision

**Product decision.** Replace every Tauri/React WebView UI surface with one owned, portable Rust/WGPU renderer. `ziuus/liquid-glass` is primary conceptual architecture reference, not an import source. `liquid-rust` is secondary pattern evidence for texture/cache and analytic-motion techniques only, not an import source. Keep `ziuus` core/material/UI/platform separation as an architectural prompt; do not retain its Wayland code, renderer implementation, or widget API.

Create a single `crates/liquid_glass` crate for material resolution, scene state, analytic motion, WGSL passes, cache lifecycle, and native WGPU presentation. Existing `crates/volumectl/src/ui` state/action/theme contracts remain application-domain input, extended before release for session controls and Settings drafts. React/Radix and `src-tauri` retire only after functional native replacement.

**Platform policy.** Windows tries explicit DX12 then explicit Vulkan fallback; macOS uses Metal; Linux uses Vulkan. Do not use `Backends::PRIMARY` because it permits multiple backends and defines no preference order. Every surface still requires adapter and capability negotiation.

**Approved version policy.** Raise workspace MSRV to Rust 1.92 and pin released `wgpu 30.0.1` plus `glyphon 0.12`, with WGSL, DX12, Metal, Vulkan, and Windows `static-dxc`. This is a planned implementation change, not yet applied. `static-dxc` trades larger Windows build/link artifacts for a self-contained release without `dxcompiler.dll`; it does not add per-frame work. `wgpu 30.0.1` and current `winit 0.30` both use `raw-window-handle 0.6`; no handle-version bridge is required.

**Product decision — bounded perceptual fluidity.** Implement material hierarchy, vibrancy, real multi-pass blur, accessibility variants, smooth SDF union, velocity stretch/trailing, interaction ripple, analytic spring settling, and subtle gravity-biased asymmetry. Do **not** implement Navier–Stokes simulation, continuous idle “breathing”, mandatory physical refraction, or mandatory chromatic dispersion.
## VolumeControl integration facts

| Observed fact | Source |
|---|---|
| Current workspace already has native `AppState`, `AppAction`, `SurfaceId`, and `NativeRenderer` contracts. | [`crates/volumectl/src/ui/model.rs`](../../../crates/volumectl/src/ui/model.rs) and [`crates/volumectl/src/ui/renderer.rs`](../../../crates/volumectl/src/ui/renderer.rs) |
| Current workspace already depends on `winit 0.30`. | [`Cargo.toml`](../../../Cargo.toml) |
| Current Tauri host is a WebView implementation for Mixer, Settings, and Help. | [`src-tauri/Cargo.toml`](../../../src-tauri/Cargo.toml) |
| Current frontend is React 19 with Tauri API, Radix primitives, Tailwind, and Framer Motion. | [`frontend/package.json`](../../../frontend/package.json) |
| Current native UI contract supports Overlay, Mixer, Settings, Help, and Tray surface identities. | [`crates/volumectl/src/ui/model.rs`](../../../crates/volumectl/src/ui/model.rs) |
| Current `AppState` has no audio-session collection and `AppAction` has no per-session volume/mute actions. | `crates/volumectl/src/ui/model.rs:86-204` |
| Host already exposes session enumeration, per-session volume, and per-session mute; current `ApplyConfig`, `CancelConfig`, and `ResetConfig` arms only log because WebView drives Settings flows. | `crates/volumectl/src/host_core.rs:570-575`; `crates/volumectl/src/host_core.rs:832-848` |
| Overlay cursor-ignore behavior is capability-gated: macOS ignores cursor events, X11 applies input shape, and Wayland input shape has no effect, leaving HUD interactive. | `src-tauri/src/window_manager.rs:383-390` |

**Inference.** Existing native UI contract is host-authority seam, not complete native-parity model. Before release, extend confirmed domain state and host-routed intents for session list/volume/mute plus Settings draft/edit/apply/cancel/reset; renderer remains a consumer of confirmed state and emitter of host intent. Native WGPU renderer then replaces per-platform drawing adapters and WebView surfaces without taking authority for audio, configuration, hotkeys, localization, or tray behavior.

## Library comparison

| Dimension | `al1h3n/liquid-rust` | `ziuus/liquid-glass` | VolumeControl conclusion |
|---|---|---|---|
| Scope | Reusable `wgpu` renderer supplied with content and target textures. | Workspace: `glass-core`, `glass-ui`, `glass-platform`, examples. | Own one equivalent renderer crate plus a thin app integration layer. |
| Graphics pipeline | Copies content, builds/reuses blur pyramid, then draws SDF glass groups by z-layer. Supports optional glass-on-glass offscreen backdrop rebuild. | WGPU/WGSL surface renderer; per-surface dynamic uniforms and clipped quads. Current renderer initializes a synthetic 512×512 background texture. | Borrow pass structure, not code. Render all native surfaces through one portable WGPU swapchain. |
| State model | `Scene` owns slot-map nodes, containers, appearance, touch, drag, and animation. | `GlassSurface` carries geometry and optical values; `glass-ui` owns widget interaction state and springs. | One scene/material model consumes confirmed VolumeControl state and emits host intents after required session and Settings-draft contract extensions. |
| Material model | Explicit `Material` fields and presets; shared `Appearance` includes transparency, contrast, motion, and adaptive-content flags. | Legacy material enum/parameters plus `VibrancyMaterial` presets and reduced-transparency flag. | Define one owned semantic material hierarchy. Do not expose raw shader knobs to app surfaces. |
| Component abstraction | Glass geometry only; host draws labels and glyphs after renderer. | `GlassCard`, `GlassButton`, `GlassToggle`, and `GlassSlider` generate `GlassSurface`s. | Implement app-specific controls only; do not create a generic third-party widget kit. |
| Input and motion | Pointer down/move/up, hit testing, press glow, drag lens, analytic interruptible springs. | Widget-local pointer handlers; semi-implicit Euler springs. | Native event loop owns pointer, keyboard, focus, and accessibility mapping; use analytic springs. |
| Caching | `content_changed` controls reusable blur cache; `Scene::update` reports rest. | No dependable content cache exposed in renderer; known limitations describe static backdrop per frame. | Cache native content/blur passes. Stop redraw when animation and input effects settle. |
| Platform integration | Caller owns `wgpu` device, target, and presentation. README also documents WebGPU/Wasm wrapper. | Has `glass-platform`, Wayland/layer-shell examples, and screencopy code. | Keep portable scene/render code free of Wayland protocol. Use a narrow Linux host layer-shell adapter only for overlay anchoring/stacking; it never owns rendering. |
| Dependencies | `wgpu 30`, `bytemuck`, `glam`, `slotmap`; dev `winit`. | `wgpu 0.19`, `bytemuck`, raw-window-handle, Serde, Wayland/SCTK workspace dependencies. | Use approved Rust 1.92, `wgpu 30.0.1`, and `glyphon 0.12`; retain `ziuus` 0.19 only as audited reference context. Keep `bytemuck`, `glam`, and existing `winit`; Linux layer-shell support remains platform-host-only, not a competing Rust widget library. |

## `liquid-rust`: observed architecture

### Modules and state

- **Observed.** Public API exports `Renderer`, `Frame`, `Scene`, `Glass`, containers, material/appearance types, and spring types. [`src/lib.rs`](https://github.com/al1h3n/liquid-rust/blob/main/src/lib.rs)
- **Observed.** `Scene` owns glass nodes and containers in `SlotMap`; nodes retain animated frame, presence, press, lift, stretch, and shape-morph state. `Glass` describes frame, shape, material, interactivity, lens mode, container, and z order. [`src/scene.rs`](https://github.com/al1h3n/liquid-rust/blob/main/src/scene.rs)
- **Observed.** Scene geometry is points in top-left coordinates. `Frame::scale` converts to pixels. [`src/scene.rs`](https://github.com/al1h3n/liquid-rust/blob/main/src/scene.rs) and [`src/renderer.rs`](https://github.com/al1h3n/liquid-rust/blob/main/src/renderer.rs)
- **Observed.** A container has merge spacing and z layer. Renderer clusters nearby members within a container; implementation states O(n²) per container. [`src/scene.rs`](https://github.com/al1h3n/liquid-rust/blob/main/src/scene.rs) and [`src/renderer.rs`](https://github.com/al1h3n/liquid-rust/blob/main/src/renderer.rs)

### Renderer lifecycle and cache

1. **Observed.** Host creates `Renderer` once with a caller-owned `wgpu::Device` and surface format. Every frame passes distinct readable `content` and writable `target` texture views, dimensions, scale, and `content_changed`. [`README.md`](https://github.com/al1h3n/liquid-rust/blob/main/README.md#contract) and [`src/renderer.rs`](https://github.com/al1h3n/liquid-rust/blob/main/src/renderer.rs)
2. **Observed.** Renderer copies content to target, builds or reuses blur pyramid in glass-reachable region, and renders each glass group as an instanced quad by layer. Higher overlapping layers can rebuild an offscreen backdrop; non-overlap does not pay this cost. [`src/renderer.rs`](https://github.com/al1h3n/liquid-rust/blob/main/src/renderer.rs)
3. **Observed.** Cached pixels are blur of unchanged content, not animation. `Scene::update` reports whether work remains, letting host stop requesting frames. [`src/renderer.rs`](https://github.com/al1h3n/liquid-rust/blob/main/src/renderer.rs) and [`README.md`](https://github.com/al1h3n/liquid-rust/blob/main/README.md#what-is-cached-and-what-never-is)
4. **Observed.** README describes half-resolution `Rgba16Float` blur pyramid with up to seven levels, and B-spline/interpolated sampling. [`src/renderer.rs`](https://github.com/al1h3n/liquid-rust/blob/main/src/renderer.rs) and [`src/shaders/glass.wgsl`](https://github.com/al1h3n/liquid-rust/blob/main/src/shaders/glass.wgsl)

### Material, motion, and input

- **Observed.** `Material` has optical and visual fields: refraction, depth, dispersion, frost, rim lighting, hairline/edge, shadow, tones, tint, and dimming. Presets are `regular`, `clear`, `clear_bar`, `tinted`, and `identity`. [`src/material.rs`](https://github.com/al1h3n/liquid-rust/blob/main/src/material.rs)
- **Observed.** `Appearance` carries dark scheme, transparency, reduced transparency, increased contrast, reduced motion, and content adaptation. [`src/material.rs`](https://github.com/al1h3n/liquid-rust/blob/main/src/material.rs)
- **Observed.** `Spring` is closed-form and retains velocity when retargeted. `without_bounce` provides a reduced-motion form. [`src/motion.rs`](https://github.com/al1h3n/liquid-rust/blob/main/src/motion.rs)
- **Observed.** Pointer APIs are `pointer_down`, `pointer_move`, and `pointer_up`; interactive glass supports press response and lens glass supports drag/lift/stretch. [`README.md`](https://github.com/al1h3n/liquid-rust/blob/main/README.md#merging-physics-and-morphs) and [`src/scene.rs`](https://github.com/al1h3n/liquid-rust/blob/main/src/scene.rs)
- **Observed.** WGSL uses signed-distance geometry and smooth union for fusion, then shades coverage, normals, frost, tone, tint, rim, shadow, and touch glow. Current source also implements Snell-style refraction and dispersion. [`src/shaders/glass.wgsl`](https://github.com/al1h3n/liquid-rust/blob/main/src/shaders/glass.wgsl)

**Inference.** Its texture contract and cache lifecycle transfer well to native WGPU presentation. Preserve caller-owned content/target textures, blur/tint/rim passes, and explicit invalidation; do not carry over its refraction and dispersion requirement.

## `liquid-glass`: observed architecture

### Modules, surfaces, and components

- **Observed.** Workspace separates `glass-core`, `glass-ui`, and `glass-platform`; README describes core optical pipeline/WGSL/material/springs, high-level UI, and Wayland/compositor bindings. [`README.md`](https://github.com/ziuus/liquid-glass/blob/master/README.md#workspace-architecture) and [`ARCHITECTURE.md`](https://github.com/ziuus/liquid-glass/blob/master/ARCHITECTURE.md)
- **Observed.** `GlassSurface` contains shape, normalized bounds, legacy material and parameters, curvature/bevel, pointer position/velocity, element type, and progress. It serializes through Serde. [`glass-core/src/geometry/mod.rs`](https://github.com/ziuus/liquid-glass/blob/master/glass-core/src/geometry/mod.rs)
- **Observed.** `glass-ui` derives render surfaces from `GlassCard`, `GlassButton`, `GlassToggle`, and `GlassSlider`. `GlassButton` owns `String` label and stateful hover/press springs. [`glass-ui/src/lib.rs`](https://github.com/ziuus/liquid-glass/blob/master/glass-ui/src/lib.rs)
- **Observed.** Its Spring uses semi-implicit Euler integration with configurable stiffness, damping, mass, velocity, and epsilon. [`glass-core/src/animation/mod.rs`](https://github.com/ziuus/liquid-glass/blob/master/glass-core/src/animation/mod.rs)

### Graphics pipeline and material model

- **Observed.** Renderer creates a Vulkan-only `wgpu` instance, a WGPU surface from `raw-window-handle`, one WGPU pipeline, and dynamic uniform storage sized for 1024 surfaces. [`glass-core/src/renderer/mod.rs`](https://github.com/ziuus/liquid-glass/blob/master/glass-core/src/renderer/mod.rs)
- **Observed.** Renderer allocates 512×512 base/background textures and fills base texture with generated dark gradient data before rendering. [`glass-core/src/renderer/mod.rs`](https://github.com/ziuus/liquid-glass/blob/master/glass-core/src/renderer/mod.rs)
- **Observed.** Architecture document says clipped quads, dynamic uniform offsets, and raw backdrop pixels uploaded using `queue.write_texture`. [`ARCHITECTURE.md`](https://github.com/ziuus/liquid-glass/blob/master/ARCHITECTURE.md)
- **Observed.** `VibrancyMaterial` offers semantic Window Background, Sidebar, Titlebar, Menu, Popover, Tooltip, HUD, Sheet, and Under Page Background presets. It includes blur, saturation, tint, opacity, blend mode, and reduced-transparency state. [`glass-core/src/material/mod.rs`](https://github.com/ziuus/liquid-glass/blob/master/glass-core/src/material/mod.rs)

### Platform status and limits

- **Observed.** Workspace pins `wgpu = "0.19"`; includes Wayland client/protocol/SCTK dependencies. [`Cargo.toml`](https://github.com/ziuus/liquid-glass/blob/master/Cargo.toml)
- **Observed.** `glass-platform` currently labels `WaylandContext` a placeholder, returns default capabilities, and sets `layer_surface = true` without actual global enumeration. [`glass-platform/src/lib.rs`](https://github.com/ziuus/liquid-glass/blob/master/glass-platform/src/lib.rs)
- **Observed.** Known limitations say background acquisition uses external `grim`; overlapping refraction uses one static backdrop instead of intermediate buffers; pointer velocity is not wired into uniforms; macOS/Windows need Wayland decoupling; GPU timing is incomplete. [`KNOWN_LIMITATIONS.md`](https://github.com/ziuus/liquid-glass/blob/master/KNOWN_LIMITATIONS.md)
- **Observed.** A screencopy prototype exists for `zwlr_screencopy_manager_v1`, with Wayland buffer conversion into RGBA bytes. [`examples/glass-panel/src/capture.rs`](https://github.com/ziuus/liquid-glass/blob/master/examples/glass-panel/src/capture.rs)

**Inference.** `ziuus/liquid-glass` is correct conceptual reference but not a Windows/macOS WebGPU dependency. Its Vulkan-only renderer, external screenshot route, Wayland coupling, and acknowledged missing interaction wiring disqualify direct reuse.
## Second-pass source audit — corrections

| Claim | Verified status | Evidence |
|---|---|---|
| Multi-pass Gaussian blur is active. | **Contradicted.** `BlurPipeline` is exported but never created or called by `GlassRenderer`; its `apply_blur` ignores source/radius/saturation inputs, encodes no passes, and returns an internal view. Active `glass.wgsl` instead uses a 16-sample spiral blur. | [`blur.rs:399-416`](https://github.com/ziuus/liquid-glass/blob/75055e67c47dfae577a35e81eb5cacbf1e1ad093/glass-core/src/renderer/blur.rs#L399-L416), [`renderer/mod.rs:1-8`](https://github.com/ziuus/liquid-glass/blob/75055e67c47dfae577a35e81eb5cacbf1e1ad093/glass-core/src/renderer/mod.rs#L1-L8), [`glass.wgsl:124-143`](https://github.com/ziuus/liquid-glass/blob/75055e67c47dfae577a35e81eb5cacbf1e1ad093/glass-core/src/renderer/glass.wgsl#L124-L143) |
| Vibrancy materials and reduced transparency reach rendering. | **Contradicted.** Draw path consumes legacy `Material` and `MaterialParams`; `GlassMaterial`/`VibrancyMaterial` values do not reach active uniforms. | [`material/mod.rs:39-228`](https://github.com/ziuus/liquid-glass/blob/75055e67c47dfae577a35e81eb5cacbf1e1ad093/glass-core/src/material/mod.rs#L39-L228), [`geometry/mod.rs:14-27`](https://github.com/ziuus/liquid-glass/blob/75055e67c47dfae577a35e81eb5cacbf1e1ad093/glass-core/src/geometry/mod.rs#L14-L27), [`renderer/mod.rs:433-461`](https://github.com/ziuus/liquid-glass/blob/75055e67c47dfae577a35e81eb5cacbf1e1ad093/glass-core/src/renderer/mod.rs#L433-L461) |
| Spring is RK4. | **Contradicted.** Implementation is semi-implicit Euler. | [`animation/mod.rs:141-164`](https://github.com/ziuus/liquid-glass/blob/75055e67c47dfae577a35e81eb5cacbf1e1ad093/glass-core/src/animation/mod.rs#L141-L164) |
| `glass-ui` is a complete native UI toolkit. | **Contradicted.** Button labels are stored but not rendered; slider lacks spring state; widgets do not propagate pointer velocity; no shared idle-redraw contract exists. | [`glass-ui/src/lib.rs:95-183`](https://github.com/ziuus/liquid-glass/blob/75055e67c47dfae577a35e81eb5cacbf1e1ad093/glass-ui/src/lib.rs#L95-L183), [`glass-ui/src/lib.rs:419-426`](https://github.com/ziuus/liquid-glass/blob/75055e67c47dfae577a35e81eb5cacbf1e1ad093/glass-ui/src/lib.rs#L419-L426) |
| Linux implementation is portable as shipped. | **Partly contradicted.** Wayland and capture code are isolated in example/platform crates, but shared `GlassRenderer` hard-codes Vulkan. | [`renderer/mod.rs:52-60`](https://github.com/ziuus/liquid-glass/blob/75055e67c47dfae577a35e81eb5cacbf1e1ad093/glass-core/src/renderer/mod.rs#L52-L60), [`glass-platform/src/lib.rs:22-47`](https://github.com/ziuus/liquid-glass/blob/75055e67c47dfae577a35e81eb5cacbf1e1ad093/glass-platform/src/lib.rs#L22-L47) |
| `glass_ui.wgsl`, `draw_glass_rect`, and 60 FPS production readiness exist. | **Contradicted or unverified.** Tracked renderer includes `glass.wgsl`; claimed API is absent; no benchmark ran. | [`renderer/mod.rs:103-106`](https://github.com/ziuus/liquid-glass/blob/75055e67c47dfae577a35e81eb5cacbf1e1ad093/glass-core/src/renderer/mod.rs#L103-L106), [`TRANSFORMATION_SUMMARY.md:173-178`](https://github.com/ziuus/liquid-glass/blob/75055e67c47dfae577a35e81eb5cacbf1e1ad093/docs/TRANSFORMATION_SUMMARY.md#L173-L178) |

**Resulting rule.** Reuse only verified patterns: serializable geometry/material roles, WGPU texture ownership, clipped SDF drawing, and explicit platform separation. Build a real blur/material pipeline and analytic spring system; do not port incomplete prototypes or documentation claims.

## Proposed owned stack

### Module boundary

```text
crates/volumectl
  host-owned audio, config, locale, hotkeys, tray, AppState, AppAction
        │ confirmed state + input events
        ▼
crates/liquid_glass
  scene + material resolver + analytic animation + WGSL render passes
        │ one WGPU renderer per native surface
        ▼
winit windows on Linux, macOS, and Windows
```

Create one owned crate, `crates/liquid_glass`. Its runtime graphics dependencies are pinned `wgpu 30.0.1` with WGSL, DX12, Metal, Vulkan, and Windows `static-dxc`, plus `glyphon 0.12`, `bytemuck`, `glam`, and existing `winit` at app boundary; workspace MSRV is Rust 1.92. It contains no Tauri, React, WebView, screenshot capture, or generic Rust UI toolkit. A Linux-only host layer-shell adapter may anchor and stack overlay but does not enter this portable crate or render pixels.

### Portable WGPU solution

Keep `ziuus/liquid-glass` as primary conceptual input and `liquid-rust` as secondary pattern evidence only. Replace `ziuus` Vulkan-only instance and incomplete material/blur path with owned, target-independent rendering; neither project enters dependency graph.

| Layer | Linux reference behavior | Required portable replacement |
|---|---|---|
| Instance | Hard-codes `wgpu::Backends::VULKAN`. | Windows tries a DX12 instance first, then a new Vulkan instance only if DX12 adapter creation fails; macOS creates Metal; Linux creates Vulkan. Never use `PRIMARY` as a preference mechanism. |
| Adapter | Requests one compatible adapter. | Request adapter with `compatible_surface: Some(&surface)`. Treat no adapter as recoverable unavailable-GPU state. |
| Surface | Uses raw window handle and selects first sRGB format. | Create each surface from `winit::window::Window`; select only reported format, present mode, and alpha mode; reconfigure on resize and scale-factor changes. |
| Background | Uses static/generated texture and experimental Wayland capture. | Render app-owned native content to readable texture. Real multi-pass blur samples that texture. No screen capture is required for baseline. |
| Device/surface acquisition | No documented recovery policy. | Acquire with `Surface::get_current_texture()` from pinned `wgpu 30.0.1`. `Success(texture)`: render, submit, present. `Suboptimal(texture)`: render, submit, present, then schedule reconfiguration. `Timeout` or `Occluded`: skip frame and await later redraw/exposure. `Outdated`: reconfigure from latest nonzero size before next acquisition. `Lost`: recreate surface, then configure from latest nonzero size. `Validation`: controlled surface-error state until configuration is corrected; never blind-retry. Rebuild adapter, device, queue, surface, and all GPU resources on device loss. |

**Observed.** Pinned WGPU 30 `Surface::get_current_texture()` returns `CurrentSurfaceTexture::{Success, Suboptimal, Timeout, Occluded, Outdated, Lost, Validation}`. `Success` and `Suboptimal` carry a presentable texture; remaining statuses do not. [`surface.rs:140-198`](https://raw.githubusercontent.com/gfx-rs/wgpu/v30.0.1/wgpu/src/api/surface.rs).

**Product decision.** Device/process out-of-memory is a controlled safe-failure path, not a `CurrentSurfaceTexture` acquisition status and never a retry loop. WGPU surface configuration occurs only after prior acquired textures are dropped; reconfiguration uses last valid configuration plus latest nonzero extent.

**Inference.** Explicit backend selection gives a testable native policy. It does not establish GPU, driver, WSI, compositor, HDR, transparency, or presentation success until runtime validation on every target.

### Renderer data model

Use compact, allocation-stable scene storage:

- `SurfaceId`, `MaterialRole`, geometry, z order, visual state, focus state, and content revision.
- `MaterialRole::{Hud, Panel, Sidebar, Menu, Popover, Control, Accent, Tooltip}`.
- `Appearance { color_scheme, transparency, contrast, reduce_motion }`.
- Pointer sample with time, position, and filtered velocity.
- `FrameInput { size, scale, content_revision, dirty_regions }`.

**Product decision.** Material resolution maps role plus appearance to blur, tint, opacity, saturation, edge/rim, shadow, and motion amplitudes. Reduced transparency or high contrast selects opaque material tokens; `increase_contrast` raises foreground separation and edge clarity. `reduce_motion` removes bounce, stretch, ripple propagation, and gravity offset, while retaining immediate state feedback. Host platform adapters must publish reduced-transparency state through `UiCapabilities`.

### Render lifecycle

1. App shell converts confirmed `AppState` into native semantic layout, control geometry, text runs, and material roles.
2. Core resolves changed roles and appearance, then uploads packed surface/material uniforms.
3. Renderer draws native content into an offscreen readable texture, builds/reuses downsampled blur levels, and draws scissored SDF material groups by z order into WGPU target.
4. Renderer draws text, icons, focus rings, and controls above glass material in the same WGPU frame.
5. Core returns `animating`; host requests another redraw only while true or after content/input changes.
6. Content revision invalidates blur cache. Geometry-only changes reuse blur levels when sampling bounds stay valid.

**Product decision.** Native content texture replaces WebView backdrop sampling. Desktop wallpaper or other-application capture is a separate permission-sensitive feature and stays out of this cutover.

### Bounded fluid behavior

| Requirement | Owned implementation | Limit |
|---|---|---|
| Smooth SDF union | Group nearby related material surfaces into one clipped SDF quad. Use smooth-min union and a small merge threshold. | Restrict each group to UI-scale counts. Do not run global pairwise unions across a window. |
| Surface tension | Derive neck width and edge highlight from union field gradient and gap. | Visual approximation only; no mass conservation or fluid solver. |
| Velocity stretch/trailing | Feed filtered pointer/drag velocity to anisotropic transform and short decaying offset. | Cap amplitude and return to zero at rest. Hit testing uses settled control bounds. |
| Spring settling | Use closed-form scalar and vector springs, interrupted with retained velocity. | Stop scheduling at rest. Do not use Euler integration in new core. |
| Interaction ripple | Add bounded radial ripple/glow inside active SDF surface. | Render visual feedback only; native control dispatch stays authoritative. |
| Gravity asymmetry | Apply small downward lighting/shadow bias and temporary post-release sag. | No permanent motion. Disable under reduced motion. |

**Observed support.** `liquid-rust` proves SDF union, interactive stretch, and analytic spring patterns in one Rust/WGPU design. [`src/scene.rs`](https://github.com/al1h3n/liquid-rust/blob/main/src/scene.rs), [`src/motion.rs`](https://github.com/al1h3n/liquid-rust/blob/main/src/motion.rs), and [`src/shaders/glass.wgsl`](https://github.com/al1h3n/liquid-rust/blob/main/src/shaders/glass.wgsl). `ziuus/liquid-glass` exposes pointer position/velocity fields but lists missing uniform wiring as a limitation. [`glass-core/src/geometry/mod.rs`](https://github.com/ziuus/liquid-glass/blob/master/glass-core/src/geometry/mod.rs) and [`KNOWN_LIMITATIONS.md`](https://github.com/ziuus/liquid-glass/blob/master/KNOWN_LIMITATIONS.md).

## Cross-platform feasibility and non-goals

| Area | Feasible direction | Constraint |
|---|---|---|
| Windows | `winit` surface plus WGPU's platform-selected adapter/presentation path. | Validate feature level, device loss recovery, DPI, system contrast, and screen-reader exposure. |
| macOS | Same WGPU core with Metal-selected adapter and native window policy. | Validate Retina scale, AppKit activation, system appearance/accessibility state, and text rendering. WWDC26 Liquid Glass behavior is design evidence only; `NSVisualEffectView` is outside initial WGPU-only cutover. |
| Linux | Same WGPU renderer with explicit Vulkan backend; preserve current product support. | Portable renderer has no Wayland protocol code, screencopy, or `grim`. Linux host owns narrow layer-shell placement/stacking adapter for bottom-right overlay. Overlay click-through is capability-gated; Wayland HUD stays interactive when input shapes do not apply. |
| Desktop wallpaper/other-app sampling | Not part of initial renderer contract. | Requires explicit native capture/compositor API and permission/security analysis. |
| Native overlay, mixer, settings, and help | All move to same WGPU core and scene model. | Each requires native text, focus, keyboard, and assistive-technology behavior before WebView retirement. |

**Non-goals.** Direct `ziuus/liquid-glass` adoption; copying either project; `grim`/screencopy usage; native cross-window texture sharing; Navier–Stokes simulation; mandatory refraction/dispersion; retaining Tauri/React/WebView UI after cutover.
## Full-native delivery requirements

WGPU draws pixels and Winit creates windows/events. Neither replaces text editing, IME, focus traversal, or accessibility. Keep one renderer; use low-level support libraries only where platform behavior would otherwise be absent:

| Need | Boundary | Required proof |
|---|---|---|
| Vietnamese text | Owned `TextSystem` wraps a compatible released `glyphon` version for `cosmic-text` shaping/fallback, glyph-atlas caching, and WGPU draw. It is not a second UI renderer. | Resolve WGPU 30 compatibility; prove Vietnamese glyph fallback and long-device-name layout on all targets. |
| IME | Native input adapter accepts `winit::WindowEvent::Ime`, enables IME only for focused editor, and updates cursor rectangle. | Vietnamese preedit, commit, cancellation, caret, selection, clipboard, and shortcut smoke. |
| Focus and pointer | Retained focus manager maps keyboard/pointer input into complete host intent contract; visual bounds and hit targets remain aligned during fluid effects. | Tab/Shift+Tab, arrow controls, Escape, disabled controls, pointer capture, click-through on capable paths, and interactive Wayland HUD. |
| Screen readers | Stable AccessKit tree with names, roles, ranges, checked state, and actions. `accesskit_winit` is candidate adapter, not another renderer. | UI Automation, VoiceOver, and AT-SPI actions update visible and accessibility state together. |
| OS glass | Initial cutover uses WGPU-owned material only. DWM, `NSVisualEffectView`, and Linux compositor material are post-cutover evaluation, never a baseline dependency. | System transparency, contrast, RDP/VM, and opaque fallback each remain readable without OS-material integration. |

**Observed.** Winit documents itself as low-level window/event infrastructure; it does not render content. [`winit README`](https://github.com/rust-windowing/winit/blob/f6893a4390dfe6118ce4b33458d458fd3efd3025/README.md#L25-L36). AccessKit maps platform accessibility systems through its adapters. [`AccessKit README`](https://github.com/AccessKit/accesskit/blob/a39329d2819381510496c17e81e87763edbb8bb/README.md#L8-L65).

**Resolved gate.** Rust 1.92, released `wgpu 30.0.1`, `glyphon 0.12`, Windows `static-dxc`, DX12 → Vulkan Windows fallback, and WGPU-only initial material baseline are approved. Before implementation, validate package resolution, target builds, Vietnamese text, IME, and platform accessibility behavior.

## Exact primary sources

### `al1h3n/liquid-rust`

- [`README.md`](https://github.com/al1h3n/liquid-rust/blob/main/README.md)
- [`Cargo.toml`](https://github.com/al1h3n/liquid-rust/blob/main/Cargo.toml)
- [`src/lib.rs`](https://github.com/al1h3n/liquid-rust/blob/main/src/lib.rs)
- [`src/scene.rs`](https://github.com/al1h3n/liquid-rust/blob/main/src/scene.rs)
- [`src/material.rs`](https://github.com/al1h3n/liquid-rust/blob/main/src/material.rs)
- [`src/motion.rs`](https://github.com/al1h3n/liquid-rust/blob/main/src/motion.rs)
- [`src/renderer.rs`](https://github.com/al1h3n/liquid-rust/blob/main/src/renderer.rs)
- [`src/shaders/glass.wgsl`](https://github.com/al1h3n/liquid-rust/blob/main/src/shaders/glass.wgsl)

### `ziuus/liquid-glass`

- [`README.md`](https://github.com/ziuus/liquid-glass/blob/master/README.md)
- [`Cargo.toml`](https://github.com/ziuus/liquid-glass/blob/master/Cargo.toml)
- [`ARCHITECTURE.md`](https://github.com/ziuus/liquid-glass/blob/master/ARCHITECTURE.md)
- [`KNOWN_LIMITATIONS.md`](https://github.com/ziuus/liquid-glass/blob/master/KNOWN_LIMITATIONS.md)
- [`glass-core/Cargo.toml`](https://github.com/ziuus/liquid-glass/blob/master/glass-core/Cargo.toml)
- [`glass-core/src/geometry/mod.rs`](https://github.com/ziuus/liquid-glass/blob/master/glass-core/src/geometry/mod.rs)
- [`glass-core/src/material/mod.rs`](https://github.com/ziuus/liquid-glass/blob/master/glass-core/src/material/mod.rs)
- [`glass-core/src/animation/mod.rs`](https://github.com/ziuus/liquid-glass/blob/master/glass-core/src/animation/mod.rs)
- [`glass-core/src/renderer/mod.rs`](https://github.com/ziuus/liquid-glass/blob/master/glass-core/src/renderer/mod.rs)
- [`glass-platform/src/lib.rs`](https://github.com/ziuus/liquid-glass/blob/master/glass-platform/src/lib.rs)
- [`glass-ui/src/lib.rs`](https://github.com/ziuus/liquid-glass/blob/master/glass-ui/src/lib.rs)
- [`examples/glass-panel/src/capture.rs`](https://github.com/ziuus/liquid-glass/blob/master/examples/glass-panel/src/capture.rs)
