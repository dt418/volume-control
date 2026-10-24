# Ticket T05 — Material Resolution & SDF Shading Pipeline

**Phase:** Phase 2 — Core Renderer
**Status:** Ready for Implementation
**Traceability:** R7, R8, D3, N5, Spec §5.1, §5.2
**Depends On:** T04
**Depended Upon By:** T07, T10, T11, T12, T13

---

## 1. Description & Scope

Implement material token resolution and signed-distance field (SDF) WGSL shading pipelines in `crates/liquid_glass/src/material/`. Resolve semantic `MaterialRole` and `Appearance` into GPU uniforms (blur radius, tint color, saturation, opacity, specular rim highlight, edge thickness). Implement smooth SDF polynomial union for slider tracks and control groups.

---

## 2. Planned Changes

1. **`crates/liquid_glass/src/material/tokens.rs`**:
   - `MaterialRole`: `Hud`, `Panel`, `Sidebar`, `Control`, `Accent`, `Tooltip`, `Menu`.
   - `Appearance`: `color_scheme`, `reduced_transparency`, `increase_contrast`, `reduce_motion`.
   - Token resolution chain: forces opaque fallback when `high_contrast || reduced_transparency`.
2. **`crates/liquid_glass/src/material/shaders/glass.wgsl`**:
   - Signed-distance evaluation for rounded rectangles with variable corner radii.
   - Smooth union polynomial function for organic control blending.
   - Dual-layer shading: background blur sampling + specular rim edge highlight + luminance tint.
3. **`crates/liquid_glass/src/material/pipeline.rs`**:
   - WGPU `RenderPipeline` with dynamic uniform buffers for per-primitive material parameters.

---

## 3. Acceptance Criteria

- [ ] High contrast or reduced transparency yields opaque tokens (blur = 0, alpha = 1.0, 2px borders).
- [ ] SDF smooth union correctly blends slider thumbs and tracks without visual seams.
- [ ] Specular rim highlights (1.0px) render sharply across 1.0x, 1.5x, and 2.0x DPI scales.
- [ ] Shaders compile cleanly via `wgpu::ShaderModuleDescriptor` across DX12, Metal, and Vulkan.

---

## 4. Verification

```bash
cargo test -p liquid_glass --lib material::tests
```
