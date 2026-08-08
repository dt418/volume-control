# Record-Keeping Guardrail Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enforce that every substantive change set updates both `feature_list.json` and `claude-progress.md`, codify the superpowers flow + hardness as the mandatory workflow, and guard it locally (pre-commit) and in CI.

**Architecture:** A single POSIX-sh guard `scripts/check-records.sh` with three modes (`--staged`, `--branch`, `--check`) applies one rule: a change set containing any substantive path must also contain both records. The pre-commit hook calls `--staged` (records must land in the same commit); CI calls `--branch` (records anywhere in the PR). A hermetic self-test `scripts/test-check-records.sh` guards the guard. The workflow is codified in a `guardrail` skill (mirrored `.agents`/`.claude`) plus `CLAUDE.md`, `GUARDRAILS.md`, `AGENTS.md`. This task itself updates the records (`vol-014`, Session 012) — demonstrating the rule.

**Tech Stack:** POSIX sh (hook-compatible, Windows Git Bash-safe), git plumbing (`git diff --cached`, `git diff base...HEAD`, `git ls-files --others`), bash for the self-test.

## Global Constraints

- Guard script MUST be POSIX sh (works under `#!/usr/bin/env sh` and Git Bash); no bashisms, no external tools beyond `git`, `sh`, `grep`, `sed`.
- Substantive paths: `crates/**`, `scripts/**`, `.github/**`, `.githooks/**`, `Cargo.toml`, `Cargo.lock`, `.agents/**`, `.claude/skills/**`, `agent/**`, `CLAUDE.md`, `AGENTS.md`, `GUARDRAILS.md`.
- Exempt paths: `feature_list.json`, `claude-progress.md`, `docs/**`, `README.md`, `README.vi.md`, `session-handoff.md`, `init.sh`, `.gitignore`, `.gitattributes`, `.rtk/**`, `.codex/**`, `.claude/*.json`.
- Exit codes: 0 pass, 1 fail, 2 usage error. `--staged` is the default mode.
- Failure message MUST name the missing records and the triggering substantive path.
- Do NOT add the guard to `scripts/format-lint-steps.json` or the PowerShell gate (duplication, rejected in the spec).
- The `guardrail` skill files MUST be byte-identical between `.agents/skills/guardrail/SKILL.md` and `.claude/skills/guardrail/SKILL.md`.
- Do not change the order of existing cargo steps in `.githooks/pre-commit`; add the records check before them.
- Commit the whole change set (code + records together) as ONE commit per the rule. Tasks 1-7 therefore do NOT commit individually (the hook activated in Task 2 would reject code-only commits); they accumulate the change set, and Task 8 commits everything once, together with the records from Task 6.

---

### Task 1: Write the guard `scripts/check-records.sh`

**Files:**
- Create: `scripts/check-records.sh`

**Interfaces:**
- Consumes: nothing (standalone).
- Produces: exit 0/1/2 as specified; `--check` reads paths from stdin one per line; `--branch [base]` accepts an optional base ref (default `origin/master`).

- [ ] **Step 1: Write the script**

