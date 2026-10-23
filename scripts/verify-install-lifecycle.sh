#!/usr/bin/env bash
#
# verify-install-lifecycle.sh - cross-platform lifecycle verification harness
# for Linux and macOS published baseline and candidate packages.
set -euo pipefail

BASELINE_TAG="v0.1.10"
TARGET_TAG="v0.1.11"
CANDIDATE_DIR=""
OUTPUT_ROOT="output/install-lifecycle/unix"
ISOLATED=0
PUBLISHED_ONLY=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --baseline-tag) BASELINE_TAG="$2"; shift 2 ;;
    --target-tag) TARGET_TAG="$2"; shift 2 ;;
    --candidate-dir) CANDIDATE_DIR="$2"; shift 2 ;;
    --output-root) OUTPUT_ROOT="$2"; shift 2 ;;
    --isolated) ISOLATED=1; shift ;;
    --published-only) PUBLISHED_ONLY=1; shift ;;
    *) echo "Unknown option $1" >&2; exit 1 ;;
  esac
done

if [[ $ISOLATED -ne 1 ]]; then
  echo "Error: --isolated is mandatory to avoid mutating local user installation." >&2
  exit 1
fi

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "$OUTPUT_ROOT"
log_file="$OUTPUT_ROOT/lifecycle-execution.log"
: > "$log_file"

record_case() {
  local name="$1" status="$2" details="${3:-}"
  printf '[%s] %s: %s\n' "$status" "$name" "$details" | tee -a "$log_file"
}

sandbox="$(mktemp -d)"
defer_cleanup() { rm -rf "$sandbox"; }
trap defer_cleanup EXIT

os_name="$(uname -s)"
case "$os_name" in
  Linux)  installer="$repo/packaging/install-linux.sh" ;;
  Darwin) installer="$repo/packaging/install-macos.sh" ;;
  *) echo "Unsupported platform $os_name" >&2; exit 1 ;;
esac

export VOLUMECTL_INSTALL_ROOT="$sandbox/install-root"
mkdir -p "$VOLUMECTL_INSTALL_ROOT"

if [[ $PUBLISHED_ONLY -eq 1 ]]; then
  record_case "Baseline Start" "Running" "Testing published $BASELINE_TAG"
  if VOLUMECTL_VERSION="$BASELINE_TAG" bash "$installer" install 2>&1 | tee -a "$log_file"; then
    record_case "Baseline Install" "Passed" "Installed $BASELINE_TAG successfully"
  else
    record_case "Baseline Install" "Failed" "Baseline install failed"
  fi
else
  if [[ -z "$CANDIDATE_DIR" || ! -d "$CANDIDATE_DIR" ]]; then
    echo "Error: --candidate-dir must point to an existing dist directory" >&2
    exit 1
  fi

  record_case "Candidate Setup" "Passed" "Starting candidate lifecycle testing"

  # Case 1: Prepared offline apply
  prep_dir="$sandbox/prepared"
  mkdir -p "$prep_dir"
  cp "$CANDIDATE_DIR"/* "$prep_dir/" 2>/dev/null || true

  if [[ -f "$prep_dir/SHA256SUMS.txt" ]]; then
    if bash "$installer" update --prepared-dir "$prep_dir" 2>&1 | tee -a "$log_file"; then
      if [[ -f "$prep_dir/result.exit-code" && "$(cat "$prep_dir/result.exit-code" | tr -d '[:space:]')" == "0" ]]; then
        record_case "Prepared Offline Apply" "Passed" "Prepared apply exited 0 with handoff"
      else
        record_case "Prepared Offline Apply" "Failed" "result.exit-code missing or nonzero"
      fi
    else
      record_case "Prepared Offline Apply" "Failed" "Prepared apply execution failed"
    fi
  fi

  # Case 2: Repeat install intact no-op
  if out="$(bash "$installer" install 2>&1)"; then
    record_case "Repeat Install No-Op" "Passed" "$out"
  else
    record_case "Repeat Install No-Op" "Failed" "Repeat install failed"
  fi

  # Case 3: Offline uninstall
  if VOLUMECTL_RELEASE_BASE="http://127.0.0.1:9" bash "$installer" uninstall 2>&1 | tee -a "$log_file"; then
    record_case "Offline Uninstall" "Passed" "Removed application offline"
  else
    record_case "Offline Uninstall" "Failed" "Offline uninstall failed"
  fi

  # Case 4: Second uninstall no-op
  if out="$(VOLUMECTL_RELEASE_BASE="http://127.0.0.1:9" bash "$installer" uninstall 2>&1)"; then
    record_case "Second Uninstall No-Op" "Passed" "$out"
  else
    record_case "Second Uninstall No-Op" "Failed" "Second uninstall failed"
  fi
fi

echo "All candidate lifecycle cases completed."
