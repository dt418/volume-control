#!/usr/bin/env bash
#
# test-format-lint.sh - smoke test for the format-lint gate scripts.
#
# Exercises both gates - scripts/format-lint.sh (bash) and the PowerShell
# twin (.agents/skills/format-lint/scripts/format-lint.ps1) - plus the
# .claude skill mirror, so drift between any of them fails loudly:
#
#   1. a clean `--skip-tests` run must exit 0 and print "Gate passed."
#   2. a forbidden diff path (scratch edit to .claude/settings.local.json)
#      must exit 1 and report "forbidden paths in the working diff"
#   3. a staged code file without record updates must exit 1 and report the
#      "record updates" step as FAILED (on both gates)
#   4. (bash gate only) an unknown flag must exit 2 with a usage message
#   5. the .claude/skills/format-lint mirrors must be byte-identical to the
#      canonical gate scripts
#   6. every manifest forbidden pattern matches a representative path, and
#      benign/near-miss paths do not, on both parsers
#
# The PowerShell gate is exercised only when `powershell`/`pwsh` is on PATH.
# Tests stay fast by using --skip-tests and relying on cached clippy
# artifacts; the full gate with tests remains CI's job.
#
# The negative tests temporarily overwrite .claude/settings.local.json
# (tracked, unmodified in a normal worktree) and stage an untracked scratch
# file; both are restored by the EXIT trap no matter how the script exits.
# The records step reads the staged set, so the smoke test requires a clean
# index up front (see the precondition below).
#
# Usage: bash scripts/test-format-lint.sh
# Exit: 0 = all checks passed; 1 = at least one check failed.
set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."   # repository root

failures=0
forbidden_file=".claude/settings.local.json"

# Hermetic precondition: the negative tests overwrite the tracked forbidden
# file, and the success tests require it not to be a forbidden diff path.
# If it is missing or already modified, refuse to run with a clear message
# instead of reporting a cascade of confusing FAILs.
if [ ! -f "$forbidden_file" ]; then
    echo "FAIL - tracked $forbidden_file is missing; cannot run smoke test" >&2
    exit 1
fi
if ! git diff --quiet HEAD -- "$forbidden_file"; then
    echo "FAIL - $forbidden_file is modified in the working tree;" >&2
    echo "      commit or stash it before running the smoke test" >&2
    exit 1
fi
# The records step (manifest v3) enforces that staged substantive changes
# also stage the records; a dirty index would make its success-path runs
# fail regardless of the gates themselves, so require a clean index up front.
if ! git diff --cached --quiet; then
    echo "FAIL - the index has staged changes; the records step reads the staged set" >&2
    echo "      and would fail regardless of the gates. Run \`git stash\` (or commit" >&2
    echo "      / reset) before running the smoke test." >&2
    exit 1
fi

tmpdir="$(mktemp -d)"
records_scratch="scripts/.smoke-records-scratch.txt"
backup="$(mktemp)"

# Preserve the tracked forbidden file byte-for-byte across the negative tests.
cp "$forbidden_file" "$backup"
restore_forbidden() {
    [ -f "$backup" ] && cp "$backup" "$forbidden_file"
}
cleanup() {
    restore_forbidden
    # Records-step negatives stage an untracked scratch file; always unstage
    # and remove it so an interrupted run leaves no trace in the index.
    git reset -q -- "$records_scratch" 2>/dev/null
    rm -f "$records_scratch"
    rm -f "$backup"
    rm -rf "$tmpdir"
}
trap cleanup EXIT

report() { # <ok|FAIL> <description> [detail]
    local status="$1" desc="$2" detail="${3:-}"
    if [ "$status" = ok ]; then
        printf 'ok   - %s\n' "$desc"
    else
        printf 'FAIL - %s%s\n' "$desc" "${detail:+ ($detail)}"
        failures=$((failures + 1))
    fi
}

# Success and negative tests run in an order that keeps the tracked forbidden
# file pristine for every success-path run: all success checks first, then
# the negative checks (which may leave scratch content until the EXIT trap).

# --- success path: bash gate -------------------------------------------------
bash scripts/format-lint.sh --skip-tests >"$tmpdir/bash-ok" 2>&1
rc=$?
if [ "$rc" -eq 0 ] && grep -q 'Gate passed.' "$tmpdir/bash-ok"; then
    report ok 'bash gate: clean --skip-tests run exits 0 with "Gate passed."'
else
    report FAIL 'bash gate: clean --skip-tests run exits 0 with "Gate passed."' "rc=$rc"
