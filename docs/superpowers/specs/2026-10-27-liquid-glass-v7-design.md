# Liquid Glass v7 Design Specification

**Date:** 2026-10-27  
**Branch:** `feat/liquid-glass-v7-spec-plan`

## Goal

Bring the approved Liquid Glass v7 visual system into the Rust/egui desktop app without regressing current VolumeControl behavior, while preserving a readable opaque fallback and platform-specific native backdrop support.

## Scope

The v7 implementation covers:

- shared theme and design-token model for Light, Dark, System, accent colors, materials, motion, contrast, and transparency preferences;
- materials: Auto, Liquid Glass, Translucent, Acrylic, Blur, Mica, Transparent, and Opaque;
- Mixer, HUD, Settings, Help, blacklist/running-app picker, update notice/card, and appearance preview;
- application-owned backdrop rendering through egui/wgpu;
- native backdrop adapters where supported by the operating system;
- motion policy with Full, Reduced, and Off modes;
- keyboard/focus/accessibility behavior;
- fallback behavior when blur/native effects are unavailable.

The implementation must preserve business behavior already present in the app. Styling must not change blacklist semantics, hotkey semantics, audio routing, update state, or save/cancel transaction behavior.

## Architecture

### Theme and tokens

The design system is driven from a single token source. Rust-facing code resolves primitive tokens into semantic roles and component roles. Surface code consumes semantic/component tokens rather than hard-coded colors.

Requested appearance and effective appearance are separate:

- **requested**: the user's saved preference;
- **effective**: the material/theme actually rendered after OS capability and accessibility policy are applied;
- **fallback reason**: why effective appearance differs from requested appearance.

Reducing transparency or increasing contrast may force Opaque without overwriting the saved requested material.

### Material rendering

Application-owned glass and native OS backdrops are separate layers.

- App glass samples application-owned backdrop content and is responsible for blur/refraction/tint/highlights inside the egui render pipeline.
- Native backdrops are provided by Windows/macOS/Linux compositor APIs where supported.
- Enabling a transparent window alone does not provide blur and does not make the desktop available as an app shader texture.

Material capability resolution must be deterministic and testable without requiring a GPU.

### Motion

Full, Reduced, and Off are policy states, not ad-hoc per-widget flags.

- Full: normal slide/fade/parallax motion.
- Reduced: short fades only; avoid parallax/translation.
- Off: state changes are immediate.
- Idle surfaces must not request continuous repaint.

### Settings transaction model

Settings preview is draft-only. Changes in Appearance affect the preview before Save, but other app surfaces do not adopt them until Save succeeds. Cancel restores the applied configuration. Save failure preserves the draft.

### Blacklist parity

Blacklist affects hotkey handling when a blacklisted process has foreground focus; it does not remove the app from Mixer.

The native flow includes:

- manual process entry with platform normalization;
- duplicate prevention;
- running-app picker with search, checkbox selection, refresh, loading/error/empty states, selected count, and keyboard behavior;
- Apply Recommended, Restore Defaults, Reload Saved, Clear All;
- all edits remain draft-only until Save succeeds.

### Help and updater parity

Help includes all eight existing actions, grouped as in the native code, with current registration status and conflict visibility.

Updater presentation is separate from updater state. Dismissing a notice must not reset an in-progress download, ready-to-install state, or failure phase.

## Canonical visual assets

The final asset bundle should preserve the approved reusable sources rather than raster-only screenshots:

- `volumecontrol-wallpaper.svg` — canonical light wallpaper/background SVG;
- `preview-scenery.html` — canonical preview-scene structure;
- `preview-scenery.css` — canonical orb/ribbon/grid styling and motion reference;
- `volumecontrol.tokens.json` — canonical token source;
- `tokens.css` — generated/reference CSS for the web demo;
- demo `index.html` and `design-system.html`;
- QA, theme/design-system, blacklist-parity, wireframes, screenshots, and reference research documents.

`volumecontrol-wallpaper.svg` belongs to the web demo/reference bundle and is **not** an input required by the native app preview. The native Appearance preview is based on the HTML/CSS `preview-scene` composition: `preview-scenery` (orbs, ribbons, grid), material caption, glass HUD, volume rail/knob, hint, and try-motion control. Rust should port those visual layers and motion semantics to egui painter/material primitives; it does not need to load the demo wallpaper SVG.

## Appearance preview — mandatory v7 feature

