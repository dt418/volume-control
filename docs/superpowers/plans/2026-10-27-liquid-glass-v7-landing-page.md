# Liquid Glass v7 Landing Page Sync Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the public GitHub Pages landing page into a premium Liquid Glass v7 product experience aligned with the Rust/egui app, while keeping the QA Demo and Design System as separate specialist surfaces under the same product ecosystem.

**Architecture:** Use a shared-design-system / separate-surface model. Keep `/` as the persuasive product landing, expose the interactive QA harness at a dedicated path such as `/qa/`, and expose the Design System at `/design-system/`. All three consume the same canonical token vocabulary and visual primitives, but each owns its information architecture and interaction model. Keep the landing zero-build/static unless the implementation proves a build step is necessary.

**Tech Stack:** Static HTML/CSS/vanilla JavaScript, GitHub Pages, shared Liquid Glass v7 design tokens/reference assets, existing GitHub Releases API integration.

**Spec:** `docs/superpowers/specs/2026-10-27-liquid-glass-v7-design.md`

## Global Constraints

- The landing page is a separate surface from the native Rust/egui app and the interactive QA harness, but all surfaces belong to one shared VolumeControl design system.
- Prefer one GitHub Pages origin with clear routes: `/` = Landing, `/qa/` = QA Demo, `/design-system/` = Design System. Do not merge their UIs into a single page.
- The public landing must not expose QA-only state injectors such as simulated save failure, simulated download failure, fake audio-unavailable state, or fake platform capability toggles.
- Visual language, naming, materials, motion policy, accessibility behavior, and component styling must remain consistent with Liquid Glass v7.
- Public copy must describe the current Rust/egui product accurately; remove stale Tauri-specific claims where they no longer describe the shipping architecture.
- Preserve the existing install/update/uninstall commands and their security semantics unless a release/install change independently requires edits.
- Keep English/Vietnamese parity.
- Respect `prefers-reduced-motion`, forced colors, keyboard focus, semantic HTML, and screen-reader behavior.
- Do not introduce a JavaScript framework or build system only for this redesign.
- Do not make the landing depend on native app assets that exist only at runtime.
- The demo wallpaper SVG remains a web/reference asset; it is not evidence of native app behavior.
- The QA demo remains the place for exhaustive state simulation; the landing preview should demonstrate the product, not test every failure state.

---

## Current Baseline

The current GitHub Pages source is `docs/index.html`, a single static document with inline CSS and JavaScript. It already provides:

- sticky header and language toggle;
- version/release banner;
- hero and interactive volume-overlay demo;
- install/update/uninstall command tabs;
- feature cards;
- cross-platform capability table;
- GitHub release notes;
- trust/integrity explanation;
- EN/VI strings;
- reduced-motion and forced-colors handling.

The redesign must preserve these useful product/documentation capabilities while replacing the older visual language and stale architecture copy.

---

## File Map

Target structure:

- Modify: `docs/index.html` — semantic page structure, content, metadata, static fallbacks.
- Create: `docs/assets/landing-v7.css` — landing-specific layout/composition using shared v7 tokens.
- Create: `docs/assets/landing-v7.js` — product preview, language, install tabs, release data, and progressive enhancement.
- Create or reuse: `docs/assets/tokens.css` — generated/shared v7 CSS token export; must derive from the canonical token source rather than hand-copy values.
- Reuse/reference: canonical Liquid Glass v7 token JSON and QA/demo assets supplied by the main v7 plan.
- Modify: `docs/og.png` only if the new landing visual direction requires a matching social card.
- Create: `scripts/check-landing-page.mjs` — static structure/content/asset checks without third-party dependencies.
- Create: `docs/testing/landing-page-v7-checklist.md` — manual responsive/accessibility/visual QA matrix.

The implementation may keep inline assets if repository constraints strongly prefer a single file, but the plan assumes extraction because `docs/index.html` is already large enough that visual, content, and behavior changes are difficult to review independently.

---

### Task 1: Lock the current landing behavior before visual changes

**Files:**
- Create: `scripts/check-landing-page.mjs`
- Test target: `docs/index.html`

**Interfaces:**
- Consumes: the current static page.
- Produces: a fast regression gate for IDs, links, install controls, i18n keys, release containers, and local assets.

