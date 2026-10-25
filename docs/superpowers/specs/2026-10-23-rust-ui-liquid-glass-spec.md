# Native Liquid Glass Product Specification (D3)

**Document ID:** D3
**Status:** Approved (Gate 3 Approved 2026-10-23)
**Parent Requirements:** [`docs/plans/rust-ui-liquid-glass/gate-1-requirements.md`](../../plans/rust-ui-liquid-glass/gate-1-requirements.md)
**Approved Design:** [`docs/superpowers/specs/2026-10-23-rust-ui-liquid-glass-design.md`](2026-10-23-rust-ui-liquid-glass-design.md)
**Master Index:** [`docs/plans/rust-ui-liquid-glass/README.md`](../../plans/rust-ui-liquid-glass/README.md)

---

## 1. Problem Statement

VolumeControl currently relies on a hybrid architecture: native platform host code in Rust communicates across Tauri IPC with webview windows running React, Tailwind, and Radix UI. While functional, this webview runtime incurs severe system penalties for a background utility:
1. **Memory & Binary Bloat**: Each open surface runs a multi-process browser engine (WebView2 on Windows, WebKitGTK on Linux, WKWebView on macOS), consuming 120–250 MB RAM and delaying window display by 200–500 ms.
2. **Glass Fidelity Inconsistencies**: Webview backdrop filters cannot efficiently coordinate multi-pass Gaussian blurs, signed-distance field (SDF) fluid union, or real-time specular highlights without high GPU battery drain.
3. **Architecture Duplication**: Maintenance requires synchronizing TypeScript frontend state with Rust host state across asynchronous IPC bridges, introducing state race conditions and draft divergence.

The solution is an owned, single-renderer native Rust UI crate (`crates/liquid_glass`) built on `wgpu 30.0.1` and `glyphon 0.12`, providing high-fidelity Apple-style Liquid Glass optics with zero webview overhead.

---

## 2. Goals & Non-Goals

### 2.1 Goals
- **Single Renderer**: Replace all webview surfaces (Overlay, Mixer, Settings, Help, Tray) with one native WGPU renderer.
- **Strict Host Authority**: Maintain `crates/volumectl` as the sole authority for audio endpoints, session streams, configuration persistence, and hotkeys.
- **Tactile Liquid Optics**: Deliver real downsampled multi-pass Gaussian blur over app-owned content, SDF control merging, closed-form analytic spring physics, and specular rim lighting.
- **Full Accessibility Parity**: Integrate AccessKit to expose a full semantic accessibility tree to Windows UI Automation, macOS VoiceOver, and Linux AT-SPI.
- **Bilingual Support**: Render all copy in English and Vietnamese with Unicode shaping, diacritic fallback fonts, and native IME composition rect tracking.
- **Robust Recovery**: Implement comprehensive WGPU surface recovery (`CurrentSurfaceTexture` states) and full GPU rebuild on device loss.

### 2.2 Non-Goals
- Direct dependency on third-party experimental repositories (`ziuus/liquid-glass` or `al1h3n/liquid-rust`).
- Desktop wallpaper, external screen capture, or other-window snooping.
- Full Navier–Stokes hydrodynamic simulation or continuous idle "breathing" animations.
- Hybrid coexistence: no webview fallback shims survive final release cutover.

---

## 3. Shared Glossary

| Term | Definition |
|---|---|
| **Host** | `crates/volumectl`: owns audio devices, sessions, `config.ini`, hotkeys, and event loop. Publishes `AppState` and consumes `AppAction`. |
| **Renderer** | `crates/liquid_glass`: receives `AppState`, manages retained scene, computes layout, executes WGSL shaders, renders via WGPU. |
| **Retained Semantic Scene** | UI hierarchy where nodes retain stable semantic IDs across frames, used identically for hit testing, focus, AccessKit, and draw passes. |
| **MaterialRole** | Semantic visual category (`Hud`, `Panel`, `Sidebar`, `Control`, `Accent`, `Tooltip`, `Menu`) mapping to optics tokens. |
| **Analytic Spring** | Closed-form second-order harmonic oscillator calculated directly from timestamp $t$, requiring zero numerical integration steps. |
| **SDF Smooth Union** | Mathematical blending of signed-distance shapes (e.g. slider track and thumb) creating organic liquid cohesion. |
| **Appearance** | Display modifiers: color scheme (Light/Dark), `reduced_transparency`, `increase_contrast`, `reduce_motion`. |
| **Opaque Fallback** | Accessible material state where blur is 0 and alpha is 1.0, triggered by high contrast or reduced transparency. |

---

## 4. Functional Requirements per Surface

