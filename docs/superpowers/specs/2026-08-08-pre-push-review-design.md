# Pre-Push Review Codification — Design

Date: 2026-08-08
Status: Implemented (vol-016)

## Problem

The adversarial three-domain review (guard core / gate chain / wiring &
records) that hardened the enforcement stack was performed once, ad hoc, with
its knowledge trapped in one conversation. Every future push needs the same
adversarial pass, or the stack can rot silently: a dropped hook invocation,
a drifted skill mirror, a gate whose `$LASTEXITCODE` handling masks a native
failure, a records entry whose evidence no longer matches reality.

## Design

Codify the review as a **skill** (not a script — the adversarial pass is
agent work; the mechanical battery already exists as the self-tests):

- `.agents/skills/pre-push-review/SKILL.md` (canonical) + `.claude/` mirror
  (byte-identical, the repo's skill-mirror pattern): three domain checklists
  with concrete hunt-for lists, the parallel-dispatch process, and a
  verification gate. The hunt-for lists are distilled from the actual
  findings of the review that hardened the stack (stderr discipline, WSL
  shim, template-hint contract on real paths, `$LASTEXITCODE` capture).
- Guardrail skill mandates the **review** phase (brainstorm -> spec -> plan
  -> execute -> verify -> review -> finish) and references the new skill.
- `scripts/ship.sh` usage/header reminds to run the review before `--push`
  (the mechanical battery stays ship's job; the review is the adversarial
  layer above it).
- Wiring assertions (the repo's drift-detection pattern): the records
  self-test now asserts both skill mirrors are byte-identical and the
  guardrail still mandates the review, so an edit that drops the review from
  the flow fails loudly.

## Files

- `.agents/skills/pre-push-review/SKILL.md` (+ `.claude/` mirror) — new
- `.agents/skills/guardrail/SKILL.md` (+ mirror) — review phase + reference
- `scripts/ship.sh` — reminder lines in header and usage
- `scripts/test-check-records.sh` — 2 new wiring checks
- `feature_list.json` (vol-016), `claude-progress.md` (Session 014) — records

## Verification

`bash scripts/test-check-records.sh`, `bash scripts/test-format-lint.sh`,
`bash scripts/test-ship.sh`, both gates, `cargo test`, and
`sh scripts/check-records.sh --staged` on the staged set — all green.