- [ ] **Step 1: Add a failing/static baseline check**

The script must parse/read `docs/index.html` and assert the presence of:

```text
#main
#install
#features
#platform
#updates
#trust
#lang-btn
#release-banner
#release-list
#snippet-panel
#demo-live
```

It must also assert unique static IDs and that every local `href`/`src` referenced by the page exists.

- [ ] **Step 2: Run the check against the current page**

Run:

```bash
node scripts/check-landing-page.mjs
```

Expected: PASS on the current baseline before structural refactoring.

- [ ] **Step 3: Extend the test for EN/VI parity**

Extract all `data-i18n` keys from markup and verify each key exists in both language dictionaries after JavaScript is extracted.

- [ ] **Step 4: Commit the regression gate**

Use the repository's required verification policy before committing.

---

### Task 2: Establish shared v7 tokens for the landing

**Files:**
- Create/reuse: `docs/assets/tokens.css`
- Modify: `docs/index.html`
- Modify: `docs/assets/landing-v7.css`
- Test: `scripts/check-landing-page.mjs`

**Interfaces:**
- Consumes: canonical `volumecontrol.tokens.json` from the v7 design/reference bundle.
- Produces: CSS custom properties shared by landing and QA/design-system pages.

- [ ] **Step 1: Add a test that fails when required semantic token names are absent**

Check the token export for semantic roles needed by the landing, including background, surface, text, muted text, border, accent, focus, success, warning/error, radii, spacing, and motion durations.

- [ ] **Step 2: Generate or copy the approved token export from the canonical source**

Do not retype token values into `landing-v7.css`.

- [ ] **Step 3: Replace old landing-only primitives with semantic v7 variables**

Keep landing-only layout variables local, but colors/material/radius/motion values must come from the shared token layer.

- [ ] **Step 4: Run the static check and commit**

---

### Task 3: Redesign the page shell, navigation, and hero as a premium v7 product experience

**Files:**
- Modify: `docs/index.html`
- Modify: `docs/assets/landing-v7.css`

**Interfaces:**
- Consumes: shared v7 tokens.
- Produces: the public-facing first impression aligned with the native/QA visual system.

- [ ] **Step 1: Add structural assertions for the v7 hero**

Require one primary H1, one primary install CTA, one release/source CTA, and one product preview region.

- [ ] **Step 2: Apply the v7 shell**

Replace the incumbent visual world rather than merely recoloring it. Use the approved Liquid Glass material language, stronger editorial hierarchy, calibrated depth, purposeful asymmetry, cleaner spacing, and motion that reinforces product behavior. Preserve the skip link, sticky navigation, keyboard focus, and responsive nav behavior.

- [ ] **Step 3: Correct stale architecture copy**

Replace public claims that identify the current product surfaces as Tauri when the shipping direction is Rust/egui. Keep user-facing capability claims grounded in actual supported behavior.

- [ ] **Step 4: Keep the hero product-focused**

The hero should communicate:

```text
VolumeControl
global hotkeys
native/system audio integration
Mixer / HUD / Settings / Help
Windows / macOS / Linux
Rust/egui native-first direction
```

Do not turn the hero into a design-system explanation.

- [ ] **Step 5: Verify responsive layout at narrow phone, tablet, laptop, and wide desktop widths**

- [ ] **Step 6: Commit**

---

### Task 4: Replace the old overlay demo with a product-facing Liquid Glass preview

**Files:**
- Modify: `docs/index.html`
- Modify: `docs/assets/landing-v7.css`
- Modify: `docs/assets/landing-v7.js`

**Interfaces:**
- Consumes: the approved HTML/CSS preview-scene visual reference, shared tokens, and product-facing shortcut behavior.
- Produces: a lightweight interactive preview suitable for public visitors.

- [ ] **Step 1: Add tests for the public preview controls and ARIA contract**

Require an accessible preview label/status and ensure keyboard/click interactions remain available.

- [ ] **Step 2: Port only the product-facing preview layers**

Use the approved scene treatment:

```text
preview scene background
orbs
ribbons
grid
material caption
glass HUD
volume rail / knob / percentage
hover / motion response
```

