#!/usr/bin/env bash
#
# ship.sh - mandatory pre-ship flow for volume-control (bash).
#
# The single entry point that guarantees the repository flow is ALWAYS
# applied before anything ships. Running this (or scripts/ship.ps1 on
# Windows, which bridges to it) is the supported path to commit + push.
#
# Hard checks - NO flag can skip these:
#   1. check-records.sh --branch origin/master  records updated somewhere
#      in the change set (skipped with a warning only when there is no
#      origin/master to diff against; the staged check and pre-commit hook
#      still enforce the rule then)
#   2. scripts/format-lint.sh                   the FULL gate, tests included
#      (fmt, diff, forbidden paths, records, clippy, test)
#   3. scripts/test-check-records.sh            the records guard itself
#      must be intact
#   4. scripts/test-format-lint.sh              the gate chain itself must
#      be intact
#   5. check-records.sh --staged                the exact commit set,
#      re-checked AFTER staging
#
# Soft preconditions - relaxed ONLY with --force, never by default:
#   - --push while HEAD is behind origin/master (refused early so you pull
#     first; with --force, git itself rejects the non-fast-forward)
#   - --push with no 'origin' remote (hard refuse; --force does not help)
#
# Usage:
#   ./scripts/ship.sh                 # verify, stage, commit (no push)
#   ./scripts/ship.sh --push          # verify, stage, commit, push
#   ./scripts/ship.sh --dry-run       # run every hard check, change nothing
#   ./scripts/ship.sh --force         # relax soft preconditions (hygiene only)
#   ./scripts/ship.sh --message "..." # commit message (default below)
#
# Exit: 0 = ok; 1 = a hard check or commit/push failed; 2 = bad usage.
set -uo pipefail

push_flag=false
force_flag=false
dry_run=false
message="chore: ship (scripts/ship.sh flow verified: records + gates + tests)"

usage() {
    echo "usage: $0 [--push] [--force] [--dry-run] [--message MSG]"
    echo
    echo "  --push        also push after the verified commit"
    echo "  --force       relax soft preconditions (git hygiene) only;"
    echo "                the hard checks (records, gates, self-tests) always run"
    echo "  --dry-run     run every hard check, change nothing"
    echo "  --message MSG commit message (default: '$message')"
    echo
    echo "There are NO bypass flags: records, format-lint (with tests), and"
    echo "both self-tests always run before anything is committed."
}

i=0
args=("$@")
while [ "$i" -lt "$#" ]; do
    arg="${args[$i]}"
    case "$arg" in
        --push)   push_flag=true ;;
        --force)  force_flag=true ;;
        --dry-run) dry_run=true ;;
        --message)
            i=$((i + 1))
            if [ "$i" -ge "$#" ]; then
                echo "error: --message requires an argument" >&2
                exit 2
            fi
            message="${args[$i]}"
            ;;
        --message=*) message="${arg#--message=}" ;;
        --help|-h) usage; exit 0 ;;
        *)
            echo "error: unknown option: $arg" >&2
            usage >&2
            exit 2
            ;;
    esac
    i=$((i + 1))
done

# Validate after the loop: an empty message would otherwise surface as a
# confusing `git commit` error instead of a proper usage error.
if [ -z "$message" ]; then
    echo "error: --message requires a non-empty message" >&2
    exit 2
fi

# -- Tool resolution ----------------------------------------------------------
# Same policy as scripts/format-lint.sh: PATH first, clear failure message.
GIT_BIN="$(command -v git 2>/dev/null || true)"
if [ -z "$GIT_BIN" ]; then
    echo "error: git not found on PATH" >&2
    exit 1
fi
SH_BIN="$(command -v sh 2>/dev/null || true)"
if [ -z "$SH_BIN" ]; then
    echo "error: sh not found on PATH" >&2
    exit 1
fi

# -- Repository root resolution ----------------------------------------------
# Walk up from the script until the workspace Cargo.toml is found, so the
# flow always runs from the repository root no matter where it is invoked.
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$SCRIPT_DIR"
while [ -n "$repo_root" ] && [ ! -f "$repo_root/Cargo.toml" ]; do
    parent="$(dirname -- "$repo_root")"
    [ "$parent" = "$repo_root" ] && break
    repo_root="$parent"
done
if [ ! -f "$repo_root/Cargo.toml" ]; then
    echo "error: could not locate the workspace root (no Cargo.toml) above $SCRIPT_DIR" >&2
    exit 1
