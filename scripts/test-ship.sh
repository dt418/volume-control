#!/usr/bin/env bash
#
# test-ship.sh - smoke test for the mandatory ship flow.
#
# scripts/ship.sh (and its PowerShell bridge scripts/ship.ps1) is the top of
# the enforcement stack: it guarantees the records guard, the full
# format-lint gate, and both guardrail self-tests run before anything can be
# committed or pushed. A future edit that drops any of those from ship.sh
# would silently reopen the exact bypass the flow exists to close, so this
# test asserts - comment-aware and line-anchored, the same way the pre-commit
# hook wiring is asserted elsewhere - that every hard check is still invoked,
# that no bypass flag exists, and that ship.ps1 still bridges to the
# canonical flow without duplicating any rule logic.
#
# Usage: bash scripts/test-ship.sh
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

# Comment-aware and progress-line-aware: the pattern must appear on a real
# code line (not a comment, not an echo/printf progress line that merely
# mentions the command), so a check that was dropped, commented out, or
# reduced to an echo of its own name fails loudly.
invokes() { # <file> <pattern> <description>
    local file="$1" pat="$2" desc="$3"
    if awk -v pat="$pat" \
        '!/^[[:space:]]*#/ && !/^[[:space:]]*echo/ && !/^[[:space:]]*printf/ && $0 ~ pat' \
        "$file" | grep -q .; then
        report ok "$desc"
    else
        report FAIL "$desc" '(dropped, commented out, or only echoed?)'
    fi
}

# --- hard checks must still be invoked by ship.sh ------------------------------
invokes scripts/ship.sh 'check-records[.]sh.*--branch origin/master' \
    'ship.sh: runs the branch records guard vs origin/master'
# The branch guard runs before the explicit staging phase, so check-records.sh
# must include ordinary working-tree changes rather than only HEAD ancestry.
if grep -q 'git diff --name-only HEAD' scripts/check-records.sh; then
    report ok 'check-records.sh: branch mode includes working-tree changes for ship preflight'
else
    report FAIL 'check-records.sh: branch mode includes working-tree changes for ship preflight'
fi
invokes scripts/ship.sh 'scripts/format-lint[.]sh"' \
    'ship.sh: runs the full format-lint gate'
# The gate must run WITH tests: the invocation line must not carry --skip-tests.
if awk -v pat='scripts/format-lint[.]sh"' \
    '!/^[[:space:]]*#/ && !/^[[:space:]]*echo/ && !/^[[:space:]]*printf/ && $0 ~ pat && $0 !~ /--skip-tests/' \
    scripts/ship.sh | grep -q .; then
    report ok 'ship.sh: the format-lint gate runs with tests (no --skip-tests)'
else
    report FAIL 'ship.sh: the format-lint gate runs with tests (no --skip-tests)'
fi
invokes scripts/ship.sh 'test-check-records[.]sh' \
    'ship.sh: runs the records-guard self-test'
invokes scripts/ship.sh 'test-format-lint[.]sh' \
    'ship.sh: runs the format-lint smoke test'
invokes scripts/ship.sh 'check-records[.]sh.*--staged' \
    'ship.sh: re-checks the records rule on the staged set after staging'

# --- no bypass flags may exist --------------------------------------------------
# "NEVER bypass by default": there is no --skip-* / --bypass flag anywhere in
# the canonical flow. (--force relaxes git hygiene only and is documented as
# such; the hard checks above always run.)
if grep -qE -- '--(skip|bypass|no-verify)' scripts/ship.sh; then
    report FAIL 'ship.sh: no bypass flags exist (--skip / --bypass / --no-verify)'
else
    report ok 'ship.sh: no bypass flags exist (--skip / --bypass / --no-verify)'
fi

# --- flag contracts: --help and unknown-flag, exercised for real -----------------
# Both run during argument parsing, before any tool resolution or gate work,
# so they are fast and hermetic.
help_out="$(bash scripts/ship.sh --help 2>&1)"
if [ $? -ne 0 ] || ! grep -q 'usage:' <<<"$help_out"; then
    report FAIL 'ship.sh: --help exits 0 with usage'
else
    report ok 'ship.sh: --help exits 0 with usage'
fi
for flag in --push --force --dry-run --message; do
    if grep -q -- "$flag" <<<"$help_out"; then
        report ok "ship.sh: usage lists $flag"
    else
        report FAIL "ship.sh: usage lists $flag"
    fi
done
if grep -qE -- '--(skip|bypass)' <<<"$help_out"; then
    report FAIL 'ship.sh: usage advertises no bypass flags'
else
    report ok 'ship.sh: usage advertises no bypass flags'
fi
bash scripts/ship.sh --definitely-not-a-flag >/dev/null 2>&1
rc=$?
if [ "$rc" -eq 2 ]; then
    report ok 'ship.sh: unknown flag exits 2 (usage error)'
else
    report FAIL 'ship.sh: unknown flag exits 2 (usage error)' "rc=$rc"
fi

# --- ship.ps1 must bridge to the canonical flow, not duplicate it ---------------
invokes scripts/ship.ps1 'ship[.]sh' \
    'ship.ps1: invokes the canonical scripts/ship.sh'
for flag in Push Force DryRun Message; do
    if grep -q "\\\$$flag" scripts/ship.ps1; then
        report ok "ship.ps1: exposes -$flag"
    else
        report FAIL "ship.ps1: exposes -$flag"
    fi
done
# The bridge must not reimplement any hard check (zero rule duplication,
# the same constraint the format-lint records step obeys).
if grep -qE 'check-records|format-lint\.sh|test-format-lint|test-check-records' scripts/ship.ps1; then
    report FAIL 'ship.ps1: no rule logic duplicated (must only bridge to ship.sh)'
else
    report ok 'ship.ps1: no rule logic duplicated (bridges to ship.sh only)'
fi

# --- the mandatory flow must be documented in the guardrail skill ---------------
if grep -q 'ship\.sh' .agents/skills/guardrail/SKILL.md && \
   grep -q 'ship\.sh' .claude/skills/guardrail/SKILL.md; then
    report ok 'guardrail skill (both mirrors): documents the mandatory ship flow'
else
    report FAIL 'guardrail skill (both mirrors): documents the mandatory ship flow'
fi

if [ "$failures" -eq 0 ]; then
    echo "All ship-flow smoke checks passed."
    exit 0
fi
echo "$failures check(s) failed." >&2
exit 1
