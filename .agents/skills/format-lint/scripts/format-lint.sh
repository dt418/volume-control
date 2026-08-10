#!/usr/bin/env bash
#
# format-lint.sh - deterministic quality gate for volume-control (bash).
#
# Mirrors the CI checks (see .github/workflows/ci.yml and .githooks/pre-commit)
# and the PowerShell gate (.agents/skills/format-lint/scripts/format-lint.ps1).
# The step list is defined ONCE in scripts/format-lint-steps.json and both
# gates execute it, so the two implementations cannot drift. Flags only
# transform the manifest's default steps (see SKILL.md).
#
# Usage:
#   ./scripts/format-lint.sh                # fmt --check, diff checks, clippy, tests
#   ./scripts/format-lint.sh --fix          # apply `cargo fmt --all`, then the full gate
#   ./scripts/format-lint.sh --skip-tests   # format/lint only
#   ./scripts/format-lint.sh --all-features # include gtk-renderer/layer-shell (needs GTK dev libs)
#
# Exit code 0 = gate passed; 1 = a step failed; 2 = bad usage.
# No `set -e`: steps are if-wrapped so every step runs and all failures are
# reported together before the final exit code is chosen.
set -uo pipefail

fix=false
skip_tests=false
all_features=false

for arg in "$@"; do
    case "$arg" in
        --fix|-fix) fix=true ;;
        --skip-tests|-skip-tests) skip_tests=true ;;
        --all-features|-all-features) all_features=true ;;
        *)
            echo "error: unknown option: $arg" >&2
            echo "usage: $0 [--fix] [--skip-tests] [--all-features]" >&2
            exit 2
            ;;
    esac
done

# -- Toolchain resolution ----------------------------------------------------
# cargo: PATH first, then ~/.cargo/bin (common when the process environment
# snapshot is stale; the pre-commit hook handles the same case).
CARGO_BIN="$(command -v cargo 2>/dev/null || true)"
if [ -z "$CARGO_BIN" ] && [ -x "${HOME:-}/.cargo/bin/cargo" ]; then
    CARGO_BIN="${HOME:-}/.cargo/bin/cargo"
fi
if [ -z "$CARGO_BIN" ]; then
    echo "error: cargo not found on PATH and not at ~/.cargo/bin/cargo" >&2
    exit 1
fi

# git: PATH is the only realistic location outside Windows.
GIT_BIN="$(command -v git 2>/dev/null || true)"
if [ -z "$GIT_BIN" ]; then
    echo "error: git not found on PATH" >&2
    exit 1
fi

# -- Repository root resolution ----------------------------------------------
# Walk up from the script until the workspace Cargo.toml is found, so the gate
# always runs from the repository root no matter where it is invoked from.
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

# -- Step manifest (single source of truth) -----------------------------------
# JSON, one step object per line, values free of embedded quotes; args are
# space-free tokens so they can be split on IFS. Keep format-lint-steps.json
# and the PowerShell gate's ConvertFrom-Json consumption in lockstep.
manifest="$repo_root/scripts/format-lint-steps.json"
if [ ! -r "$manifest" ]; then
    echo "error: step manifest not found at $manifest" >&2
    exit 1
fi
manifest_version="$(sed -nE 's/.*"version": ([0-9]+).*/\1/p' "$manifest" | head -1)"
if [ "$manifest_version" != "3" ]; then
    echo "error: unsupported manifest version $manifest_version (expected 3)" >&2
    exit 1
fi

