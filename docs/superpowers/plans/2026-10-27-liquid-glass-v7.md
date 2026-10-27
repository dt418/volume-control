# Liquid Glass v7 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply the approved Liquid Glass v7 design system to the Rust/egui VolumeControl app while preserving existing behavior and keeping native effects optional.

**Architecture:** Keep theme/token policy, app-owned material rendering, and platform-native backdrop adapters separate. Migrate surfaces incrementally under TDD, preserving current Settings transactions, blacklist semantics, Help behavior, updater state, and an opaque fallback.

**Tech Stack:** Rust, eframe/egui 0.36.x, egui-wgpu/wgpu, existing VolumeControl core, platform backdrop APIs, canonical SVG/design-token assets.

**Spec:** `docs/superpowers/specs/2026-10-27-liquid-glass-v7-design.md`

## Global Constraints

- The Appearance live preview is mandatory and must ship in v7. Only extraction into a reusable custom widget may be deferred.
- Do not change business behavior to match the web prototype.
- Requested appearance and effective appearance are separate.
- Accessibility/capability fallback must never overwrite the saved requested material.
- App-owned backdrop rendering and native OS backdrop rendering are separate.
- Opaque fallback must keep the app fully usable.
- No continuous repaint while UI/data are idle.
- Blacklist suppresses hotkey handling for a foreground process; it does not hide Mixer sessions.
- Dismissing an update notice must not reset updater state.
- Use the canonical wallpaper/scenery/token assets when the asset bundle is added to the branch.

---

## File Map

The implementation should converge on these responsibilities without unrelated refactors:

- `crates/volumecontrol-egui/src/style.rs`: Rust token/theme resolution or adapter into smaller focused theme modules if required.
- `crates/volumecontrol-egui/src/settings/views.rs`: Settings surface composition, including Appearance and blacklist views.
- `crates/volumecontrol-egui/src/settings/blacklist.rs`: pure blacklist draft operations.
- `crates/volumecontrol-egui/src/settings/update_card.rs`: updater presentation/state mapping.
- `crates/volumecontrol-egui/src/surfaces/help.rs`: Help models and rendering.
- `crates/volumecontrol-egui/src/vibrancy.rs`: native backdrop adapter boundary.
- `crates/liquid_glass/`: app-owned wgpu material renderer.
- `docs/plans/liquid-glass-v7/assets/`: canonical design/reference assets when added.
- `docs/plans/liquid-glass-v7/reference/`: demo/design-system/reference documents when added.

---

### Task 1: Lock existing behavior with policy/state tests

**Files:**
- Modify: existing Rust unit-test modules adjacent to appearance/settings/update/blacklist logic.
- Modify only production files required to expose pure testable policy helpers.

**Interfaces:**
- Consumes: current config, material, motion, blacklist, updater state.
- Produces: regression tests defining current behavior before visual migration.

- [ ] **Step 1: Write failing/characterization tests for appearance policy**

Cover requested/effective theme/material and accessibility fallback without GPU dependencies.

- [ ] **Step 2: Run the focused tests**

Run the crate's focused Rust tests and confirm failures are understood before production changes.

- [ ] **Step 3: Add characterization tests for Settings draft/apply/cancel/save-error behavior**

The tests must prove preview changes stay draft-only until Save succeeds.

- [ ] **Step 4: Add tests for blacklist semantics**

Cover normalization, duplicate rejection, recommended merge, restore defaults, reload saved, clear, and foreground-hotkey semantics.

- [ ] **Step 5: Add tests for updater notice/state separation**

Prove notice dismissal does not reset downloading, ready, installing, or error phases.

- [ ] **Step 6: Run focused tests and commit**

Commit only after the repo's required verification gate can run in the implementation environment.

---

### Task 2: Introduce the shared theme/token resolver

**Files:**
- Modify: `crates/volumecontrol-egui/src/style.rs`
- Create focused theme/token modules only if `style.rs` becomes too large.
- Test: adjacent unit tests.

**Interfaces:**
- Consumes: requested theme, accent, material, motion, accessibility/capability inputs.
- Produces: semantic/component tokens and effective appearance used by all surfaces.

- [ ] **Step 1: Write failing tests for semantic token resolution**
- [ ] **Step 2: Implement minimal token resolver**
- [ ] **Step 3: Write tests for requested vs effective appearance**
- [ ] **Step 4: Implement accessibility/capability fallback without mutating saved preference**
- [ ] **Step 5: Run tests and commit**

---

### Task 3: Add canonical visual assets and asset-loading policy

