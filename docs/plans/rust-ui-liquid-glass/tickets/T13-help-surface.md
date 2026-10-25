# Ticket T13 — Native Help Surface & Keyboard Reference

**Phase:** Phase 3 — Surfaces
**Status:** Ready for Implementation
**Traceability:** R2, R5, Spec §4.4
**Depends On:** T04, T05, T06, T08, T09
**Depended Upon By:** T16

---

## 1. Description & Scope

Implement the native Help surface dialog in `crates/liquid_glass/src/surfaces/help.rs` (520×500 logical px). Replace the React help webview with an owned WGPU glass card displaying bilingual (EN/VI) keyboard shortcut mappings, documentation links, and version information.

---

## 2. Planned Changes

1. **`crates/liquid_glass/src/surfaces/help.rs`**:
   - `HelpSurface`: Layout with header, scrollable shortcut table, and close button.
   - Shortcut reference rows: Key badge (e.g. `Ctrl + Alt + Up`) and localized description.
   - Bilingual copy loaded dynamically from current `AppState.language`.
   - Dismissible via Escape key, close button, or clicking outside.

---

## 3. Acceptance Criteria

- [ ] Help window opens cleanly with smooth entrance fade.
- [ ] Keyboard shortcut list matches configured global and local shortcuts.
- [ ] Language toggle instantly switches copy between English and Vietnamese.
- [ ] Screen readers traverse the shortcut table with proper row/cell semantics.

---

## 4. Verification

```bash
cargo test -p liquid_glass --lib surfaces::help::tests
```
