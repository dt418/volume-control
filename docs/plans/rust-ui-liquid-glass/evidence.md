# D1 — Native Liquid Glass Evidence Ledger

## Method and limits

This evidence supports native-renderer design. It does not prove visual or runtime parity on macOS, Windows, or Linux. Every future platform claim needs target runtime proof.

- Product source and visual captures were read without modifying product code.
- Frontend baseline was built in throwaway Git worktree `.worktrees/liquid-glass-d1` on branch `chore/liquid-glass-d1-evidence`.
- Command: `npm ci && npm run build`, from `.worktrees/liquid-glass-d1/frontend`.
- Result: TypeScript check and Vite production build passed; 2,272 modules transformed; build duration 3.60 s after dependency install.
- Public-source evidence was read from vendor documentation and public repositories on 2026-10-23.

## Q1 — Existing WebView payload baseline

The current bundle is migration scope evidence, not a performance target for the native renderer.

| Output | Raw | Gzip |
|---|---:|---:|
| `styles-EVhlGCpW.css` | 39.29 kB | 7.12 kB |
| `appLabel-Yh1OWHxd.js` | 0.29 kB | 0.19 kB |
| `SignalRail-BTka2wpF.js` | 1.14 kB | 0.53 kB |
| `surface-close-button-DAHAF4I6.js` | 1.94 kB | 1.04 kB |
| `modifierOptions-BJqogT8O.js` | 4.52 kB | 1.69 kB |
| `help-CR-Dd4o6.js` | 7.62 kB | 2.75 kB |
| `overlay-DQE7Gegp.js` | 14.42 kB | 3.87 kB |
| `settings-DhTkyu4E.js` | 37.87 kB | 9.07 kB |
| `mixer-BLBWLe6Q.js` | 162.05 kB | 52.25 kB |
| `styles-B0tDd8AW.js` | 244.74 kB | 76.62 kB |

**Observed.** Build output has one 244.74 kB shared JavaScript asset and a 162.05 kB Mixer chunk. The baseline has browser assets for all four WebView entries: Overlay, Mixer, Settings, and Help.

**Observed.** Frontend dependency graph includes React, Tauri API, Radix, Tailwind, Framer Motion, and Lucide. `frontend/package.json:6-40`.

**Product decision.** Native cutover does not set a byte-for-byte bundle replacement target. It removes browser runtime dependency after full parity and will establish native binary/startup/frame budgets during implementation design.

## Q2 — Rust/UI audit

