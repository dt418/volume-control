# Phase 1: Overlay + Mixer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Overlay content rendering (Signal Rail + volume text) and mixer interactive controls on macOS/Linux.

**Architecture:** Shared `Canvas` trait + `OverlayContentRenderer`. Platform backends: CoreGraphics (macOS), Cairo (Linux). Mixer uses native widgets wired to `HostHandle::enqueue(AppAction)`.

**Tech Stack:** Rust, Core Graphics (macOS), GTK4 + Cairo + libadwaita (Linux), existing `signal_rail` module.

## Global Constraints

- All shared code compiles and unit-tests on every platform (no display)
- Platform code gated: `#[cfg(target_os = "macos")]` / `#[cfg(feature = "gtk-renderer")]`
- Existing `plan_surfaces()` + `SurfacePlan` unchanged
- Overlay: 336x88, Mixer: 400x224, thumb 6px, diamond 6px half-size
- `HostHandle::enqueue` is the only renderer→host communication
- `Canvas` trait in `crates/volumectl/src/ui/canvas.rs`

## File Map

| File | Action | Purpose |
|------|--------|---------|
| `crates/volumectl/src/ui/canvas.rs` | Create | Canvas trait, RectF, PointF, TextAlign, OverlayContentRenderer, MixerLayout |
| `crates/volumectl/src/ui/mod.rs` | Modify | Add `mod canvas; pub use` |
| `crates/volumectl/src/ui/platform/macos/canvas.rs` | Create | CoreGraphics Canvas impl |
| `crates/volumectl/src/ui/platform/macos/mod.rs` | Modify | Add `mod canvas;` |
| `crates/volumectl/src/ui/platform/linux/canvas.rs` | Create | Cairo Canvas impl |
| `crates/volumectl/src/ui/platform/linux/mod.rs` | Modify | Add `mod canvas;` |
| `crates/volumectl/src/ui/platform/macos/renderer.rs` | Modify | Overlay content view + mixer controls |
| `crates/volumectl/src/ui/platform/linux/renderer.rs` | Modify | Overlay DrawingArea + mixer controls |

---

### Task 1: Shared Canvas trait + OverlayContentRenderer + MixerLayout

**Files:** Create `canvas.rs`, modify `ui/mod.rs`

**Steps:**

1. Create `crates/volumectl/src/ui/canvas.rs` with:
   - `Canvas` trait: `fill_rect`, `stroke_rect`, `fill_circle`, `stroke_circle`, `fill_diamond`, `stroke_diamond`, `draw_text`
   - `RectF`, `PointF`, `TextAlign` types (matching Windows primitives)
   - `OverlayContentRenderer::render()` — draws background, title, value, output label, Signal Rail track + fill + marker using shared `signal_rail` geometry
   - `MixerLayout` — static methods returning `RectF` for slider, mute/reset/close buttons, title/subtitle/value labels
   - `MockCanvas` + unit tests: overlay renders background + text + rail, muted shows diamond, mixer layout within bounds

2. Add `mod canvas;` and `pub use canvas::{Canvas, MixerLayout, OverlayContentRenderer, PointF, RectF, TextAlign};` to `ui/mod.rs`

3. Run: `cargo test --workspace --no-default-features -- ui::canvas`
4. Run: `cargo test --workspace --no-default-features` (full suite, no regressions)
5. Commit: `feat(ui): add shared Canvas trait and OverlayContentRenderer`

---

### Task 2: macOS CoreGraphics Canvas

**Files:** Create `platform/macos/canvas.rs`, modify `platform/macos/mod.rs`

**Steps:**

1. Create `canvas.rs` implementing `Canvas` for `CoreGraphicsCanvas`:
   - Wraps CGContext pointer from `NSGraphicsContext::currentContext()`
   - `fill_rect` → `CGContextSetRGBFillColor` + `CGContextFillRect`
   - `fill_circle` → `CGContextFillEllipseInRect`
   - `stroke_diamond` → `CGContextMoveToPoint` + `AddLineToPoint` × 4 + `StrokePath`
   - `draw_text` → `NSString drawInRect:attributes:` (stub initially)

2. Add `mod canvas;` to `macos/mod.rs`
3. Cross-check: `cargo check --target x86_64-apple-darwin -p volumectl --no-default-features`
4. Commit: `feat(ui/macos): add CoreGraphics Canvas implementation`

---

### Task 3: Linux Cairo Canvas

**Files:** Create `platform/linux/canvas.rs`, modify `platform/linux/mod.rs`

**Steps:**

1. Create `canvas.rs` implementing `Canvas` for `CairoCanvas`:
   - Wraps `cairo::Context`
   - `fill_rect` → `ctx.set_source_rgba()` + `ctx.rectangle()` + `ctx.fill()`
   - `fill_circle` → `ctx.arc()` + `ctx.fill()`
   - `stroke_diamond` → `ctx.move_to()` + `line_to()` × 4 + `ctx.stroke()`
   - `draw_text` → toy text API (stub initially)

