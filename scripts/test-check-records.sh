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
check_rc 1 '.claude/skills/volume-control/config.json\n' \
    '--check: .claude/skills/* stays substantive even for JSON (not caught by .claude/*.json)'
check_rc 1 'weird.txt\n' \
    '--check: unclassified path is substantive (fail-closed)'

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
# mktemp -d alone yields an MSYS /tmp path that native git.exe cannot enter
# (e.g. on Windows, `git -C /tmp/...` fails), so pin the template under the
# host temp dir and convert to a Windows path when cygpath is available.
# On Linux/macOS, TEMP/TMPDIR are unset and cygpath is absent: /tmp stays.
tmpdir="$(mktemp -d "${TEMP:-${TMPDIR:-/tmp}}/check-records.XXXXXX")"
if command -v cygpath >/dev/null 2>&1; then
    tmpdir="$(cygpath -w "$tmpdir")"
fi
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
    'if [ "${1:-}" = -c ]; then shift 2; fi' \
    'if [ "${1:-}" = diff ] || [ "${1:-}" = ls-files ]; then printf "%s\\n" "warning: synthetic git warning" >&2; fi' \
    'if [ "${CHECK_RECORDS_FAIL_DIFF:-0}" = 1 ] && [ "${1:-}" = diff ] && [ "${2:-}" = --name-only ] && [ "${3:-}" = HEAD ]; then' \
    '    printf "%s\\n" "fatal: synthetic git failure" >&2' \
    '    exit 42' \
    'fi' \
    'if [ "${CHECK_RECORDS_FAIL_STAGED:-0}" = 1 ] && [ "${1:-}" = diff ] && [ "${2:-}" = --cached ] && [ "${3:-}" = --name-only ]; then' \
    '    printf "%s\\n" "fatal: synthetic git failure (staged)" >&2' \
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

# --staged: the failing run must NOT have auto-created the records (the guard
# only prints templates; a regression adding `: > feature_list.json` would
# still report rc=1 and still print both templates, so assert absence now).
if [ ! -e "$tmpdir/feature_list.json" ] && [ ! -e "$tmpdir/claude-progress.md" ]; then
    report ok '--staged: failing run never writes the records (only prints templates)'
else
    report FAIL '--staged: failing run never writes the records (only prints templates)'
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

# --staged: deleting both record files is not a valid update. The guard must
# inspect deletion status separately because --name-only still lists deleted
# paths as if they were edited files.
git -C "$tmpdir" rm -q --cached feature_list.json claude-progress.md
staged_deleted_out="$(guard_in_tmp --staged 2>&1)"
rc=$?
if [ "$rc" -eq 1 ] && \
   printf '%s' "$staged_deleted_out" | grep -q 'feature_list.json - add a new entry' && \
   printf '%s' "$staged_deleted_out" | grep -q 'claude-progress.md - append a new session entry'; then
    report ok '--staged: deleted records fail closed'
else
    report FAIL '--staged: deleted records fail closed' "rc=$rc; output=$staged_deleted_out"
fi
git -C "$tmpdir" add feature_list.json claude-progress.md

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

# --staged: a failing git command must abort with the diagnostic retained
# (the same fail-closed contract as the --branch case below; a regression
# that swallows `git diff --cached` errors would silently pass here).
staged_failure_out="$(cd "$tmpdir" && CHECK_RECORDS_FAIL_STAGED=1 PATH="$noisy_bin:$PATH" sh "$guard_abs" --staged 2>&1)"
rc=$?
if [ "$rc" -eq 1 ] && \
   printf '%s' "$staged_failure_out" | grep -q 'git diff --cached failed' && \
   printf '%s' "$staged_failure_out" | grep -q 'synthetic git failure (staged)'; then
    report ok '--staged: failed git commands retain their stderr diagnostic'
else
    report FAIL '--staged: failed git commands retain their stderr diagnostic' "rc=$rc; output=$staged_failure_out"
fi

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

# --branch: same no-auto-write contract on the real-path failure (records were
# removed on line above, so the failing run must leave them absent).
if [ ! -e "$tmpdir/feature_list.json" ] && [ ! -e "$tmpdir/claude-progress.md" ]; then
    report ok '--branch: failing run never writes the records (only prints templates)'
