# Ticket 04: Settings Appearance Preview & Bilingual Localizations

**What to build:**
Upgrade the Settings dialog Appearance section (`frontend/src/settings/AppearanceSection.tsx`) and live preview (`AppearancePreview.tsx`) to support Liquid Glass optical controls and real-time visualization. Add interactive controls for `MaterialMode` (`Auto`, `Translucent`, `Opaque`), `MotionMode` (`Full`, `Reduced`, `Disabled`), and `AccentMode` (`System`, `Blue`, `Green`, `Purple`, `Orange`), reflecting changes immediately in an inline Liquid Glass preview card. Ensure 100% bilingual translation parity across English and Vietnamese in `frontend/src/lib/i18n.tsx`.

**Blocked by:**
Ticket 01: Liquid Glass CSS Tokens & Multi-Stage Surface Shell
Ticket 02: Rust/UI CardSm and Tactile Pill Primitives

**Status:**
ready-for-agent

**Acceptance criteria:**
- [ ] `AppearancePreview.tsx` renders a simulated Liquid Glass card featuring background blur, specular rim, and live accent coloration.
- [ ] Changing Theme, Material, Motion, or Accent dropdowns immediately updates the preview card without page reload.
- [ ] `frontend/src/lib/i18n.tsx` includes complete English and Vietnamese strings for all newly added appearance tokens (e.g., "Liquid Glass", "Kính chất lỏng", "Specular Rim", "Viền phản xạ khúc xạ").
- [ ] All label and helper copy achieves WCAG AA contrast ratio (≥ 4.5:1 for body copy).
- [ ] Vitest tests in `AppearanceSection.test.tsx` and `AppearancePreview.test.tsx` pass cleanly.
- [ ] `config.ini` single-source-of-truth round-trip serialization is strictly maintained.