| Source | Observed | Native WGPU conclusion |
|---|---|---|
| [`rust-ui/ui` README](https://github.com/rust-ui/ui) | Repository calls itself a Dioxus, Tailwind CSS v4, copy-paste component registry. | Do not import it. Its DOM/layout/runtime are outside owned WGPU architecture. |
| [`rust-ui/ui` `Cargo.toml`](https://raw.githubusercontent.com/rust-ui/ui/main/Cargo.toml) | Current repository is package `rust-ui-dioxus`, uses `dioxus = =0.7.9`, `wasm-bindgen`, `web-sys`, Tailwind merge, and web/desktop features. | Browser/DOM-oriented source cannot be a renderer dependency. |
| [`theme_selector.rs`](https://raw.githubusercontent.com/rust-ui/ui/main/app_crates/app_domain/src/themes/components/theme_selector.rs) | `ThemeSelector` holds Dioxus `use_signal` state and `use_memo` values; `use_effect` calls DOM `eval` to set CSS variables; copy uses `navigator.clipboard` through async `eval`. RSX carries Tailwind classes such as `w-full sm:w-96` and `flex flex-col gap-8`; radius buttons declare `role="group"`, `role="radio"`, `aria-checked`, `aria-label`, and `tabindex`. | Translate signal ownership, visual hierarchy, and explicit semantic roles into owned scene state, native focus/input, clipboard, and accessibility contracts. Do not port Dioxus signals/RSX, browser `eval`, DOM APIs, Tailwind classes, or browser ARIA behavior as implementation dependencies. |
| [`rust-ui.com` `llms.txt`](https://rust-ui.com/llms.txt) | Site claims a Leptos 0.8 stack, conflicting with current repository's Dioxus manifest. | Treat site framework claims as non-authoritative until a versioned source maps them. |

**Inference.** Preserve only framework-neutral ideas: component state machines, semantic roles, spacing/radius/typography hierarchy, focus order, and control affordances. Native translation must provide its own retained state, event dispatch, text editing/clipboard, focus traversal, and accessibility tree; DOM mutation, CSS cascade, and browser accessibility semantics do not transfer to WGPU.

## Q3 — Reference application

**Observed.** User designated `ziuus/liquid-glass` as primary reference. `liquid-rust` remains secondary pattern evidence only, not an import or source dependency. REA and static-source audits are recorded in [`rust-native-liquid-glass-research.md`](rust-native-liquid-glass-research.md). The audited `ziuus` repository has inactive blur/material paths and a Vulkan-only renderer; direct reuse is excluded.

## G1–G16 material guidance ledger

| ID | Guidance | Evidence / decision |
|---|---|---|
| G1 | Treat material as functional layer above content, not default content fill. | **Observed.** Apple HIG separates Liquid Glass functional layer from content layer. |
| G2 | Keep interactive navigation/control glass sparse. | **Observed.** Apple advises against glass in content layer and overuse of custom effects. |
| G3 | Do not stack glass on glass. | **Observed.** Apple WWDC25 219 warns against stacking; Windows Acrylic warns against multiple adjacent acrylic panes. |
| G4 | Resolve material from semantic role, not apparent color. | **Observed.** Apple standard-material guidance prefers semantic usage. |
| G5 | Let thickness/opacity serve legibility and hierarchy. | **Observed.** Apple says thicker materials improve fine-detail contrast; thinner ones preserve context. |
| G6 | Let interaction energize material, but bound it. | **Observed.** WWDC25 219 describes interaction flex/light feedback. WWDC26 [269](https://developer.apple.com/videos/play/wwdc2026/269/) says interactive custom Liquid Glass elements respond more fluidly to clicks; WWDC26 [289](https://developer.apple.com/videos/play/wwdc2026/289/) limits bounce to controls, buttons, or containers of interactive controls and says use it sparingly. **Product decision.** Use bounded velocity/ripple/analytic settling only. |
| G7 | Adapt material to owned backdrop content and component size. | **Observed.** Apple describes adaptive regular material, tint, shadows, and dynamic range. **Product decision.** Sample app-owned texture, never desktop capture. |
| G8 | Apply transparency, contrast, and motion accessibility modifiers before decoration. | **Observed.** Apple documents reduced transparency, increased contrast, and reduced motion variants. |
| G9 | Preserve light, dark, and system theme choices. | **Observed.** GNOME HIG supports light/dark/system selection; current `ThemeMode` has equivalent options. |
| G10 | Use Windows Mica as window-foundation reference; reserve Acrylic for transient UI. | **Observed.** Microsoft documents Mica as opaque foundation and Acrylic as transient/light-dismiss material. |
| G11 | Keep opaque fallback under transparency/battery/hardware constraints. | **Observed.** Microsoft documents Acrylic solid fallback for high contrast, disabled transparency, Battery Saver, and low-end hardware. |
| G12 | Avoid accent text as sole readable foreground over transparent material. | **Observed.** Microsoft warns accent-colored text on Acrylic can miss contrast requirements. |
| G13 | Linux baseline is readable theme/token hierarchy, not mandatory desktop blur. | **Observed.** GNOME HIG prioritizes light/dark, high contrast, minimal custom styling; libadwaita exposes paired foreground/background variables. |
| G14 | Accent is an interaction/emphasis role, not a large-surface fill. | **Observed.** libadwaita says accent commonly denotes important, interactive, or active widgets and should not cover large surfaces. |
| G15 | Backend/platform material enhancements stay optional outside shared renderer. | **Inference.** Apple, WinUI, and GTK/libadwaita expose platform-specific systems; no common WGPU compositor material exists. |
| G16 | Preserve current deterministic capability fallback. | **Observed.** Current `resolve_material` forces opaque under high contrast and degrades blurred → translucent → opaque. `crates/volumectl/src/ui/capabilities.rs:53-108`. |

**Observed.** WWDC26 269 records Liquid Glass tint responsiveness, fluid interaction for marked interactive elements, and inactive-window dimming. WWDC26 289 records control-only bounce, keyboard navigation, and hit-test pass-through guidance for overlays. These are Apple-platform design evidence, not portable renderer APIs.

**Applicability limit.** Owned cross-platform WGPU reproduces only semantic visual and interaction principles. It cannot claim SwiftUI/AppKit automatic Liquid Glass behavior, macOS 27 system materials, or Apple accessibility/window APIs. macOS host integration remains optional; Windows and Linux use same owned capability-gated renderer and must prove behavior independently.

Primary sources: [Apple HIG Materials](https://developer.apple.com/design/human-interface-guidelines/materials), [Apple WWDC25 219](https://developer.apple.com/videos/play/wwdc2025/219/), [Apple WWDC26 269](https://developer.apple.com/videos/play/wwdc2026/269/), [Apple WWDC26 289](https://developer.apple.com/videos/play/wwdc2026/289/), [Microsoft System Backdrops](https://learn.microsoft.com/en-us/windows/apps/develop/ui/system-backdrops), [Microsoft Acrylic](https://learn.microsoft.com/en-us/windows/apps/design/style/acrylic), [GNOME UI Styling](https://developer.gnome.org/hig/guidelines/ui-styling.html), and [libadwaita CSS variables](https://gnome.pages.gitlab.gnome.org/libadwaita/doc/main/css-variables.html).

## Current product evidence

| Area | Observed contract | Design consequence |
|---|---|---|
| UI state | `AppState` is confirmed host state and `AppAction` carries intent, but neither contains per-session state/control. Host exposes session list, volume, and mute operations; Settings Apply/Cancel/Reset host arms currently only log. `crates/volumectl/src/ui/model.rs:86-204`; `crates/volumectl/src/host_core.rs:570-575`; `crates/volumectl/src/host_core.rs:832-848`. | Extend domain state/actions and host routing for session controls and Settings drafts before WebView removal. Native scene does not become another authority. |
| Appearance persistence | `config.ini` serializes `[appearance] theme/material/motion/accent`. `crates/volumectl/src/config_ini.rs:151-169`, `280-293`. | Native Settings draft must preserve apply/cancel semantics. |
| Accessibility fallback | High contrast forces opaque; reduced motion downgrades full motion. `capabilities.rs:53-108`. | Material/motion resolver remains pure and testable. |
| Surface geometry | Current WebView constants define mixer, settings, help, overlay dimensions and placement. `src-tauri/src/window_manager.rs:57-129`. | First native layout preserves semantics and design geometry before visual refinement. |
| Wayland overlay input | Cursor-ignore request has no Wayland input-shape effect, so Wayland HUD remains interactive. `src-tauri/src/window_manager.rs:383-390`. | Treat click-through as capability-gated, not as cross-platform overlay invariant. |
| Baseline visuals | Captures show Vietnamese Mixer and Settings, dark hierarchy, rails, sidebar, and settings action bar. | Text shaping, bilingual labels, focus, and form behavior are release blockers. |

## Open evidence needed during implementation

1. WGPU surface/presentation/runtime proof on real Windows, macOS, and Linux hardware, including Windows DX12 → Vulkan fallback and `static-dxc` release packaging.
2. Compatible `glyphon` version resolution with WGPU 30.0.1, then Vietnamese text shaping, fallback, IME, and accessibility adapter benchmark.
3. Frame cost, redraw-idle behavior, blur-cache invalidation, and GPU-memory budget.
4. DWM, `NSVisualEffectView`, and Linux compositor enhancements are post-cutover, out-of-baseline evaluation only.
