# Ticket T02 — Native Text System (Glyphon 0.12 & Cosmic-Text)

**Phase:** Phase 1 — Foundation
**Status:** Ready for Implementation
**Traceability:** R5, D1, Spec §4, §5
**Depends On:** T00
**Depended Upon By:** T04, T11, T12, T13

---

## 1. Description & Scope

Implement `TextSystem` in `crates/liquid_glass/src/text/` using `glyphon 0.12` and `cosmic-text 0.19`. Provide Unicode text shaping, fallback fonts for Vietnamese diacritics, cached glyph rasterization via WGPU texture atlas, and layout measurement for multi-line copy and truncated device names.

---

## 2. Planned Changes

1. **`crates/liquid_glass/src/text/system.rs`**:
   - `TextSystem::new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self`.
   - Wraps `glyphon::FontSystem`, `glyphon::SwashCache`, `glyphon::TextAtlas`, `glyphon::TextRenderer`.
2. **`crates/liquid_glass/src/text/layout.rs`**:
   - `TextRun` layout descriptor: font family, size, line height, color, weight, clipping rect.
   - Text truncation algorithms: ellipsis trailing (`...`) for long audio endpoint/session labels.
3. **`crates/liquid_glass/src/text/fonts.rs`**:
   - Configure system font fallbacks: Segoe UI (Windows), SF Pro (macOS), Cantarell/DejaVu Sans (Linux) with fallback chains ensuring full Vietnamese character coverage (e.g. `ơ`, `ư`, `ạ`, `ế`, `ộ`).

---

## 3. Acceptance Criteria

- [ ] English and Vietnamese text strings layout and render without missing glyphs (`.notdef` replacement).
- [ ] Long audio device names (e.g. "Speakers (Realtek High Definition Audio Device)") truncate with ellipsis.
- [ ] Glyph atlas caches rasterized glyphs; zero texture re-allocations during regular typing or scrolling.
- [ ] `TextRenderer::prepare()` and `render()` integrate cleanly into WGPU render passes.

---

## 4. Verification

```bash
cargo test -p liquid_glass --lib text::tests
```
