# Ticket 02: Rust/UI CardSm and Tactile Pill Primitives

**What to build:**
Port Rust/UI (`rust-ui.com`) compact component patterns into VolumeControl's UI library under `frontend/src/components/ui/`. Introduce `CardSm`—a high-density card variant specifically designed for compact desktop utility panels with reduced padding (`p-3`/`p-4`), translucent background tinting (`color-mix(in oklab, var(--card) 60%, transparent)`), subtle borders (`border-border/40`), and zero nested blur (ADR 0002). Upgrade `Badge` to support full capsule pill profiles with micro-spring active press states (`active:scale-[0.97]`).

**Blocked by:**
Ticket 01: Liquid Glass CSS Tokens & Multi-Stage Surface Shell

**Status:**
ready-for-agent

**Acceptance criteria:**
- [ ] `Card` component in `frontend/src/components/ui/card.tsx` exports a `CardSm` variant or density prop supporting `p-3` and `p-4` padding.
- [ ] `CardSm` applies semi-translucent tint `bg-card/60` and border `border-border/40` without any `backdrop-filter: blur()` utility.
- [ ] `Badge` component in `frontend/src/components/ui/badge.tsx` supports a `pill` variant rendering full capsule radius (`rounded-full px-2.5 py-0.5`).
- [ ] Buttons and badges provide tactile micro-press feedback (`active:scale-[0.97] transition-transform duration-100 ease-out`).
- [ ] Vitest unit tests verify `CardSm` renders correct padding classes and adheres to the Anti-Blur constraint.
- [ ] All 43 existing test suites continue to pass without regression.