# Forbidden-diff-path patterns (single source of truth in the manifest).
# The JSON source escapes a literal backslash as \\; unescape \\ -> \ so the
# extracted patterns are the real EREs.
forbidden_patterns=()
while IFS= read -r line; do
    p="$(sed -nE 's/^    "([^"]*)",?$/\1/p' <<<"$line")"
    p="${p//\\\\/\\}"
    [ -n "$p" ] && forbidden_patterns+=("$p")
done < <(grep -E '^    "' "$manifest")
if [ "${#forbidden_patterns[@]}" -eq 0 ]; then
    echo "error: no forbidden_patterns found in $manifest" >&2
    exit 1
fi

step_lines=()
while IFS= read -r line; do
    step_lines+=("$line")
done < <(grep -E '^    \{ "id": ' "$manifest")
if [ "${#step_lines[@]}" -eq 0 ]; then
    echo "error: no steps found in $manifest" >&2
    exit 1
fi

failed=false

# Run one gate step: print [name], run the command, report OK or FAILED with
# its exit code, and keep going so the summary shows every failure.
run_step() {
    local name="$1"
    shift
    echo
    printf '[%s]\n' "$name"
    if "$@"; then
        printf '[%s] OK\n' "$name"
    else
        local rc=$?
        printf '[%s] FAILED (exit %s)\n' "$name" "$rc"
        failed=true
    fi
}

# Record-keeping guard: reject a commit that changes substantive files
# without updating both records. Delegates to the single implementation in
# scripts/check-records.sh --staged (no duplicated rule logic here).
check_record_updates() {
    if ! sh "$repo_root/scripts/check-records.sh" --staged; then
        echo '  stage updates to feature_list.json and claude-progress.md with this change' >&2
        return 1
    fi
}

# Local mirror of scripts/ci-diff-check.sh: reject forbidden paths in the
# working diff (tracked edits and untracked additions). The pattern list is
# read from the manifest (forbidden_patterns), joined into one ERE.
check_forbidden_paths() {
    local pattern tracked untracked
    pattern="$(IFS='|'; printf '%s' "${forbidden_patterns[*]}")"
    # Let git's stderr through (e.g. the LF/CRLF warning) so genuine errors
    # are visible, matching the PowerShell twin; the exit code is checked.
    tracked="$(git diff HEAD --name-only --diff-filter=ACMRD)"
    if [ $? -ne 0 ]; then
        echo '  git command failed while listing diff paths' >&2
        return 1
    fi
    untracked="$(git ls-files --others --exclude-standard)"
    if [ $? -ne 0 ]; then
        echo '  git command failed while listing untracked files' >&2
        return 1
    fi
    local matches
    matches="$(printf '%s\n%s\n' "$tracked" "$untracked" | grep -E "$pattern" || true)"
    if [ -n "$matches" ]; then
        echo '  forbidden paths in the working diff:' >&2
        printf '%s\n' "$matches" | sed 's/^/  /' >&2
        return 1
    fi
}

# Apply the flag transforms to a manifest step's args (default form -> flag
# form). Emits one word per line.
transform_args() {
    local id="$1"
    shift
    local w
    for w in "$@"; do
        case "$id" in
            fmt)
                [ "$fix" = true ] && [ "$w" = "--check" ] && continue
                ;;
            clippy|test)
                if [ "$all_features" = true ] && [ "$w" = "--no-default-features" ]; then
                    w="--all-features"
                fi
                ;;
        esac
        printf '%s\n' "$w"
    done
}

for line in "${step_lines[@]}"; do
    id="$(sed -nE 's/.*"id": "([^"]+)".*/\1/p' <<<"$line")"
    name="$(sed -nE 's/.*"name": "([^"]+)".*/\1/p' <<<"$line")"
    skip="$(sed -nE 's/.*"skip_when": "([^"]+)".*/\1/p' <<<"$line")"
    is_internal=false
    grep -q '"internal":' <<<"$line" && is_internal=true

    # Fail loudly on an unparseable line instead of running under a truncated
    # or empty id/name.
    if [ -z "$id" ] || [ -z "$name" ]; then
        echo "error: unparseable step line in $manifest: $line" >&2
        exit 1
    fi

    # Flag transforms on the displayed name.
    case "$id" in
        fmt)
            [ "$fix" = true ] && name="${name% --check}"
            ;;
        clippy|test)
            [ "$all_features" = true ] && name="${name//--no-default-features/--all-features}"
            ;;
    esac

    if [ -n "$skip" ]; then
        case "$skip" in
            skip_tests) [ "$skip_tests" = true ] && continue ;;
            *)
                echo "error: unknown skip_when '$skip' in manifest step '$id'" >&2
                exit 1
                ;;
        esac
    fi

    if [ "$is_internal" = true ]; then
        internal_id="$(sed -nE 's/.*"internal": "([^"]+)".*/\1/p' <<<"$line")"
        case "$internal_id" in
            forbidden_paths) internal_fn=check_forbidden_paths ;;
            record_updates)  internal_fn=check_record_updates ;;
            *)
                echo "error: unknown internal step '$internal_id' in manifest step '$id'" >&2
                exit 1
                ;;
        esac
        run_step "$name" "$internal_fn"
        continue
    fi

    argsraw="$(sed -nE 's/.*"args": \[(.*)\].*/\1/p' <<<"$line")"
    if [ -z "$argsraw" ]; then
        echo "error: step '$id' has no args in $manifest" >&2
        exit 1
    fi
    args="$(sed -E 's/", "/ /g; s/"//g; s/\[//g; s/\]//g' <<<"$argsraw")"
    read -r -a words <<<"$args"

    case "${words[0]}" in
        cargo) tool="$CARGO_BIN" ;;
        git)   tool="$GIT_BIN" ;;
        *)
            echo "error: unknown tool '${words[0]}' in manifest step '$id'" >&2
            exit 1
            ;;
    esac

    # shellcheck disable=SC2207
    words=( $(transform_args "$id" "${words[@]}") )
    run_step "$name" "$tool" "${words[@]:1}"
done

if [ "$failed" = true ]; then
    echo
    echo "Gate FAILED - fix the reported step before committing." >&2
    exit 1
fi
echo
echo "Gate passed."
exit 0
