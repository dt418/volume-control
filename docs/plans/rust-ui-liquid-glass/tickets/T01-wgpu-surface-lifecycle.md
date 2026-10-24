# Ticket T01 — WGPU Surface Lifecycle & Multi-Backend Context

**Phase:** Phase 1 — Foundation
**Status:** Ready for Implementation
**Traceability:** N4, R10, D1, D4, Spec §5, §6.1, §7 (Scenarios 5, 6)
**Depends On:** T00
**Depended Upon By:** T04, T06, T10, T15

---

## 1. Description & Scope

Implement `GpuContext` and `SurfaceManager` in `crates/liquid_glass/src/gpu/` wrapping `wgpu 30.0.1`. Enforce the deterministic platform backend policy:
- Windows: Attempt DX12 instance first; fallback to explicit Vulkan instance if DX12 adapter acquisition fails.
- macOS: Metal instance only.
- Linux: Vulkan instance only.

Implement the strict 7-state `CurrentSurfaceTexture` acquisition lifecycle and error recovery per N4:
- `Success(texture)`: Render, submit command buffer, and present.
- `Suboptimal(texture)`: Submit and present, then schedule surface reconfiguration.
- `Timeout` or `Occluded`: Skip current frame, return to event loop, await next redraw request.
- `Outdated`: Reconfigure surface using latest nonzero window extent before next frame.
- `Lost` (surface only): Recreate `wgpu::Surface` via `instance.create_surface()`, reconfigure, and resume.
- `Lost` (device loss): Recreate entire GPU stack (adapter, device, queue, surfaces, textures, pipelines).
- `Validation`: Enter controlled surface-error state until valid extents exist; never loop.
- `OutOfMemory`: Clean graceful teardown without infinite retry loops.

---

## 2. Planned Changes

1. **`crates/liquid_glass/src/gpu/context.rs`**:
   - `GpuContext::new(window: &winit::window::Window) -> Result<Self, GpuError>`.
   - Explicit backend negotiation logic.
2. **`crates/liquid_glass/src/gpu/surface.rs`**:
   - `SurfaceManager::acquire(&mut self) -> Result<AcquiredFrame, SurfaceError>`.
   - `SurfaceManager::reconfigure(&mut self, width: u32, height: u32)`.
   - `SurfaceManager::recreate_surface(&mut self, window: &winit::window::Window)`.
3. **`crates/liquid_glass/src/gpu/error.rs`**:
   - Injectable fault injection points for unit/integration testing surface loss and device recovery.

---

## 3. Acceptance Criteria

- [ ] Surface acquisition handles all 7 `CurrentSurfaceTexture` variants per specification.
- [ ] Windows tries DX12 first, Vulkan fallback; macOS uses Metal; Linux uses Vulkan.
- [ ] Resize triggers `Outdated` recovery without surface panic.
- [ ] Injectable device loss test verifies complete GPU resource recreation without leaking memory.
- [ ] Zero heap allocations in steady-state `acquire()` / `present()`.

---

## 4. Verification

```bash
cargo test -p liquid_glass --lib gpu::tests
```
