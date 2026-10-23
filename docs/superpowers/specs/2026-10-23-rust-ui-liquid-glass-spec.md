# Rust/UI and Liquid Glass Formal Specification

> Functional and Non-Functional Specification with Given/When/Then Acceptance Criteria for the VolumeControl Liquid Glass & Rust/UI Modernization.

**Document ID**: D3-SPEC
**Date**: 2026-10-23
**Status**: Formal Specification Approved (Gate 3)
**Product**: VolumeControl
**Parent Plan**: `local://rust-ui-liquid-glass-docs-plan.md`
**Language**: English (Official Specification)

---

## 1. Problem Statement

VolumeControl's current user interface surfaces (Volume Mixer HUD, Settings, Help, and Overlay) provide reliable audio management, but visually rely on flat, opaque slate boxes, standard rectangular cards, and a rudimentary 12px blur backdrop. This creates three critical friction points:

1. **Optical Flatness**: Surfaces lack depth and distinction when placed over complex desktop wallpapers, causing window boundaries to blend indistinctly or appear muddy.
2. **Component Density Mismatch**: Standard form layouts and cards occupy excessive screen area for a peripheral desktop utility, reducing the visible number of concurrent audio sessions in the 400×224px mixer.
3. **Missing Material Hierarchy**: The interface lacks the tactile refinement expected on modern desktop platforms (such as macOS Sequoia Control Center glass, Windows 11 Mica Alt, and high-density Rust/UI desktop patterns).

---

## 2. Solution

Integrate a unified **Liquid Glass & Rust/UI Visual System** across all webview surfaces:
- **Liquid Glass Optics**: Implement a 4-tier elevation architecture (Z0–Z3) delivering a single-pass 20px refraction blur, OKLab luminance tinting, 1px directional specular rim lighting, and subtle ambient occlusion shadowing.
- **Rust/UI Tactile Primitives**: Adopt `rust-ui.com`-inspired compact layout patterns (`CardSm`), organic capsule pill badges, micro-spring active press states, and tactile range slider tracks.
- **Strict Degradation & Accessibility**: Guarantee WCAG AA contrast (≥ 4.5:1), instant reduced-motion collapse (0.001s), anti-blur multiplication prevention (ADR 0002), and graceful solid fallbacks for software-rendered Linux environments.

---

## 3. Goals and Non-Goals

### 3.1. Goals
- **G-01**: Upgrade all 4 webview surfaces (Mixer, Settings, Help, Overlay) to the multi-layer Liquid Glass optical system.
- **G-02**: Integrate Rust/UI compact design tokens (`CardSm`, capsule pills, tactile sliders) into `frontend/src/components/ui/`.
- **G-03**: Preserve all existing backend audio authority, host IPC contracts, and `config.ini` single-source-of-truth invariants.
- **G-04**: Maintain 100% test pass rate across all 43 Vitest suites without regressions.
- **G-05**: Ensure complete bilingual English/Vietnamese fidelity for all newly introduced material and appearance controls.

### 3.2. Non-Goals
- **NG-01**: Rewriting frontend webviews into WebAssembly (Leptos or Dioxus).
- **NG-02**: Introducing heavy 3D/WebGL graphics engines (Three.js, PixiJS) or canvas-based blurs.
- **NG-03**: Altering core audio engine semantics (`audio_windows.rs`, `audio_linux.rs`, `audio_macos.rs`).
- **NG-04**: Forcing unsupported blur filters on legacy Linux compositors or software Mesa renderers.
- **NG-05**: Reverse engineering proprietary third-party desktop binaries.

---

## 4. Shared Domain Glossary

Refer to `CONTEXT.md` for authoritative definitions:
- **Surface Shell (Z1)**: Outer window frame interfacing with the desktop compositor (`.surface-shell`, `.glass-surface`).
- **Content Layer (Z2)**: Foreground container for interactive controls and session cards (`CardSm`).
- **Liquid Glass**: Multi-stage optical system (refraction blur + OKLab tint + specular rim).
- **Specular Rim**: 1px directional highlight gradient simulating ambient refraction.
- **Anti-Blur Multiplication**: Invariant prohibiting nested `backdrop-filter: blur()` containers.
- **MaterialMode**: Translucency policy (`Auto`, `Translucent`, `Opaque`) in `config.ini`.
- **MotionMode**: Animation policy (`Full`, `Reduced`, `Disabled`) in `config.ini`.
- **AccentMode**: Preferred highlight palette (`System`, `Blue`, `Green`, `Purple`, `Orange`).
- **Signal Rail**: Volume visualization track with hardware threshold fill bands.

---

## 5. Extensive User Stories

