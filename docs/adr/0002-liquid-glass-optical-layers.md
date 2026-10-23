# 0002: Single-Pass Backdrop Blur and Anti-Blur Multiplication

- **Status**: Accepted
- **Date**: 2026-10-23
- **Context**: Apple Liquid Glass requires multi-stage depth (refraction blur, tint, specular rim). Stacking multiple blur filters across parent shells and nested child cards causes extreme GPU fill-rate spikes, dropped frames (down to 15–20 FPS on integrated GPUs), and severe legibility degradation over complex desktop backgrounds.
- **Decision**: Restrict `backdrop-filter: blur()` strictly to the outermost Surface Shell (`.glass-surface` / `.surface-shell`). Child content containers (cards, session rows, popovers) must use luminance-calculated translucent tints via `color-mix(in oklab, ...)` without nested `backdrop-filter` rules.
- **Consequences**:
  - Eliminates compositor stutter on Windows WebView2, macOS WKWebView, and Linux WebKitGTK.
  - Ensures WCAG AA contrast (≥ 4.5:1) for body text and control labels across all desktop wallpapers.
  - Guarantees instant graceful degradation on software-rendered Linux environments.
