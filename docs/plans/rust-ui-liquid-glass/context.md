# D0: System Context & Baseline Verification

**Document ID**: D0-CONTEXT
**Date**: 2026-10-23
**Status**: Approved Baseline
**Scope**: VolumeControl UI Visual System & Architectural Foundation
**Language**: English (Official Specification)

---

## 1. Executive Summary

VolumeControl is a cross-platform, native-first desktop utility providing global hotkeys, system tray integration, high-performance volume HUD overlays, and multi-surface management for Windows, macOS, and Linux. This document establishes the empirical baseline for upgrading the current visual language—combining inspirations from [Rust/UI](https://rust-ui.com) with an Apple-style **Liquid Glass** visual system (as introduced in modern Apple HIG, WWDC25 Session 219, and WWDC26).

All statements herein are verified against read-only inspections of the codebase and real composited screen captures of the running application.

---

## 2. Visual Baseline from Ground-Truth UI Captures

Visual captures were captured from live application executions on Windows 11 and preserved at `C:\Users\Thanh\AppData\Local\Temp\volume-control-ui-captures\`.

| Capture Asset | Physical Dimensions | Logical Target | Surface Type | Key Observed Visual Characteristics |
|---|---|---|---|---|
| `mixer.png` | 416 × 233 px | 400 × 224 pt | Transient HUD / Mixer | Dark slate card (`rgb(21, 21, 21)` to `rgb(9, 9, 11)`), single compact master volume output row (`[data-testid="system-output-row"]`), thin vertical/horizontal dividers, muted status badges, SignalRail color band preview (`#0078d4` blue / `#27ae60` green / `#888888` gray), 12px blur backdrop emulation. |
| `settings_general.png` | 776 × 659 px | 760 × 620 pt | Modal Dialog Window | Split-pane navigation (left sidebar: General, Hotkeys, Appearance, Feedback, Blacklist, Storage; right pane: volume steps, large step, duration, autostart toggle). Surface shell uses `color-mix(in oklab, var(--background) 94%, transparent)`, dark border lines (`hsl(240 3.7% 15.9%)`), crisp typography (`Segoe UI` / system font stack). |
| `settings_appearance.png` | 776 × 659 px | 760 × 620 pt | Modal Dialog Window | Theme selection dropdown (`ThemeMode::Dark` / `Light` / `System`), Material selection dropdown (`MaterialMode::Auto` / `Translucent` / `Opaque`), Motion mode dropdown (`MotionMode::Full` / `Reduced` / `Disabled`), and Accent picker (`AccentMode::System` / `Blue` / `Green` / `Purple` / `Orange`). Live inline preview card showing simulated surface material. |

### Visual Baseline Analysis
1. **Current Glass Quality**: The current implementation in `frontend/src/styles.css` uses a flat dual-declaration `@supports` rule with `backdrop-filter: blur(12px)` and `color-mix(in oklab, var(--background) 88%, transparent)`. It lacks directional specular highlights, multi-stop refraction rims, refractive inner borders, and dynamic noise diffusion typical of Apple's Liquid Glass.
2. **Current Component Weight**: Form controls and sliders rely on standard Shadcn-like box outlines (`border-input`), solid background fills, and flat rectangular or rounded-md shapes. Rust/UI components offer tighter semantic density, tactile spring states, and pill-shaped or organic capsule geometries.
3. **Contrast & Legibility**: In dark mode (`hsl(240 10% 3.9%)`), white and light-gray text maintains high readability, but layered surfaces blend together without luminous elevation gradients or ambient edge lighting.

---

## 3. Resolution of Repository Contradictions

Several documentation-versus-code discrepancies exist across past design iterations. This specification resolves them definitively in code's favor:

### 3.1. Single Source of Truth for Configuration: `config.ini`
- **Contradiction**: Legacy notes occasionally referenced a JSON-based configuration schema (`config.json`).
- **Ground Truth**: The active, authoritative configuration file is `config.ini` located in the platform-specific configuration directory (`%APPDATA%\VolumeControl\config.ini` on Windows, `~/.config/volume-control/config.ini` on Linux, `~/Library/Application Support/VolumeControl/config.ini` on macOS).
- **Code Anchor**: `crates/volumectl/src/config_ini.rs` owns parsing, serialization, and migration. The `[appearance]` section schema is explicitly:
  ```ini
  [appearance]
  theme=Dark        # System | Light | Dark
  material=Auto     # Auto | Translucent | Opaque
  motion=Full       # Full | Reduced | Disabled
  accent=System     # System | Blue | Green | Purple | Orange
  ```

### 3.2. Rendering Architecture: Dual Native Engine and Tauri Webview
- **Contradiction**: Prior documentation implied that all UI is rendered exclusively via Tauri Webviews.
- **Ground Truth**: The application maintains a **hybrid dual architecture**:
  1. **Native platform renderers**: Located under `crates/volumectl/src/ui/platform/` (`windows/d2d.rs`, `macos/renderer.rs`, `linux/renderer.rs`), providing instant-render Direct2D, AppKit CoreGraphics, and Cairo/GTK fallbacks.
  2. **Tauri Webview surfaces**: Managed by `src-tauri/src/window_manager.rs`, hosting React 19 + Tailwind CSS v4 interfaces for rich interactive surfaces (Mixer, Settings, Help, Overlay).
- **Rule**: Any visual system modernization must provide the primary implementation in the webview surface layer (`frontend/src/`) while defining clean architectural degradation contracts for native platform fallback canvases.

### 3.3. Wayland Click-Through HUD Degradation
- **Contradiction**: Marketing docs claimed universal click-through overlay support across all Linux desktop environments.
- **Ground Truth**: Under Linux Wayland compositors (GNOME Shell, KWin Wayland), protocol constraints (such as `wl_pointer` and `xdg_shell` input region restrictions) prevent seamless transparent pass-through click events without losing mouse focus or violating sandbox rules.
- **Code Behavior**: As established in `crates/volumectl/src/linux_app.rs` and `crates/volumectl/src/ui/capabilities.rs`, Wayland environments gracefully degrade HUD overlays to standard transient surface layers with defined grab and auto-hide policies.

---

## 4. Current Architectural & Code Anchors

### 4.1. Frontend Design Tokens (`frontend/src/styles.css`)
- **Framework**: Tailwind CSS v4 (`@import "tailwindcss";`, `@theme inline`).
- **Color Variables**: HSL channels defined for `:root` and `.dark` (`--background`, `--foreground`, `--card`, `--popover`, `--primary`, `--secondary`, `--muted`, `--accent`, `--border`, `--ring`).
- **Surface Utilities**:
  - `.surface-shell`: 100dvh root container with `color-mix(in oklab, var(--background) 94%, transparent)`.
  - `.glass-surface`: Blur 12px fallback with 88% opaque blend.
  - `.reduced-motion`: Forces `animation-duration: 0.001s !important;` and `transition-duration: 0.001s !important;`.
- **Signal Rail Bands**: Hardware-aligned volume level color indicators:
  - `.band-gray` (`#888888` - Muted / 0%)
  - `.band-green` (`#27ae60` - Low/Normal)
  - `.band-blue` (`#0078d4` - Medium/Elevated)
  - `.band-orange` (`#e05c00` - High/Capped)

### 4.2. Surface Lifecycle & Geometry (`src-tauri/src/window_manager.rs`)
| Surface ID | Label | Window Title | Logical Dimensions | Layout Placement Policy |
|---|---|---|---|---|
| `SurfaceId::Mixer` | `window-mixer` | `Volume Mixer` | 400 × 224 pt | Anchored bottom-right above overlay; 16px gap. |
| `SurfaceId::Settings` | `window-settings` | `Volume Control Settings` | 760 × 620 pt (min 620 × 520) | Screen centered. |
| `SurfaceId::Help` | `window-help` | `Volume Control Help` | 520 × 500 pt | Bottom-right with 24px/48px margin. |
| `SurfaceId::Overlay` | `window-overlay` | `Volume Control Overlay` | 336 × 88 pt | Bottom-right with 20px/40px margin; transient. |

- **Lifecycle Rules**: Auto-hide on blur for transient surfaces; `closes_before_opening` mutex to prevent multi-instance race conditions; transparent window flag enabled across all webview builder invocations.

### 4.3. Internationalization Engine (`frontend/src/lib/i18n.tsx`)
- Supported Locales: English (`en`) and Vietnamese (`vi`).
- Stored in `config.ini` under `[general] language=English` or `language=Vietnamese`.
- Synchronized through Rust `AppState.language` across IPC bridge to React context. All new design tokens, badges, labels, and tooltips must retain complete parity in both languages.

---

## 5. Visual System Goals & Constraints

1. **Apple Liquid Glass Fidelity**:
   - Deliver multi-stage optical rendering: background refraction blur (16–28px), dynamic tint saturation boost (+15–20%), top-edge specular highlight (1px gradient `rgba(255,255,255,0.2)` to `transparent`), and subtle bottom drop shadow (`0 16px 36px rgba(0,0,0,0.3)`).
2. **Rust/UI Component Inspiration**:
   - Adopt compact, tactile, high-density component semantics: capsule pills, reactive pressed states, micro-spring transitions, and crisp inset borders.
3. **Cross-Platform Host Consistency**:
   - Windows 11: Seamless harmony with Mica Alt and Acrylic desktop backdrop compositing.
   - macOS: Native feel resembling Sonoma/Sequoia Control Center materials.
   - Linux: GNOME Shell / KDE Plasma dark modern aesthetics with solid graceful fallback.
4. **Performance & Bundle Bounds**:
   - Zero added heavy JavaScript runtime dependencies (no Three.js, no framer-motion; keep bundle lean with pure CSS and lightweight React hooks).
   - Solid 60/120 FPS continuous compositor performance during slider dragging and surface open/close animations.
