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

# --- recovery templates: a failure must suggest, never create -----------------
# The whole point of the guard's failure output is a one-step recovery path:
# copy the template, fill it in, re-run. Assert the hint fires per missing
# record and never fires on a pass, so a regression that hides the templates
# (or auto-writes the records) fails loudly.
both_out="$(printf 'crates/a.rs\n' | sh "$guard" --check 2>&1 || true)"
if printf '%s' "$both_out" | grep -q 'feature_list.json - add a new entry' && \
   printf '%s' "$both_out" | grep -q 'claude-progress.md - append a new session entry'; then
    report ok 'failure: suggests both record templates when both are missing'
else
    report FAIL 'failure: suggests both record templates when both are missing'
fi
progress_out="$(printf 'crates/a.rs\nfeature_list.json\n' | sh "$guard" --check 2>&1 || true)"
if printf '%s' "$progress_out" | grep -q 'claude-progress.md - append a new session entry' && \
   ! printf '%s' "$progress_out" | grep -q 'feature_list.json - add a new entry'; then
    report ok 'failure: hints only the missing record'
else
    report FAIL 'failure: hints only the missing record'
fi
pass_out="$(printf 'crates/a.rs\nfeature_list.json\nclaude-progress.md\n' | sh "$guard" --check 2>&1 || true)"
if printf '%s' "$pass_out" | grep -q 'To fix:'; then
    report FAIL 'pass: no recovery hint on a passing check'
else
    report ok 'pass: no recovery hint on a passing check'
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
# Hermetic: pin line endings so the host machine's global core.autocrlf
# cannot affect the diff/index semantics under test.
git -C "$tmpdir" config core.autocrlf false
printf 'base\n' > "$tmpdir/base.txt"
git -C "$tmpdir" add base.txt
git -C "$tmpdir" commit -qm base
base_sha="$(git -C "$tmpdir" rev-parse HEAD)"

guard_in_tmp() { # <args...>  runs the guard from inside the temp repo
    ( cd "$tmpdir" && sh "$guard_abs" "$@" )
}

# Git can exit successfully while warning on stderr (for example about line
# endings). The guard must suppress that warning on successful captures, but it
# must merge stderr into the diagnostic when a git command actually fails.
real_git="$(command -v git)"
noisy_bin="$tmpdir/noisy-git"
mkdir -p "$noisy_bin"
printf '%s\n' \
    '#!/usr/bin/env sh' \
    'if [ "${1:-}" = diff ] || [ "${1:-}" = ls-files ]; then printf "%s\\n" "warning: synthetic git warning" >&2; fi' \
    'if [ "${CHECK_RECORDS_FAIL_DIFF:-0}" = 1 ] && [ "${1:-}" = diff ] && [ "${2:-}" = --name-only ] && [ "${3:-}" = HEAD ]; then' \
    '    printf "%s\\n" "fatal: synthetic git failure" >&2' \
    '    exit 42' \
    'fi' \
    "exec \"$real_git\" \"\$@\"" > "$noisy_bin/git"
chmod +x "$noisy_bin/git"

# --staged: stage a code file only -> fail, and must suggest both templates
# (the recovery-hint contract the pre-commit hook's users actually hit,
# mirroring the --check unit assertions above).
mkdir -p "$tmpdir/crates/volumectl/src"
printf 'code\n' > "$tmpdir/crates/volumectl/src/app.rs"
git -C "$tmpdir" add crates/volumectl/src/app.rs
staged_fail_out="$(guard_in_tmp --staged 2>&1)"
rc=$?
if [ "$rc" -eq 1 ] && \
   printf '%s' "$staged_fail_out" | grep -q 'feature_list.json - add a new entry' && \
   printf '%s' "$staged_fail_out" | grep -q 'claude-progress.md - append a new session entry'; then
    report ok '--staged: staged code without records fails and suggests both templates'
else
    report FAIL '--staged: staged code without records fails and suggests both templates' "rc=$rc"
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

# --staged: empty staged set passes (common real-world case)
git -C "$tmpdir" reset -q
guard_in_tmp --staged >/dev/null 2>&1
rc=$?
if [ "$rc" -eq 0 ]; then
    report ok '--staged: empty staged set passes'
else
    report FAIL '--staged: empty staged set passes' "rc=$rc"
fi
git -C "$tmpdir" add crates/volumectl/src/app.rs feature_list.json claude-progress.md

# Reset the index AND remove the untracked record files (left over from the
# staged test) so the branch change set is truly code-only -> --branch fails.
git -C "$tmpdir" reset -q
rm -f "$tmpdir/feature_list.json" "$tmpdir/claude-progress.md"
git -C "$tmpdir" add crates/volumectl/src/app.rs
git -C "$tmpdir" commit -qm 'code only'
branch_fail_out="$(guard_in_tmp --branch "$base_sha" 2>&1)"
rc=$?
if [ "$rc" -eq 1 ] && \
   printf '%s' "$branch_fail_out" | grep -q 'feature_list.json - add a new entry' && \
   printf '%s' "$branch_fail_out" | grep -q 'claude-progress.md - append a new session entry'; then
    report ok '--branch: committed code without records fails and suggests both templates'
else
    report FAIL '--branch: committed code without records fails and suggests both templates' "rc=$rc"
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

