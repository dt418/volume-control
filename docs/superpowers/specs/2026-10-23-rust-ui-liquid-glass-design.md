# Rust/UI and Liquid Glass Visual System Design

> Approved architectural design for modernizing VolumeControl UI surfaces with Rust/UI component density and Apple-style Liquid Glass optics.

**Document ID**: D2-DESIGN
**Date**: 2026-10-23
**Status**: Formal Design Approved (Gate 2)
**Product**: VolumeControl
**Target Architecture**: Tauri v2 Hybrid Desktop (Windows 11, macOS Sonoma/Sequoia, Linux Ubuntu 24.04+)
**Language**: English (Official Specification)

---

## 1. Executive Summary & Design Vision

VolumeControl currently relies on a functional, dark-themed user interface utilizing standard Shadcn/Tailwind box geometry and a flat 12px backdrop blur. While stable and responsive, the visual language lacks the optical depth, ambient refraction, and tactile density found in modern desktop operating systems.

This design introduces a unified **Liquid Glass & Rust/UI Visual System**:
1. **Apple Liquid Glass Optics**: Multi-stage physical material simulation featuring background refraction blur, luminance-preserving OKLab tinting, 1px directional specular rim lighting, and subtle ambient occlusion shadows.
2. **Rust/UI Component Ergonomics**: Adopts compact, tactile UI primitives inspired by `rust-ui.com`—specifically `CardSm` high-density containers, pill-shaped badges, and micro-spring interactive feedback.
3. **Preserved Engineering Invariants**: Zero new heavy runtime JS dependencies, full preservation of the 43 existing test suites, strict single-pass backdrop filtering (anti-blur multiplication), and 100% compliance with `config.ini` as the single source of truth.

---

## 2. Architectural Evaluation & Selected Strategy

Three architectural paths were evaluated for integrating Rust/UI and Liquid Glass into VolumeControl:

| Evaluation Dimension | Option A: Port Rust/UI to React 19 + Tailwind v4 (Selected) | Option B: Full Rewrite in Leptos / Dioxus WASM | Option C: Native-Only Renderers (Direct2D / AppKit / Cairo) |
|---|---|---|---|
| **Bundle Footprint Impact** | **+2.1 kB CSS, 0 kB JS** (Total stays ~520 kB raw, ~159 kB gzip). | **+2.4 MB** (WASM runtime + Trunk/wasm-bindgen bloat). | **0 kB** web assets; increases binary size by ~180 kB C/Rust. |
| **Cold Start Latency** | **< 16 ms** (instant DOM mount from cached Webview). | **120–250 ms** (WASM instantiation + JIT compilation per window). | **< 8 ms** (immediate OS HWND/NSView creation). |
| **Test Suite Preservation** | **100% preserved** (all 43 Vitest suites continue running without change). | **0% preserved** (must rewrite all 43 tests in wasm-bindgen-test). | **N/A** (UI logic split across 3 platform C/Rust bindings). |
| **Visual Expressiveness** | **Maximum** (modern CSS backdrop-filter, OKLab, SVG filters, spring transitions). | **High** (Tailwind with Leptos rsx). | **Severely limited** (custom shader pipelines required for Direct2D/Cairo blur). |
| **Multi-Platform Parity** | **Identical** DOM & styling across WebView2, WKWebView, WebKitGTK. | Identical DOM via WASM, but tricky WebKitGTK canvas support. | **High divergence** (3 distinct custom drawing engines to maintain). |
| **Recommendation** | **SELECTED (Optimal)** | Rejected (Unnecessary weight & startup lag). | Rejected (High maintenance, low visual flexibility). |

### Decision Summary
We adopt **Option A**: porting Rust/UI's component semantics, Tailwind CSS 4.0 classes, and compact layout patterns (`CardSm`, pill badges, capsule sliders) into VolumeControl's existing React 19 + Tailwind v4 webview layer (`frontend/src/`). Native renderers in `crates/volumectl/src/ui/platform/` continue to provide immediate fallback canvases.

---

## 3. Optical Architecture: Multi-Layer Liquid Glass System