**Files:**
- Add: canonical `volumecontrol-wallpaper.svg`
- Add: `preview-scenery.html`
- Add: `preview-scenery.css`
- Add: canonical `volumecontrol.tokens.json`
- Add: generated/reference `tokens.css`
- Add: demo `index.html` and `design-system.html`
- Add: QA, THEME-DESIGN-SYSTEM, BLACKLIST-PARITY, screenshots, wireframes, and research references.
- Add or modify: focused asset-loading code only where native implementation needs it.

**Interfaces:**
- Consumes: approved v7 design references.
- Produces: stable source-of-truth assets usable by preview/tests/native rendering.

- [ ] **Step 1: Add the approved sources unchanged where practical**
- [ ] **Step 2: Add integrity/version notes so outdated screenshots cannot become canonical accidentally**
- [ ] **Step 3: If Rust rasterization is needed, write a focused test around asset lookup/cache identity**
- [ ] **Step 4: Load/rasterize static SVG assets once per renderer/scale, never once per frame**
- [ ] **Step 5: Verify the web demo/design-system still resolves its local assets**
- [ ] **Step 6: Commit**

---

### Task 4: Implement deterministic material capability resolution

**Files:**
- Modify/Create: appearance/material policy module.
- Test: pure Rust unit tests.

**Interfaces:**
- Consumes: requested material, OS/runtime capabilities, reduce-transparency, high-contrast.
- Produces: effective material plus fallback reason.

- [ ] **Step 1: Write table-driven failing tests for all eight materials**
- [ ] **Step 2: Implement minimal resolver**
- [ ] **Step 3: Add tests for missing blur/native support**
- [ ] **Step 4: Add tests for accessibility overrides**
- [ ] **Step 5: Verify requested material remains unchanged**
- [ ] **Step 6: Commit**

---

### Task 5: Integrate app-owned Liquid Glass renderer

**Files:**
- Modify: `crates/liquid_glass/`
- Modify: egui/wgpu integration boundary in `crates/volumecontrol-egui/`
- Test: renderer policy/unit tests plus GPU integration smoke tests where available.

**Interfaces:**
- Consumes: app-owned backdrop texture, resolved material parameters, target rect/radius.
- Produces: renderable glass surface without business logic.

- [ ] **Step 1: Write failing tests for material parameter mapping**
- [ ] **Step 2: Add backdrop sampling/blur/refraction primitives**
- [ ] **Step 3: Verify foreground text/icons/focus remain outside the distorted background pass**
- [ ] **Step 4: Add cache/lifetime handling for pipelines/textures**
- [ ] **Step 5: Verify idle state does not trigger continuous repaint**
- [ ] **Step 6: Capture GPU/manual evidence and commit**

---

### Task 6: Integrate native backdrop adapters with safe fallback

**Files:**
- Modify: `crates/volumecontrol-egui/src/vibrancy.rs`
- Create platform-focused modules if needed.

**Interfaces:**
- Consumes: effective material and platform capability.
- Produces: applied native effect or an explicit fallback result.

- [ ] **Step 1: Write pure tests for platform selection/fallback decisions**
- [ ] **Step 2: Keep Windows Acrylic/Mica behavior behind the adapter**
- [ ] **Step 3: Add macOS runtime availability path**
- [ ] **Step 4: Add Linux capability path without making native blur mandatory**
- [ ] **Step 5: Verify failure falls back to app-owned/Opaque UI**
- [ ] **Step 6: Commit after platform evidence is captured**

---

### Task 7: Implement the mandatory Appearance live preview

**Files:**
- Modify: `crates/volumecontrol-egui/src/settings/views.rs`
- Reuse: theme/material/asset helpers from previous tasks.

**Interfaces:**
- Consumes: Settings draft, canonical wallpaper/scenery assets, effective-preview resolver.
- Produces: the required v7 live preview visual and preview-only interaction. This task is release-blocking.

- [ ] **Step 1: Write tests for preview draft isolation and motion policy**
- [ ] **Step 2: Implement the required preview in the Appearance view using existing painter/material helpers, unless a dedicated widget is already justified**
- [ ] **Step 3: Recreate wallpaper/scenery, material label, center volume card, slider state, effective-material status, and try-motion interaction**
- [ ] **Step 4: Verify Full/Reduced/Off behavior**
- [ ] **Step 5: Verify the preview stops repainting when idle**
- [ ] **Step 6: Check the custom-widget activation criteria below**
- [ ] **Step 7: Commit**

#### Optional custom-widget abstraction gate

