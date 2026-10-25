# Ticket T07 — Closed-Form Analytic Spring Motion & Bounded Fluids

**Phase:** Phase 2 — Core Renderer
**Status:** Ready for Implementation
**Traceability:** R9, D3, N3, Spec §5.3
**Depends On:** T04, T05
**Depended Upon By:** T10, T11

---

## 1. Description & Scope

Implement closed-form analytic spring physics and bounded liquid visual effects in `crates/liquid_glass/src/motion/`. Implement second-order harmonic oscillator evaluation directly from time $t$, ensuring velocity continuity on retargeting. Implement settling detection to stop redraw loops. Integrate reduced-motion overrides.

---

## 2. Planned Changes

1. **`crates/liquid_glass/src/motion/spring.rs`**:
   - `AnalyticSpring`: Parameters $(\zeta, \omega_n)$, initial position/velocity, target value.
   - Evaluation: $x(t) = A \cdot e^{-\zeta \omega_n t} \cos(\omega_d t + \phi) + x_{\text{target}}$.
   - `retarget(&mut self, new_target: f32, current_time: Instant)`.
   - `is_settled(&self, current_time: Instant) -> bool`.
2. **`crates/liquid_glass/src/motion/fluid.rs`**:
   - Pointer stretch and trailing offset calculations based on pointer velocity.
   - Interaction ripple waves: expanding radial decay rings.
   - Subtle downward gravity bias for floating HUDs.
3. **`crates/liquid_glass/src/motion/policy.rs`**:
   - When `reduce_motion == true`, all amplitudes (stretch, ripple, bounce) are forced to 0; state changes become immediate.

---

## 3. Acceptance Criteria

- [ ] Closed-form spring evaluation matches analytic reference values within $10^{-4}$ tolerance.
- [ ] Retargeting mid-animation retains velocity without positional jumps.
- [ ] When all springs settle, `is_settled()` returns `true` and frame loop enters idle (0 redraws).
- [ ] Reduced motion bypasses fluid deformation completely.

---

## 4. Verification

```bash
cargo test -p liquid_glass --lib motion::tests
```
