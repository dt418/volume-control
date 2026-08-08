# Record-Keeping Guardrail — Design

Date: 2026-08-08
Status: Proposed

## Problem

The repository's system of record is `feature_list.json` (per-feature status,
verification, and evidence) and `claude-progress.md` (per-session progress
log). `CLAUDE.md` *asks* for these to be updated, and the completion gate says
a feature reaches `passing` only with recorded evidence — but nothing
**enforces** that a change set that touches code also updates the records.
The user's directive: *every task must follow the superpowers flow and
hardness, and every change must update `feature_list.json` and
`claude-progress.md` — create a guard if needed.*

This spec designs that guard.

## Goals

1. Every substantive change set (code, scripts, CI, skills, hooks, rules
   docs) must update **both** `feature_list.json` and `claude-progress.md`
   before it can be committed or merged.
2. The superpowers flow (brainstorm → spec → plan → execute → verify →
   finish) and hardness (evidence-before-claims) become the codified,
   documented workflow for every task, not just a habit.
3. Enforcement is local (pre-commit hook) and remote (CI), with a hermetic
   self-test so the guard itself cannot drift or silently pass.
4. Single implementation, no bash/PowerShell duplication. The guard is a
   POSIX-sh script invoked by the hook and CI; it is **not** added to the
   format-lint manifest (which would force a PowerShell twin and a manifest
   version bump for marginal benefit).

## The Rule

A change set is either **substantive** or **exempt**:

- **Substantive paths** (require record updates): `crates/**`, `scripts/**`,
  `.github/**`, `.githooks/**`, `Cargo.toml`, `Cargo.lock`, `.agents/**`,
  `.claude/skills/**`, `agent/**`, `CLAUDE.md`, `AGENTS.md`, `GUARDRAILS.md`.
- **Exempt paths** (record update not required even if changed):
  `feature_list.json`, `claude-progress.md`, `docs/**`, `README.md`,
  `README.vi.md`, `session-handoff.md`, `init.sh`, `.gitignore`,
  `.gitattributes`, `.rtk/**`, `.codex/**`, `.claude/*.json` (settings,
  identity, ecc config).

**Guard decision:** if a change set contains at least one substantive path,
it must also contain both `feature_list.json` and `claude-progress.md`.
Otherwise it passes. An empty change set passes.

### Rationale for the exempt list

- The two records are exempt so a record-only commit (e.g. a docs/session
  update) does not require *itself*.
- `docs/**` holds the superpowers spec/plan trail — the process is already
  recorded there; forcing a `feature_list` entry for every plan edit would be
  noise.
- READMEs, `init.sh`, `.gitignore/.gitattributes`, and agent-tool config
  (`.rtk/**`, `.codex/**`, `.claude/*.json`) are docs/config, not behavior;
  they do not need a feature entry.
- Everything else — including `CLAUDE.md`/`AGENTS.md`/`GUARDRAILS.md`, which
  define the rules themselves — counts as substantive so that changing the
  workflow is itself a recorded change.

## The Guard: `scripts/check-records.sh`

A single POSIX-sh script (works under the hook's `#!/usr/bin/env sh` and on
Windows Git Bash) with three modes:

- `--staged` (default): apply the rule to `git diff --cached --name-only`.
  Used by the pre-commit hook; enforces that records land **in the same
  commit** as code (the only locally enforceable atomic rule).
- `--branch [base]`: apply the rule to the cumulative change set vs
  `origin/master` (merge-base `base...HEAD` plus untracked additions).
  Used by CI; a PR whose branch updates the records anywhere passes.
  `base` defaults to `origin/master`. If the base ref does not exist,
  exits 2 with a clear message.
- `--check`: apply the rule to a path list read from stdin, one per line.
  Used by the self-test; no git interaction, fully hermetic.

Exit codes: 0 = pass, 1 = fail (missing record updates), 2 = usage error.

Failure message lists which records are missing and which substantive path
triggered the requirement, so the fix is obvious.

### Mode semantics and edge cases

- `--staged` uses `git diff --cached --name-only` (default diff filters, so
  added/modified/renamed/deleted files all count). Untracked-but-unstaged
  files are not part of a commit and are ignored — the hook cannot fail on
  work the commit would not include anyway.
- `--branch` = `git diff --name-only base...HEAD` (three-dot, merge-base
  semantics) plus `git ls-files --others --exclude-standard` for untracked
  additions in the working tree.
