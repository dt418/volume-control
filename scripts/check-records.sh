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
        # An empty change set feeds one empty heredoc line; skip it so
        # empty lists pass (nothing changed -> nothing to require).
        [ -z "$path" ] && continue
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
    staged_list="$(git diff --cached --name-only 2>&1)"
    rc=$?
    if [ "$rc" -ne 0 ]; then
        echo "FAIL - git diff --cached failed (exit $rc):" >&2
        printf '%s\n' "$staged_list" | sed 's/^/      /' >&2
        return 1
    fi
    collect_list <<EOF
$staged_list
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
    branch_list="$(git diff --name-only "$base...HEAD" 2>&1)"
    rc=$?
    if [ "$rc" -ne 0 ]; then
        echo "FAIL - git diff vs '$base' failed (exit $rc):" >&2
        printf '%s\n' "$branch_list" | sed 's/^/      /' >&2
        return 1
    fi
    untracked_list="$(git ls-files --others --exclude-standard 2>&1)"
    rc=$?
    if [ "$rc" -ne 0 ]; then
        echo "FAIL - git ls-files failed (exit $rc):" >&2
        printf '%s\n' "$untracked_list" | sed 's/^/      /' >&2
        return 1
    fi
    collect_list <<EOF
$branch_list
$untracked_list
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
