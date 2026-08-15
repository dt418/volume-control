# ECC for Codex CLI

This supplements the root `AGENTS.md` with a repo-local ECC baseline.

## Repo Skill

- Repo-generated Codex skill: `.agents/skills/volume-control/SKILL.md`
- Claude-facing companion skill: `.claude/skills/volume-control/SKILL.md`
- Keep user-specific credentials and private MCPs in `~/.codex/config.toml`, not in this repo.

## MCP Baseline

Treat `.codex/config.toml` as the default ECC-safe baseline for work in this repository.
The generated baseline enables GitHub, Context7, Exa, Memory, Playwright, and Sequential Thinking.

## Multi-Agent Support

- Coordinator: delivery decomposition, ownership, handoffs, and evidence enforcement
- Planner: read-only discovery and executable delivery plans
- Implementer: approved workspace changes, tests, and record synchronization
- Release reviewer: read-only correctness, security, UI/UX, performance, and cross-platform gate review
- Explorer: read-only evidence gathering
- Reviewer: correctness, security, and regression review
- Docs researcher: API and release-note verification

The coordinator pipeline profiles are configured in `.codex/config.toml` and must
remain relative files under `.codex/agents/`. Validate the complete contract with
`python scripts/test-codex-config.py` before handing work to the shared checks job.

## Workflow Files

- No dedicated workflow command files were generated for this repo.

Use these workflow files as reusable task scaffolds when the detected repository workflows recur.