The web landing may reuse CSS/SVG/web assets because it is itself a web surface.

- [ ] **Step 3: Keep useful shortcut interaction**

Retain the current simulated volume-up/down/mute/reset interaction where it helps visitors understand the product, but restyle it to v7.

- [ ] **Step 4: Do not copy the QA state-injector panel**

The landing must not expose controls for simulated save/update failures, fake blur support, fake audio failure, or fixture blacklist states.

- [ ] **Step 5: Respect reduced motion**

When `prefers-reduced-motion: reduce` is active, remove parallax/tilt and keep state changes understandable without animation.

- [ ] **Step 6: Commit**

---

### Task 5: Synchronize product sections and platform copy with the egui app

**Files:**
- Modify: `docs/index.html`
- Modify: landing EN/VI dictionaries in `docs/assets/landing-v7.js`

**Interfaces:**
- Consumes: current native app/spec behavior.
- Produces: public documentation that does not contradict the Rust/egui implementation.

- [ ] **Step 1: Audit every feature card against current app behavior**

At minimum verify:

```text
Global hotkeys
HUD / overlay
Mixer
System tray
Settings
Help
Live config/reload behavior
EN/VI
Updater
Blacklist behavior
Material/theme support when v7 ships
```

- [ ] **Step 2: Remove stale Tauri-specific wording**

Do not replace it with claims not yet implemented. If v7 is not yet merged, phrase the landing update so deployment occurs with the matching release.

- [ ] **Step 3: Update the platform table**

Keep Windows/macOS/Linux claims conservative and evidence-based. Distinguish native/app-owned visual effects from OS-native blur/backdrops when relevant.

- [ ] **Step 4: Add Liquid Glass/material messaging only at the level actually shipped**

The landing may show the approved visual direction before release only if clearly marked preview/coming release; otherwise publish claims with the corresponding native release.

- [ ] **Step 5: Mirror all changed copy in Vietnamese**

- [ ] **Step 6: Run i18n parity checks and commit**

---

### Task 6: Restyle install, release, platform, and trust sections without weakening their behavior

**Files:**
- Modify: `docs/index.html`
- Modify: `docs/assets/landing-v7.css`
- Modify: `docs/assets/landing-v7.js`

**Interfaces:**
- Consumes: existing install snippets and GitHub Releases behavior.
- Produces: v7-styled informational sections with unchanged security semantics.

- [ ] **Step 1: Preserve installer command generation exactly unless release tooling changed**

The redesign must not silently alter the download → SHA-256 verify → execute flow.

- [ ] **Step 2: Apply the v7 component language to tabs, chips, code boxes, status banners, tables, and release cards**

- [ ] **Step 3: Preserve keyboard tablist behavior and copy-command feedback**

- [ ] **Step 4: Preserve GitHub release-banner/release-list progressive enhancement**

Network failure must leave a usable page and direct Releases links.

- [ ] **Step 5: Keep trust-boundary copy prominent**

Checksums versus signatures, unsigned builds, immutable release assets, and no-pipe-to-shell behavior must remain easy to find.

- [ ] **Step 6: Commit**

---

### Task 7: Integrate Landing, QA Demo, and Design System as separate routes in one product ecosystem

**Files:**
- Modify: `docs/index.html`
- Modify: footer/developer resource area.
- Modify/create documentation links only after their deployed paths are stable.

**Interfaces:**
- Consumes: public landing, QA demo, design-system page.
- Produces: clear artifact roles with no misleading state simulation.

- [ ] **Step 1: Establish and document the route/surface contract**

```text
Landing        = public product/installation/release surface
QA Demo        = interactive state simulation and visual regression harness
Design System  = tokens/components/material reference
```

- [ ] **Step 2: Keep QA/Design links secondary**

If the QA and Design System routes are public, expose them from a clearly labeled Developer/Design resources area, footer, or repository documentation—not from the primary conversion path. The main nav should stay focused on product, install, features, platform support, releases, and source.

- [ ] **Step 3: Reuse a curated product preview on the landing, not the QA control surface**

The landing may reuse the same preview-scene visual primitives and interaction model, but only expose product-facing controls. No simulated save failure, download failure, audio outage, fake blur capability, or fixture blacklist selectors.

