# Ticket T10 — Native Overlay Surface (Bottom-Right HUD)

**Phase:** Phase 3 — Surfaces
**Status:** Ready for Implementation
**Traceability:** R1, R2, R4, R8, R9, Spec §4.3, §7 (Scenario 1)
**Depends On:** T03, T04, T05, T06, T07, T08, T09
**Depended Upon By:** T15, T16

---

## 1. Description & Scope

Implement the native Bottom-Right Overlay surface in `crates/liquid_glass/src/surfaces/overlay.rs`. Replace the current WebView overlay window with a lightweight WGPU glass HUD (336×88 logical px). Provide fluid volume fill gauge, device name label, mute indicator, auto-hide timer, and capability-gated pointer click-through.

---

## 2. Planned Changes

1. **`crates/liquid_glass/src/surfaces/overlay.rs`**:
   - `OverlaySurface`: Builds and manages overlay scene nodes (`MaterialRole::Hud`).
   - Dynamic layout: Master volume gauge bar, percentage text, mute icon badge, active audio device name.
   - Auto-hide animation state: Slide/fade-in on appearance, timer tick (default 1500 ms), smooth fade-out.
2. **Click-Through Coordination**:
   - macOS: `winit::platform::macos::WindowExtMacOS::set_ignores_mouse_events(true)`.
   - Windows: Win32 `WS_EX_TRANSPARENT | WS_EX_LAYERED` extended window styles.
   - Linux: Gated by capability (Wayland HUD stays interactive without crashing).

---

## 3. Acceptance Criteria

- [ ] Volume hotkey presses display overlay at bottom-right of primary display work area within 16 ms.
- [ ] Liquid fill gauge reflects volume percentage smoothly.
- [ ] Overlay auto-hides cleanly after timeout without leaving residual window shadows.
- [ ] Reduced motion disables slide animation, using a simple 120ms alpha fade.
- [ ] Screen readers announce volume updates immediately upon display.

---

## 4. Verification

```bash
cargo test -p liquid_glass --lib surfaces::overlay::tests
```
