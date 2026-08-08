# Guardrails

Hard rules that must not be bypassed without explicit user approval.

## Format and lint

- Every commit, PR, and merge must pass `cargo fmt --all --check`,
  `git diff --check`, and
  `cargo clippy --workspace --all-targets --no-default-features -- -D warnings`.
- Every commit, PR, and merge must pass
  `cargo test --workspace --no-default-features`.
- Do not weaken `-D warnings` or suppress project warnings to force a green
  check; fix the code instead.
- Keep `.githooks/pre-commit` in sync with the CI checks so local and remote
  gates agree.

## Diff hygiene

- Never commit forbidden paths: `target/`, `.superpowers/`,
  `.claude/worktrees/`, `config.json`, or `*.log` (see
  `scripts/ci-diff-check.sh`).

## Record keeping

- Every commit, PR, and merge that changes substantive files (code, scripts,
  CI, hooks, skills) must also update both `feature_list.json` and
  `claude-progress.md`; see `scripts/check-records.sh`. Do not bypass with
  `--no-verify` without explicit user approval.
