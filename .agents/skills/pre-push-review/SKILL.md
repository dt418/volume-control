---
name: pre-push-review
description: Adversarial three-domain review of the volume-control enforcement stack before every commit/push. Use before pushing (after substantive changes to tooling or the guards), and as the mandatory review phase of the guardrail flow. Codifies the guard-core / gate-chain / wiring-records domains with concrete hunt-for checklists.
---

# Pre-Push Review (three domains)

The enforcement stack (records guard, format-lint gates, ship flow, CI, skill
mirrors, records) only stays honest if it is adversarially re-checked before
every push. This skill codifies the three-domain review into reusable
checklists so every push gets the same pass. It is the mandatory review phase
of the guardrail flow: brainstorm -> spec -> plan -> execute -> verify ->
**review** -> finish.

## When to run

- Before every push (the guardrail rule: a feature is `passing` only with
  recorded evidence; the review is the final adversarial pass over that
  evidence).
- Always when the change set touches the enforcement stack: `scripts/`,
  `.githooks/`, `.github/`, skills, `feature_list.json`, `claude-progress.md`,
  `CLAUDE.md`/`GUARDRAILS.md`/`AGENTS.md`.
- Ship flow: run the review before `scripts/ship.sh --push` (or
  `scripts/ship.ps1 -Push`).

## The three domains

Dispatch ONE reviewer per domain, in parallel (see
`dispatching-parallel-agents`): each gets its own files, its own checklist,
and no shared state. Each reviewer hunts for GENUINE defects only —
off-by-one/boundary errors, empty/null input, ordering and init races, state
desync, resource leaks, and cases the code clearly intends but misses — and
reports findings with severity, not style nits.

### Domain A — Guard core (records guard + its self-test)

Files: `scripts/check-records.sh`, `scripts/test-check-records.sh`,
`.githooks/pre-commit` (records invocation).

Hunt for:

- **dash/POSIX compatibility**: no `local`, no bashisms, `set -u` edge cases
  (unset-variable expansions, `${x:-}` guards), heredoc behavior.
- **Empty change set passes** (the `[ -z "$path" ] && continue` guard): nothing
  changed -> nothing required.
- **Fail-closed classification**: ANY non-exempt path requires records;
  unclassified paths count as substantive; exempt list is exactly the rule.
- **No fail-open on git errors**: every git invocation checked; failures exit
  non-zero with the diagnostic. Success captures discard stderr (`2>/dev/null`)
  so CRLF-style warnings cannot become "unclassified paths"; the error branch
  re-runs `2>&1` for the diagnostic.
- **Recovery templates**: quoted `<<'EOF'` heredocs (nothing expands), hint
  only the missing record(s), never auto-create/edit the records, no hint on
  pass. The self-test must assert the hint contract on the REAL paths
  (`--staged`/`--branch` integration), not only `--check`.
- **Self-test coverage**: would a regression that hides templates,
  auto-writes records, drops the pre-commit hook invocation, or opens a
  fail-open path fail the suite?

### Domain B — Gate chain (manifest + both gates + smoke)

Files: `scripts/format-lint-steps.json`, `scripts/format-lint.sh`,
`.agents/skills/format-lint/scripts/format-lint.ps1`,
`scripts/test-format-lint.sh`.

Hunt for:

- **Manifest single source of truth**: version check on BOTH gates; both
  parsers (line-oriented sed vs `ConvertFrom-Json`) agree; unknown
  version/step/internal-id fails loudly.
- **PowerShell $LASTEXITCODE discipline**: it is runspace-global and NOT reset
  by cmdlets — capture immediately after each native command in multi-command
  steps; `$script:stepFailed` semantics; fail-closed when `$LASTEXITCODE` is
  `$null`; a trailing cmdlet must not mask a native failure.
- **bash bridge**: `Get-Bash` never returns the WSL shim
  (`System32\bash.exe`); prefers the git-adjacent bash (walk-up for
  `bin\bash.exe`); missing bash degrades to a step-level failure, not a
  startup abort.
- **Flag transforms** (`--fix`/`--all-features`) match between gates (header
  parity, not just exit codes).
- **Smoke test completeness**: clean-index precondition, records-step parity,
  staged-scratch negatives on BOTH gates, pre-commit hook wiring assertions
  (comment-aware/line-anchored), manifest v3/6-step assertions, forbidden
  patterns verified per-pattern on both parsers.
- **Mirror byte-identity**: `.claude/skills/format-lint/scripts/*` equal the
  canonical gates.

### Domain C — Wiring, skills, docs, records consistency

Files: `.github/workflows/ci.yml`, `.githooks/pre-commit`, `scripts/ship.sh`,
`scripts/ship.ps1`, `.agents/skills/*` + `.claude/skills/*`, `feature_list.json`,
`claude-progress.md`, `CLAUDE.md`, `GUARDRAILS.md`, `AGENTS.md`.

Hunt for:

- **CI runs what hooks/gates run**: the checks job covers fmt, clippy, diff,
  records `--branch`, and every self-test; the windows job runs the bash-gated
  self-tests with `shell: bash`.
- **Line-ending integrity**: `.gitattributes` pins `*.sh` to LF; the
  extensionless hook stays LF; ps1 files LF-normalized in the index.
- **Skill mirrors byte-identical** (`.agents` <-> `.claude`) for every skill.
- **feature_list.json honesty**: entries carry verification + evidence that
  match reality — check counts must equal the self-tests' ACTUAL current
  output (at the time of writing: records 24, format-lint 33, ship 21; if
  any self-test has grown, the count is stale and so is the evidence);
  statuses and `last_updated` reflect the actual landings; rules extended
  only deliberately.
- **claude-progress.md**: a session entry exists for every landing; follow-ups
  are recorded; nothing claims unverified evidence.
- **Documentation matches enforcement**: `CLAUDE.md`/`GUARDRAILS.md`/`AGENTS.md`
  mandatory-workflow sections agree with what the hooks/gates/ship actually
  enforce.
- **ship flow**: no bypass flags exist; `ship.ps1` bridges without duplicating
  rule logic; the ship wiring test would catch a dropped hard check.
- **Records rule holds**: the change set carries both records
  (`check-records.sh --branch` passes).

## Process

1. Dispatch the three domain reviewers in parallel (one prompt per domain,
   each self-contained with its file list and hunt-for checklist, plus the
   rule: fix genuine defects, report with severity).
2. Integrate: read each summary, fix every genuine defect by editing files
   directly, and for each fix verify it is LIVE — a negative test (stripped
   copy, warning-emitting wrapper, removed invocation) must fail the relevant
   self-test.
3. Re-run the full battery: `bash scripts/test-check-records.sh`,
   `bash scripts/test-format-lint.sh`, `bash scripts/test-ship.sh`, both
   format-lint gates, `cargo test`, then `sh scripts/check-records.sh
   --staged` on the staged set.
4. Only then commit and push (records land in the same commit, per the
   guardrail rule).

## Verification gate

The review is complete only when: every domain's findings are triaged, every
genuine defect is fixed with a live negative verification, the full battery
is green, and the change set passes `check-records.sh --branch`. Record the
review as evidence in `feature_list.json` and `claude-progress.md` before the
commit.
