# Ticket 05: Volume Overlay HUD & Help Surface Modernization

**What to build:**
Modernize the transient on-screen display Volume Overlay (`frontend/src/overlay/`) and the Help shortcut cheat-sheet (`frontend/src/help/`) to harmonize with the Liquid Glass visual system. The volume overlay (336×88 pt) becomes a continuous glass capsule floating over active applications with centered Signal Rail and instant dismiss timing. The Help surface adopts tactile `Kbd` shortcut indicators and translucent conflict callout cards.

**Blocked by:**
Ticket 01: Liquid Glass CSS Tokens & Multi-Stage Surface Shell
Ticket 02: Rust/UI CardSm and Tactile Pill Primitives

**Status:**
ready-for-agent

**Acceptance criteria:**
- [ ] `OverlaySurface.tsx` renders as a floating glass capsule with 20px blur, 1px specular rim, and centered Signal Rail.
- [ ] Overlay appearance respects `MotionMode::Reduced` by suppressing slide animations and using instant opacity transitions.
- [ ] `HelpSurface.tsx` shortcut keys render with tactile `Kbd` components featuring inset keycap borders and monospaced typography.
- [ ] Conflict callout banners render as high-contrast warning cards with crisp border accents.
- [ ] Vitest tests in `OverlaySurface.test.tsx` and `HelpSurface.test.tsx` pass without error.
- [ ] Production build succeeds with all 4 surfaces compiled cleanly.
