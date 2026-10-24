# Ticket T06 — Multi-Pass Blur Pipeline & Pyramid Caching

**Phase:** Phase 2 — Core Renderer
**Status:** Ready for Implementation
**Traceability:** R8, D2, N3, Spec §4.2, §5
**Depends On:** T01, T04
**Depended Upon By:** T10, T11, T12, T13

---

## 1. Description & Scope

Implement the offscreen content composition pass and downsampled multi-pass Gaussian blur pyramid in `crates/liquid_glass/src/blur/`. Render app-owned backdrop content to a readable texture, execute horizontal and vertical separable blur passes with progressive mip/downsampling, and cache blur results using `content_revision`.

---

## 2. Planned Changes

1. **`crates/liquid_glass/src/blur/pyramid.rs`**:
   - `BlurPyramid`: Texture views at 1/2, 1/4, 1/8 resolutions for efficient large-radius blurs.
   - Cache key: `(content_revision, width, height, scale_factor)`.
2. **`crates/liquid_glass/src/blur/shaders/blur.wgsl`**:
   - 9-tap separable 1D Gaussian kernel WGSL shader.
   - Dual-pass ping-pong texture rendering.
3. **`crates/liquid_glass/src/blur/pipeline.rs`**:
   - `BlurPipeline::execute(&mut self, encoder: &mut wgpu::CommandEncoder, source: &wgpu::TextureView)`.
   - Reuses cached blur textures when `content_revision` is unchanged.

---

## 3. Acceptance Criteria

- [ ] Blur pyramid recomputes only when `content_revision` changes or surface is resized.
- [ ] Large blur radii (24–32px) render at 60+ FPS via downsampled passes without GPU stutter.
- [ ] Background sampling is strictly app-owned content; zero OS screen-capture APIs invoked.
- [ ] Ping-pong textures are allocated once and resized only on window resize.

---

## 4. Verification

```bash
cargo test -p liquid_glass --lib blur::tests
```
