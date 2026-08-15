# Volume Control Orchestrator

This project uses a coordinator-led delivery pipeline:

1. `coordinator` — decomposes work, assigns ownership, orders handoffs, and enforces evidence.
2. `planner` — read-only discovery, documentation verification, and an executable plan.
3. `implementer` — workspace changes and tests within the approved file ownership.
4. `release_reviewer` — read-only adversarial review and release-gate decision.

The existing `explorer`, `docs_researcher`, and `reviewer` profiles remain available
for focused evidence gathering and independent review.

## Model routing

The intended model routing after the next Codex restart is:

| Role | Target model | Reasoning |
| --- | --- | --- |
| `coordinator` | `gpt-5.6-terra` (`ocx-gpt-5-6-terra`) | `medium` |
| `planner` | `gpt-5.6-terra` (`ocx-gpt-5-6-terra`) | `high` |
| `implementer` | `gpt-5.6-luna` (`ocx-gpt-5-6-luna`) | `max` |
| `release_reviewer` | `gpt-5.6-terra` (`ocx-gpt-5-6-terra`) | `high` |

The model fields are deliberately not active in the new profiles during the
current session because the runtime reported that the model catalog changed after
startup. This prevents stale model identifiers from being applied accidentally.
After restarting Codex, the comments in each profile are the exact fields to
activate if the runtime does not select them automatically.

## Cost/quality policy

- `quick path` (small docs, formatting, isolated low-risk fixes): use the existing
  read-only explorer/docs researcher and the existing quick reviewer. Do not spend
  Terra High or Luna Max unless the change affects behavior.
- `feature path` (UI, IPC, shortcuts, configuration, persistence, or platform code):
  coordinator (Terra Medium) → planner/spec (Terra High) → implementer (Luna Max)
  → release review (Terra High).
- `release path` (packaging, migration, crash, security, or cross-platform claims):
  always use the full feature path and the complete quality gate.
- The coordinator may skip a stage only when it records the reason and the risk is
  demonstrably low. It must never skip release review for a release-bound change.
- `max_threads = 4` and `max_depth = 2` cap concurrent/nested work so a mistaken
  delegation cannot multiply model usage. Shared-file changes remain serialized.

## Handoff contract

- Planner cites the files and symbols it inspected and records acceptance criteria.
- Coordinator records task ownership, dependency order, parallelization decisions,
  and the final handoff state.
- Implementer changes only the approved scope, keeps `feature_list.json` and
  `claude-progress.md` synchronized when applicable, and reports test evidence.
- Release reviewer checks the full diff and quality gate, then returns findings by
  severity. A release is blocked by a crash, data-loss risk, security regression,
  missing required test, or an unverified platform claim.
- The parent agent owns integration and the final commit. Agents do not reset the
  worktree or overwrite unrelated edits.

## Required verification

The release reviewer expects the repository gate to be run before handoff:

```text
cargo fmt --all --check
git diff --check
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo test --workspace --no-default-features
sh scripts/check-records.sh --staged
```

For the Windows-first Tauri work, attach UI/E2E artifacts and explicitly report
which Linux/macOS checks were executed or remain environment-blocked.