fi
cd "$repo_root"

fail() {
    echo "ship FAILED - $1" >&2
    exit 1
}

# Push the current branch. Soft preconditions apply here (see the header);
# defined before use so the phase bodies below can call it.
do_push() {
    if "$GIT_BIN" rev-parse --verify --quiet origin >/dev/null 2>&1; then
        if "$GIT_BIN" rev-parse --verify --quiet origin/master >/dev/null 2>&1; then
            behind="$("$GIT_BIN" rev-list --count HEAD..origin/master 2>/dev/null || echo 0)"
            if [ "${behind:-0}" -gt 0 ] && [ "$force_flag" = false ]; then
                echo "error: HEAD is $behind commit(s) behind origin/master; pull first" >&2
                echo "       (or re-run with --force to let git itself reject a non-fast-forward)" >&2
                exit 1
            fi
        fi
        if "$GIT_BIN" rev-parse --abbrev-ref --symbolic-full-name '@{u}' >/dev/null 2>&1; then
            "$GIT_BIN" push
        else
            echo "no upstream set for the current branch; pushing with -u origin HEAD"
            "$GIT_BIN" push -u origin HEAD
        fi
    else
        echo "error: no 'origin' remote configured; cannot push" >&2
        exit 1
    fi
}

echo "[1/5] records guard (branch change set vs origin/master)"
if "$GIT_BIN" rev-parse --verify --quiet origin/master >/dev/null 2>&1; then
    if ! "$SH_BIN" "$repo_root/scripts/check-records.sh" --branch origin/master; then
        fail "the change set misses feature_list.json and/or claude-progress.md (see templates above)"
    fi
else
    echo "      no origin/master to diff against - branch check skipped;"
    echo "      the staged check (step 5) and the pre-commit hook still enforce the rule"
fi

# ---- Phase 2: full format-lint gate (tests included) -----------------------
echo "[2/5] format-lint gate (full, tests included)"
if ! bash "$repo_root/scripts/format-lint.sh"; then
    fail "the format-lint gate reported failures above"
fi

# ---- Phase 3: records-guard self-test ---------------------------------------
echo "[3/5] records-guard self-test"
if ! bash "$repo_root/scripts/test-check-records.sh"; then
    fail "the records-guard self-test reported failures above"
fi

# ---- Phase 4: format-lint smoke test -----------------------------------------
# test-format-lint.sh requires a clean index (its records-step negatives read
# the staged set); warn early so a pre-staged index does not surprise.
if ! "$GIT_BIN" diff --cached --quiet; then
    echo "note: the index already has staged changes; test-format-lint.sh requires" >&2
    echo "      a clean index. If it fails below, run 'git reset' first, then re-run ship." >&2
fi
echo "[4/5] format-lint smoke test"
if ! bash "$repo_root/scripts/test-format-lint.sh"; then
    fail "the format-lint smoke test reported failures above"
fi

# ---- dry-run: verified, change nothing ---------------------------------------
if [ "$dry_run" = true ]; then
    echo
    echo "ship dry-run: all hard checks passed; nothing was changed."
    exit 0
fi

# ---- Phase 5: stage everything, re-check the exact commit set ----------------
echo "[5/5] staging the working tree"
if ! "$GIT_BIN" add -A; then
    fail "git add -A failed (see the output above); the staged set may be incomplete"
fi
if ! "$SH_BIN" "$repo_root/scripts/check-records.sh" --staged; then
    fail "the staged set violates the records rule (see templates above)"
fi

staged_count="$("$GIT_BIN" diff --cached --name-only | wc -l | tr -d ' ')"
if [ "$staged_count" -eq 0 ]; then
    if [ "$push_flag" = true ]; then
        echo "nothing new to commit; pushing existing commits"
        do_push
        exit 0
    fi
    echo "nothing to commit - the working tree and index are clean."
    echo "ship ok (nothing to do)."
    exit 0
fi

echo
echo "Committing $staged_count file(s):"
"$GIT_BIN" status --short | sed 's/^/  /'
echo
echo "Running: git commit -m \"$message\""
echo "(the pre-commit hook re-checks records, fmt, and clippy on this set)"
if ! "$GIT_BIN" commit -m "$message"; then
    fail "the commit was rejected (see the hook output above)"
fi

if [ "$push_flag" = true ]; then
    do_push
fi

echo
echo "ship ok - the flow was applied and the change is committed."
exit 0