### 4.1 Volume Mixer HUD (`mixer.png`)
- **Baseline Reference**: `C:\Users\Thanh\AppData\Local\Temp\volume-control-ui-captures\mixer.png`
- **Geometry**: Floating card, design dimensions 400×224 logical px, anchored above taskbar or centered per user configuration. Always-on-top.
- **Components**:
  - System output device rail with master volume slider (0–100%), mute button, and active device label.
  - Search filter input with Vietnamese IME support to filter application sessions.
  - Scrollable list of active audio session rails (app icon, process name, volume slider, mute toggle).
- **Interactions**:
  - Dragging a slider emits `AppAction::SetVolumePercent` (master) or `AppAction::SetSessionVolume` (session).
  - Clicking mute emits `AppAction::ToggleMute` or `AppAction::ToggleSessionMute`.
  - Keyboard navigation: Up/Down moves between rails; Left/Right adjusts volume by 1% (or 5% with Shift); 'M' toggles mute; Escape closes window.

### 4.2 General & Appearance Settings (`settings_general.png`, `settings_appearance.png`)
- **Baseline Reference**: `C:\Users\Thanh\AppData\Local\Temp\volume-control-ui-captures\settings_general.png`, `settings_appearance.png`
- **Geometry**: Windowed surface, 760×620 design dimensions (resizable, minimum 620×520 logical px).
- **Components**:
  - Left navigation sidebar (`Sidebar` material role) with sections: General, Appearance, Hotkeys, Blacklist, Updates.
  - Main scrollable content form (`Panel` material role).
  - Appearance controls: Theme picker (`ThemeMode`), Material picker (`MaterialMode`), Motion picker (`MotionMode`), Accent picker (`AccentMode`), Language picker (`Language`).
  - Bottom action bar: Reset to Defaults, Cancel Changes, Save (`ApplyConfig`).
- **Draft State Contract**:
  - Changing an appearance control immediately updates the local draft state and refreshes the preview.
  - Unsaved draft changes do **not** mutate `config.ini` or host `AppState`.
  - Clicking Cancel restores the original confirmed `AppState`.
  - Clicking Save dispatches `AppAction::ApplyConfig`, prompting host to persist changes to `config.ini` and publish updated `AppState`.

### 4.3 Bottom-Right Overlay HUD
- **Geometry**: 336×88 logical px, positioned at bottom-right of primary display work area (accounting for DPI and negative monitor offsets).
- **Behavior**:
  - Displays master volume percentage gauge, mute status icon, and current device name.
  - Slides/fades in on volume change, auto-hides after configured timeout (default 1500 ms).
  - Click-through: pointer events pass through to underlying windows on macOS and Linux X11; Wayland HUD remains interactive without input shape corruption.

### 4.4 Help Reference Card
- **Geometry**: 520×500 logical px dialog with glass backdrop.
- **Content**: Bilingual keyboard shortcut reference table, application version, and documentation links. Escape or Close button dismisses.

### 4.5 System Tray Context Menu
- **Components**: Standard tray menu offering quick volume toggles, surface launchers (Mixer, Settings, Help), Config location shortcut, and Exit.
- **Style**: Rendered using native OS tray primitives or compact WGPU popup menu (`Menu` material role).

---

## 5. Non-Functional Requirements

| ID | Category | Requirement Specification | Measurement & Target |
|---|---|---|---|
| **N1** | Architecture | Single owned renderer crate (`crates/liquid_glass`). No webviews, React, or competing Rust UI engines. | `cargo tree` audit verifies zero webview or second UI crate dependencies. |
| **N2** | Platform Stack | Rust MSRV 1.92, pinned `wgpu 30.0.1`, `glyphon 0.12`, WGSL shaders, Windows `static-dxc`. | Clean workspace compilation with locked dependencies. |
| **N3** | Performance | Allocation-stable frame loop. Multi-pass Gaussian blur pyramid cached by `content_revision`. Zero redraws when settled. | 0 heap allocations during steady-state animation; 0% CPU/GPU usage when idle. |
| **N4** | GPU Reliability | Handle all 7 `CurrentSurfaceTexture` states. Full GPU stack recreation on device loss. Safe exit on OOM. | 100% pass rate across injectable WGPU fault simulation tests. |
| **N5** | Accessibility | High contrast and reduced transparency force opaque fallback. Full AccessKit tree. WCAG AA compliance. | Automated contrast ratio >= 4.5:1; UI Automation and VoiceOver navigation verified. |
| **N6** | Clean Cutover | No release shims, bridge aliases, or transitional webview code survive in production release. | Final code audit confirms total deletion of Tauri frontend assets and bridge code. |

---

## 6. Component Mapping Architecture

