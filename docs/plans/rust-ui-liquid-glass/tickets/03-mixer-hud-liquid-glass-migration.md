# Ticket 03: Volume Mixer HUD Liquid Glass & Density Migration

**What to build:**
Modernize the primary Volume Mixer HUD surface (`frontend/src/mixer/`) to utilize the Liquid Glass Surface Shell and high-density `CardSm` session rows. Replace flat rectangular container styles with optical refraction, specular rim highlighting, and organic capsule tracks for the master Signal Rail and individual application audio sessions. Grounded in the `mixer.png` baseline capture, ensure at least two application session rows remain visible simultaneously in the 400×224px viewport without vertical scrolling.

**Blocked by:**
Ticket 01: Liquid Glass CSS Tokens & Multi-Stage Surface Shell
Ticket 02: Rust/UI CardSm and Tactile Pill Primitives

**Status:**
ready-for-agent

**Acceptance criteria:**
- [ ] `MixerSurface` container renders as a Liquid Glass shell with 20px blur, 18px squircle radius, and top specular rim.
- [ ] Master system output row (`SystemOutputRow.tsx`) renders inside a `CardSm` container with translucent tint (`bg-card/60`).
- [ ] Individual application session rows (`SessionRow.tsx`) render as floating `CardSm` tiles with subtle hover luminance and zero nested blur.
- [ ] Mute button and device selector pills adopt tactile capsule styling with instant reactive toggle state.
- [ ] Signal Rail volume visualization preserves authoritative hardware threshold color bands (`#888888`, `#27ae60`, `#0078d4`, `#e05c00`) within a tactile pill track.
- [ ] Vitest tests in `MixerSurface.test.tsx`, `SystemOutputRow.test.tsx`, and `SignalRail.test.tsx` pass 100%.
- [ ] Visual verification confirms at least two application session rows are visible without scrolling in 400×224px bounds.
