# CLAUDE.md

You are working in the VolumeControl repository — a native multiplatform
volume controller (Rust / Cargo workspace) with global hotkeys, a system
tray, and an on-screen volume overlay. Windows is the fully implemented
target; macOS/Linux backends are compile-gated scaffolds.

Prioritize reliable completion, continuity across sessions, and explicit
verification over speed.

## Project facts

- Workspace root: `crates/volumectl` is the app (lib + bin).
- Audio abstraction: `crate::audio::AudioBackend` trait; Windows impl in
  `audio_windows.rs` (Win32 WASAPI via `windows-sys`).
- Hotkeys: Win32 `RegisterHotKey` in `hotkeys_win32.rs`; actions in
  `crate::hotkeys::HotkeyAction`.
- Config: JSON at user config dir, `crate::config` module.
- Shared logic: `crate::core` (threshold colors, step/clamp math).
- Non-Windows builds fall back to a CLI stub (`cli.rs`) using the
  `volumecontrol` crate.

## Operating Loop

At the start of every session:

1. Run `pwd` and confirm you are in the expected repository root.
2. Read `claude-progress.md`.
3. Read `feature_list.json`.
4. Review recent commits with `git log --oneline -5`.
5. Run `./init.sh` (or manually: `cargo build` then `cargo test`).
6. Check whether the baseline smoke or end-to-end path is already broken.

Then select exactly one unfinished feature and work only on that feature until
you either verify it or document why it is blocked.

## Rules

- One active feature at a time.
- Do not claim completion without runnable evidence.
- Do not rewrite the feature list to hide unfinished work.
- Do not remove or weaken tests just to make the task look complete.
- Use repository artifacts as the system of record.
- Windows-only code must be gated behind `#[cfg(target_os = "windows")]` so
  the crate still builds on macOS/Linux.
- `windows-sys` is preferred over the `windows` crate (low-level FFI, stable
  signatures). Verify FFI signatures against the crate source before use.

## Mandatory Workflow

Every task follows the superpowers flow and hardness:

1. Load the `guardrail` skill (and process skills: brainstorming before
   planning, writing-plans before code).
2. Brainstorm -> spec (`docs/superpowers/specs/`) -> plan
   (`docs/superpowers/plans/`) -> execute -> verify -> finish.
3. No completion claim without fresh verification evidence
   (verification-before-completion: run the command, read output, check the
   exit code, then claim).
4. Every substantive change (code, scripts, CI, hooks, skills, this file)
   updates BOTH `feature_list.json` and `claude-progress.md` in the same
   commit. `scripts/check-records.sh` enforces this in the pre-commit hook
   and CI. Never bypass with `--no-verify` unless the user explicitly asks.

## Required Files

- `feature_list.json`
- `claude-progress.md`
- `init.sh`
- `session-handoff.md` when a compact handoff is useful

## Completion Gate

A feature can move to `passing` only after the required verification succeeds
(`cargo build` / `cargo test` on the current target) and the result is
recorded in `claude-progress.md`.

## Before You Stop

1. Update the progress log.
2. Update the feature state.
3. Record what is still broken or unverified.
4. Commit once the repository is safe to resume.
5. Leave a clean restart path for the next session.

<!-- rtk-instructions v2 -->
# RTK (Rust Token Killer) - Token-Optimized Commands

## Golden Rule

**Always prefix commands with `rtk`**. If RTK has a dedicated filter, it uses it. If not, it passes through unchanged. This means RTK is always safe to use.

**Important**: Even in command chains with `&&`, use `rtk`:
```bash
# ❌ Wrong
git add . && git commit -m "msg" && git push

# ✅ Correct
rtk git add . && rtk git commit -m "msg" && rtk git push
```

## RTK Commands by Workflow

### Build & Compile (80-90% savings)
```bash
rtk cargo build         # Cargo build output
rtk cargo check         # Cargo check output
rtk cargo clippy        # Clippy warnings grouped by file (80%)
rtk tsc                 # TypeScript errors grouped by file/code (83%)
rtk lint                # ESLint/Biome violations grouped (84%)
rtk prettier --check    # Files needing format only (70%)
rtk next build          # Next.js build with route metrics (87%)
```