else
    report FAIL '--branch: failing run never writes the records (only prints templates)'
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

# --branch: a commit that deletes both record files must fail, even though the
# deleted names remain present in `git diff --name-only`.
delete_repo="$tmpdir/delete-records"
git init -q "$delete_repo"
git -C "$delete_repo" config user.email test@example.com
git -C "$delete_repo" config user.name test
git -C "$delete_repo" config core.autocrlf false
mkdir -p "$delete_repo/crates/volumectl/src"
printf 'code\n' > "$delete_repo/crates/volumectl/src/app.rs"
printf '{"last_updated":"base"}\n' > "$delete_repo/feature_list.json"
printf '# Progress\n' > "$delete_repo/claude-progress.md"
git -C "$delete_repo" add crates/volumectl/src/app.rs feature_list.json claude-progress.md
git -C "$delete_repo" commit -qm base
delete_base="$(git -C "$delete_repo" rev-parse HEAD)"
printf 'changed code\n' > "$delete_repo/crates/volumectl/src/app.rs"
git -C "$delete_repo" rm -q feature_list.json claude-progress.md
git -C "$delete_repo" add crates/volumectl/src/app.rs
git -C "$delete_repo" commit -qm 'delete records'
delete_branch_out="$(cd "$delete_repo" && sh "$guard_abs" --branch "$delete_base" 2>&1)"
rc=$?
if [ "$rc" -eq 1 ] && \
   printf '%s' "$delete_branch_out" | grep -q 'feature_list.json - add a new entry' && \
   printf '%s' "$delete_branch_out" | grep -q 'claude-progress.md - append a new session entry'; then
    report ok '--branch: committed record deletions fail closed'
else
    report FAIL '--branch: committed record deletions fail closed' "rc=$rc; output=$delete_branch_out"
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
# silently disable local enforcement; assert the invocation is present, not
# commented out, AND guarded by a fail-closed `if ! ... exit 1` branch. A bare
# invocation, a positive `if`, a `|| true`/`then :` fail-open, or a
# COMMENTED-OUT `exit 1` inside the branch would all still "invoke" the guard
# yet never abort the commit, so only the exact fail-closed idiom counts.
if awk 'BEGIN{found=0;scanning=0} \
    /check-records\.sh --staged/ && !/^[[:space:]]*#/ && /if[[:space:]]*!/ {scanning=1; next} \
    scanning && !/^[[:space:]]*#/ && /exit[[:space:]]+1/ {found=1; scanning=0} \
    scanning && /^[[:space:]]*fi[[:space:]]*$/ {scanning=0} \
    END{exit !found}' .githooks/pre-commit; then
    report ok 'pre-commit hook: records guard aborts on failure (fail-closed if ! ... exit 1)'
else
    report FAIL 'pre-commit hook: records guard aborts on failure (fail-closed if ! ... exit 1)' \
        '(guard dropped, commented out, or made fail-open in .githooks/pre-commit?)'
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
if [ -f .agents/skills/windows-host/SKILL.md ] && \
   [ -f .claude/skills/windows-host/SKILL.md ] && \
   [ -f .agents/skills/windows-host/scripts/ensure-pkg-config-stub.ps1 ] && \
   [ -f .claude/skills/windows-host/scripts/ensure-pkg-config-stub.ps1 ] && \
   cmp -s .agents/skills/windows-host/SKILL.md .claude/skills/windows-host/SKILL.md && \
   cmp -s .agents/skills/windows-host/scripts/ensure-pkg-config-stub.ps1 \
         .claude/skills/windows-host/scripts/ensure-pkg-config-stub.ps1; then
    report ok 'windows-host skill: .agents/.claude mirrors are byte-identical'
else
    report FAIL 'windows-host skill: .agents/.claude mirrors differ (resync .claude/skills/windows-host/)'
fi

if [ "$failures" -eq 0 ]; then
    echo "All record-keeping guard checks passed."
    exit 0
fi
echo "$failures check(s) failed." >&2
exit 1
