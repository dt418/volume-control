# Ticket T15 — Linux Wayland Layer-Shell Placement Adapter

**Phase:** Phase 3 — Surfaces
**Status:** Ready for Implementation
**Traceability:** R4, R10, D1, D4, Spec §4.3, §6.2
**Depends On:** T01, T10
**Depended Upon By:** T16

---

## 1. Description & Scope

Implement a narrow, Linux-only platform-host adapter in `crates/volumectl/src/ui/platform/linux/` providing Wayland layer-shell protocol integration. Specifically solve Winit's documented inability to position overlay surfaces on Wayland by configuring `zwlr_layer_shell_v1` anchors (bottom-right), layer stacking (`Layer::Overlay`), and keyboard interactivity.

**Crucial Invariant:** Zero Wayland protocol code enters the portable `crates/liquid_glass` renderer crate. The adapter operates strictly within the Linux platform layer of the host application.

---

## 2. Planned Changes

1. **`crates/volumectl/src/ui/platform/linux/layer_shell.rs`**:
   - Manages Wayland `WlSurface` and `ZwlrLayerSurfaceV1` handles.
   - Sets anchor to `Anchor::BOTTOM | Anchor::RIGHT` with configured margin offsets.
   - Configures layer to `Layer::Overlay`.
   - Leaves HUD interactive when compositor ignores input shape pass-through requests.

---

## 3. Acceptance Criteria

- [ ] On Wayland compositors (GNOME Mutter, Sway, KDE KWin), the volume overlay anchors securely at the bottom-right.
- [ ] No Wayland crate dependencies or protocol bindings exist inside `crates/liquid_glass`.
- [ ] X11 fallback continues to function without regressions via standard X11 window placement.

---

## 4. Verification

```bash
cargo test -p volumectl --lib ui::platform::linux::tests
```