fi

# --- success path: PowerShell gate (when available) ---------------------------
ps="$(command -v powershell 2>/dev/null || command -v pwsh 2>/dev/null || true)"
if [ -z "$ps" ]; then
    echo "skip - PowerShell not found; PowerShell gate checks skipped"
else
    "$ps" -NoProfile -ExecutionPolicy Bypass -File \
        .agents/skills/format-lint/scripts/format-lint.ps1 -SkipTests \
        >"$tmpdir/ps-ok" 2>&1
    rc=$?
    if [ "$rc" -eq 0 ] && grep -q 'Gate passed.' "$tmpdir/ps-ok"; then
        report ok 'PowerShell gate: clean -SkipTests run exits 0 with "Gate passed."'
    else
        report FAIL 'PowerShell gate: clean -SkipTests run exits 0 with "Gate passed."' "rc=$rc"
    fi

    # --- gate-vs-gate parity: step names and order must agree -----------------
    # Exit codes alone cannot catch a step renamed or dropped in one gate while
    # keeping exit 0; compare the ordered [step] headers from both outputs.
    step_list() {
        awk 'match($0, /^\[[^]]*\]/) {
            s = substr($0, RSTART, RLENGTH)
            if (s != prev) print s
            prev = s
        }' "$1"
    }
    bash_steps="$(step_list "$tmpdir/bash-ok")"
    ps_steps="$(step_list "$tmpdir/ps-ok")"
    if [ -n "$bash_steps" ] && [ -n "$ps_steps" ] && [ "$bash_steps" = "$ps_steps" ]; then
        report ok 'gates agree: same step names in the same order'
    else
        report FAIL 'gates agree: same step names in the same order' 'step lists differ between bash and PowerShell gates'
    fi

    # --- records step parity: both gates must run the v3 records step ---------
    if grep -qF '[record updates (feature_list.json + claude-progress.md)]' "$tmpdir/bash-ok"; then
        report ok 'bash gate: runs the manifest v3 record-updates step'
    else
        report FAIL 'bash gate: runs the manifest v3 record-updates step'
    fi
    if grep -qF '[record updates (feature_list.json + claude-progress.md)]' "$tmpdir/ps-ok"; then
        report ok 'PowerShell gate: runs the manifest v3 record-updates step'
    else
        report FAIL 'PowerShell gate: runs the manifest v3 record-updates step'
    fi
fi

# --- flag transforms: --fix / --all-features headers must match the manifest ---
# The transforms are the only logic still duplicated between the gates; assert
# both emit the transformed [step] headers. Assert the header, not the exit
# code: --all-features clippy legitimately fails on machines without GTK dev
# libraries, and the header prints before the step runs.
bash scripts/format-lint.sh --fix --skip-tests >"$tmpdir/bash-fix" 2>&1
if grep -qF '[cargo fmt --all]' "$tmpdir/bash-fix"; then
    report ok 'bash gate: --fix shows the transformed "cargo fmt --all" header'
else
    report FAIL 'bash gate: --fix shows the transformed "cargo fmt --all" header'
fi
bash scripts/format-lint.sh --all-features --skip-tests >"$tmpdir/bash-af" 2>&1
if grep -qF -- '--all-features -- -D warnings' "$tmpdir/bash-af"; then
    report ok 'bash gate: --all-features shows the transformed clippy header'
else
    report FAIL 'bash gate: --all-features shows the transformed clippy header'
fi
if [ -n "$ps" ]; then
    "$ps" -NoProfile -ExecutionPolicy Bypass -File \
        .agents/skills/format-lint/scripts/format-lint.ps1 -Fix -SkipTests >"$tmpdir/ps-fix" 2>&1
    if grep -qF '[cargo fmt --all]' "$tmpdir/ps-fix"; then
        report ok 'PowerShell gate: -Fix shows the transformed "cargo fmt --all" header'
    else
        report FAIL 'PowerShell gate: -Fix shows the transformed "cargo fmt --all" header'
    fi
    "$ps" -NoProfile -ExecutionPolicy Bypass -File \
        .agents/skills/format-lint/scripts/format-lint.ps1 -AllFeatures -SkipTests >"$tmpdir/ps-af" 2>&1
    if grep -qF -- '--all-features -- -D warnings' "$tmpdir/ps-af"; then
        report ok 'PowerShell gate: -AllFeatures shows the transformed clippy header'
    else
        report FAIL 'PowerShell gate: -AllFeatures shows the transformed clippy header'
    fi