2. Add `#[cfg(feature = "gtk-renderer")] mod canvas;` to `linux/mod.rs`
3. Cross-check: `cargo check --target x86_64-unknown-linux-gnu -p volumectl --no-default-features --features gtk-renderer`
4. Commit: `feat(ui/linux): add Cairo Canvas implementation`

---

### Task 4: macOS Overlay Content View

**Files:** Modify `platform/macos/renderer.rs` (Panel + MacosRenderer)

**Steps:**

1. Add `overlay_view: Option<Retained<NSView>>` to `appkit::Panel`
2. Add `Panel::set_overlay_content()` — creates NSView, installs as contentView
3. Add `Panel::render_overlay(volume, muted, tokens, green_up_to, blue_up_to)` — gets CGContext, creates CoreGraphicsCanvas, calls `OverlayContentRenderer::render()`
4. In `MacosRenderer::publish()`: after `apply_plan()`, call `set_overlay_content()` + `render_overlay()` for visible overlay
5. Cross-check compilation
6. Commit: `feat(ui/macos): wire overlay content rendering`

---

### Task 5: Linux Overlay DrawingArea

**Files:** Modify `platform/linux/renderer.rs` (GtkPanel + LinuxRenderer)

**Steps:**

1. Add `overlay_draw_area: Option<gtk::DrawingArea>` to `GtkPanel`
2. Add `GtkPanel::set_overlay_content(volume, muted, tokens, green_up_to, blue_up_to)` — creates DrawingArea with `set_draw_func` that creates CairoCanvas + calls `OverlayContentRenderer::render()`
3. In `LinuxRenderer::publish()`: after `apply_plan()`, call `set_overlay_content()` for visible overlay
4. Cross-check compilation
5. Commit: `feat(ui/linux): wire overlay content rendering`

---

### Task 6: macOS Mixer Controls

**Files:** Modify `platform/macos/renderer.rs` (Panel + MacosRenderer)

**Steps:**

1. Add mixer fields to `Panel`: `mixer_slider`, `mixer_mute_btn`, `mixer_reset_btn`, `mixer_close_btn`, `mixer_value_label`
2. Add `Panel::set_mixer_controls(host)` — creates NSSlider (0–100), NSButton × 3 (Mute/Reset/Close), NSTextField (value), positions per `MixerLayout`
3. Add `Panel::update_mixer_value(volume, muted)` — updates slider value, label text, mute button title
4. Wire slider change → `host.enqueue(SetVolumePercent)`, buttons → `host.enqueue(ToggleMute/ResetVolume/HideSurface)`
5. In `MacosRenderer::publish()`: call `set_mixer_controls()` + `update_mixer_value()` for visible mixer
6. Cross-check compilation
7. Commit: `feat(ui/macos): add mixer controls with native widgets`

---

### Task 7: Linux Mixer Controls

**Files:** Modify `platform/linux/renderer.rs` (GtkPanel + LinuxRenderer)

**Steps:**

1. Add mixer fields to `GtkPanel`: `mixer_scale`, `mixer_mute_btn`, `mixer_reset_btn`, `mixer_close_btn`, `mixer_value_label`
2. Add `GtkPanel::set_mixer_controls(host)` — creates gtk::Scale (0–100), gtk::Button × 3, gtk::Label, positions per `MixerLayout`
3. Add `GtkPanel::update_mixer_value(volume, muted)` — updates scale value, label, button labels
4. Wire `scale.connect_value_changed()` → `host.enqueue(SetVolumePercent)`, buttons → `host.enqueue(...)`
5. In `LinuxRenderer::publish()`: call controls for visible mixer
6. Cross-check compilation
7. Commit: `feat(ui/linux): add mixer controls with native widgets`

---

### Task 8: Smoke Tests

**Files:** Modify `tests/appkit_smoke.rs`, `tests/gtk_smoke.rs`

**Steps:**

1. macOS smoke test: publish overlay state, assert panel has content view; publish mixer state, assert slider exists
2. Linux smoke test: publish overlay state, assert DrawingArea child; publish mixer state, assert Scale widget
3. Run smoke tests on CI
4. Commit: `test: add overlay/mixer content smoke tests`

---

## Verification Commands

```bash
# Shared tests (Windows host)
cargo test --workspace --no-default-features

# Format + lint
cargo fmt --all --check
cargo clippy --workspace --all-targets --no-default-features -- -D warnings

# macOS cross-check
cargo check --target x86_64-apple-darwin -p volumectl --no-default-features

# Linux cross-check
cargo check --target x86_64-unknown-linux-gnu -p volumectl --no-default-features --features gtk-renderer
```
