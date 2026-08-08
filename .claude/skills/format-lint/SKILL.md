---
name: format-lint
description: Use when changing Rust code in volume-control, preparing a commit or PR, or investigating CI failures involving cargo fmt, Clippy, warnings, tests, or whitespace/diff checks.
---

# Format and lint volume-control

Run the repository's deterministic quality gate before claiming a Rust change
is ready. Prefer the bundled scripts so local checks and CI checks use the
same command order and warning policy on every platform.

## Standard command

From the repository root, run the gate for your platform (same steps, same
exit codes: 0 = passed, 1 = a step failed, 2 = bad usage):

Windows:

```powershell
powershell -ExecutionPolicy Bypass -File .agents/skills/format-lint/scripts/format-lint.ps1
```

Linux/macOS (also usable from any POSIX shell):

```bash
bash scripts/format-lint.sh
```

Both are kept in sync with each other and with CI. If one drifts, fix both
and the CI checks (`.github/workflows/ci.yml`, `.githooks/pre-commit`,
`scripts/ci-diff-check.sh`).

## Single source of truth

The gate's six steps (names, order, commands, the internal forbidden-path
and record-updates steps, and the `-SkipTests` skip rule) and the
forbidden-diff-path pattern list (`forbidden_patterns`) are defined once in
`scripts/format-lint-steps.json` (version 3). Both gates read that manifest
and execute it, so they cannot drift. To change the gate, edit the manifest
(bump its `version` if the schema changes); the flag switches only transform
the manifest's default steps (`-Fix` drops `--check` from the fmt step,
`-AllFeatures` swaps `--no-default-features` for `--all-features` on clippy
and test).

The `record updates` internal step enforces that staged substantive
changes also stage `feature_list.json` and `claude-progress.md`. The rule
lives ONCE in `scripts/check-records.sh`; the bash gate calls it directly
and the PowerShell gate bridges to it via Git Bash (resolved next to git),
so neither gate duplicates the exempt/substantive tables.

Manifest constraints: one step object per line and one `forbidden_patterns`
entry per line, both at 4-space indent, JSON values free of embedded quotes
(patterns escape literal backslashes as `\\`), and `args` entries must not
contain spaces or glob characters (both parsers split on whitespace). The manifest lives only at `scripts/` — the
skill copies of the gates read it from the repository root, so it is not
mirrored into `.claude/skills/format-lint/` or `.agents/skills/format-lint/`.

## Smoke test

After changing the manifest or either gate script (or its
`.claude/skills/format-lint/` mirror), run `bash scripts/test-format-lint.sh`
from the repository root. It asserts the exit codes and forbidden-path
handling of both gates (the PowerShell gate when `powershell`/`pwsh` is
available), that the mirror copies are byte-identical, and that the manifest
parses on both sides, so drift fails loudly instead of silently.

The default gate runs (mirroring the CI `checks` job and the pre-commit
hook, including the forbidden-diff-path gate from `scripts/ci-diff-check.sh`):

1. `cargo fmt --all --check`
2. `git diff --check HEAD` (whitespace hygiene, staged and unstaged)
3. Forbidden diff paths (`target/`, `.superpowers/`, `.claude/settings.local.json`, `.claude/worktrees/`, `config.json`, `*.log`)
4. Record updates: staged substantive changes must also stage `feature_list.json` + `claude-progress.md` (`scripts/check-records.sh --staged`)
5. `cargo clippy --workspace --all-targets --no-default-features -- -D warnings`
6. `cargo test --workspace --no-default-features`

Use `-Fix` when formatting changes are authorized:

```powershell
powershell -ExecutionPolicy Bypass -File .agents/skills/format-lint/scripts/format-lint.ps1 -Fix
```

Use `-SkipTests` / `--skip-tests` only when the user explicitly wants
format/lint without test execution. Use `-AllFeatures` / `--all-features`
only on a machine with the GTK4/libadwaita and optional layer-shell
development libraries installed; otherwise the default feature-light gate is
the correct check for this repository.

## Rust toolchain resolution

The script uses `cargo` from `PATH`. If the current process has a stale
environment snapshot, it falls back to `%USERPROFILE%\.cargo\bin\cargo.exe`.
`git` is resolved the same way, falling back to the standard Git for Windows
install locations. Both fail with a clear message when unavailable; do not
silently skip checks.

## Result handling

- Treat any non-zero command exit code as a failed gate.
- Treat compiler warnings as failures because Clippy uses `-D warnings`.
- Distinguish project warnings from dependency future-incompatibility notices;
  report both, but do not weaken the project warning policy to hide source
  warnings.
- If Linux cross-checking from Windows fails at `pkg-config`, report the
  missing Linux sysroot/system libraries and rely on the Ubuntu CI job for the
  native GTK/PulseAudio build.
- Report the exact failing command and its output before suggesting a fix.

## Common mistakes

- Running `cargo fmt` without `--all`, leaving workspace members unformatted.
- Running Clippy with default features when the GTK system packages are absent.
- Treating a successful `cargo fmt` as a successful lint/test gate.
- Continuing after a non-zero step and claiming the whole check passed.

After a `-Fix` run, inspect `git diff`, rerun the check-only command, and only
then stage or commit the formatted files.
