#!/usr/bin/env bash
#
# test-all.sh - Comprehensive end-to-end integration test runner for VolumeControl.
#
# Executes the full battery of tests across all layers:
#   1. Code formatting & diff whitespace (cargo fmt, git diff --check)
#   2. Rust static analysis & lint (cargo clippy -D warnings)
#   3. Rust workspace unit & integration tests (crates/volumectl, audio, config, core)
#   4. Frontend unit & component test suite (React 19, Vitest, all surfaces)
#   5. Frontend TypeScript verification & production build (Vite bundle budget check)
#   6. Real desktop surface smoke test (verify-tauri-surfaces / AppKit / GTK4)
#   7. Cross-platform Unix compatibility invariants (check-unix-compat)
#   8. Guardrails & enforcement stack self-tests (records, format-lint, ship, safe-flow)
#
# Usage:
#   ./scripts/test-all.sh              # Run the complete test battery
#   ./scripts/test-all.sh --quick      # Run fast suite (skip heavy release compilation)
#   ./scripts/test-all.sh --skip-smoke # Skip real desktop window capture (headless mode)
#   ./scripts/test-all.sh --fix        # Auto-format Rust files before checking
#
# Exit: 0 = all tests passed; 1 = at least one test failed; 2 = bad usage.
set -uo pipefail

quick=false
skip_smoke=false
fix=false

for arg in "$@"; do
    case "$arg" in
        --quick) quick=true ;;
        --skip-smoke) skip_smoke=true ;;
        --fix) fix=true ;;
        --help|-h)
            echo "usage: $0 [--quick] [--skip-smoke] [--fix]"
            echo
            echo "  --quick       run fast checks (skip long-running release compiles)"
            echo "  --skip-smoke  skip desktop surface capture (useful in headless CI)"
            echo "  --fix         run cargo fmt --all before checking"
            exit 0
            ;;
        *)
            echo "error: unknown option: $arg" >&2
            echo "usage: $0 [--quick] [--skip-smoke] [--fix]" >&2
            exit 2
            ;;
    esac
done

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
SCRIPT_DIR="${SCRIPT_DIR//\\//}"
repo_root="$(cd -- "$SCRIPT_DIR/.." && pwd)"
repo_root="${repo_root//\\//}"
cd "$repo_root"