The visual system is structured as four strictly ordered elevation tiers (Z0 through Z3) adhering to Apple Liquid Glass principles G1–G16 and ADR 0002.

```
+-------------------------------------------------------------------------+
| Z3: Interactive Controls & Badges (Capsules, Signal Rail, Buttons)     |
|     - Solid or high-contrast translucent pills (color-mix 85-95%)      |
|     - Tactile spring micro-interactions on hover/active                |
+-------------------------------------------------------------------------+
| Z2: Content Layer / Grouped Cards (CardSm, Session Rows, Inputs)        |
|     - Semi-translucent tint: color-mix(in oklab, var(--card) 60%, trans)|
|     - Crisp 1px border: hsl(var(--border) / 0.4)                       |
|     - STRICT INVARIANT: ZERO backdrop-filter: blur() (Anti-Blur G12)    |
+-------------------------------------------------------------------------+
| Z1: Surface Shell (Window Frame / HUD Container)                        |
|     - Single-pass backdrop blur: backdrop-filter: blur(20px)           |
|     - Saturation boost: saturate(180%)                                 |
|     - OKLab Luminance tint: color-mix(in oklab, var(--background) 80%)  |
|     - 1px Specular Rim Highlight: inset 0 1px 0 0 rgba(255,255,255,0.22)|
|     - Ambient Occlusion Drop Shadow: 0 16px 36px rgba(0, 0, 0, 0.35)   |
|     - Curvature: Continuous squircle border-radius: 18px               |
+-------------------------------------------------------------------------+
| Z0: Desktop Operating System Wallpaper / Background Windows             |
+-------------------------------------------------------------------------+
```

### 3.1. Optical Phase Breakdown

1. **Phase 1: Refraction Blur**:
   - `backdrop-filter: blur(20px) saturate(180%);`
   - `-webkit-backdrop-filter: blur(20px) saturate(180%);`
   - High-frequency desktop textures (wallpaper patterns, small text) are diffused into smooth, ambient luminance gradients.
2. **Phase 2: Perceptual Luminance Tint**:
   - Instead of flat alpha blending, uses CSS `color-mix(in oklab, var(--background) 82%, transparent)`.
   - Preserves contrast across high-key and low-key desktop backgrounds without clipping saturated channels.
3. **Phase 3: Specular Rim Lighting (G3)**:
   - Simulates ambient room light refracting along the bevel of physical glass.
   - Dark Theme: `box-shadow: inset 0 1px 0 0 rgba(255, 255, 255, 0.22), 0 16px 36px -4px rgba(0, 0, 0, 0.4);`
   - Light Theme: `box-shadow: inset 0 1px 0 0 rgba(255, 255, 255, 0.70), 0 16px 36px -4px rgba(20, 24, 40, 0.12);`
4. **Phase 4: Dynamic Focus Modulation (G7)**:
   - When a surface loses focus (`:not(:focus-within)` or window blur event), specular opacity dims to `0.08`, and base tint increases to 92%, causing the window to quietly settle into the background.

---

## 4. Rust/UI Component Integration Contract

Inspired by `rust-ui.com`, VolumeControl's UI components in `frontend/src/components/ui/` adopt dense, tactile patterns:

### 4.1. `CardSm` High-Density Container
Standard Radix/Shadcn cards have generous 24px padding (`p-6`), which consumes excessive screen real estate in a 400×224px mixer or compact settings dialog. We introduce `CardSm`:
- Padding: `p-3` (12px) for mixer rows, `p-4` (16px) for settings sections.
- Radius: `rounded-xl` (12px) nested inside the `rounded-2xl` (18px) shell.
- Background: `bg-card/60 backdrop-blur-none border border-border/40`.
- Hover State: Inset border glow via accent color (`hover:border-accent/50 transition-colors duration-150`).

### 4.2. Tactile Pill Buttons & Badges
- Pill Geometry: Full capsule radius (`rounded-full px-3 py-1`).
- Active State: Physical compression effect (`active:scale-[0.97] transition-transform duration-100 ease-out`).
- Signal Rail Integration: Thumbs and indicators adopt capsule pill profiles with smooth hardware color threshold transitions (`#27ae60`, `#0078d4`, `#e05c00`).

