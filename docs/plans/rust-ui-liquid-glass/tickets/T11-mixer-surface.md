# Ticket T11 — Native Volume Mixer Surface

**Phase:** Phase 3 — Surfaces
**Status:** Ready for Implementation
**Traceability:** R1, R2, R4, R5, Spec §4.1, §7 (Scenario 2)
**Baseline UI:** `C:\Users\Thanh\AppData\Local\Temp\volume-control-ui-captures\mixer.png`
**Depends On:** T03, T04, T05, T06, T07, T08, T09
**Depended Upon By:** T16

---

## 1. Description & Scope

Implement the native Volume Mixer floating surface in `crates/liquid_glass/src/surfaces/mixer.rs`. Replace the existing React mixer webview with a compact WGPU glass card (400×224 logical px), matching the visual hierarchy captured in `mixer.png`: system output rail, search filter, per-application audio session rails, interactive sliders, mute toggles, and Vietnamese copy.

---

## 2. Planned Changes

1. **`crates/liquid_glass/src/surfaces/mixer.rs`**:
   - `MixerSurface`: Manages mixer scene layout, scroll offset, and filter state.
   - Master volume row: Speaker/Headphones icon, device name label, master slider, mute toggle.
   - Search box: Input field for filtering sessions by process/app name with Vietnamese IME support.
   - Audio session list: Virtualized list of app sessions with app icon, title, volume slider, and mute button.
2. **Action Dispatch**:
   - Sliders emit `AppAction::SetVolumePercent` (master) or `AppAction::SetSessionVolume` (session).
   - Mute buttons emit `AppAction::ToggleMute` or `AppAction::ToggleSessionMute`.
   - Escape key dismisses mixer.

---

## 3. Acceptance Criteria

- [ ] Visual layout, colors, and typography match `mixer.png` baseline.
- [ ] Active WASAPI sessions appear automatically; terminated sessions drop cleanly without panics.
- [ ] Session slider drags update app volumes in real-time with zero audio glitching.
- [ ] Keyboard navigation (Tab/Shift+Tab, Arrow keys, Escape) operates all mixer controls.
- [ ] Search filter dynamically narrows the visible session list.

---

## 4. Verification

```bash
cargo test -p liquid_glass --lib surfaces::mixer::tests
```
