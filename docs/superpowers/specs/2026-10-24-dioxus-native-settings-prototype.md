# Dioxus Native Settings Prototype Specification

**Status:** Draft for review
**Parent production path:** `crates/liquid_glass` + `crates/volumecontrol-host`

## Problem

Manual retained-scene layout makes compact native surfaces fast but costly to iterate. Evaluate Dioxus Native with Blitz only as an isolated Settings UI prototype. It must not change or replace the production Winit/WGPU Liquid Glass host.

## Goals

- Create one separately runnable Settings-only prototype using Dioxus Native/Blitz.
- Reproduce native Settings information architecture: General and Appearance tabs, draft-only controls, and explicit Save/Cancel actions.
- Keep configuration data local to the prototype. The prototype MUST NOT write `config.ini`, access audio endpoints, register hotkeys, or modify `crates/volumecontrol-host`.
- Expose accessible tab, button, and selected-value semantics. Verify them through Windows UI Automation using computer use.
- Measure startup, RSS, idle CPU, slider-drag CPU, and 1% low frame time on this Windows workstation against the production native host's Settings surface.
- Capture visual evidence of whether Blitz can produce an equivalent Liquid Glass appearance.

## Non-Goals

- `dioxus-desktop`, Wry, WebView2, HTML in a browser engine, or Tauri integration.
- Production cutover, compatibility layer, or migration of Mixer, Help, Overlay, Tray, host state, configuration persistence, or audio actions.
- Claiming shader parity from CSS backdrop blur. Native Liquid Glass parity requires the existing WGPU blur pyramid and SDF glass pass.

## Architecture

Create `crates/dioxus-settings-prototype` as an independent executable. It depends on an explicitly pinned Dioxus Native/Blitz release compatible with Rust 1.92. Its app owns only a `SettingsDraft` signal and renders Settings component state declaratively.

The existing `crates/volumecontrol-host` remains unmodified except for benchmark-safe debug launch support. It remains baseline renderer and authority for production behavior.

```text
Dioxus signals → RSX Settings component → Dioxus Native / Blitz → WGPU/Vello
                                      ↘ benchmark event timestamps

production host: HostCore → LiquidGlass scene → WGPU blur pyramid + WGSL → AccessKit
```

## Prototype Interaction Contract

- General tab: 1%, 2%, and 5% volume-step buttons. Selecting a value updates only local draft state.
- Appearance tab: System, Light, Dark and Auto Glass, Translucent, Opaque options. Selection updates only local draft state and preview.
- Cancel restores default draft state.
- Save only reports an in-memory saved state; it MUST NOT persist.
- All focusable controls support mouse activation and keyboard focus activation.
- Vietnamese labels must render correctly, including diacritics.

## Liquid Glass Evaluation

The prototype may use Blitz-supported styling to approximate tinted translucent panels. It passes visual comparison only if capture evidence shows:

1. Correct rounded geometry, text shaping, selected states, and readable contrast.
2. No clipped glyphs, stale content, or focus-state artifacts during tab changes.
3. An exposed accessible semantic tree equivalent to the production Settings tab/button/value tree.

It fails shader parity unless it can use the existing WGPU blur pyramid and SDF glass pass. CSS or native-backdrop blur alone is recorded as an approximation, not parity.

## Benchmark Contract

Run every measurement after a cold process start on the same machine, with no intentional configuration changes. Record raw data and method; do not invent normalizations.

| Metric | Baseline | Prototype | Method |
|---|---|---|---|
| Startup | `volumecontrol-host` until Settings first visible | prototype until Settings first visible | process launch and visible-window timestamps |
| RSS | idle after 10 seconds | idle after 10 seconds | Windows process working set |
| Idle CPU | 30 seconds without input | 30 seconds without input | process CPU-time delta |
| Drag CPU | 60 seconds controlled repeated slider drag | same workload | process CPU-time delta |
| 1% low frame time | repeated settings selection / slider changes | same workload | renderer frame timing samples |
| Accessibility | Settings AX tree | equivalent AX tree | computer-use AX capture |
| Glass fidelity | WGPU native evidence | visual capture | side-by-side screenshots |

## Decision Rule

Keep production on `liquid_glass` unless prototype proves all semantic and visual requirements and its measured resource cost is no worse than an agreed performance budget. A mismatch in native shader parity blocks cutover regardless of declarative UI ergonomics.

## Verification

- Unit tests cover draft selection, Cancel, Save-without-persistence, and keyboard activation.
- Computer-use captures General and Appearance surfaces, selected state, focus traversal, Vietnamese text, and AX tree.
- Benchmark output is stored as raw measurements with command lines and timestamps.
- Production `cargo test --workspace --no-default-features`, Clippy, formatting, and Windows format-lint still pass.
