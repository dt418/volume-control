# Ticket T12 — Native Settings Surface (General & Appearance)

**Phase:** Phase 3 — Surfaces
**Status:** Ready for Implementation
**Traceability:** R1, R2, R3, R5, Spec §4.2, §7 (Scenarios 3, 4)
**Baseline UI:** `C:\Users\Thanh\AppData\Local\Temp\volume-control-ui-captures\settings_general.png`, `settings_appearance.png`
**Depends On:** T03, T04, T05, T06, T07, T08, T09
**Depended Upon By:** T16

---

## 1. Description & Scope

Implement the native Settings window surface in `crates/liquid_glass/src/surfaces/settings.rs`. Replace the React settings webview with an owned WGPU window (760×620 design size, min 620×520), faithfully reproducing `settings_general.png` and `settings_appearance.png`. Implement sidebar navigation, scrollable form controls, bilingual labels, and the strict draft/commit lifecycle (Apply, Cancel, Reset).

---

## 2. Planned Changes

1. **`crates/liquid_glass/src/surfaces/settings/`**:
   - `sidebar.rs`: Category items (General, Appearance, Hotkeys, Blacklist, Updates) using `MaterialRole::Sidebar`.
   - `general.rs`: Startup toggles, volume step, audio feedback options.
   - `appearance.rs`: Live draft controls for `ThemeMode`, `MaterialMode`, `MotionMode`, `AccentMode`, and `Language`.
   - `actions.rs`: Reset to Defaults, Cancel, Save action bar.
2. **Draft & Commit Management**:
   - Local `DraftState` mutates on user interaction and updates the live glass preview.
   - Cancel discards draft state and reloads confirmed `AppState`.
   - Save dispatches `AppAction::ApplyConfig`, prompting the host to persist changes to `config.ini`.

---

## 3. Acceptance Criteria

- [ ] Window is resizable with minimum bounds enforced at 620×520 logical px.
- [ ] Draft appearance changes preview immediately without modifying `config.ini`.
- [ ] Clicking Save commits changes to `config.ini`; clicking Cancel restores confirmed state.
- [ ] Vietnamese copy and IME work flawlessly across all text input fields.
- [ ] High contrast and reduced transparency force opaque fallback styling across all settings cards.

---

## 4. Verification

```bash
cargo test -p liquid_glass --lib surfaces::settings::tests
```
