---
name: guardrail
description: Mandatory workflow and record-keeping rules for volume-control - every task follows the superpowers flow, verification-before-completion, and updates feature_list.json + claude-progress.md. Use before starting any task in this repository.
---

# Guardrail: Mandatory Workflow and Records

## The Rule

Every task in this repository MUST:

1. **Follow the superpowers flow**: brainstorm (spec) → plan → execute →
   verify → finish. Process skills come first (brainstorming, then
   writing-plans), then implementation skills. Do not skip to code.
2. **Apply hardness (verification-before-completion)**: no completion claim
   without fresh verification evidence. Run the full verification command in
   this message, read the output, check the exit code, then claim. Evidence
   before assertions, always.
3. **Update the records with every substantive change**: any change set that
   touches code (crates/, scripts/, .github/, .githooks/, Cargo.toml,
   Cargo.lock, skills, CLAUDE.md/AGENTS.md/GUARDRAILS.md) must also update
   BOTH `feature_list.json` (a feature entry with verification + evidence,
   or a status update to an existing entry) and `claude-progress.md` (a
   session entry). Records land in the same commit as the code.

## The Guard

`scripts/check-records.sh` enforces the records rule:

- Pre-commit hook runs `--staged`: committing code without staged record
  updates fails fast.
- Both format-lint gates run `--staged` as the manifest v3 `record updates`
  step (the bash gate calls the script directly; the PowerShell gate bridges
  to it via Git Bash).
- CI runs `--branch`: a PR whose branch never updates the records fails.
- `--check` reads a path list from stdin (used by the self-test).

When the guard fails: it prints a copy-paste template for each missing
record (`feature_list.json` entry shape + `claude-progress.md` session
entry) — fill in the blanks, add both to the same change set, re-stage,
and commit. The guard only suggests; it never creates or edits the records
itself. Never bypass with `--no-verify` except when the user explicitly
requests it.

## Ship (mandatory before any commit + push)

`scripts/ship.sh` (or `scripts/ship.ps1` on Windows, which bridges to it)
is the supported path to commit + push. It runs, in order, and none of
these can be skipped by any flag:

1. `check-records.sh --branch origin/master` — records updated somewhere
   in the change set.
2. `scripts/format-lint.sh` — the FULL gate, tests included.
3. `scripts/test-check-records.sh` and `scripts/test-format-lint.sh` — the
   guardrails themselves must be intact.
4. After staging, `check-records.sh --staged` — the exact commit set.

Then it stages, commits, and (with `--push`) pushes. `--dry-run` verifies
without changing anything; `--force` relaxes ONLY git-hygiene warnings
(never the hard checks above). There are no `--skip-*` flags. Bypassing
ship (e.g. `git commit --no-verify`) requires explicit user approval.

## Exempt (no record update required)

`feature_list.json`, `claude-progress.md`, `docs/**`, `README.md`,
`README.vi.md`, `session-handoff.md`, `init.sh`, `.gitignore`,
`.gitattributes`, `.rtk/**`, `.codex/**`, `.claude/*.json`.

## Verification

After the guard passes, run the full battery: `bash scripts/format-lint.sh`
(or `--skip-tests` for speed), `cargo test`, and the self-tests
(`bash scripts/test-format-lint.sh`, `bash scripts/test-check-records.sh`)
before claiming completion.
