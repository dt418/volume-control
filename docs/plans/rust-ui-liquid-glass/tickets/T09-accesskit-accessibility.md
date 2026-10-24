# Ticket T09 — AccessKit Assistive-Technology Tree Integration

**Phase:** Phase 2 — Core Renderer
**Status:** Ready for Implementation
**Traceability:** R6, N5, D1, Spec §4, §5
**Depends On:** T04, T08
**Depended Upon By:** T10, T11, T12, T13

---

## 1. Description & Scope

Integrate `accesskit` and `accesskit_winit` in `crates/liquid_glass/src/a11y/`. Construct and update an accessible semantic tree mirroring the retained scene using identical `SemanticId` values. Expose proper roles (Slider, Button, Checkbox, Text, Window), names, states (muted, disabled, focused), values (volume 0–100%), and support accessibility action dispatch (Increment, Decrement, Click).

---

## 2. Planned Changes

1. **`crates/liquid_glass/src/a11y/adapter.rs`**:
   - `A11yAdapter`: Wraps `accesskit_winit::Adapter`.
   - Converts `winit` window events to AccessKit events.
2. **`crates/liquid_glass/src/a11y/tree.rs`**:
   - `build_tree(scene: &Scene) -> accesskit::TreeUpdate`.
   - Maps `SceneNode` to `accesskit::Node`:
     - Volume slider: `Role::Slider`, `numeric_value: Some(vol as f64)`, `min_numeric_value: Some(0.0)`, `max_numeric_value: Some(100.0)`.
     - Mute button: `Role::CheckBox` / `Role::ToggleButton`, `checked_state: Some(if muted { Checked } else { Unchecked })`.
     - Session rail: `Role::ListItem` with application label.
3. **`crates/liquid_glass/src/a11y/handler.rs`**:
   - Handles `accesskit::ActionRequest`: maps `Action::SetValue` or `Action::Default` directly to corresponding `AppAction`.

---

## 3. Acceptance Criteria

- [ ] Windows Accessibility Insights, macOS VoiceOver, and Linux Accerciser navigate the UI tree.
- [ ] Screen readers announce volume percentages when sliders move.
- [ ] Screen reader invocation of slider decrement/increment alters confirmed volume.
- [ ] Mute state toggles reflect immediately in assistive technology announcements.

---

## 4. Verification

```bash
cargo test -p liquid_glass --lib a11y::tests
```