1. **US-01**: As a desktop user, I want the Volume Mixer HUD to feature a translucent Liquid Glass backdrop, so that it floats naturally above my desktop wallpaper without obscuring my workspace.
2. **US-02**: As an audio enthusiast, I want the master volume Signal Rail to display distinct color-banded threshold highlights (gray, green, blue, orange) within a refined glass groove, so that I immediately recognize high volume levels.
3. **US-03**: As a multi-tasker, I want individual application session rows in the mixer to render as compact `CardSm` floating tiles, so that at least two application rows remain visible simultaneously without scrolling.
4. **US-04**: As a user with visual sensitivity, I want text and icons over glass surfaces to meet WCAG AA contrast standards, so that I can easily read labels regardless of my desktop wallpaper.
5. **US-05**: As a user with vestibular motion sensitivity, I want setting `motion=Reduced` to instantly eliminate spring bounces and scale animations, so that the UI updates without triggering discomfort.
6. **US-06**: As a Windows 11 user, I want VolumeControl surfaces to harmonize with Fluent Mica Alt materials, so that the application feels like a first-party operating system utility.
7. **US-07**: As a macOS user, I want the volume mixer to resemble Apple Control Center capsules, so that it matches native system controls.
8. **US-08**: As a Linux user running GNOME Wayland or X11, I want the interface to automatically degrade to high-contrast solid slate if hardware compositing is unavailable, so that the application remains fully functional and legible.
9. **US-09**: As a Vietnamese speaker, I want all appearance controls, material labels, and tooltips to be accurately localized in Vietnamese, so that I can configure my preferences in my native language.
10. **US-10**: As a power user adjusting appearance in Settings, I want an interactive Liquid Glass preview card, so that I can observe the visual result of changing Theme, Material, Motion, and Accent before closing the window.

---

## 6. Functional Requirements (FR)

### FR-1: Volume Mixer HUD Surface (`mixer.png` baseline)
- **FR-1.1**: The outer container MUST render as a Liquid Glass Surface Shell with a 20px backdrop blur, 18px continuous squircle radius, and 1px specular rim highlight.
- **FR-1.2**: Master output row MUST render as a `CardSm` container (`p-3`) with translucent background tint (`color-mix(in oklab, var(--card) 60%, transparent)`).
- **FR-1.3**: Application session rows MUST render as floating `CardSm` items with subtle hover lighting and no nested `backdrop-filter`.
- **FR-1.4**: Mute and device selection badges MUST render as tactile pill capsules with active scale feedback (`scale(0.97)`).

### FR-2: Settings Dialog Surface (`settings_general.png` & `settings_appearance.png` baseline)
- **FR-2.1**: The window shell MUST render with a 24px squircle radius, 20px blur, and micro-diffusion noise texture to eliminate banding.
- **FR-2.2**: The left navigation sidebar MUST render as an inset translucent panel (`bg-secondary/40`).
- **FR-2.3**: The Appearance section MUST include an interactive **Liquid Glass Preview Card** displaying real-time updates for Theme, Material, Motion, and Accent settings.
- **FR-2.4**: Form controls (inputs, dropdowns, switches) MUST adopt Rust/UI compact styling with crisp inset borders and focus rings matching the active `AccentMode`.

### FR-3: Volume Overlay HUD Surface (`window-overlay`)
- **FR-3.1**: The transient on-screen display (336×88 pt) MUST render as a high-density glass capsule with 20px blur, 1px specular rim, and centered Signal Rail.
- **FR-3.2**: When presented via global volume hotkeys, the overlay MUST enter via a smooth spring transition (`cubic-bezier(0.16, 1, 0.3, 1)`) unless reduced motion is active.

### FR-4: Help Window Surface (`window-help`)
- **FR-4.1**: Keyboard shortcut indicators MUST render as Rust/UI-inspired `Kbd` components with tactile keycap depth and legible contrast.
- **FR-4.2**: Conflict callout banners MUST render as translucent warning cards with solid border accents.

---

## 7. Non-Functional Requirements (NFR)

### NFR-1: Performance & Frame Rate
- **NFR-1.1**: Continuous 60/120 FPS compositor frame rates during slider dragging and surface open/close animations.
- **NFR-1.2**: Cold-start webview display latency MUST remain strictly under 16 milliseconds.

### NFR-2: Bundle Footprint & Dependency Zero-Growth
- **NFR-2.1**: Compiled CSS payload increase MUST NOT exceed 2.5 kB gzipped.
- **NFR-2.2**: Total production JS bundle MUST NOT introduce any new runtime npm packages.

### NFR-3: Accessibility & Contrast
- **NFR-3.1**: All text, control labels, and icons over glass MUST achieve a minimum WCAG AA contrast ratio of 4.5:1 for standard text and 3.0:1 for large text.
- **NFR-3.2**: In `MotionMode::Reduced` or `Disabled`, all animation durations MUST drop to `0.001s !important`.

### NFR-4: Multi-OS Degradation & Architectural Parity
- **NFR-4.1**: Under software rendering or environments lacking `backdrop-filter`, surfaces MUST immediately fall back to high-opacity solid containers (94% `var(--background)`).
- **NFR-4.2**: The Anti-Blur Multiplication constraint (ADR 0002) MUST be preserved: zero nested `backdrop-filter` declarations.