- [ ] **Step 4: Ensure no QA-only labels or simulation controls leak into the public product flow**

- [ ] **Step 4: Commit**

---

### Task 8: SEO, social preview, accessibility, and performance pass

**Files:**
- Modify: `docs/index.html`
- Modify: `docs/og.png` if needed.
- Modify: `docs/testing/landing-page-v7-checklist.md`

**Interfaces:**
- Consumes: finished v7 landing.
- Produces: release-ready public GitHub Pages artifact.

- [ ] **Step 1: Update title, meta description, OG/Twitter copy to match the current architecture**

- [ ] **Step 2: Refresh `og.png` if the old visual no longer represents the page/product**

- [ ] **Step 3: Verify accessibility**

Check:

```text
skip link
heading order
focus visibility
tablist keyboard navigation
button names
status live regions
forced colors
prefers-reduced-motion
contrast
zoom at 200%
```

- [ ] **Step 4: Verify responsive behavior**

Record evidence for approximately 360 px, 390 px, 768 px, 1366 px, and a wide desktop viewport.

- [ ] **Step 5: Verify performance**

Avoid oversized visual assets, avoid continuous animation/repaint when offscreen/idle, and keep the static page usable before JavaScript finishes.

- [ ] **Step 6: Run the static regression check and manual checklist**

- [ ] **Step 7: Commit**

---

### Task 9: GitHub Pages release and parity gate

**Files:**
- Modify: release/QA documentation only as needed.

**Interfaces:**
- Consumes: completed landing redesign.
- Produces: deployed landing synchronized with the matching VolumeControl release.

- [ ] **Step 1: Confirm the deployment source for GitHub Pages remains compatible with the `docs/` layout**

Do not introduce a build output directory unless the repository explicitly changes its Pages configuration.

- [ ] **Step 2: Verify the deployed URL after merge**

Check all local assets, release API behavior, language switch, install tabs, preview interaction, and direct release/repository links.

- [ ] **Step 3: Compare the deployed landing against the v7 QA demo/design-system**

Check token/color/material/typography/component parity, not pixel identity. Landing content hierarchy is allowed to differ because its purpose is marketing/documentation.

- [ ] **Step 4: Compare public claims against the released native app**

Do not publish capability claims that are only present in the QA simulation.

- [ ] **Step 5: Record evidence and close the landing sync task**

---

## Dependency and Release Order

Recommended order:

```text
Main v7 spec/tokens/reference bundle
              ↓
Landing Tasks 1-2
              ↓
Landing Tasks 3-7
              ↓
Native v7 reaches release candidate
              ↓
Landing Tasks 5/8 final copy verification
              ↓
Landing Task 9 deploy with matching release
```

The landing can be developed in parallel with native work, but final public capability copy should be frozen only after the native release candidate defines what actually ships.

## Recommended Surface Strategy

Use **independent surfaces with shared foundations**, not one combined page:

```text
GitHub Pages / same origin
├── /                 Landing — Persuade
├── /qa/              QA Demo — Operate/Test
└── /design-system/   Design System — Read/Reference

Shared
├── tokens
├── material names
├── typography/radii/spacing
├── motion terminology
├── icons
└── product facts
```

This keeps the public experience focused while preventing visual drift across QA, documentation, and the native app.

## Shared-vs-specific rule

Share across Native / QA / Landing:

- semantic token names and approved values;
- typography hierarchy;
- radii/spacing conventions;
- material naming;
- motion policy terminology;
- accessibility terminology;
- icons where licensing/format permits;
- product copy facts.

Do not force-share:

- native renderer implementation;
- QA state injectors;
- native Settings draft state;
- landing install/release JavaScript;
- page-specific layout;
- demo-only wallpaper composition.

## Self-review

- The plan preserves the current landing's installer, release, i18n, accessibility, and trust functionality.
- It removes stale Tauri-specific public wording as part of product-parity work.
- It keeps the QA harness separate from the public product experience.
- It synchronizes visual language through shared tokens rather than copying CSS values by hand.
- It does not require a framework or new build system.
- Final deployment is gated on native release claims, preventing the landing from advertising QA-only behavior.
