# Phase 1: Overlay + Mixer for macOS and Linux

**Date:** 2026-08-10
**Status:** Approved
**Scope:** Implement overlay content rendering (Signal Rail + volume text) and mixer interactive controls (native slider + buttons) on macOS (AppKit) and Linux (GTK4/libadwaita).

## 1. Goal

Bring macOS and Linux overlay/mixer from "empty glass panels" to functional parity with the Windows implementation. The overlay shows the Signal Rail with volume visualization, volume text, and device name. The mixer provides a working volume slider, mute/reset/close buttons, and keyboard navigation.

## 2. Architecture

### 2.1 Shared Layer (platform-neutral Rust)

All shared code lives in `crates/volumectl/src/ui/` and compiles on every platform. It is unit-tested in CI without a display server.

**`Canvas` trait** — drawing primitives that platform backends implement:

```rust
pub trait Canvas {
    fn fill_rect(&mut self, rect: RectF, color: Rgba);
    fn draw_line(&mut self, from: PointF, to: PointF, width: f32, color: Rgba);
    fn draw_circle(&mut self, center: PointF, radius: f32, color: Rgba);
    fn draw_diamond(&mut self, center: PointF, half_size: f32, color: Rgba);
    fn draw_text(&mut self, text: &str, rect: RectF, font_size: f32, color: Rgba, align: TextAlign);
    fn draw_rail(&mut self, rail: &TrackRect, fill: &RailFill, thumb: Option<PointF>, diamond: Option<PointF>, tokens: &ThemeTokens);
}
```

**`RailFill`** — the portion of the rail to fill, computed from volume percent and threshold colors:

```rust
pub struct RailFill {
    pub segments: Vec<(f32, f32, Rgba)>, // (start_frac, end_frac, color)
}
```

**`OverlayContentRenderer`** — owns the overlay drawing logic:

```rust
pub struct OverlayContentRenderer;

impl OverlayContentRenderer {
    pub fn render(canvas: &mut dyn Canvas, state: &AppState, tokens: &ThemeTokens, rail_rect: &TrackRect, marker: &MarkerGeometry) {
        // 1. Draw rail background
        // 2. Compute threshold fill segments from state.volume_percent
        // 3. Draw filled segments
        // 4. Draw thumb circle at volume position (unless muted)
        // 5. Draw diamond marker at mute position (if muted)
        // 6. Draw volume text "72%" right-aligned
        // 7. Draw device name below volume text
    }
}
```

**`MixerContent`** — describes the mixer's control layout (pure geometry, no widgets):

```rust
pub struct MixerLayout {
    pub slider_rect: RectF,
    pub mute_button_rect: RectF,
    pub reset_button_rect: RectF,
    pub close_button_rect: RectF,
    pub title_label: &'static str,
    pub subtitle_label: &'static str,
}
```

### 2.2 Platform Layer (gated)

**macOS (`#[cfg(target_os = "macos")]`):**

- `CoreGraphicsCanvas` — wraps `NSGraphicsContext` current context, implements `Canvas`
  - `fill_rect` → `CGContext FillRect`
  - `draw_line` → `CGContext MoveToPoint` + `AddLineToPoint` + `StrokePath`
  - `draw_circle` → `CGContext AddEllipseInRect` + `FillPath`
  - `draw_diamond` → `CGContext MoveToPoint` + 4x `AddLineToPoint` + `FillPath`
  - `draw_text` → `NSString drawInRect` with `CTFont` attributes
  - `draw_rail` → calls `fill_rect` + `draw_circle` + `draw_diamond` with computed geometry
- `appkit::Panel` gains `set_overlay_content()` — installs an `NSView` subclass that draws via `CoreGraphicsCanvas` on `draw(_:)`
- `appkit::Panel` gains `set_mixer_controls(slider, mute_btn, reset_btn, close_btn)` — creates `NSSlider` + `NSButton` children
- Mixer callbacks: `NSSlider::setTarget` + `setAction` → closure that calls `host.enqueue(SetVolumePercent)`; buttons → `host.enqueue(ToggleMute)` etc.

**Linux (`#[cfg(feature = "gtk-renderer")]`):**

- `CairoCanvas` — wraps `cairo::Context`, implements `Canvas`
  - `fill_rect` → `ctx.rectangle()` + `ctx.fill()`
  - `draw_line` → `ctx.move_to()` + `ctx.line_to()` + `ctx.stroke()`
  - `draw_circle` → `ctx.arc()` + `ctx.fill()`
  - `draw_diamond` → `ctx.move_to()` + 4x `ctx.line_to()` + `ctx.fill()`
  - `draw_text` → `ctx.show_text()` or `pango` layout for multi-line
  - `draw_rail` → calls `fill_rect` + `draw_circle` + `draw_diamond`
- `GtkPanel` gains `set_overlay_content()` — installs a `gtk::DrawingArea` with `set_draw_func` that draws via `CairoCanvas`
- `GtkPanel` gains `set_mixer_controls(...)` — creates `gtk::Scale` + `gtk::Button` children
- Mixer callbacks: `scale.connect_value_changed()` → closure that calls `host.enqueue(SetVolumePercent)`; buttons → `host.enqueue(ToggleMute)` etc.

### 2.3 Rendering Flow