- Empty change set → exit 0 (e.g. CI on a push-to-master where
  `origin/master == HEAD`; the pre-commit hook is the master-branch
  enforcement point, and every commit there already passed `--staged`).
- Paths are compared line-by-line, never word-split, so paths containing
  spaces are handled correctly.
- Matching uses POSIX `case` patterns: `docs/*` matches nested paths because
  `*` spans `/` in `case`.

## Enforcement Points

1. **`.githooks/pre-commit`**: run `scripts/check-records.sh --staged`
   first — it is fast (git + sh only) and fails before the slow cargo steps
   (fmt/clippy/tests). `git commit --no-verify` remains the documented escape
   hatch, as with the existing hooks.
2. **CI `checks` job**: add two steps after the format-lint smoke test:
   - `bash scripts/check-records.sh --branch` (the PR-level guard)
   - `bash scripts/test-check-records.sh` (self-test for the guard itself)

The guard is intentionally **not** a format-lint manifest step: that would
require a PowerShell twin (duplication we have been eliminating) and a
manifest version bump. The hook + CI cover both worlds with one sh script.

## Self-Test: `scripts/test-check-records.sh`

Hermetic and self-contained (no format-lint dependencies):

1. Unit tests of `--check` with piped path lists (no git needed):
   - substantive-only list → exit 1 with a message naming both records
   - substantive + both records → exit 0
   - substantive + one record → exit 1
   - exempt-only list (docs, README, records, config) → exit 0
   - empty list → exit 0
   - unknown flag → exit 2
2. Integration tests in a temporary git repo (created with `git init`, torn
   down by an EXIT trap):
   - `--staged`: stage a code file only → fail; stage records too → pass
   - `--branch`: two commits on a branch (code only, then records) → pass;
     a branch with code but no records vs base → fail; missing base ref →
     exit 2
3. Mirror check: `.agents/skills/guardrail/SKILL.md` vs
   `.claude/skills/guardrail/SKILL.md` byte-identical.

## Workflow Codification (the "flow and hardness" part)

- **`.agents/skills/guardrail/SKILL.md`** (canonical) + byte-identical
  `.claude/skills/guardrail/SKILL.md` mirror: a skill that states the
  mandatory workflow — superpowers flow (brainstorm → spec → plan → execute →
  verify → finish), verification-before-completion (evidence before claims,
  per the `verification-before-completion` skill), and the records rule with
  the guard's usage and failure-recovery steps. Fills the currently-empty
  guardrail stub.
- **`CLAUDE.md`**: new "Mandatory workflow" section pointing to the skill,
  the records rule, and the guard; the operating loop and completion gate
  already reference the records.
- **`GUARDRAILS.md`**: new "Record keeping" section with the hard rule.
- **`AGENTS.md`**: one line noting the record-keeping guard in the gate
  checklist.

## Records Update (this task itself demonstrates the rule)

- `feature_list.json`: add `vol-014` "Record-keeping guardrail (mandatory
  workflow + enforced records)" with status `passing`, verification steps
  (run `scripts/test-check-records.sh`; run the guard both modes; run the
  format-lint smoke test; run `cargo test`), and evidence. Bump
  `last_updated`; extend the `rules` block with
  `records_required_with_every_change: true`.
- `claude-progress.md`: Session 012 entry describing the guardrail landing.

## Out of Scope

- Adding the guard to the format-lint manifest / PowerShell gate (rejected:
  duplication + manifest bump for no enforcement gain).
- Pre-commit hook rewrites beyond the added check (ordering of the existing
  cargo steps is preserved).
- Enforcing the *process* steps themselves (brainstorm/spec/plan) —
  automation stops at "records updated"; the skill + CLAUDE.md codify the
  flow, which is the enforceable bound for a git-level guard.

## Verification

1. `bash scripts/test-check-records.sh` — all checks pass, exit 0.
2. `bash scripts/check-records.sh --staged` on this task's staged set —
   passes (it updates both records).
3. `bash scripts/check-records.sh --branch` in a temp repo — fail/pass
   scenarios per the self-test.
4. `bash scripts/test-format-lint.sh` — still 24/24 (no regression).
5. `bash scripts/format-lint.sh --skip-tests` — gate passes.
6. `cargo test` — green.
7. Mirror byte-identity via `cmp`.