The live preview may **not** be deferred. Only the `LiquidGlassPreview` abstraction may be deferred. Do not create the abstraction as part of Task 7 unless evidence shows direct composition is inadequate.

Activate the custom-widget follow-up only if one or more are true:

1. the native preview is reused by at least two production/test surfaces;
2. material or animation logic is duplicated;
3. the Appearance view becomes materially harder to understand/test because preview state/rendering dominates it;
4. profiling requires a dedicated cached render/asset path;
5. direct composition blocks unit testing of preview state transitions.

If none apply, close this gate with a short note: **“Custom preview widget not justified; direct composition retained.”**

If one applies, open a separate bounded follow-up task/issue. That follow-up must keep business logic and persistence outside `Widget::ui`, cache SVG-derived textures, and retain the same material/motion resolver. It is intentionally outside the v7 critical path.

---

### Task 8: Migrate Mixer, HUD, Settings, and Help to shared components/tokens

**Files:**
- Modify: current surface/view files only as needed.

**Interfaces:**
- Consumes: semantic/component tokens and material helpers.
- Produces: consistent v7 surfaces without duplicated policy.

- [ ] **Step 1: Migrate Mixer and HUD**
- [ ] **Step 2: Migrate Settings controls/cards/fields/footer**
- [ ] **Step 3: Migrate Help tabs and synchronize selector/content motion**
- [ ] **Step 4: Verify keyboard focus and Reduced/Off motion**
- [ ] **Step 5: Verify narrow-window and DPI behavior**
- [ ] **Step 6: Commit**

---

### Task 9: Finish blacklist and updater parity

**Files:**
- Modify: `settings/views.rs`, `settings/blacklist.rs`, updater presentation/state files.

**Interfaces:**
- Consumes: existing core behavior.
- Produces: v7 visuals with native behavior preserved.

- [ ] **Step 1: Add/fix tests proving manual blacklist input persists across frames**
- [ ] **Step 2: Implement the full running-app picker state/component set**
- [ ] **Step 3: Verify blacklist never hides Mixer sessions**
- [ ] **Step 4: Add/fix tests for updater notice dismissal**
- [ ] **Step 5: Separate notice visibility from updater phase if currently coupled**
- [ ] **Step 6: Verify retry behavior for the corresponding updater failure phase**
- [ ] **Step 7: Commit**

---

### Task 10: Performance, platform QA, and release gate

**Files:**
- Modify: QA/evidence documentation.
- Modify production code only for issues discovered by verification.

**Interfaces:**
- Consumes: completed v7 implementation.
- Produces: release evidence and rollback decision.

- [ ] **Step 1: Run the full Rust/test/lint/release gate required by the repository**
- [ ] **Step 2: Test Light/Dark/System and all eight materials**
- [ ] **Step 3: Test Full/Reduced/Off motion and accessibility fallbacks**
- [ ] **Step 4: Test Mixer, HUD, Settings, Help, blacklist picker, and updater flows**
- [ ] **Step 5: Measure CPU/GPU cost and idle repaint behavior**
- [ ] **Step 6: Test supported Windows/macOS/Linux environments and record unavailable coverage explicitly**
- [ ] **Step 7: Verify Opaque fallback independently**
- [ ] **Step 8: Record evidence and commit**

---

## Critical Path

Tasks 1 → 2 → 3 → 4 → 5/6 → 7/8/9 → 10.

The **Appearance live preview is on the critical path** through Task 7. The optional `LiquidGlassPreview` abstraction is not on the critical path and may be evaluated only after the required preview works.

## Time-control rule

If a visual refinement threatens the delivery schedule:

1. preserve correctness and Opaque fallback;
2. preserve token/material consistency;
3. preserve canonical wallpaper/scenery usage;
4. reduce optical complexity before adding abstraction;
5. keep the required preview functional and visually faithful to the canonical SVG/scenery even if optical complexity must be reduced;
6. defer only the custom-widget abstraction or non-essential polish;
7. record deferred polish instead of extending the main implementation task.

This rule specifically exists to prevent optional abstraction/polish from consuming disproportionate implementation time while keeping the preview itself mandatory.

## Self-review

- Spec coverage: theme/tokens, materials, renderer, native backdrop, preview, surfaces, blacklist, Help/updater, accessibility, performance, and release evidence all have tasks.
- Preview scope: mandatory in Task 7 and release-blocking.
- Custom-widget scope: optional and guarded by measurable activation criteria.
- v7 acceptance requires the preview, but does not require the reusable widget abstraction.
