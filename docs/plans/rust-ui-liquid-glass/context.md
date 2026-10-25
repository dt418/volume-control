# Rust-Native Liquid Glass Context

## Scope

This is the pre-implementation context for replacing every VolumeControl Tauri/React WebView surface with one owned Rust/WGPU renderer. It defines preserved product behavior, vocabulary, and irreversible decisions. It does not change product code.

The current UI is a behavioral baseline, not an implementation dependency. React, Radix, Tailwind, Framer Motion, Tauri WebViews, and current platform renderers retire only after native parity is proven.

## Baseline evidence

| Surface | Visual evidence | Existing behavior that must remain |
|---|---|---|
| Mixer | `C:\Users\Thanh\AppData\Local\Temp\volume-control-ui-captures\mixer.png` | Compact dark, border-treated always-on-top card; search field; system output and session rails; Vietnamese labels; Escape closes. Current WebView design size is 400×224 logical px. |
| Settings / General | `C:\Users\Thanh\AppData\Local\Temp\volume-control-ui-captures\settings_general.png` | Windowed settings with sidebar navigation, scrollable form, Vietnamese labels, native text fields/selects, and Reset/Cancel/Save action bar. Current design size is 760×620, minimum 620×520 logical px. |
| Settings / Appearance | `C:\Users\Thanh\AppData\Local\Temp\volume-control-ui-captures\settings_appearance.png` | Theme, material, motion, and accent controls modify draft appearance; preview follows draft; Save commits. |
| Overlay | No supplied capture in this set. | Compact non-focusable volume feedback at bottom-right. Current WebView design size is 336×88 logical px; Windows already routes overlay to native HUD. Click-through is capability-gated: macOS and X11 pass pointer input through where supported; Wayland input shapes have no effect, so its HUD remains interactive. |
| Help | No supplied capture in this set. | Separate help surface at current 520×500 logical px size. |

**Observed.** Current Tauri surface constants and placement are Mixer 400×224, Settings 760×620 with 620×520 minimum, Help 520×500, Overlay 336×88. Mixer aligns above overlay at a 16 px gap; overlay uses 20 px right and 40 px bottom margins. `src-tauri/src/window_manager.rs:57-75`, `src-tauri/src/window_manager.rs:81-129`.

## Preserved application boundary

The host remains authoritative. Renderer code receives confirmed `AppState`, emits `AppAction`, never mutates audio/configuration/hotkeys directly, and receives a new state publication after every host-confirmed change. Before native release, host-owned state and intent contracts must extend to cover audio sessions and Settings draft commands. `crates/volumectl/src/ui/renderer.rs:1-80`.

| Existing contract | Native-cutover treatment |
|---|---|
| `SurfaceId::{Overlay, Mixer, Settings, Help, Tray}` | Keep semantic surface identities. Native WGPU windows and tray integration replace WebView labels and URLs. |
| `AppState` | Keep confirmed volume, mute, device, visibility, appearance, language, and status as renderer input. Extend it with host-confirmed session state and any Settings draft/result state needed for native parity. |
| `AppAction` | Keep actions as user intent. Extend it with host-routed session controls and Settings draft/edit/apply/cancel/reset commands; native UI does not add a parallel state authority. |
| `ThemeMode`, `MaterialMode`, `MotionMode`, `AccentMode` | Keep persisted user choices and resolve them into native material/motion tokens. |
| `UiCapabilities` | Keep host capability snapshot. Extend it with system reduced-transparency state. High contrast or reduced transparency always forces opaque; unavailable blur degrades to translucent or opaque. |

**Observed.** `AppState` has no session collection and `AppAction` has no per-session volume or mute intent. The host can enumerate, set volume for, and mute sessions, while current `ApplyConfig`, `CancelConfig`, and `ResetConfig` integration arms only log because WebView owns those flows. `crates/volumectl/src/ui/model.rs:86-204`; `crates/volumectl/src/host_core.rs:570-575`; `crates/volumectl/src/host_core.rs:832-848`.

**Observed.** Overlay cursor ignoring is requested after first show. macOS ignores cursor events, X11 uses an input shape, and Wayland ignores that input shape; Wayland HUD therefore remains interactive. `src-tauri/src/window_manager.rs:383-390`.

**Observed.** `MaterialMode::Auto` resolves from capability, explicit `Translucent` never silently becomes blurred, and high contrast forces opaque. `MotionMode::Full` downgrades to reduced when system preference requires it. Current `UiCapabilities` has no reduced-transparency input, so host platform adapters must add and publish it before native cutover. `crates/volumectl/src/ui/capabilities.rs:13-27`; `crates/volumectl/src/ui/capabilities.rs:53-108`.

## Persisted configuration

`config.ini` is the canonical persistence format. Legacy `config.json` is migration backup only. `crates/volumectl/src/config.rs:1-10`.

| INI section | Native UI responsibility |
|---|---|
| `[general]` | Edit and validate volume steps, overlay duration, language, label style, and autostart through existing host actions/configuration path. |
| `[hotkeys]` | Present bindings; native renderer does not register hotkeys itself. |
| `[appearance]` | Preserve `theme`, `material`, `motion`, and `accent`; draft changes do not persist before Apply/Save. |
| `[feedback]`, `[color_thresholds]`, `[blacklist]` | Preserve current settings behavior and readable error state. |