---

## 5. Spring Kinematics & Motion Policy

### 5.1. Spring Dynamics (MotionMode::Full)
- **Modal Entry**: Scale up from `0.96` to `1.0` and fade from `0` to `1` over `200ms` with spring curve `cubic-bezier(0.16, 1, 0.3, 1)`.
- **Mixer HUD Presentation**: Slide 8px upwards from the tray with subtle overshoot damping.
- **Slider Dragging**: Friction-damped thumb tracking with zero latency, accompanied by smooth color threshold interpolation.

### 5.2. Strict Reduced Motion Invariant (MotionMode::Reduced & Disabled)
Adheres strictly to G9 and existing code in `frontend/src/styles.css`:
```css
.reduced-motion *,
.reduced-motion *::before,
.reduced-motion *::after {
  animation-duration: 0.001s !important;
  transition-duration: 0.001s !important;
  transform: none !important;
}
```
When `config.ini` defines `motion=Reduced` or `motion=Disabled`, all scale transforms, slide offsets, and spring physics are completely neutralized.

---

## 6. Ground-Truth Surface Mapping

Mapped against the three verified UI snapshots from `C:\Users\Thanh\AppData\Local\Temp\volume-control-ui-captures\`:

### 6.1. Volume Mixer HUD (`mixer.png`)
- **Current State**: Compact 416×233px slate window, solid dark master slider, flat row dividers, basic system output row.
- **Liquid Glass Transformation**:
  - Surface shell receives 20px blur, 18px rounded squircle, and top specular rim.
  - Master Signal Rail becomes an organic capsule track with inset specular groove.
  - App session rows transform into `CardSm` floating tiles with subtle hover luminance.
  - Status badges (Mute, Device) adopt tactile pill geometries with accent ring outlines.

### 6.2. Settings Dialog - General & Appearance (`settings_general.png`, `settings_appearance.png`)
- **Current State**: 776×659px dialog, dark background with 94% opacity, solid tabs in left sidebar, flat form dropdowns and sliders.
- **Liquid Glass Transformation**:
  - Surface shell upgraded to 24px squircle with subtle SVG micro-diffusion noise (G6) to prevent gradient banding.
  - Left navigation sidebar adopts translucent inset paneling (`bg-secondary/40`).
  - Appearance section incorporates a real-time **Liquid Glass Material Preview Card**, demonstrating the active combination of `theme`, `material`, `motion`, and `accent`.
  - Contrast ratios for all helper labels rigorously audited against WCAG AA standards (maintaining ≥ 4.8:1).

---

## 7. Cross-Platform Compatibility & Degradation Matrix

| Platform / Host Webview | Compositing Mode | Primary Glass Pipeline | Fallback Degradation Pipeline |
|---|---|---|---|
| **Windows 11 (WebView2)** | Hardware DWM Composited | Native `backdrop-filter: blur(20px)` + DWM Mica Alt window transparency. | Solid high-opacity card (`var(--card)`) when transparency disabled in Windows Settings. |
| **macOS (WKWebView)** | Apple Metal Composited | `-webkit-backdrop-filter: blur(20px)` + AppKit window vibrancy. | Solid high-opacity card (`var(--card)`) under system reduced transparency. |
| **Linux (WebKitGTK 2.44+)** | Mesa OpenGL Composited | `backdrop-filter: blur(20px)` under Wayland / X11 compositors. | Solid 94% opaque shell (`color-mix`) when `WEBKIT_DISABLE_COMPOSITING_MODE=1`. |

---

## 8. Verification & Gate 2 Criteria

- **Zero Test Regressions**: All 43 existing Vitest unit and integration test files continue to pass 100%.
- **Bundle Footprint Gate**: Compiled CSS payload increases by ≤ 2.5 kB gzipped; zero added npm runtime dependencies.
- **Anti-Blur Proof**: Static analysis confirms no child selector in `frontend/src/` declares `backdrop-filter`.
- **Bilingual Parity**: All new design tokens and settings preview controls render seamlessly in English and Vietnamese.
