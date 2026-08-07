---
name: format-lint
description: Use when changing Rust code in volume-control, preparing a commit or PR, or investigating CI failures involving cargo fmt, Clippy, warnings, tests, or whitespace/diff checks.
---

# Format and lint volume-control

Run the repository's deterministic quality gate before claiming a Rust change
is ready. Prefer the bundled script so local Windows checks and CI checks use
the same command order and warning policy.

## Standard command

From the repository root, run:

```powershell
powershell -ExecutionPolicy Bypass -File .agents/skills/format-lint/scripts/format-lint.ps1
```

The default gate runs:

1. `cargo fmt --all --check`
2. `git diff --check`
3. `cargo clippy --workspace --all-targets --no-default-features -- -D warnings`
4. `cargo test --workspace --no-default-features`

Use `-Fix` when formatting changes are authorized:

```powershell
powershell -ExecutionPolicy Bypass -File .agents/skills/format-lint/scripts/format-lint.ps1 -Fix
```

Use `-SkipTests` only when the user explicitly wants format/lint without test
execution. Use `-AllFeatures` only on a machine with the GTK4/libadwaita and
optional layer-shell development libraries installed; otherwise the default
feature-light gate is the correct check for this repository.

## Rust toolchain resolution

The script uses `cargo` from `PATH`. If the current process has a stale
environment snapshot, it falls back to `%USERPROFILE%\.cargo\bin\cargo.exe`.
It fails clearly when neither location exists; do not silently skip checks.

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