**Observed.** Parser accepts `[appearance]` keys `theme`, `material`, `motion`, and `accent`; serializer writes them into canonical INI. `crates/volumectl/src/config_ini.rs:151-169`, `crates/volumectl/src/config_ini.rs:280-293`.

## Vocabulary

| Term | Canonical meaning |
|---|---|
| **Host** | Application authority for audio, configuration, hotkeys, tray, persistence, and lifecycle. |
| **Native renderer** | Owned Rust/WGPU UI process integration that consumes confirmed state and emits actions. It is not a webview or generic UI toolkit. |
| **Surface** | One user-visible application window or overlay identified by `SurfaceId`. |
| **Content layer** | App-owned controls, text, icons, rails, focus rings, and status drawn to native render targets. |
| **Glass layer** | Material pass behind content: blurred owned content, tint, border/rim, shadow, and bounded interaction effects. It never requires desktop capture. |
| **Material role** | Semantic reason a glass layer exists: HUD, panel, sidebar, menu, popover, control, accent, or tooltip. |
| **Resolved material** | Actual opaque/translucent/blurred result after preference and system capability resolution. |
| **Fluid effect** | Bounded visual deformation driven by input and analytic settling. It never changes semantic bounds or hit testing. |
| **Appearance draft** | Uncommitted settings state displayed by Settings preview. Only Apply/Save commits through host. |
| **Accessibility mode** | High contrast, reduced transparency, reduced motion, focus, IME, and assistive-technology behavior that overrides decorative material decisions. |

## Approved decisions

1. Use one owned `crates/liquid_glass` renderer crate. It owns scene, material resolution, analytic motion, WGSL passes, cache lifecycle, text integration boundary, and WGPU presentation.
2. Raise workspace MSRV to Rust 1.92 when implementation starts. Pin `wgpu 30.0.1` with WGSL, DX12, Metal, Vulkan, and Windows `static-dxc`; use `glyphon 0.12` behind `TextSystem`. This change is not applied by this documentation work.
3. Use explicit backend policy: DX12 then Vulkan fallback on Windows, Metal on macOS, Vulkan on Linux. Never infer DX12 preference from `Backends::PRIMARY`.
4. Preserve existing `crates/volumectl/src/ui` host contracts as host authority, but extend their missing session and Settings-draft state/action coverage before release. Remove WebView paths only after all surfaces achieve native parity.
5. Use real owned-content multi-pass blur. Desktop wallpaper and other-app capture are outside initial renderer contract.
6. Implement bounded liquid behavior: semantic material hierarchy, tint/saturation, SDF union, pointer stretch/trailing, ripple, analytic springs, and temporary gravity/light bias. Exclude Navier–Stokes, permanent idle motion, mandatory physical refraction, and chromatic dispersion.
7. Keep one renderer. `winit` provides windows and IME events; `glyphon` sits behind owned `TextSystem` for shaping, fallback, glyph atlas, and WGPU text draw; AccessKit provides assistive-technology integration. None is a separate UI framework.
8. Ship WGPU-only material baseline. DWM, `NSVisualEffectView`, and Linux compositor materials are excluded from initial cutover and may be evaluated only after native parity.
9. Use retained semantic scene architecture: layout, hit testing, focus, accessibility, and WGPU primitives share stable semantic IDs; render passes consume flattened primitives.

## Invariants and risks

| Invariant | Risk if violated |
|---|---|
| Hit testing uses semantic control geometry, not deformed visual geometry. | Drag/stretch can make actions land on wrong controls. |
| High contrast and reduced transparency retain opaque, readable fallback. | Glass treatment can hide content or violate accessibility. |
| Host state/action coverage is confirmed and complete for every native surface. | Missing session controls or Settings draft commands force WebView fallback or an unauthorized renderer authority. |
| Every rendered text role supports English and Vietnamese. | Current Vietnamese UI can lose glyphs, IME behavior, or layout. |
| Animation schedules redraw only while active. | Idle overlays consume CPU/GPU and battery. |
| Surface geometry handles DPI and negative monitor origins. | Overlay/mixer placement regresses on multi-monitor systems. |
| Overlay click-through is capability-gated. | Wayland HUD must remain interactive when its protocol ignores input-shape requests. |
| Text, keyboard, focus, and assistive tree ship before WebView removal. | Functional regression despite visual parity. |

## Research references

- `docs/plans/rust-ui-liquid-glass/rust-native-liquid-glass-research.md` — REA evidence, primary `ziuus/liquid-glass` architecture audit, secondary `liquid-rust` pattern evidence, WGPU, and platform findings.
- `crates/volumectl/src/ui/model.rs` — state/action contracts.
- `crates/volumectl/src/ui/theme.rs` — current theme, material, typography, radius, spacing, focus, and threshold token baseline.
- `crates/volumectl/src/ui/capabilities.rs` — capability fallback semantics.
- `src-tauri/src/window_manager.rs` — current dimensions, placement, lifecycle, and overlay constraints.