### Test (60-99% savings)
```bash
rtk cargo test          # Cargo test failures only (90%)
rtk go test             # Go test failures only (90%)
rtk jest                # Jest failures only (99.5%)
rtk vitest              # Vitest failures only (99.5%)
rtk playwright test     # Playwright failures only (94%)
rtk pytest              # Python test failures only (90%)
rtk rake test           # Ruby test failures only (90%)
rtk rspec               # RSpec test failures only (60%)
rtk test <cmd>          # Generic test wrapper - failures only
```

### Git (59-80% savings)
```bash
rtk git status          # Compact status
rtk git log             # Compact log (works with all git flags)
rtk git diff            # Compact diff (80%)
rtk git show            # Compact show (80%)
rtk git add             # Ultra-compact confirmations (59%)
rtk git commit          # Ultra-compact confirmations (59%)
rtk git push            # Ultra-compact confirmations
rtk git pull            # Ultra-compact confirmations
rtk git branch          # Compact branch list
rtk git fetch           # Compact fetch
rtk git stash           # Compact stash
rtk git worktree        # Compact worktree
```

Note: Git passthrough works for ALL subcommands, even those not explicitly listed.

### GitHub (26-87% savings)
```bash
rtk gh pr view <num>    # Compact PR view (87%)
rtk gh pr checks        # Compact PR checks (79%)
rtk gh run list         # Compact workflow runs (82%)
rtk gh issue list       # Compact issue list (80%)
rtk gh api              # Compact API responses (26%)
```

### JavaScript/TypeScript Tooling (70-90% savings)
```bash
rtk pnpm list           # Compact dependency tree (70%)
rtk pnpm outdated       # Compact outdated packages (80%)
rtk pnpm install        # Compact install output (90%)
rtk npm run <script>    # Compact npm script output
rtk npx <cmd>           # Compact npx command output
rtk prisma              # Prisma without ASCII art (88%)
rtk uv run <cmd>        # Compact uv project command output
```

### Files & Search (60-75% savings)
```bash
rtk ls <path>           # Tree format, compact (65%)
rtk read <file>         # Code reading with filtering (60%)
rtk grep <pattern>      # Search grouped by file (75%). Format flags (-c, -l, -L, -o, -Z) run raw.
rtk find <pattern>      # Find grouped by directory (70%)
```

### Analysis & Debug (70-90% savings)
```bash
rtk err <cmd>           # Filter errors only from any command
rtk log <file>          # Deduplicated logs with counts
rtk json <file>         # JSON structure without values
rtk deps                # Dependency overview
rtk env                 # Environment variables compact
rtk summary <cmd>       # Smart summary of command output
rtk diff                # Ultra-compact diffs
```

### Infrastructure (85% savings)
```bash
rtk docker ps           # Compact container list
rtk docker images       # Compact image list
rtk docker logs <c>     # Deduplicated logs
rtk kubectl get         # Compact resource list
rtk kubectl logs        # Deduplicated pod logs
```

### Network (65-70% savings)
```bash
rtk curl <url>          # Compact HTTP responses (70%)
rtk wget <url>          # Compact download output (65%)
```

### Meta Commands
```bash
rtk gain                # View token savings statistics
rtk gain --history      # View command history with savings
rtk discover            # Analyze Claude Code sessions for missed RTK usage
rtk proxy <cmd>         # Run command without filtering (for debugging)
rtk init                # Add RTK instructions to CLAUDE.md
rtk init --global       # Add RTK to ~/.claude/CLAUDE.md
```

## Token Savings Overview

| Category | Commands | Typical Savings |
|----------|----------|-----------------|
| Tests | vitest, playwright, cargo test | 90-99% |
| Build | next, tsc, lint, prettier | 70-87% |
| Git | status, log, diff, add, commit | 59-80% |
| GitHub | gh pr, gh run, gh issue | 26-87% |
| Package Managers | pnpm, npm, npx | 70-90% |
| Files | ls, read, grep, find | 60-75% |
| Infrastructure | docker, kubectl | 85% |
| Network | curl, wget | 65-70% |

Overall average: **60-90% token reduction** on common development operations.
<!-- /rtk-instructions -->