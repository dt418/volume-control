# Format-Lint Gate Toolchain Landing — Design

**Date:** 2026-08-08
**Status:** Approved for implementation planning

## Goal

Land the format-lint gate toolchain built this session — the manifest-driven
bash and PowerShell quality gates, their shared step manifest, the 24-check
smoke test, and the skill (with both mirrors) — and wire the smoke test into
CI so gate drift breaks the build on both Linux and Windows.

## Background

The repository's quality gate was previously duplicated logic in two scripts.
This session converged it into a single source of truth:

- `scripts/format-lint-steps.json` (v2) defines the 5 gate steps (fmt, diff,
  forbidden paths, clippy, test) and the 6 forbidden-diff-path patterns.
- `scripts/format-lint.sh` (bash) and
  `.agents/skills/format-lint/scripts/format-lint.ps1` (PowerShell) both read
  and execute that manifest; flags only transform its default steps
  (`--fix`/`-Fix` drops `--check` from fmt; `--all-features`/`-AllFeatures`
  swaps the feature token on clippy/test).
- `scripts/test-format-lint.sh` asserts both gates' exit codes, forbidden-path
  handling, flag transforms, manifest parse on both sides, per-pattern
  forbidden-path matching, and mirror byte-identity (24 checks).
- The skill is mirrored at `.claude/skills/format-lint/` (byte-identical).

All of the above is verified working locally (24/24 smoke checks; both full
gates pass with the test suite) and is currently **uncommitted**.

## Scope

### Included — Commit 1 (toolchain)

| File | State on disk |
|---|---|
| `.gitignore` | modified (cleanup; `/Cargo.lock` un-ignored) |
| `Cargo.lock` | untracked (57 KB, generated offline, MSRV-aware) |
| `scripts/format-lint-steps.json` | untracked (manifest v2) |
| `scripts/format-lint.sh` | untracked (bash gate) |
| `scripts/test-format-lint.sh` | untracked (smoke test, executable) |
| `.agents/skills/format-lint/` | untracked (SKILL.md + scripts/format-lint.ps1) |
| `.claude/skills/format-lint/` | untracked (mirror, byte-identical) |

### Included — Commit 2 (CI wiring)

`.github/workflows/ci.yml`, two one-step additions:

1. **`checks` job (ubuntu-24.04)**, immediately after the existing
   `Test (no GTK features)` step:

   ```yaml
         - name: Smoke-test format-lint gates
           run: bash scripts/test-format-lint.sh
   ```

2. **`windows` job (windows-latest)**, immediately after the existing
   `Test` step. `shell: bash` is required because windows job steps default
   to pwsh:

   ```yaml
         - name: Smoke-test format-lint gates
           shell: bash
           run: bash scripts/test-format-lint.sh
   ```

### Out of scope

- Changes to `.githooks/pre-commit` (a separate decision).
- Any Rust source changes.

## Design decisions

- **Commit strategy:** two logical commits (toolchain, then CI), each
  independently green, per the approved Approach 1.
- **CI placement:** both `checks` (ubuntu — bash gate only; PowerShell is not
  present on ubuntu runners and the smoke test skips it gracefully) and
  `windows` (both gates; Git Bash and PowerShell are both present).
- **CI precondition (verified):** the smoke test refuses to run if
  `.claude/settings.local.json` is missing or modified. That file is tracked
  in the repository, so a fresh CI checkout has it present and clean.
- **Known accepted cost:** on the ubuntu `checks` job the smoke test's
  `--all-features` gate run attempts a clippy build that fails fast at
  `gtk4-sys` pkg-config (no GTK dev libs installed there). The smoke test
  asserts headers, not exit codes, so it still passes; the cost is a few
  wasted seconds per run.

## Verification (definition of done)

1. `bash scripts/test-format-lint.sh` → all 24 checks pass, exit 0.
2. `bash scripts/format-lint.sh` and
   `powershell -NoProfile -ExecutionPolicy Bypass -File .agents/skills/format-lint/scripts/format-lint.ps1`
   → both full gates pass with tests, exit 0.
3. Mirror byte-identity holds: `cmp -s` on both gate scripts and SKILL.md
   between `.agents/skills/format-lint/` and `.claude/skills/format-lint/`.
4. `ci.yml` parses as valid YAML; the two new steps reference files committed
   in Commit 1.
5. `git status` after each commit contains only the intended files.

## Global constraints (for the implementation plan)

- Manifest version is `2`; both gates reject any other version loudly.
- `scripts/format-lint-steps.json` layout: one step object and one
  `forbidden_patterns` entry per line at 4-space indent; JSON values free of
  embedded quotes; `args` entries contain no spaces or glob characters.
- The smoke test must never require network, GTK dev libraries, or modify
  tracked files permanently (it restores `.claude/settings.local.json`
  byte-for-byte via its EXIT trap).
- No Rust code changes; no pre-commit hook changes.
