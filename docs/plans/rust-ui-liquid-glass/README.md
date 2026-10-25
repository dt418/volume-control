# Native Liquid Glass — Master Index

## Document hierarchy

| Document | Purpose | Status |
|---|---|---|
| [context.md](context.md) | Vocabulary, application boundary, persisted config, approved decisions, invariants | Living; updated with each gate |
| [evidence.md](evidence.md) | Official material guidelines evidence and observation matrix | Approved Gate 1 |
| [gate-1-requirements.md](gate-1-requirements.md) | R1–R10 functional, N1–N6 non-functional requirements, fixed decisions, deferred decisions | **Approved 2026-10-23** |
| [rust-native-liquid-glass-research.md](rust-native-liquid-glass-research.md) | Library comparison, WGPU solution, renderer data model, cross-platform feasibility, primary sources | Approved Gate 1 |
| [design.md](design.md) | D1–D4: architecture, data flow, material/motion, error/verification | **Approved 2026-10-23 (Gate 2)** |
| [specification.md](specification.md) | T0–T16 implementation tickets, dependency graph, phases, traceability matrix | Draft |

## Gate history

| Gate | Decision | Date |
|---|---|---|
| Gate 1 | R1–R10, N1–N6 binding. Authorizes evidence collection and design. | 2026-10-23 |
| Gate 2 | D1–D4 binding. Authorizes specification, tickets, implementation planning. | 2026-10-23 |
| Gate 3 | Implementation start. Requires T0 scaffold verification. | Pending |

## Binding decisions summary

1. One owned `crates/liquid_glass` renderer crate.
2. Rust 1.92, `wgpu 30.0.1`, `glyphon 0.12`, Windows `static-dxc`.
3. DX12 → Vulkan (Windows), Metal (macOS), Vulkan (Linux).
4. Host authority preserved; extend `AppState`/`AppAction`/`UiCapabilities` pre-cutover.
5. App-owned content blur; no desktop capture.
6. Bounded liquid behavior; no Navier–Stokes or permanent idle motion.
7. One renderer: `winit` + `glyphon` + AccessKit; no second UI framework.
8. WGPU-only material baseline; DWM/AppKit/compositor materials post-cutover.
9. Retained semantic scene with stable IDs shared across layout, hit test, focus, a11y, rendering.

## Deferred decisions

1. `glyphon 0.12` package resolution, Vietnamese shaping benchmark under Rust 1.92.
2. Native settings layout coherence with material hierarchy.
3. Host `UiCapabilities` reduced-transparency detection per platform.

## Reading order

New reader: `context.md` → `gate-1-requirements.md` → `evidence.md` → `design.md` → `specification.md`.

Implementer: `specification.md` (tickets + deps) → `design.md` (contracts) → `context.md` (invariants).