```sh
#!/usr/bin/env sh
#
# check-records.sh - enforce that substantive changes update the repository
# records (feature_list.json + claude-progress.md).
#
# Rule: if the change set contains at least one substantive path, it must
# also contain BOTH records. Exempt paths (docs, READMEs, config, the
# records themselves) never require record updates.
#
# Modes:
#   --staged             apply the rule to `git diff --cached --name-only`
#                        (default; used by the pre-commit hook)
#   --branch [base]      apply the rule to the cumulative change set vs base
#                        (merge-base base...HEAD + untracked; used by CI)
#   --check              apply the rule to a path list from stdin (self-test)
#
# Exit: 0 = pass, 1 = fail (missing record updates), 2 = usage error.
set -u

# ---- Rule tables -----------------------------------------------------------
# Fail-closed: ANY path not exempt is treated as substantive and requires
# record updates. Matching is POSIX case, so `docs/*`-style globs span
# slashes. Substantive paths are therefore the complement of this list:
# crates/*, scripts/*, .github/*, .githooks/*, agent/*, .agents/*,
# .claude/skills/*, Cargo.toml, Cargo.lock, CLAUDE.md, AGENTS.md,
# GUARDRAILS.md, and any unclassified path.
exempt_hit() { # $1 = path
    case "$1" in
        feature_list.json|claude-progress.md) return 0 ;;
        docs/*|README.md|README.vi.md|session-handoff.md|init.sh) return 0 ;;
        .gitignore|.gitattributes|.rtk/*|.codex/*|.claude/*.json) return 0 ;;
    esac
    return 1
}

# ---- Rule evaluation -------------------------------------------------------
# Reads a path list from stdin, one per line, into a newline-joined LIST var
# (never word-split, so paths containing spaces are handled correctly).
collect_list() {
    LIST=""
    while IFS= read -r path || [ -n "$path" ]; do
        [ -z "$path" ] && continue
        LIST="${LIST}${LIST:+
}$path"
    done
}

# Decide on a collected LIST. Sets has_records (1 = both present), needs_records
# (1 = a non-exempt path present), trigger (first such path).
decide() {
    has_feature=0; has_progress=0; needs_records=0; trigger=""
    while IFS= read -r path; do
        case "$path" in
            feature_list.json) has_feature=1 ;;
            claude-progress.md) has_progress=1 ;;
        esac
        if ! exempt_hit "$path"; then
            # Any non-exempt path (substantive or unclassified) requires records.
            needs_records=1
            [ -z "$trigger" ] && trigger="$path"
        fi
    done <<EOF
$LIST
EOF
    if [ "$has_feature" -eq 1 ] && [ "$has_progress" -eq 1 ]; then
        has_records=1
    else
        has_records=0
    fi
}

# Report pass/fail for a collected LIST with a mode-specific failure header.
# No `local` (dash-compatible): state lives in globals set by decide().
report() { # <fail_header_line1> [fail_header_line2]
    header1="$1"
    header2="${2:-}"
    if [ "$needs_records" -eq 0 ] || [ "$has_records" -eq 1 ]; then
        return 0
    fi
    missing=""
    [ "$has_feature" -eq 0 ] && missing="$missing feature_list.json"
    [ "$has_progress" -eq 0 ] && missing="$missing claude-progress.md"
    echo "FAIL - $header1" >&2
    [ -n "$header2" ] && echo "      $header2" >&2
    echo "      trigger: $trigger; missing:$missing" >&2
    return 1
}

# ---- Mode: --check (stdin) --------------------------------------------------
check_stdin() {
    collect_list
    decide
    report "substantive change requires record updates"
}

# ---- Mode: --staged ----------------------------------------------------------
check_staged() {
    collect_list <<EOF
$(git diff --cached --name-only 2>/dev/null)
EOF
    decide
    report "this commit changes substantive files but not the records" \
           "stage updates to feature_list.json and claude-progress.md with this change"
}

# ---- Mode: --branch -----------------------------------------------------------
check_branch() {
    base="${1:-origin/master}"
    if ! git rev-parse --verify --quiet "$base" >/dev/null; then
        echo "FAIL - base ref '$base' not found; pass one explicitly (e.g. origin/master)" >&2
        return 2
    fi
    collect_list <<EOF
$( { git diff --name-only "$base...HEAD" 2>/dev/null; git ls-files --others --exclude-standard; } )
EOF
    decide
    report "the branch change set touches substantive files but not the records"
}

# ---- Main -------------------------------------------------------------------
mode="${1:---staged}"
case "$mode" in
    --staged) check_staged ;;
    --branch) shift; check_branch "${1:-origin/master}" ;;
    --check) check_stdin ;;
    -h|--help)
        echo "usage: $0 [--staged|--branch [base]|--check]" >&2
        exit 0
        ;;
    *)
        echo "unknown mode: $mode" >&2
        echo "usage: $0 [--staged|--branch [base]|--check]" >&2
        exit 2
        ;;
esac
```

- [ ] **Step 2: Sanity-check the script**

Run: `sh -n scripts/check-records.sh`
Expected: no output, exit 0 (syntax valid).

Run: `printf 'crates/volumectl/src/app.rs\n' | sh scripts/check-records.sh --check; echo rc=$?`
Expected: exit 1, prints `FAIL - substantive change (trigger: crates/...) requires record updates for: feature_list.json claude-progress.md`.

Run: `printf 'feature_list.json\nclaude-progress.md\ncrates/x\n' | sh scripts/check-records.sh --check; echo rc=$?`
Expected: exit 0.

Run: `printf 'docs/superpowers/plans/x.md\n' | sh scripts/check-records.sh --check; echo rc=$?`
Expected: exit 0 (exempt).

- [ ] **Step 3: Leave uncommitted**

Do NOT commit yet — the whole change set commits once in Task 8, together with the records from Task 6 (per the ONE-commit global constraint).

---

### Task 2: Wire the guard into the pre-commit hook

**Files:**
- Modify: `.githooks/pre-commit` (add the records check before the cargo steps)

**Interfaces:**
- Consumes: `scripts/check-records.sh --staged`.
- Produces: a commit that stages code without the records now fails fast with a clear message.

- [ ] **Step 1: Add the check**

Insert immediately after the `set -e` line and before the "Resolve cargo" block:

```sh
# Record-keeping guard: substantive changes must stage updates to
# feature_list.json and claude-progress.md (see scripts/check-records.sh).
echo "[pre-commit] check record updates (feature_list.json + claude-progress.md)"
if ! sh scripts/check-records.sh --staged; then
  exit 1
fi
```

- [ ] **Step 2: Verify the hook fails correctly**

Run: `git add scripts/check-records.sh && git commit -m "temp" --no-verify` — expect the hook to fail because this staged set touches `scripts/**` (substantive) without the records. (Use `--no-verify` ONLY to observe the failure without committing; then `git reset` the staged files. The records arrive in Task 6.)

Run: `git reset` to unstage, then `git add scripts/check-records.sh feature_list.json claude-progress.md` (records updated in Task 6) and `git commit` — expect pass (this is the real Task 8 commit path).

- [ ] **Step 3: Leave uncommitted**

Do NOT commit the hook change alone — it is part of the single Task 8 commit.

---

### Task 3: Write the hermetic self-test `scripts/test-check-records.sh`

**Files:**
- Create: `scripts/test-check-records.sh`

**Interfaces:**
- Consumes: `scripts/check-records.sh` (all three modes).
- Produces: exit 0 = all checks pass; used by CI. Prints `ok`/`FAIL` lines and a final summary.

- [ ] **Step 1: Write the self-test**

```bash
#!/usr/bin/env bash
#
# test-check-records.sh - hermetic self-test for scripts/check-records.sh.
# Runs the --check unit tests (no git) and --staged/--branch integration
# tests in a temporary git repo, plus the guardrail-skill mirror check.
#
# Usage: bash scripts/test-check-records.sh
# Exit: 0 = all checks passed; 1 = at least one check failed.
set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."   # repository root

failures=0
report() { # <ok|FAIL> <description> [detail]
    local status="$1" desc="$2" detail="${3:-}"
    if [ "$status" = ok ]; then
        printf 'ok   - %s\n' "$desc"
    else
        printf 'FAIL - %s%s\n' "$desc" "${detail:+ ($detail)}"
        failures=$((failures + 1))
    fi
}

guard=scripts/check-records.sh

# --- unit tests: --check with piped lists (no git) ---------------------------
check_rc() { # <expected_rc> <stdin_list> <description>
    local expected="$1" list="$2" desc="$3" rc
    printf '%b' "$list" | sh "$guard" --check >/dev/null 2>&1
    rc=$?
    if [ "$rc" -eq "$expected" ]; then
        report ok "$desc"
    else
        report FAIL "$desc" "expected rc=$expected got rc=$rc"
    fi
}

check_rc 1 'crates/volumectl/src/app.rs\n' \
    '--check: substantive-only list fails (rc 1)'
check_rc 0 'crates/a.rs\nfeature_list.json\nclaude-progress.md\n' \
    '--check: substantive + both records passes'
check_rc 1 'crates/a.rs\nfeature_list.json\n' \
    '--check: substantive + one record fails (rc 1)'
check_rc 0 'docs/superpowers/plans/x.md\nREADME.md\n' \
    '--check: exempt-only list passes'
check_rc 0 'feature_list.json\nclaude-progress.md\n' \
    '--check: records-only list passes'
check_rc 0 '' \
    '--check: empty list passes'
check_rc 0 'scripts/format-lint.sh\nfeature_list.json\nclaude-progress.md\n' \
    '--check: scripts/ change + records passes'
check_rc 1 '.github/workflows/ci.yml\n' \
    '--check: CI-only change without records fails'
check_rc 0 '.claude/settings.json\n.rtk/filters.toml\n.codex/config.toml\n' \
    '--check: agent-tool config is exempt'

# unknown mode
sh "$guard" --bogus >/dev/null 2>&1
rc=$?
if [ "$rc" -eq 2 ]; then
    report ok 'unknown mode exits 2'
else
    report FAIL 'unknown mode exits 2' "rc=$rc"
fi

# --- integration: temporary git repo ------------------------------------------
# The guard resolves nothing from cwd except git, but --staged/--branch read
# the repo the guard is INVOKED FROM, so run it inside the temp repo.
guard_abs="$(cd "$(dirname "$guard")" && pwd)/$(basename "$guard")"
tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT
git -C "$tmpdir" init -q
git -C "$tmpdir" config user.email test@example.com
git -C "$tmpdir" config user.name test
printf 'base\n' > "$tmpdir/base.txt"
git -C "$tmpdir" add base.txt
git -C "$tmpdir" commit -qm base
base_sha="$(git -C "$tmpdir" rev-parse HEAD)"

guard_in_tmp() { # <args...>  runs the guard from inside the temp repo
    ( cd "$tmpdir" && sh "$guard_abs" "$@" )
}

# --staged: stage a code file only -> fail
printf 'code\n' > "$tmpdir/crates/volumectl/src/app.rs"
git -C "$tmpdir" add crates/volumectl/src/app.rs
guard_in_tmp --staged >/dev/null 2>&1
rc=$?
if [ "$rc" -eq 1 ]; then
    report ok '--staged: staged code without records fails'
else
    report FAIL '--staged: staged code without records fails' "rc=$rc"
fi

# --staged: add records too -> pass
printf '{"last_updated":"x"}\n' > "$tmpdir/feature_list.json"
printf '# Progress\n' > "$tmpdir/claude-progress.md"
git -C "$tmpdir" add feature_list.json claude-progress.md
guard_in_tmp --staged >/dev/null 2>&1
rc=$?
if [ "$rc" -eq 0 ]; then
    report ok '--staged: code + both records passes'
else
    report FAIL '--staged: code + both records passes' "rc=$rc"
fi

# --branch: commit code only on a branch vs base -> fail
git -C "$tmpdir" commit -qm 'code only'
guard_in_tmp --branch "$base_sha" >/dev/null 2>&1
rc=$?
if [ "$rc" -eq 1 ]; then
    report ok '--branch: committed code without records fails vs base'
else
    report FAIL '--branch: committed code without records fails vs base' "rc=$rc"
fi

# --branch: add a records commit -> passes (records anywhere in the branch)
printf '# Progress\nSession 1\n' >> "$tmpdir/claude-progress.md"
printf '{"last_updated":"y"}\n' > "$tmpdir/feature_list.json"
git -C "$tmpdir" add feature_list.json claude-progress.md
git -C "$tmpdir" commit -qm 'records'
guard_in_tmp --branch "$base_sha" >/dev/null 2>&1
rc=$?
if [ "$rc" -eq 0 ]; then
    report ok '--branch: records anywhere in the branch passes'
else
    report FAIL '--branch: records anywhere in the branch passes' "rc=$rc"
fi

# --branch: missing base ref -> exit 2
guard_in_tmp --branch no-such-ref >/dev/null 2>&1
rc=$?
if [ "$rc" -eq 2 ]; then
    report ok '--branch: missing base ref exits 2'
else
    report FAIL '--branch: missing base ref exits 2' "rc=$rc"
fi

# --branch: exempt-only branch passes
git -C "$tmpdir" rm -q --cached crates/volumectl/src/app.rs
git -C "$tmpdir" commit -qm 'drop code'
printf 'doc\n' > "$tmpdir/docs/x.md"
git -C "$tmpdir" add docs/x.md
git -C "$tmpdir" commit -qm 'docs only'
guard_in_tmp --branch "$base_sha" >/dev/null 2>&1
rc=$?
if [ "$rc" -eq 0 ]; then
    report ok '--branch: exempt-only branch passes'
else
    report FAIL '--branch: exempt-only branch passes' "rc=$rc"
fi

# --- mirror check: guardrail skill ---------------------------------------------
if cmp -s .agents/skills/guardrail/SKILL.md .claude/skills/guardrail/SKILL.md; then
    report ok 'guardrail skill: .agents/.claude mirrors are byte-identical'
else
    report FAIL 'guardrail skill: .agents/.claude mirrors differ (resync .claude/skills/guardrail/)'
fi

if [ "$failures" -eq 0 ]; then
    echo "All record-keeping guard checks passed."
    exit 0
fi
echo "$failures check(s) failed." >&2
exit 1
```

- [ ] **Step 2: Run the self-test**

Run: `bash scripts/test-check-records.sh`
Expected: all checks `ok`, final `All record-keeping guard checks passed.`, exit 0.

- [ ] **Step 3: Leave uncommitted**

Do NOT commit yet (single Task 8 commit).

---

### Task 4: Write the guardrail skill + mirror

**Files:**
- Create: `.agents/skills/guardrail/SKILL.md`
- Create: `.claude/skills/guardrail/SKILL.md` (byte-identical copy)

**Interfaces:**
- Consumes: `scripts/check-records.sh` (documented, not embedded).
- Produces: the codified mandatory workflow for every task.

- [ ] **Step 1: Write the canonical skill**

```markdown
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
- CI runs `--branch`: a PR whose branch never updates the records fails.
- `--check` reads a path list from stdin (used by the self-test).

When the guard fails: add the missing record updates to the same change set
(`feature_list.json` entry + `claude-progress.md` session entry), re-stage,
and commit. Never bypass with `--no-verify` except when the user explicitly
requests it.

## Exempt (no record update required)

`feature_list.json`, `claude-progress.md`, `docs/**`, `README.md`,
`README.vi.md`, `session-handoff.md`, `init.sh`, `.gitignore`,
`.gitattributes`, `.rtk/**`, `.codex/**`, `.claude/*.json`.

## Verification

After the guard passes, run the full battery: `bash scripts/format-lint.sh`
(or `--skip-tests` for speed), `cargo test`, and the self-tests
(`bash scripts/test-format-lint.sh`, `bash scripts/test-check-records.sh`)
before claiming completion.
```

- [ ] **Step 2: Mirror and verify**

Run: `cp .agents/skills/guardrail/SKILL.md .claude/skills/guardrail/SKILL.md && cmp .agents/skills/guardrail/SKILL.md .claude/skills/guardrail/SKILL.md`
Expected: `cmp` exits 0.

- [ ] **Step 3: Leave uncommitted**

Do NOT commit yet (single Task 8 commit).

---

### Task 5: Codify the workflow in CLAUDE.md, GUARDRAILS.md, AGENTS.md

**Files:**
- Modify: `CLAUDE.md` (add "Mandatory workflow" section after "## Rules")
- Modify: `GUARDRAILS.md` (add "## Record keeping" section)
- Modify: `AGENTS.md` (add one line to the gate checklist)

**Interfaces:**
- Consumes: the guardrail skill naming; `scripts/check-records.sh`.
- Produces: the documented, cross-tool workflow contract.

- [ ] **Step 1: CLAUDE.md — add the mandatory workflow section**

After the `## Rules` list, insert:

```markdown
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
```

- [ ] **Step 2: GUARDRAILS.md — add the hard rule**

Append:

```markdown
## Record keeping

- Every commit, PR, and merge that changes substantive files (code, scripts,
  CI, hooks, skills) must also update both `feature_list.json` and
  `claude-progress.md`; see `scripts/check-records.sh`. Do not bypass with
  `--no-verify` without explicit user approval.
```

- [ ] **Step 3: AGENTS.md — add a line to the gate checklist**

After item 4 (`cargo test ...`), insert:

```text
5. `sh scripts/check-records.sh --staged` (record-keeping guard; the
   pre-commit hook runs this automatically)
```

- [ ] **Step 4: Verify**

Run: `bash scripts/test-check-records.sh` — still all `ok`. Do NOT commit yet — CLAUDE.md/GUARDRAILS.md/AGENTS.md are substantive and their record updates (Task 6) land in the same final commit.

---

### Task 6: Update the records (feature_list.json + claude-progress.md)

**Files:**
- Modify: `feature_list.json` (add `vol-014`, bump `last_updated`, extend `rules`)
- Modify: `claude-progress.md` (append Session 012)

**Interfaces:**
- Consumes: nothing.
- Produces: the record updates that satisfy the guard for the whole change set.

- [ ] **Step 1: feature_list.json**

Add to `rules`:

```json
"records_required_with_every_change": true
```

Add `vol-014` as the first entry in `features`:

```json
{
  "id": "vol-014",
  "priority": 14,
  "area": "tooling",
  "title": "Record-keeping guardrail (mandatory workflow + enforced records)",
  "user_visible_behavior": "Developer-facing: every substantive change must update feature_list.json and claude-progress.md or the pre-commit hook and CI reject it; the superpowers flow (brainstorm/spec/plan/execute/verify/finish) and evidence-before-claims are the codified mandatory workflow.",
  "status": "passing",
  "verification": [
    "Run `bash scripts/test-check-records.sh` - all checks pass, exit 0.",
    "Run `bash scripts/check-records.sh --staged` on a code-only staged set - must exit 1 with a message naming both records.",
    "Run `bash scripts/check-records.sh --branch <base>` on a code-only branch - must exit 1; with a records commit - exit 0.",
    "Run `bash scripts/test-format-lint.sh` and `bash scripts/format-lint.sh --skip-tests` - no regression.",
    "Run `cargo test` - green."
  ],
  "evidence": [
    "Self-test covers --check unit cases (substantive-only fail, +both records pass, +one record fail, exempt pass, empty pass, unknown mode exit 2), --staged and --branch integration in a temp git repo, and the guardrail skill mirror byte-identity.",
    "Pre-commit hook rejects code-only staged sets and accepts code+records (verified).",
    "CI checks job runs `bash scripts/check-records.sh --branch` and `bash scripts/test-check-records.sh`.",
    "Session 012 (claude-progress.md) records this landing; the guard's own commit updates both records."
  ],
  "notes": "Tooling feature; enforces the record-keeping rule that was previously documentation-only. Deliberately not part of the format-lint manifest (avoids PowerShell duplication)."
}
```

Bump `last_updated` to `2026-08-08T<time>` and validate: `python3 -c "import json; json.load(open('feature_list.json'))"` — expect no error.

- [ ] **Step 2: claude-progress.md**

Append a Session 012 entry following the established format:

```markdown
## Session 012 (2026-08-08) — Record-keeping guardrail

- Goal: enforce the user's directive that every task follows the superpowers
  flow + hardness and that every change updates feature_list.json and
  claude-progress.md; add a guard so the rule cannot be forgotten.
- What landed:
  - `scripts/check-records.sh` — POSIX-sh guard (modes `--staged`, `--branch`,
    `--check`) applying one rule: a change set with any substantive path must
    also contain both records; exempt: docs, READMEs, config, the records
    themselves. Fail-closed on unclassified paths.
  - Pre-commit hook runs `--staged` before the cargo steps; CI `checks` job
    runs `--branch` + the self-test.
  - `scripts/test-check-records.sh` — hermetic self-test (unit + temp-repo
    integration + mirror check).
  - `guardrail` skill (`.agents` + `.claude` mirror, byte-identical) and
    CLAUDE.md/GUARDRAILS.md/AGENTS.md codify the mandatory workflow.
- Verification: self-test all green; guard fails code-only sets and accepts
  code+records; format-lint smoke 24/24 no regression; cargo test green;
  the change set itself updates both records (this entry + vol-014).
```

- [ ] **Step 3: Leave uncommitted**

Do NOT commit yet — the records close out the single Task 8 commit.

---

### Task 7: Wire the guard into CI

**Files:**
- Modify: `.github/workflows/ci.yml` (add two steps to the `checks` job after `Smoke-test format-lint gates`)

**Interfaces:**
- Consumes: `scripts/check-records.sh --branch`, `scripts/test-check-records.sh`.
- Produces: PR-level enforcement of the records rule on ubuntu.

- [ ] **Step 1: Add the steps**

After the `Smoke-test format-lint gates` step in the `checks` job, insert:

```yaml
      - name: Record-keeping guard (branch change set)
        run: bash scripts/check-records.sh --branch
      - name: Smoke-test record-keeping guard
        run: bash scripts/test-check-records.sh
```

- [ ] **Step 2: Validate**

Run: `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/ci.yml')); print('yaml ok')"` (if pyyaml available; else rely on diff review — the inserted steps mirror sibling step formatting exactly).

- [ ] **Step 3: Leave uncommitted**

Do NOT commit yet (single Task 8 commit).

---

### Task 8: Final verification, single commit, whole-branch review

**Files:** all of the above (verification + the one commit)

- [ ] **Step 1: Full battery**

Run:
```bash
bash scripts/test-check-records.sh
bash scripts/test-format-lint.sh
bash scripts/format-lint.sh --skip-tests
cargo test
cmp .agents/skills/guardrail/SKILL.md .claude/skills/guardrail/SKILL.md
```
Expected: all green; smoke 24/24; gate passes; tests green; mirrors identical.

- [ ] **Step 2: Guard-on-itself**

Run: `git add -A && sh scripts/check-records.sh --staged`
Expected: exit 0 (the whole change set updates both records).

- [ ] **Step 3: Commit the whole change set**

```bash
git add scripts/check-records.sh scripts/test-check-records.sh .githooks/pre-commit .agents/skills/guardrail/SKILL.md .claude/skills/guardrail/SKILL.md CLAUDE.md GUARDRAILS.md AGENTS.md .github/workflows/ci.yml feature_list.json claude-progress.md docs/superpowers/specs/2026-08-08-record-keeping-guardrail-design.md docs/superpowers/plans/2026-08-08-record-keeping-guardrail.md
git commit -m "feat: enforce record-keeping guardrail (mandatory workflow + records guard)"
```

The pre-commit hook runs `--staged` on this commit: the staged set includes the records from Task 6, so it passes.

- [ ] **Step 4: Review + wrap up**

Dispatch a code reviewer over the full diff; fix any findings; update
`claude-progress.md` if the review changed anything; then amend or add a
second commit whose staged set also includes the records it requires.

---

## Self-Review Notes (filled during plan writing)

- Spec coverage: guard script (Task 1), hook (Task 2), self-test (Task 3),
  skill + mirror (Task 4), workflow docs (Task 5), records (Task 6), CI
  (Task 7), verification (Task 8) — all spec sections mapped.
- No placeholders: every step contains concrete content.
- Type consistency: `--staged`/`--branch`/`--check` mode names, exit codes
  0/1/2, and the substantive/exempt tables are identical across the spec,
  script, hook, self-test, skill, and docs.
