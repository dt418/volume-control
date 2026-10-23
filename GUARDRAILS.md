# Guardrails

Hard rules that must not be bypassed without explicit user approval.

## Format and lint

- Every commit, PR, and merge must pass `cargo fmt --all --check`,
  `git diff --check`, and
  `cargo clippy --workspace --all-targets --no-default-features -- -D warnings`.
- Every commit, PR, and merge must pass
  `cargo test --workspace --no-default-features` through the full gate and
  CI. The lightweight pre-commit hook does not run the workspace test suite.
- Do not weaken `-D warnings` or suppress project warnings to force a green
  check; fix the code instead.
- Keep the pre-commit hook's fmt, whitespace, and clippy checks aligned with
  the checks it actually runs; the full gate and CI remain authoritative for
  workspace tests.

## Smoke testing & UI surface verification

- Any commit, PR, or ship that changes user-visible features, frontend code
  (`frontend/`), or UI surfaces (Mixer, Settings, Help, Overlay) must run the
  real desktop surface smoke test:
  `VOLUMECTL_E2E_DEBUG=1 pwsh -File scripts/verify-tauri-surfaces.ps1 -Binary target/debug/VolumeControl.exe`
  (or platform equivalent).
- The smoke test verifies process lifecycle, HWND visibility, legacy geometry
  parity, and non-blank client area rendering with evidence saved under
  `output/tauri-surface-evidence/`.
- Record updates in `feature_list.json` and `claude-progress.md` must include
  smoke test evidence for UI and feature changes.

## Branching, Worktree & Pull Request integration

- **Worktree by default**: All new feature and bugfix work must be developed
  in an isolated git worktree under `.worktrees/<feature-name>`, never directly
  on the root checkout or `main` branch.
- **No direct merges into main**: Direct local or remote merges into `main`
  (`git merge` while on `main`) are strictly prohibited upon task completion.
- **Mandatory Pull Request (PR)**: All completed tasks must be integrated via
  a GitHub Pull Request (`gh pr create`), gated by CI status checks and GitHub
  branch protection rulesets.
- **Protected main branch**: Branch protection rules are permanently active
  for `main` (deletion blocked, force-push blocked, pull request required).

## Diff hygiene

- Never commit forbidden paths: `target/`, `.superpowers/`,
  `.claude/worktrees/`, `config.json`, or `*.log` (see
  `scripts/ci-diff-check.sh`).

## Cross-platform Unix compatibility

- Conditionally-compiled parameters across target cfgs must be underscored
  (e.g. `_version: &str`) or suppressed with `let _ = ...;` so `-D warnings`
  does not break Unix builds with unused-variables.
- Signal handler threads on Unix must not loop with a single-iteration `break`
  over `signals.forever()`; use `if let Some(sig) = signals.forever().next()`.
- Multi-section surface E2E tests (`e2e/tauri/specs/settings.e2e.ts`) must
  explicitly navigate to the target section instead of assuming initial tab state.
- Run `bash scripts/check-unix-compat.sh` (or `powershell -File scripts/check-unix-compat.ps1`)
  when `src-tauri` or `e2e/tauri` changes.

## Record keeping

- Every commit, PR, and merge that changes substantive files (code, scripts,
  CI, hooks, skills) must also update both `feature_list.json` and
  `claude-progress.md`; see `scripts/check-records.sh`. Do not bypass with
  `--no-verify` without explicit user approval.

## Shipping

- The supported path to commit + push is `scripts/ship.sh` (or
  `scripts/ship.ps1` on Windows). It runs the records guard (branch +
  staged), the full format-lint gate with tests, both self-tests, the
  frontend build, the Tauri WebDriver E2E gate, and `tauri build
  --no-bundle` before committing; none of those can be skipped by any flag.
  `--force` only relaxes git-hygiene warnings, and `--dry-run` verifies
  without changing anything.
- Do not bypass ship with `--no-verify` without explicit user approval.
