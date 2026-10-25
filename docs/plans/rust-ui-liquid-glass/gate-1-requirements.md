# Gate 1 — Native Liquid Glass Requirements

## Purpose

Approve requirements before native renderer design. This document binds later specification, tickets, and implementation plans. It supersedes the earlier webview-oriented alternatives in the documentation workflow plan where they conflict with the approved full native Rust/WGPU cutover.

## Inputs

- Baseline and glossary: [`context.md`](context.md).
- Renderer/platform/reference audit: [`rust-native-liquid-glass-research.md`](rust-native-liquid-glass-research.md).
- Current visual proof: `mixer.png`, `settings_general.png`, and `settings_appearance.png` in `C:\Users\Thanh\AppData\Local\Temp\volume-control-ui-captures\`.

## Functional requirements

| ID | Requirement | Acceptance boundary |
|---|---|---|
| R1 | Native renderer consumes confirmed host `AppState` and emits host-routed `AppAction`; extend both contracts before release for session state/actions and Settings draft lifecycle commands. Renderer never mutates host-owned audio, configuration, hotkeys, tray, or persistence. | Every migrated surface dispatches complete intents through host boundary. Confirmed host state repaints surface; renderer never becomes another authority. |
| R2 | Replace Overlay, Mixer, Settings, Help, and tray-related surface interaction with one owned native Rust/WGPU UI architecture. | No release WebView is needed to show or operate any named surface after cutover. |
| R3 | Preserve canonical `config.ini`, including `[appearance]` values `theme`, `material`, `motion`, and `accent`; wire host-owned Settings draft/edit/Apply/Cancel/Reset commands for native UI. | Only host-confirmed Apply/Save changes persisted configuration. Cancel restores confirmed appearance. |
| R4 | Preserve current surface geometry and semantic hierarchy: compact mixer, bottom-right overlay, centered resizable Settings, separate Help, visible rails, volume state, mute/reset controls, search, and keyboard-close behavior. Overlay click-through is capability-gated; Wayland HUD remains interactive when its input-shape request has no effect. | Native screenshots and interaction smoke cover supplied visual baseline states. Target proof shows pointer pass-through on supported macOS/X11 paths and usable interactive Wayland HUD. |
| R5 | Render all user-facing copy in English and Vietnamese. Support Unicode shaping/fallback, IME for editable fields, logical focus traversal, keyboard actions, pointer capture, and clipboard semantics. | Vietnamese preedit/commit and bilingual settings/mixer/help smoke pass on each target. |
| R6 | Publish one accessibility tree with correct names, roles, states, ranges, and actions. | Screen-reader action changes host-confirmed and visual state consistently. |
| R7 | Resolve theme/material/motion/accent from existing preference and capability contracts. High contrast or reduced transparency forces opaque; unavailable blur degrades to readable translucent or opaque; reduced motion disables fluid deformation. | Capability matrix is covered by deterministic tests and runtime smoke on each target. |
| R8 | Render glass from app-owned native content: background/content composition, real downsampled multi-pass blur, tint/saturation, edge/rim, shadows, and role-based material hierarchy. | No desktop or other-application capture permission is required for baseline appearance. |
| R9 | Provide bounded fluid feedback: SDF union for related material groups, velocity stretch/trail, ripple, analytic settling, and temporary gravity/light bias. | Effects are bounded, interruptible, disabled by reduced motion, and do not change semantic hit bounds. |
| R10 | Run on Windows, macOS, and Linux with explicit target backend policy: DX12 then Vulkan fallback on Windows, Metal on macOS, Vulkan on Linux. | Target integration tests create/present a WGPU surface through every selected backend path and exercise opaque fallback. |

## Non-functional requirements

| ID | Requirement | Measure or proof |
|---|---|---|
| N1 | One renderer and one scene/material model. No React, Tauri WebView, Dioxus, Leptos, egui, Iced, or second UI/render engine enters release architecture. | Dependency and architecture review. |
| N2 | Rust MSRV becomes 1.92 and renderer pins released `wgpu 30.0.1` plus `glyphon 0.12`, with WGSL, DX12, Metal, Vulkan, and Windows `static-dxc`. | Workspace metadata, locked dependency resolution, and self-contained Windows release artifact at implementation time. |
| N3 | Native state remains allocation-stable in frame loop. Cache blur by content revision; request redraw only while input or animation is active. | Frame profiling and idle redraw observation during implementation validation. |
| N4 | Acquire WGPU surfaces with version-pinned `wgpu 30.0.1` `CurrentSurfaceTexture`: render, submit, and present `Success`; present `Suboptimal` then schedule reconfiguration; skip `Timeout` and `Occluded`; reconfigure after `Outdated`; recreate lost surface then configure after `Lost`; transition `Validation` to controlled surface-error state until valid configuration exists. Rebuild adapter, device, queue, surface, and GPU resources after device loss. Device/process out-of-memory fails safely without an acquisition retry loop. | Target runtime smoke and injectable error-path tests cover every acquisition status and controlled device-failure path. |
| N5 | Material accessibility has priority over decoration. High contrast, transparency reduction, motion reduction, VM/RDP, and absent compositor each retain readable UI. | Cross-platform capability test matrix. |
| N6 | No compatibility alias or release shim survives final cutover. Transitional scaffolding may exist only in development branches and must be deleted before release switch. | Final dependency, surface, and source-reference audit. |

## Design decisions fixed at Gate 1

| Decision | Chosen direction | Reason |
|---|---|---|
| UI ownership | Owned `crates/liquid_glass` plus existing VolumeControl domain contracts. | Preserves host authority while replacing browser runtime. |
| GPU API | `wgpu 30.0.1`, WGSL, Rust 1.92, `glyphon 0.12`, Windows `static-dxc`. | Matches released WGPU text stack and removes `dxcompiler.dll` deployment dependency. |
| Native text | Owned `TextSystem` API wraps `glyphon 0.12`. | Reuses WGPU glyph shaping/raster/atlas infrastructure without adding a widget framework or exposing it to scene code. |
| Scene architecture | Retained semantic scene flattened into render primitives and material groups. | Stable semantic IDs keep layout, hit testing, focus, AccessKit, caching, and WGPU draw order aligned. |
| Desktop backend | Windows tries DX12, then explicit Vulkan fallback; macOS uses Metal; Linux uses Vulkan. | Keeps preference deterministic, extends Windows coverage, and avoids undefined `PRIMARY` ordering. |
| Glass backdrop | App-owned content texture only. | Avoids privacy, permission, and compositor-capture dependency. |
| System materials | Excluded from initial cutover; evaluate only after WGPU-native parity. | DWM, `NSVisualEffectView`, and Linux compositor effects lack common parity and would split the baseline. |
| Fluidity | Bounded visual feedback, not physical fluid simulation. | Protects hit testing, accessibility, idle cost, and maintainability. |
| Migration | Native parity first, then one clean release cutover that deletes WebView UI paths. | Avoids permanent duplicated UI ownership. |
| Reference roles | `ziuus/liquid-glass` is primary conceptual architecture reference; `liquid-rust` is secondary pattern evidence only. Neither is an import or source dependency. | Retains useful audited patterns without coupling release renderer to third-party code. |

## Deferred decisions

These require implementation evidence, not a product-direction reversal:
1. Package resolution, Vietnamese shaping, fallback, IME, and glyph-atlas benchmark for `glyphon 0.12` with `wgpu 30.0.1` under Rust 1.92.
2. Native settings layout details that preserve current controls while making material hierarchy coherent.
3. Host `UiCapabilities` extension and per-target system reduced-transparency detection.

## Gate 1 approval

Approval means R1–R10 and N1–N6 are binding for design. It authorizes D1 evidence collection and native design work. It does **not** authorize product-code changes, dependency changes, or removal of current UI paths.

**Approved 2026-10-23.** User approved Gate 1 in this session.
