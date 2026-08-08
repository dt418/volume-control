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

# --staged: stage a code file only -> fail
mkdir -p "$tmpdir/crates/volumectl/src"
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