```
Host calls renderer.publish(state, tokens, caps)
  → plan_surfaces()  [existing, pure]
  → for each visible surface:
      panel.apply_plan(plan, caps)  [existing, positioning/material]
      panel.render_content(state, tokens)  [NEW]
        → overlay: CoreGraphicsCanvas/CairoCanvas + OverlayContentRenderer::render()
        → mixer: updates NSSlider/gtk::Scale value, button labels
```

## 3. Overlay Content Details

### 3.1 Signal Rail

- **Geometry:** From shared `rail_geometry(OVERLAY_WIDTH, OVERLAY_HEIGHT)` → `TrackRect` with x, y, width, height
- **Background:** `tokens.rail_background` (full width, 4px height)
- **Fill:** Threshold-colored segments computed from `state.volume_percent` and `ColorThresholds`:
  - Below first threshold: `tokens.rail_low`
  - Between thresholds: `tokens.rail_mid`
  - Above second threshold: `tokens.rail_high`
- **Thumb:** 12px diameter circle at `volume_percent` position along rail, `tokens.accent` color
- **Diamond marker:** 12px diamond at the same position when `state.muted`, `tokens.muted_indicator` color; replaces thumb

### 3.2 Volume Text

- **Position:** Right-aligned in the overlay, above the rail
- **Font:** 28px bold, `tokens.text_primary`
- **Content:** `"72%"` (current volume) or `"Muted"` when muted
- **Device name:** 12px regular, `tokens.text_secondary`, below volume text, left-aligned

### 3.3 Auto-hide

Already handled by the host timer (`config.overlay_duration_ms`, default 1800ms). The host calls `HideSurface(Overlay)` when the timer fires. No changes needed.

## 4. Mixer Content Details

### 4.1 Controls

| Control | Widget (macOS) | Widget (Linux) | Action |
|---------|---------------|----------------|--------|
| Volume slider | `NSSlider` (0–100) | `gtk::Scale` (0–100) | `SetVolumePercent { percent }` |
| Mute/Unmute | `NSButton` (toggle) | `gtk::Button` (toggle label) | `ToggleMute` |
| Reset to 50% | `NSButton` | `gtk::Button` | `ResetVolume` |
| Close mixer | `NSButton` (× glyph) | `gtk::Button` (× glyph) | `HideSurface(Mixer)` |

### 4.2 Labels

- Title: `"VOLUME MIXER"` (14px bold, `tokens.text_primary`)
- Subtitle: `"System output"` (12px, `tokens.text_secondary`)
- Live value: `"72%"` (28px bold, `tokens.text_primary`, right-aligned)
- Mute button label: `"Mute"` / `"Unmute"` toggled by `state.muted`

### 4.3 Keyboard Navigation

- Tab / Shift+Tab: cycle focus among slider, mute, reset, close
- Escape: `HideSurface(Mixer)`
- Enter / Space: activate focused button
- Arrow keys: adjust slider when focused

### 4.4 Layout

Mixer is 400×224 (logical). Layout computed in shared `MixerLayout::compute()`:

```
┌──────────────────────────────────────────┐
│ VOLUME MIXER                        [×]  │
│ System output                            │
│                                  72%     │
│ ═══════════════════════════════════════   │
│ [Mute]  [Reset to 50%]                   │
└──────────────────────────────────────────┘
```

## 5. Testing Strategy

### Shared (CI, no display)

- `OverlayContentRenderer::render()` — mock canvas, assert drawing calls match expected geometry
- `MixerLayout::compute()` — assert control positions at various DPI scales
- `RailFill::compute()` — assert threshold segments for various volume levels

### macOS smoke test (`tests/appkit_smoke.rs`)

- Create `MacosRenderer`, publish overlay state
- Assert panel has content view (NSView subclass)
- Assert slider exists in mixer panel
- Assert a11y labels on controls

### Linux smoke test (`tests/gtk_smoke.rs`)

- Create `LinuxRenderer`, publish overlay state
- Assert panel has DrawingArea child
- Assert Scale widget exists in mixer panel
- Assert button labels under Xvfb

## 6. Files Changed

| File | Change |
|------|--------|
| `crates/volumectl/src/ui/canvas.rs` | NEW — `Canvas` trait, `RailFill`, `OverlayContentRenderer`, `MixerLayout` |
| `crates/volumectl/src/ui/mod.rs` | Add `pub mod canvas;` |
| `crates/volumectl/src/ui/platform/macos/renderer.rs` | Add `render_content()` to `MacosRenderer`, content view for overlay, mixer controls |
| `crates/volumectl/src/ui/platform/linux/renderer.rs` | Add `render_content()` to `LinuxRenderer`, DrawingArea for overlay, mixer controls |
| `crates/volumectl/src/macos_app.rs` | Wire `render_content()` in the publish loop |
| `crates/volumectl/src/linux_app.rs` | Wire `render_content()` in the publish loop |
| `tests/appkit_smoke.rs` | Add overlay/mixer content assertions |
| `tests/gtk_smoke.rs` | Add overlay/mixer content assertions |

## 7. Out of Scope

- Settings window (Phase 2)
- Help window (Phase 2)
- System tray (Phase 3)
- Config reload wiring (Phase 3)
- Animation (motion policy respected — no entry/exit animation)
- Per-app volume controls
- Multi-monitor placement (single monitor only, like Windows overlay)
