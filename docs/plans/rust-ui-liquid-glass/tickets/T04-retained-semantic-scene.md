# Ticket T04 — Retained Semantic Scene & Node Hierarchy

**Phase:** Phase 2 — Core Renderer
**Status:** Ready for Implementation
**Traceability:** D1, D2, N3, Spec §3.1, §4.2, §6
**Depends On:** T00, T01, T02
**Depended Upon By:** T05, T06, T07, T08, T09

---

## 1. Description & Scope

Implement the retained semantic scene graph in `crates/liquid_glass/src/scene/`. Scene nodes retain stable semantic IDs across state updates. Provide allocation-stable node storage, layout calculation for subtrees, diffing from incoming `AppState`, and flattening into ordered `RenderPrimitive` streams.

---

## 2. Planned Changes

1. **`crates/liquid_glass/src/scene/node.rs`**:
   - `SemanticId(u64)`: Stable node identifier.
   - `SceneNode`:
     ```rust
     pub struct SceneNode {
         pub id: SemanticId,
         pub role: MaterialRole,
         pub layout_bounds: Rect,
         pub z_order: u16,
         pub focusable: bool,
         pub content_revision: u64,
         pub children: Vec<SemanticId>,
         pub text_run: Option<TextRunId>,
         pub a11y_node: Option<AccessKitNodeData>,
     }
     ```
2. **`crates/liquid_glass/src/scene/graph.rs`**:
   - `Scene`: Backed by allocation-stable slot map or arena (`Vec<Option<SceneNode>>`).
   - `SceneDiff`: Identifies dirty nodes from changed `AppState` fields without reconstructing clean nodes.
3. **`crates/liquid_glass/src/scene/flatten.rs`**:
   - Emits sorted `RenderPrimitive` collection ready for WGPU render passes:
     ```rust
     pub struct RenderPrimitive {
         pub id: SemanticId,
         pub bounds: Rect,
         pub role: MaterialRole,
         pub z_order: u16,
         pub clip: Option<Rect>,
     }
     ```

---

## 3. Acceptance Criteria

- [ ] Node IDs remain stable across successive `AppState` updates.
- [ ] Diff engine recomputes layout bounds only for dirty subtrees.
- [ ] Flattening produces strictly z-sorted primitives with correct clipping bounds.
- [ ] Zero heap allocations during steady-state frame rendering.

---

## 4. Verification

```bash
cargo test -p liquid_glass --lib scene::tests
```
