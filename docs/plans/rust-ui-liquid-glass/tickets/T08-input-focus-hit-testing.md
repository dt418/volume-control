# Ticket T08 — Input, Focus Traversal & Semantic Hit Testing

**Phase:** Phase 2 — Core Renderer
**Status:** Ready for Implementation
**Traceability:** R4, R5, D1, D2, Spec §4.1, §4.2
**Depends On:** T04
**Depended Upon By:** T09, T10, T11, T12, T13, T14

---

## 1. Description & Scope

Implement user input handling, keyboard focus traversal, pointer capture, and hit testing in `crates/liquid_glass/src/input/`. Enforce the critical invariant that hit testing always evaluates against static semantic control bounds, never visually stretched or deformed fluid geometry. Implement Vietnamese IME composition rectangle tracking.

---

## 2. Planned Changes

1. **`crates/liquid_glass/src/input/hit_test.rs`**:
   - `hit_test(scene: &Scene, point: Point) -> Option<SemanticId>`.
   - Traverses z-ordered nodes in reverse; ignores visual fluid displacement offsets.
2. **`crates/liquid_glass/src/input/focus.rs`**:
   - `FocusManager`: Logical Tab and Shift+Tab traversal across focusable nodes.
   - Arrow key navigation for sliders and list rails.
   - Escape key dismiss handler.
3. **`crates/liquid_glass/src/input/ime.rs`**:
   - Translates `winit::event::Ime` events (Preedit, Commit, Disabled).
   - Coordinates with `winit::window::Window::set_ime_cursor_area` to position OS IME composition windows at text cursor rects.
4. **`crates/liquid_glass/src/input/dispatch.rs`**:
   - Translates UI gestures into `AppAction` intents and queues them via `HostHandle`.

---

## 3. Acceptance Criteria

- [ ] Hit testing hits the correct control even while pointer stretch or trailing animations are active.
- [ ] Tab and Shift+Tab traverse controls in visual logical order with visible focus ring.
- [ ] Arrow keys adjust slider positions by 1% (or 5% with Shift).
- [ ] Vietnamese IME preedit text renders with underline and candidate window aligns to cursor.
- [ ] Escape key dispatches dismiss action for Overlay, Mixer, and Help surfaces.

---

## 4. Verification

```bash
cargo test -p liquid_glass --lib input::tests
```