fi

# --- forbidden diff path: bash gate -------------------------------------------
printf 'smoke test scratch\n' > "$forbidden_file"
bash scripts/format-lint.sh --skip-tests >"$tmpdir/bash-fb" 2>&1
rc=$?
if [ "$rc" -eq 1 ] && grep -q 'forbidden paths in the working diff' "$tmpdir/bash-fb"; then
    report ok 'bash gate: forbidden diff path exits 1 and is reported'
else
    report FAIL 'bash gate: forbidden diff path exits 1 and is reported' "rc=$rc"
fi

# --- forbidden diff path: PowerShell gate (when available) ----------------------
if [ -n "$ps" ]; then
    printf 'smoke test scratch\n' > "$forbidden_file"
    "$ps" -NoProfile -ExecutionPolicy Bypass -File \
        .agents/skills/format-lint/scripts/format-lint.ps1 -SkipTests \
        >"$tmpdir/ps-fb" 2>&1
    rc=$?
    if [ "$rc" -eq 1 ] && grep -q 'forbidden paths in the working diff' "$tmpdir/ps-fb"; then
        report ok 'PowerShell gate: forbidden diff path exits 1 and is reported'
    else
        report FAIL 'PowerShell gate: forbidden diff path exits 1 and is reported' "rc=$rc"
    fi
fi
restore_forbidden

# --- records step: staged code without record updates must fail ----------------
# Stage an untracked substantive scratch file (under scripts/, not ignored,
# not a forbidden path) and run both gates: the records step must fail with
# exit 1, proving the v3 step is wired into the staged-set check on both
# sides. The EXIT trap unstages and removes the scratch.
printf 'records smoke scratch\n' > "$records_scratch"
git add "$records_scratch"
bash scripts/format-lint.sh --skip-tests >"$tmpdir/bash-records" 2>&1
rc=$?
if [ "$rc" -eq 1 ] && grep -q 'record updates (feature_list.json + claude-progress.md)] FAILED' "$tmpdir/bash-records"; then
    report ok 'bash gate: staged code without record updates fails the records step'
else
    report FAIL 'bash gate: staged code without record updates fails the records step' "rc=$rc"
fi
git reset -q -- "$records_scratch"

if [ -n "$ps" ]; then
    git add "$records_scratch"
    "$ps" -NoProfile -ExecutionPolicy Bypass -File \
        .agents/skills/format-lint/scripts/format-lint.ps1 -SkipTests \
        >"$tmpdir/ps-records" 2>&1
    rc=$?
    if [ "$rc" -eq 1 ] && grep -q 'record updates (feature_list.json + claude-progress.md)] FAILED' "$tmpdir/ps-records"; then
        report ok 'PowerShell gate: staged code without record updates fails the records step'
    else
        report FAIL 'PowerShell gate: staged code without record updates fails the records step' "rc=$rc"
    fi
    git reset -q -- "$records_scratch"
fi
rm -f "$records_scratch"

# --- unknown flag: bash gate only ----------------------------------------------
# (The PowerShell twin rejects unknown parameters at parse time with a
# different exit code, so only the bash contract is asserted here.)
bash scripts/format-lint.sh --definitely-not-a-flag >"$tmpdir/bash-usage" 2>&1
rc=$?
if [ "$rc" -eq 2 ] && grep -q 'unknown option' "$tmpdir/bash-usage"; then
    report ok 'bash gate: unknown flag exits 2 with a usage message'
else
    report FAIL 'bash gate: unknown flag exits 2 with a usage message' "rc=$rc"
fi

# --- manifest: the single source of truth must parse on both sides ---------------
# Both gates read scripts/format-lint-steps.json; if the manifest breaks one
# parser, both the bash and PowerShell gates are suspect.
manifest_steps="$(grep -cE '^    \{ "id": ' scripts/format-lint-steps.json 2>/dev/null || true)"
if [ "$manifest_steps" -eq 6 ]; then
    report ok 'manifest: 6 steps defined in scripts/format-lint-steps.json'
else
    report FAIL 'manifest: 6 steps defined in scripts/format-lint-steps.json' "found $manifest_steps"
fi
manifest_version="$(sed -nE 's/.*"version": ([0-9]+).*/\1/p' scripts/format-lint-steps.json 2>/dev/null | head -1)"
if [ "$manifest_version" = 3 ]; then
    report ok 'manifest: version 3 (records step added)'
