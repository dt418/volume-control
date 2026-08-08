# AGENTS.md

## Format and lint gate

Run the full quality gate before committing or opening a PR:

1. `cargo fmt --all --check`
2. `git diff --check`
3. `cargo clippy --workspace --all-targets --no-default-features -- -D warnings`
4. `cargo test --workspace --no-default-features`
5. `sh scripts/check-records.sh --staged` (record-keeping guard; the
   pre-commit hook runs this automatically)

Install the pre-commit hook so format and lint checks run automatically on
every commit:

- `scripts/install-hooks.sh` (POSIX shells, WSL)
- `scripts/install-hooks.ps1` (PowerShell, Windows)

The hook lives in `.githooks/pre-commit` and runs `cargo fmt --all --check`,
`git diff --cached --check`, and clippy with `-D warnings`. Use
`git commit --no-verify` only when the user explicitly requests it.

The pre-commit hook is intentionally lightweight: it runs the records guard,
format, cached whitespace, and clippy checks. The full workspace test suite is
enforced by the full gate and CI, not by every local commit hook invocation.

CI enforces the same checks in `.github/workflows/ci.yml`. Never weaken the
`-D warnings` policy or hide project warnings to make a check pass. See
`GUARDRAILS.md` for the hard rules.