The Appearance live preview is a **required v7 feature and part of the critical path**. It must visually represent the currently drafted material, theme, accent, and motion mode and must remain isolated from applied app state until Save succeeds. Shipping v7 without this preview does not satisfy the specification.

The HTML/CSS Appearance preview structure and styling are implementation references for this required feature. In particular, the native preview must faithfully reproduce the `preview-scenery` layers (orbs, ribbons, grid), glass HUD, motion response, labels, and controls. The demo wallpaper SVG is not part of the native preview requirement.

The approved preview includes:

- preview-scene background treatment derived from CSS, not a wallpaper image;
- orb/ribbon/grid scenery from `preview-scenery`;
- material label;
- central volume glass card;
- volume percentage/track/knob;
- hover response where Full motion is enabled;
- optional “try motion” interaction;
- text/status showing the effective material.

### Optional preview-widget abstraction

The **preview itself is mandatory**. Only the decision to package that mandatory preview into a reusable `LiquidGlassPreview` custom widget is optional/deferred.

This distinction is intentional: the team may first implement the required preview directly in the Appearance view to control schedule, but may not omit or downgrade the preview feature itself.

For the first pass, the preview may be composed directly inside the Appearance view using existing painter/material helpers, provided that:

1. the preview faithfully ports the canonical HTML/CSS preview scenery and glass treatment;
2. material resolution is shared with the rest of the app;
3. Full/Reduced/Off motion behavior remains correct;
4. preview-only repaint does not continue while idle;
5. draft/apply isolation is preserved;
6. there is no duplicated material-policy logic.

Create a dedicated custom widget only when at least one of these conditions is met:

- Appearance preview code becomes difficult to understand or exceeds a reasonable focused view implementation;
- the same preview must be reused by two or more native surfaces such as Settings and a design-gallery/test surface;
- duplicated animation/material logic appears;
- direct composition prevents clean unit testing;
- profiling shows a dedicated cached asset/render path is needed.

If activated later, the widget boundary should expose approximately:

- immutable/cached preview assets;
- external preview state;
- resolved preview configuration;
- response events such as volume change or try-motion activation;
- no business logic, file I/O, updater work, or settings persistence inside `Widget::ui`.

If a future widget introduces cached textures for any reason, it must manage them outside per-frame rendering. No wallpaper SVG dependency is required for the native preview.

## Platform behavior

### Windows

Use the existing wgpu/DX12 path. Native Acrylic/Mica support must be capability-checked and must not be required for app usability.

### macOS

Native material integration must use availability checks at runtime. The egui content layer and app-owned preview remain functional without native Liquid Glass.

### Linux

Wayland native blur is optional and compositor-dependent. X11 must retain full usability with app-owned glass or Opaque fallback.

## Performance requirements

- No continuous repaint when all animations/data are idle.
- Reuse pipelines, textures, and bind groups.
- Glass effects must degrade gracefully before core UI responsiveness is affected.
- Performance measurements must separate CPU layout/update cost from GPU material rendering.

## Testing requirements

Use TDD for pure policy/state behavior before renderer integration.

Required test coverage includes:

- material capability resolution and fallback;
- requested vs effective appearance;
- Full/Reduced/Off motion policy;
- Settings preview/save/cancel/save-error transaction;
- blacklist normalization, duplicate handling, recommended/default/reload/clear semantics;
- Help action grouping/status/conflict model;
- updater notice dismissal independent from updater phase;
- no idle repaint from preview/motion controllers;
- deterministic material mapping for all eight materials.

GPU/native rendering requires integration/manual evidence on supported target systems and is not proven by pure unit tests.

## Acceptance

The v7 work is acceptable when:

- current VolumeControl behavior is preserved;
- all primary surfaces use the shared design-token/theme system;
- all eight materials resolve and fallback consistently;
- Accessibility policies do not destroy saved appearance choices;
- blacklist and updater behavior match native semantics;
- Appearance preview matches the approved HTML/CSS preview composition and interaction;
- native material failure leaves a usable Opaque/app-owned fallback;
- the app does not repaint indefinitely while idle;
- CI/repo release gates pass on an environment that provides the required Rust and desktop tooling.

## Non-goals

- Rebuilding the application around a web runtime.
- Making native blur a hard dependency.
- Prematurely extracting the mandatory Appearance preview into a reusable custom widget before there is evidence that the abstraction is useful. The preview feature itself is not a non-goal.
- Replacing business logic merely to match the prototype.
