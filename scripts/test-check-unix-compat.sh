#!/usr/bin/env bash
set -euo pipefail

# test-check-unix-compat.sh - regression smoke tests for check-unix-compat.sh

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo"

pass() {
  echo "ok   - $1"
}

fail() {
  echo "FAIL - $1" >&2
  exit 1
}

# 1. Clean run passes
if bash scripts/check-unix-compat.sh >/dev/null 2>&1; then
  pass "clean repo passes check-unix-compat.sh"
else
  fail "clean repo failed check-unix-compat.sh"
fi

# 2. Rejection of signals.forever() in for-loop
tmp_file="src-tauri/src/__test_sig_leak.rs"
trap 'rm -f "$tmp_file"' EXIT

echo 'fn dummy(signals: &signal_hook::iterator::Signals) { for s in signals.forever() { break; } }' > "$tmp_file"
if bash scripts/check-unix-compat.sh >/dev/null 2>&1; then
  fail "check-unix-compat.sh should have rejected for .. in signals.forever()"
else
  pass "check-unix-compat.sh catches for .. in signals.forever()"
fi
rm -f "$tmp_file"

echo "All check-unix-compat tests passed."
