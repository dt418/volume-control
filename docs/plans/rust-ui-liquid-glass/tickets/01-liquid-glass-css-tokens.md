# Ticket 01: Liquid Glass CSS Tokens & Multi-Stage Surface Shell

**What to build:**
Upgrade VolumeControl's global CSS surface treatment to modern Apple-style Liquid Glass optics. This establishes the Z1 Surface Shell foundation: replacing the flat 12px blur with a multi-stage optical system featuring a 20px refraction blur, 180% saturation boost, OKLab luminance-preserving tinting, 1px directional specular rim lighting (`inset 0 1px 0 0`), soft ambient drop shadowing, and dynamic `:focus-within` modulation. Enforces the Anti-Blur Multiplication rule (ADR 0002) so that child elements do not declare blur filters.

**Blocked by:**
None (can start immediately).

**Status:**
ready-for-agent

**Acceptance criteria:**
- [ ] `.glass-surface` declares `backdrop-filter: blur(20px) saturate(180%)` with `-webkit-` vendor prefix.
- [ ] Surface shell tint uses `color-mix(in oklab, var(--background) 82%, transparent)` in dark mode and `85%` in light mode.
- [ ] Specular rim highlight renders 1px directional top-edge reflection (`inset 0 1px 0 0 rgba(255, 255, 255, 0.22)` in dark mode, `0.65` in light mode).
- [ ] Surface shell maintains continuous squircle border-radius of `18px` (`rounded-2xl`) for mixer/overlay and `24px` (`rounded-3xl`) for settings.
- [ ] Fallback declaration provides 94% opaque background when backdrop-filter is unsupported or compositing is disabled.
- [ ] When `motion=Reduced` or `.reduced-motion` is active, all transition and animation durations collapse to `0.001s !important`.
- [ ] Static scan verifies no child class outside `.glass-surface` introduces nested `backdrop-filter`.
- [ ] Production build succeeds with CSS payload increase ≤ 2.5 kB gzipped.
