# Ticket T14 — Native System Tray Integration & Context Menu

**Phase:** Phase 3 — Surfaces
**Status:** Ready for Implementation
**Traceability:** R2, Spec §4.5
**Depends On:** T03, T08
**Depended Upon By:** T16

---

## 1. Description & Scope

Implement native system tray integration and context menu handling. Replace Tauri's webview-bound tray menu with native platform tray hooks (or compact WGPU popup menu using `MaterialRole::Menu`). Provide quick surface launchers (Mixer, Settings, Help), config folder shortcut, update checker, and application termination.

---

## 2. Planned Changes

1. **`crates/volumectl/src/ui/tray.rs`**:
   - Platform tray adapters (Win32 Shell_NotifyIcon, macOS NSStatusItem, Linux StatusNotifierItem / libappindicator).
   - Menu item dispatches directly to `HostHandle::enqueue(AppAction)`.
2. **Context Menu Actions**:
   - Open Mixer (`AppAction::ShowSurface(SurfaceId::Mixer)`)
   - Open Settings (`AppAction::ShowSurface(SurfaceId::Settings)`)
   - Open Help (`AppAction::ShowSurface(SurfaceId::Help)`)
   - Open Config Location (`AppAction::OpenConfigLocation`)
   - Check Updates (`AppAction::CheckUpdates`)
   - Exit (`AppAction::Exit`)

---

## 3. Acceptance Criteria

- [ ] Tray icon appears reliably in system notification area on Windows, macOS, and Linux.
- [ ] Left-click or right-click triggers menu or toggles Mixer window.
- [ ] Menu items dispatch proper `AppAction` intents without latency.
- [ ] Exit cleanly terminates background loops and releases tray icon.

---

## 4. Verification

```bash
cargo test -p volumectl --lib ui::tray::tests
```