noisy_out="$(cd "$tmpdir" && PATH="$noisy_bin:$PATH" sh "$guard_abs" --branch "$base_sha" 2>&1)"
rc=$?
if [ "$rc" -eq 0 ] && ! printf '%s' "$noisy_out" | grep -q 'synthetic git warning'; then
    report ok '--branch: successful git warnings stay out of the path list'
else
    report FAIL '--branch: successful git warnings stay out of the path list' "rc=$rc; output=$noisy_out"
fi

failure_out="$(cd "$tmpdir" && CHECK_RECORDS_FAIL_DIFF=1 PATH="$noisy_bin:$PATH" sh "$guard_abs" --branch "$base_sha" 2>&1)"
rc=$?
if [ "$rc" -eq 1 ] && \
   printf '%s' "$failure_out" | grep -q "git diff of the working tree failed" && \
   printf '%s' "$failure_out" | grep -q 'synthetic git failure'; then
    report ok '--branch: failed git commands retain their stderr diagnostic'
else
    report FAIL '--branch: failed git commands retain their stderr diagnostic' "rc=$rc; output=$failure_out"
fi

# --branch: ordinary working-tree changes count toward the local change set.
# This mirrors scripts/ship.sh, which runs the branch guard before staging: a
# tracked record edit plus an untracked substantive file must pass without
# requiring the caller to mutate the index first.
working_repo="$tmpdir/working-tree-check"
git init -q "$working_repo"
git -C "$working_repo" config user.email test@example.com
git -C "$working_repo" config user.name test
git -C "$working_repo" config core.autocrlf false
mkdir -p "$working_repo/crates/volumectl/src"
printf '{"last_updated":"base"}\n' > "$working_repo/feature_list.json"
printf '# Progress\n' > "$working_repo/claude-progress.md"
printf 'base\n' > "$working_repo/README.md"
git -C "$working_repo" add feature_list.json claude-progress.md README.md
git -C "$working_repo" commit -qm base
working_base="$(git -C "$working_repo" rev-parse HEAD)"
printf '{"last_updated":"working"}\n' > "$working_repo/feature_list.json"
printf '# Progress\nWorking change\n' > "$working_repo/claude-progress.md"
printf 'new macOS host\n' > "$working_repo/crates/volumectl/src/macos_app.rs"
working_out="$(cd "$working_repo" && sh "$guard_abs" --branch "$working_base" 2>&1)"
rc=$?
if [ "$rc" -eq 0 ]; then
    report ok '--branch: working-tree records cover an untracked substantive file'
else
    report FAIL '--branch: working-tree records cover an untracked substantive file' "rc=$rc; output=$working_out"
fi

# --branch: missing base ref -> exit 2
missing_base_out="$(guard_in_tmp --branch no-such-ref 2>&1)"
rc=$?
if [ "$rc" -eq 2 ] && printf '%s' "$missing_base_out" | grep -q "base ref 'no-such-ref' not found"; then
    report ok '--branch: missing base ref exits 2'
else
    report FAIL '--branch: missing base ref exits 2' "rc=$rc; output=$missing_base_out"
fi

# --branch: exempt-only branch passes
git -C "$tmpdir" rm -q --cached crates/volumectl/src/app.rs
git -C "$tmpdir" commit -qm 'drop code'
mkdir -p "$tmpdir/docs"
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

# --- guard wiring: the pre-commit hook must still invoke the guard -----------
# A future hook edit that drops or comments out the records check would
# silently disable local enforcement; assert the invocation is present and
# not commented out.
if awk '!/^[[:space:]]*#/ && /check-records\.sh --staged/' .githooks/pre-commit | grep -q .; then
    report ok 'pre-commit hook: invokes check-records.sh --staged'
else
    report FAIL 'pre-commit hook: invokes check-records.sh --staged' \
        '(guard dropped or commented out in .githooks/pre-commit?)'
fi

# --- mirror checks: skills ------------------------------------------------------
# The guardrail and pre-push-review skills are the workflow's contract; a
# drifted mirror silently splits what agents see, so both must stay
# byte-identical and the guardrail must still mandate the review phase.
if cmp -s .agents/skills/guardrail/SKILL.md .claude/skills/guardrail/SKILL.md; then
    report ok 'guardrail skill: .agents/.claude mirrors are byte-identical'
else
    report FAIL 'guardrail skill: .agents/.claude mirrors differ (resync .claude/skills/guardrail/)'
fi
if [ -f .agents/skills/pre-push-review/SKILL.md ] && \
   [ -f .claude/skills/pre-push-review/SKILL.md ] && \
   cmp -s .agents/skills/pre-push-review/SKILL.md .claude/skills/pre-push-review/SKILL.md; then
    report ok 'pre-push-review skill: .agents/.claude mirrors are byte-identical'
else
    report FAIL 'pre-push-review skill: .agents/.claude mirrors differ (resync .claude/skills/pre-push-review/)'
fi
if grep -q 'pre-push-review' .agents/skills/guardrail/SKILL.md && \
   grep -q 'pre-push-review' .claude/skills/guardrail/SKILL.md; then
    report ok 'guardrail skill (both mirrors): mandates the three-domain pre-push review'
else
    report FAIL 'guardrail skill (both mirrors): mandates the three-domain pre-push review'
fi

if [ "$failures" -eq 0 ]; then
    echo "All record-keeping guard checks passed."
    exit 0
fi
echo "$failures check(s) failed." >&2
exit 1