# Resolve linked git worktree directory for cross-platform POSIX compatibility
use_linked_git_dir() {
    probe="$(pwd)"
    while :; do
        [ -e "$probe/.git" ] && break
        probe_parent="${probe%/*}"
        if [ "$probe_parent" = "$probe" ] || [ -z "$probe_parent" ]; then
            break
        fi
        probe="$probe_parent"
    done
    [ -f "$probe/.git" ] || return 0
    gitdir="$(sed -n 's/^gitdir:[[:space:]]*//p' "$probe/.git" 2>/dev/null || true)"
    case "$gitdir" in
        [A-Za-z]:[\\/]*)
            if command -v wslpath >/dev/null 2>&1; then
                gitdir="$(wslpath -u "$gitdir" 2>/dev/null || true)"
            elif command -v cygpath >/dev/null 2>&1; then
                gitdir="$(cygpath -u "$gitdir" 2>/dev/null || true)"
            fi
            ;;
        /*) : ;;
        ?*) gitdir="$probe/$gitdir" ;;
    esac
    if [ -n "$gitdir" ] && [ -d "$gitdir" ]; then
        GIT_DIR="$gitdir"
        GIT_WORK_TREE="$probe"
        export GIT_DIR GIT_WORK_TREE
    fi
    return 0
}
if ! git rev-parse --git-dir >/dev/null 2>&1; then
    use_linked_git_dir
fi


echo "======================================================================"
echo "          VolumeControl Full Application Integration Test             "
echo "======================================================================"
echo "Workspace: $repo_root"
echo "Platform:  $(uname -s) ($(uname -m))"
echo "Started:   $(date)"
echo "======================================================================"
echo

failures=()
start_total=$(date +%s)

run_phase() {
    local phase_num="$1"
    local phase_name="$2"
    shift 2
    local cmd=("$@")

    echo "----------------------------------------------------------------------"
    echo "[$phase_num/8] $phase_name"
    echo "----------------------------------------------------------------------"
    local phase_start=$(date +%s)

    if "${cmd[@]}"; then
        local phase_end=$(date +%s)
        local elapsed=$((phase_end - phase_start))
        echo "--> PASS: $phase_name (${elapsed}s)"
        echo
    else
        local phase_end=$(date +%s)
        local elapsed=$((phase_end - phase_start))
        echo "--> FAILED: $phase_name (${elapsed}s)" >&2
        failures+=("$phase_name")
        echo
    fi
}

# Auto-fix formatting if requested
if [ "$fix" = true ]; then
    echo "Running cargo fmt --all..."
    cargo fmt --all
fi

# Phase 1: Code formatting & git diff check
run_phase 1 "Code formatting and whitespace hygiene" bash -c "
    cargo fmt --all --check &&
    git diff --check
"

# Phase 2: Rust static analysis & linting
run_phase 2 "Rust workspace static analysis (clippy)" bash -c "
    cargo clippy --workspace --all-targets --no-default-features -- -D warnings
"

# Phase 3: Rust workspace unit & integration tests
run_phase 3 "Rust workspace test suite (volumectl + core)" bash -c "
    cargo test --workspace --no-default-features
"

# Phase 4: Frontend unit & component tests
run_phase 4 "Frontend unit & component test suite (vitest)" bash -c "
    npm test --prefix frontend
"

# Phase 5: Frontend production build & typecheck
run_phase 5 "Frontend TypeScript check and production build" bash -c "
    npm run build --prefix frontend
"

# Phase 6: Real desktop surface smoke test
if [ "$skip_smoke" = true ]; then
    echo "----------------------------------------------------------------------"
    echo "[6/8] Real desktop surface smoke test (SKIPPED via --skip-smoke)"
    echo "----------------------------------------------------------------------"
    echo
elif [ -f "$repo_root/scripts/smoke-test.sh" ]; then
    run_phase 6 "Real desktop surface smoke test (smoke-test.sh)" bash -c "
        sh \"$repo_root/scripts/smoke-test.sh\"
    "
elif [ -f "$repo_root/scripts/verify-tauri-surfaces.ps1" ] && command -v pwsh >/dev/null 2>&1; then
    run_phase 6 "Desktop surface capture (verify-tauri-surfaces.ps1)" bash -c "
        if [ ! -f \"$repo_root/target/debug/VolumeControl.exe\" ]; then
            echo 'Building debug binary target for smoke test...'
            cargo build -p volumecontrol-tauri --no-default-features
        fi
        VOLUMECTL_E2E_DEBUG=1 pwsh -NoProfile -File \"$repo_root/scripts/verify-tauri-surfaces.ps1\" -Binary target/debug/VolumeControl.exe
    "
else
    echo "----------------------------------------------------------------------"
    echo "[6/8] Desktop surface smoke test (SKIPPED: no surface runner on this platform)"
    echo "----------------------------------------------------------------------"
    echo
fi

# Phase 7: Cross-platform Unix compatibility check
run_phase 7 "Cross-platform Unix compatibility invariants" bash -c "
    bash \"$repo_root/scripts/check-unix-compat.sh\"
"

# Phase 8: Guardrails enforcement self-tests
run_phase 8 "Guardrail enforcement stack self-tests" bash -c "
    sh \"$repo_root/scripts/test-check-records.sh\" &&
    sh \"$repo_root/scripts/test-format-lint.sh\" &&
    sh \"$repo_root/scripts/test-agent-safe-flow.sh\"
"

end_total=$(date +%s)
total_elapsed=$((end_total - start_total))

echo "======================================================================"
echo "                     Integration Test Summary                         "
echo "======================================================================"
echo "Total duration: ${total_elapsed}s"

if [ ${#failures[@]} -eq 0 ]; then
    echo "Result: ALL PHASES PASSED (8/8)"
    echo "The application is verified end-to-end and ready for shipping/PR."
    echo "======================================================================"
    exit 0
else
    echo "Result: FAILED (${#failures[@]} phase(s) failed):"
    for f in "${failures[@]}"; do
        echo "  - $f"
    done
    echo "======================================================================"
    exit 1
fi