else
    report FAIL 'manifest: version 3 (records step added)' "found $manifest_version"
fi
if [ -n "$ps" ]; then
    if "$ps" -NoProfile -Command \
        'try { Get-Content -Raw scripts/format-lint-steps.json | ConvertFrom-Json | Out-Null; exit 0 } catch { Write-Error $_; exit 1 }' \
        >/dev/null 2>&1; then
        report ok 'manifest: parses as JSON in PowerShell'
    else
        report FAIL 'manifest: parses as JSON in PowerShell'
    fi
fi

# --- forbidden patterns: each manifest pattern must match a representative ---
# sample, and benign/near-miss paths must not match the combined check. This
# exercises the manifest patterns themselves on both parsers.
manifest_patterns=()
while IFS= read -r line; do
    p="$(sed -nE 's/^    "([^"]*)",?$/\1/p' <<<"$line")"
    p="${p//\\\\/\\}"
    [ -n "$p" ] && manifest_patterns+=("$p")
done < <(grep -E '^    "' scripts/format-lint-steps.json)
samples=(
    "target/release/volumectl.exe"
    ".superpowers/scratch.txt"
    ".claude/settings.local.json"
    ".claude/worktrees/agent-a/file"
    "config.json"
    "var/log/volume-control.log"
)
if [ "${#manifest_patterns[@]}" -ne "${#samples[@]}" ]; then
    report FAIL 'manifest: forbidden pattern count matches samples' \
        "${#manifest_patterns[@]} patterns vs ${#samples[@]} samples"
else
    for i in "${!manifest_patterns[@]}"; do
        if printf '%s\n' "${samples[$i]}" | grep -qE -- "${manifest_patterns[$i]}"; then
            report ok "forbidden pattern matches ${samples[$i]}"
        else
            report FAIL "forbidden pattern matches ${samples[$i]}" "${manifest_patterns[$i]}"
        fi
    done
fi
combined_pattern="$(IFS='|'; printf '%s' "${manifest_patterns[*]}")"
# Near-misses that must NOT match. Under ci-diff-check.sh semantics the
# patterns are prefix/contains anchored, so e.g. 'config.json.bak' and
# 'foo.log.bak' WOULD match (CI parity) and are deliberately absent here.
for benign in 'src/main.rs' 'target-foo.txt' 'configx.json' 'logo.txt'; do
    if printf '%s\n' "$benign" | grep -qE -- "$combined_pattern"; then
        report FAIL "forbidden patterns reject benign path $benign"
    else
        report ok "forbidden patterns reject benign path $benign"
    fi
done
if [ -n "$ps" ]; then
    if "$ps" -NoProfile -Command \
        '
        $m = Get-Content -Raw scripts/format-lint-steps.json | ConvertFrom-Json
        $fp = @($m.forbidden_patterns)
        $samples = @("target/release/volumectl.exe", ".superpowers/scratch.txt", ".claude/settings.local.json", ".claude/worktrees/agent-a/file", "config.json", "var/log/volume-control.log")
        $ok = $true
        for ($i = 0; $i -lt $fp.Count; $i++) {
            if ($samples[$i] -notmatch $fp[$i]) { Write-Output "FAIL $($fp[$i])"; $ok = $false }
        }
        if ("src/main.rs" -match ($fp -join "|")) { Write-Output "FAIL benign"; $ok = $false }
        if ($ok) { exit 0 } else { exit 1 }
        ' \
        >/dev/null 2>&1; then
        report ok 'forbidden patterns: all match their samples in PowerShell'
    else
        report FAIL 'forbidden patterns: all match their samples in PowerShell'
    fi
fi

# --- mirror drift: .claude skill copies must match the canonical gates ---------
if cmp -s .agents/skills/format-lint/scripts/format-lint.ps1 \
          .claude/skills/format-lint/scripts/format-lint.ps1; then
    report ok 'mirrors: format-lint.ps1 copies are byte-identical'
else
    report FAIL 'mirrors: format-lint.ps1 copies differ (resync .claude/skills/format-lint/)'
fi
if cmp -s scripts/format-lint.sh \
          .claude/skills/format-lint/scripts/format-lint.sh; then
    report ok 'mirrors: format-lint.sh copies are byte-identical'
else
    report FAIL 'mirrors: format-lint.sh copies differ (resync .claude/skills/format-lint/)'
fi


if [ "$failures" -eq 0 ]; then
    echo "All format-lint smoke checks passed."
    exit 0
fi
echo "$failures check(s) failed." >&2
exit 1
