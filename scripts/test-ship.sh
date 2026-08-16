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

if bash scripts/test-agent-safe-flow.sh >/tmp/volume-control-agent-safe-flow.log 2>&1; then
    report ok 'agent safe-flow hook blocks unsafe git/review bypasses'
else
    report FAIL 'agent safe-flow hook blocks unsafe git/review bypasses' \
        "see /tmp/volume-control-agent-safe-flow.log"
fi

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
invokes scripts/ship.sh 'check-records[.]sh.*--branch origin/main' \
    'ship.sh: runs the branch records guard vs origin/main'
# The branch guard runs before the explicit staging phase, so check-records.sh
# must include ordinary working-tree changes rather than only HEAD ancestry.
if grep -Eq 'git (-c core\.quotepath=true )?diff --name-only HEAD' scripts/check-records.sh; then
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
# --- ship must build the frontend, run WDIO E2E, and build the release binary -
invokes scripts/ship.sh 'npm run build --prefix' \
    'ship.sh: release flow builds the frontend explicitly before the tauri build'
invokes scripts/ship.sh 'verify-tauri-e2e' \
    'ship.sh: release flow runs the fail-closed Tauri WebDriver E2E gate'
if grep -qF 'Push-Location $e2e' scripts/verify-tauri-e2e.ps1; then
    report ok 'verify-tauri-e2e.ps1: evidence check resolves tsx from the isolated E2E package'
else
    report FAIL 'verify-tauri-e2e.ps1: evidence check resolves tsx from the isolated E2E package'
fi
if grep -qF 'pushd "$repo/e2e/tauri"' scripts/verify-tauri-e2e.sh; then
    report ok 'verify-tauri-e2e.sh: evidence check resolves tsx from the isolated E2E package'
else
    report FAIL 'verify-tauri-e2e.sh: evidence check resolves tsx from the isolated E2E package'
fi
if grep -qF 'output_root="$repo/$output_root"' scripts/verify-tauri-e2e.sh; then
    report ok 'verify-tauri-e2e.sh: normalizes relative evidence roots before changing cwd'
else
    report FAIL 'verify-tauri-e2e.sh: normalizes relative evidence roots before changing cwd'
fi
invokes scripts/ship.sh 'node_modules/[.]bin/tauri' \
    'ship.sh: release flow resolves the local tauri CLI (never npx tauri)'
invokes scripts/ship.sh 'build --no-bundle' \
    'ship.sh: release flow runs tauri build --no-bundle after E2E'

# --- desktop CI schedule and artifact contracts ------------------------------
# Keep Windows and the shared checks unconditional. Linux/macOS are expensive
# hosted checks, so a docs/tooling-only pull request may use the bounded-cost
# path; runtime/UI/E2E/workflow/release changes and every main/release event
# must still run the full matrix. The release gate must fail closed either way.
ci_workflow=.github/workflows/ci.yml
job_block() { # <job>
    awk -v job="$1" '
        $0 ~ "^  " job ":$" { inside=1; next }
        inside && $0 ~ "^  [A-Za-z0-9_-]+:" { exit }
        inside { print }
    ' "$ci_workflow"
}

windows_job="$(job_block windows)"
if grep -Eq '^    if:' <<<"$windows_job"; then
    report FAIL 'ci.yml: Windows job remains unconditional for pull requests'
else
    report ok 'ci.yml: Windows job remains unconditional for pull requests'
fi

scope_job="$(job_block scope)"
if grep -q 'platform_required:.*steps.scope.outputs.platform_required' <<<"$scope_job" && \
   grep -q 'git diff --name-only' <<<"$scope_job" && \
   grep -q 'EVENT_NAME.*pull_request' <<<"$scope_job"; then
    report ok 'ci.yml: scope job classifies pull-request diffs and fails open when uncertain'
else
    report FAIL 'ci.yml: scope job classifies pull-request diffs and fails open when uncertain'
fi

for desktop_job in macos ubuntu; do
    desktop_block="$(job_block "$desktop_job")"
    if grep -Eq '^    needs: scope$' <<<"$desktop_block" && \
       grep -Eq 'needs\.scope\.outputs\.platform_required == '\''true'\''' <<<"$desktop_block"; then
        report ok "ci.yml: $desktop_job follows the cost-balanced platform scope"
    else
        report FAIL "ci.yml: $desktop_job follows the cost-balanced platform scope"
    fi
done

release_gate_block="$(job_block release-gate)"
if grep -q '^    if:.*always' <<<"$release_gate_block" && \
   grep -q 'needs: \[scope, checks, windows, macos, ubuntu\]' <<<"$release_gate_block" && \
   grep -q 'PLATFORM_REQUIRED' <<<"$release_gate_block"; then
    report ok 'ci.yml: release gate evaluates every required job and skipped-platform policy'
else
    report FAIL 'ci.yml: release gate evaluates every required job and skipped-platform policy'
fi

# Every CI desktop E2E artifact step must fail closed. Match the step boundary
# so a nearby non-E2E upload cannot satisfy this contract accidentally.
e2e_upload_contracts="$(awk '
    /^[[:space:]]*- name: .*Tauri E2E artifacts/ { pending=1; next }
    pending && /if-no-files-found: error/ { passed++; pending=0; next }
    pending && /^[[:space:]]*- name:/ { pending=0 }
    END { print passed + 0 }
' "$ci_workflow")"
e2e_upload_steps="$(grep -Ec '^[[:space:]]*- name: .*Tauri E2E artifacts' "$ci_workflow" || true)"
if [ "$e2e_upload_steps" -gt 0 ] && [ "$e2e_upload_contracts" -eq "$e2e_upload_steps" ]; then
    report ok "ci.yml: all $e2e_upload_steps desktop E2E uploads fail when artifacts are absent"
else
    report FAIL 'ci.yml: all desktop E2E uploads fail when artifacts are absent' "found $e2e_upload_contracts/$e2e_upload_steps"
fi

# Release is tag/manual-dispatch only; its reusable validation must remain
# unconditional even while ordinary pull requests skip the hosted Linux/macOS
# jobs above.
if grep -q '^    uses: \./\.github/workflows/desktop-validation\.yml$' .github/workflows/release.yml && \
   grep -q '^    needs: \[preflight, validate\]$' .github/workflows/release.yml; then
    report ok 'release.yml: publishing remains gated by reusable desktop validation'
else
    report FAIL 'release.yml: publishing remains gated by reusable desktop validation'
fi

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
