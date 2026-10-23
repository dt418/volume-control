# Phase 1 Decision Ledger: Rust/UI & Liquid Glass Integration

**Document ID**: DECISIONS-LEDGER-PHASE1
**Date**: 2026-10-23
**Status**: Settled for Gate 1 Approval

---

## 1. Architectural & Material Decisions

### Decision 1: Functional Glass vs. Content Layer Boundary
- **Settled Rule**: The Surface Shell (`.surface-shell`, `.glass-surface`) owns the optical glass effect (blur 20px, specular rim 1px, luminance tint). The Content Layer (cards, rows, inputs) uses translucent tints (`color-mix` with `var(--background)`) with solid readable contrast.
- **Enforcement**: Anti-Blur Multiplication rule (G12). No child component may declare `backdrop-filter: blur()`.

### Decision 2: Theme, Material, Motion & Accent Migration
- **Material Mode**:
  - `MaterialMode::Translucent`: Activates full Liquid Glass multi-stage optics (refraction blur, specular highlight, luminance tint).
  - `MaterialMode::Auto`: Activates Liquid Glass if OS compositor supports hardware-accelerated transparency; otherwise falls back to opaque.
  - `MaterialMode::Opaque`: Renders solid card surfaces (`var(--card)`) without blur or specular highlights.
- **Motion Mode**:
  - `MotionMode::Full`: Fluid spring transitions (180–240ms, damping 30) for dialogs, cards, and sliders.
  - `MotionMode::Reduced` / `Disabled`: Instant transitions (`0.001s !important`), zero spring bounce.
- **Accent Mode**:
  - Dynamically modulates the 1px specular rim gradient and active Signal Rail color accents across `System`, `Blue`, `Green`, `Purple`, `Orange`.

### Decision 3: Bilingual Support (EN / VI)
- **Standard**: All new UI strings, configuration labels, material descriptions, and tooltips are added simultaneously to `frontend/src/lib/i18n.tsx` in English and Vietnamese.
- **Glossary Alignment**: Canonical Vietnamese translations adhere strictly to `CONTEXT.md`.

### Decision 4: Reference Source Scope
- **Standard**: Rely on public web evidence (`rust-ui.com`), Apple Human Interface Guidelines, WWDC25 Session 219, WWDC26, and verified repository UI captures. Proprietary third-party desktop binary audits are excluded.