```text
Scene Hierarchy (crates/liquid_glass)
  ├── WindowRoot (SurfaceId)
  │     ├── BackgroundBackdrop (App-owned Content Texture)
  │     │     └── BlurPyramidPass (Downsampled Gaussian WGSL)
  │     ├── MaterialLayer (SDF Quads & Smooth Unions)
  │     │     ├── HudSurface (MaterialRole::Hud)
  │     │     ├── CardPanel (MaterialRole::Panel)
  │     │     └── SliderTrack & Thumb (MaterialRole::Control + SDF Union)
  │     └── ForegroundContent
  │           ├── TextRuns (TextSystem -> Glyphon -> Cosmic-Text Atlas)
  │           ├── VectorIcons (WGSL Path / Texture Mask)
  │           └── FocusRing (SDF Border Highlight)
```

---

## 7. Given / When / Then Acceptance Criteria

### Scenario 1: Master Volume Adjustment (Overlay HUD)
- **Given** VolumeControl is running in the background and the Overlay is hidden.
- **When** the user presses a global volume hotkey (e.g., Volume Up).
- **Then** the host dispatches `AppAction::AdjustVolume`, adjusts hardware volume, and publishes updated `AppState`.
- **And** the native Overlay HUD appears at the bottom-right within 16 ms (1 frame), displaying the new volume percentage and an animated liquid fill.
- **And** the Overlay automatically begins its fade-out after the configured display duration.

### Scenario 2: Per-App Session Mute in Mixer
- **Given** the Volume Mixer HUD is visible and shows an active audio session for "Spotify".
- **When** the user clicks the mute button on the Spotify session rail.
- **Then** the renderer emits `AppAction::ToggleSessionMute { id: "Spotify" }`.
- **And** the host invokes the WASAPI session mute interface and publishes updated `AppState.sessions`.
- **And** the Spotify mute toggle updates its visual state and AccessKit `checked` property without altering other session sliders.

### Scenario 3: Settings Appearance Draft and Cancel
- **Given** the Settings window is open on the Appearance tab with confirmed material `Auto`.
- **When** the user selects `Translucent` in the material dropdown.
- **Then** the Settings preview surface immediately re-renders using the translucent material tokens.
- **And** `config.ini` remains unmodified on disk.
- **When** the user clicks the Cancel button.
- **Then** the draft settings are discarded, the UI reverts to `Auto`, and no file I/O occurs.

### Scenario 4: Accessibility High Contrast & Reduced Transparency
- **Given** the operating system has High Contrast Mode or "Reduce Transparency" enabled.
- **When** the Mixer or Settings window is rendered.
- **Then** `UiCapabilities` reflects `high_contrast: true` or `reduced_transparency: true`.
- **And** `resolve_material` forces `ResolvedMaterial::Opaque` across all material roles.
- **And** all glass surfaces render fully opaque backgrounds (alpha = 1.0, blur = 0) with high-contrast borders and text contrast exceeding WCAG AA 4.5:1.

### Scenario 5: GPU Surface Invalidation & Recovery
- **Given** the Volume Mixer is open and rendering via WGPU.
- **When** the user resizes the window or triggers a display resolution change resulting in `CurrentSurfaceTexture::Outdated`.
- **Then** the renderer drops previous frame textures, queries the latest nonzero window dimensions, and calls `Surface::configure()`.
- **And** rendering resumes seamlessly on the subsequent frame without dropped input or panic.

### Scenario 6: GPU Device Loss Recovery
- **Given** a catastrophic GPU driver reset occurs, resulting in `CurrentSurfaceTexture::Lost` or device loss.
- **When** the renderer detects device failure.
- **Then** it cleanly tears down existing textures, recreates the WGPU adapter, device, queue, surface, and pipelines.
- **And** the scene re-uploads uniforms and restores visible UI state cleanly.

---

## 8. Open Questions, Technical Assumptions, and Mitigations

| Item | Nature | Assumption / Decision | Mitigation Strategy |
|---|---|---|---|
| **`glyphon 0.12` on Rust 1.92** | Dependency | Requires Rust 1.92 due to `cosmic-text 0.19`. Approved in architecture review. | Workspace MSRV will be bumped to 1.92 in Ticket T0. |
| **Linux Wayland Overlay Placement** | Platform | Standard Winit cannot position overlay windows on Wayland. | Use narrow host-only layer-shell adapter in `crates/volumectl/src/ui/platform/linux/`. |
| **Vietnamese IME Composition** | Localization | Custom text fields must track IME candidate windows. | Native input adapter listens to `winit::WindowEvent::Ime` and calls `set_ime_cursor_area`. |
| **Reduced Transparency Detection** | Capability | Operating systems expose transparency settings through varying APIs. | Windows reads DWM registry/SPI; macOS reads `NSWorkspace`; Linux reads GSettings. |
| **Font Fallback Coverage** | Typography | Vietnamese characters require fallback glyphs if default font lacks coverage. | `cosmic-text` in `glyphon` configured with system fallback font list (Segoe UI, SF Pro, Cantarell). |