### NFR-5: Internationalization (i18n)
- **NFR-5.1**: All newly added appearance labels, tooltips, and material descriptions MUST have full English (`en`) and Vietnamese (`vi`) translations in `frontend/src/lib/i18n.tsx`.

---

## 8. Component Mapping: Rust/UI to React 19 + Tailwind v4

| Rust/UI Prototype (rust-ui.com) | VolumeControl Target Component | Target File | Key Styling & Behavioral Attributes |
|---|---|---|---|
| `CardSm` | `CardSm` / `Card` | `frontend/src/components/ui/card.tsx` | High-density padding (`p-3`/`p-4`), `rounded-xl`, `bg-card/60`, `border-border/40`, zero nested blur. |
| `Slider` | `AppSlider` / `SignalRail` | `frontend/src/mixer/AppSlider.tsx`, `SignalRail.tsx` | Tactile pill thumb, organic track groove, hardware color thresholds (`#27ae60`, `#0078d4`, `#e05c00`). |
| `Badge` (Pill) | `Badge` / `HotkeyStatusBadge` | `frontend/src/components/ui/badge.tsx` | `rounded-full px-2.5 py-0.5`, subtle border stroke, accent tinting. |
| `Kbd` | `Kbd` | `frontend/src/components/ui/kbd.tsx` | Tactile keycap shading, inset bottom border, monospaced shortcut font. |
| `Animate` | Framer Motion / Spring CSS | `frontend/src/styles.css` | Spring curve `cubic-bezier(0.16, 1, 0.3, 1)`, 200ms, reduced-motion bypass. |

---

## 9. Given / When / Then Acceptance Criteria

### Scenario 1: Liquid Glass Surface Presentation (FR-1, NFR-1)
- **Given** VolumeControl is running on Windows 11 with `material=Translucent` and `theme=Dark` in `config.ini`
- **When** the user clicks the system tray icon to reveal the Volume Mixer HUD
- **Then** the window shell renders with a 20px backdrop blur, 18px squircle radius, and a 1px white specular rim highlight (`inset 0 1px 0 0 rgba(255,255,255,0.22)`)
- **And** the compositor maintains 60+ FPS with no visible stutter or rectangular redraw artifacts.

### Scenario 2: Anti-Blur Multiplication Enforcement (FR-1, NFR-4)
- **Given** the Volume Mixer HUD is visible with active application audio sessions
- **When** the session list mounts multiple session rows
- **Then** each session row renders as a `CardSm` using `color-mix(in oklab, var(--card) 60%, transparent)`
- **And** no session row declares `backdrop-filter: blur()`, preserving single-pass compositor efficiency.

### Scenario 3: Reduced Motion Override (FR-2, NFR-3)
- **Given** the user selects `motion=Reduced` in Settings or has OS `prefers-reduced-motion` enabled
- **When** any surface is opened, resized, or a slider is dragged
- **Then** all spring animations and scale transforms instantly resolve with a duration of `0.001s`
- **And** no bouncy or sliding transitions occur.

### Scenario 4: Software Rendering Degradation (NFR-4)
- **Given** VolumeControl is running on Linux under WebKitGTK with `WEBKIT_DISABLE_COMPOSITING_MODE=1`
- **When** the Settings dialog is displayed
- **Then** the surface shell falls back to an opaque solid dark slate card (`color-mix(in oklab, var(--background) 94%, transparent)`)
- **And** all text, buttons, and form inputs remain completely readable with contrast ratio ≥ 4.8:1.

### Scenario 5: Bilingual Appearance Preview (FR-2, NFR-5)
- **Given** the user configures `language=Vietnamese` in `config.ini`
- **When** navigating to the Appearance section in Settings
- **Then** all material options ("Kính chất lỏng", "Tự động", "Trong mờ", "Đục") and accent colors ("Xanh dương", "Xanh lá", "Tím", "Cam") render accurately in Vietnamese
- **And** the inline Material Preview Card immediately reflects the selected accent and material.

---

## 10. Testing Decisions

1. **Unit & Component Testing (`vitest`)**:
   - Verify `CardSm` renders correct high-density padding and does not emit nested blur utility classes.
   - Verify `SignalRail` and `AppSlider` calculate correct percentage fills and color bands across edge thresholds (0%, 50%, 85%, 100%).
   - Verify `applyAppearance()` correctly toggles `.reduced-motion` class on root elements.
   - Verify `i18n.tsx` resolves all new material and appearance keys in both English and Vietnamese without falling back to raw keys.
2. **Bundle Verification**:
   - Run `npm run build` in `frontend/` to confirm compiled CSS size remains under 42 kB raw (≤ 8 kB gzipped).
3. **Static Anti-Blur Lint Gate**:
   - Execute static grep across `frontend/src/` to verify `backdrop-filter` occurs only within `.glass-surface` in `styles.css`.

---

## 11. Out of Scope

- Audio DSP processing or equalizer frequency shaping.
- Native AppKit / Win32 custom C++ window frame decorations outside the Tauri webview boundary.
- Decompiling third-party proprietary desktop apps.
