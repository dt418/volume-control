#!/usr/bin/env bash
set -euo pipefail

# Cross-platform Unix compatibility guard (see .agents/skills/cross-platform-unix-guard/SKILL.md).
#
# Enforces the invariants that prevent Unix (Linux/macOS) CI breakage:
# 1. No never_loop patterns on signal_hook::iterator::Signals::forever().
# 2. Conditionally-compiled parameters across target cfgs are suppressed or underscored.
# 3. E2E Settings specs isolate surface section selection before checking section content.

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo"

failures=0
fail() {
  echo "FAIL: $1" >&2
  failures=1
}
pass() {
  echo "ok: $1"
}

# 1. Signal hook never_loop check.
# `for sig in signals.forever()` followed by `break` inside the loop triggers clippy::never_loop.
# Single-iteration signal handling should use `if let Some(sig) = signals.forever().next()`.
if find src-tauri/src crates/volumectl/src -name '*.rs' -exec awk '
  FNR == 1 { in_loop = 0; brace_depth = 0; seen_open = 0; found_break = 0 }
  /for[[:space:]]+.*[[:space:]]+in[[:space:]]+.*signals\.forever\(\)/ {
    in_loop = 1; brace_depth = 0; seen_open = 0; found_break = 0
    idx = match($0, /for[[:space:]]+.*[[:space:]]+in[[:space:]]+.*signals\.forever\(\)/)
    line = substr($0, idx)
  }
  in_loop {
    if (!line) line = $0
    sub(/\/\/.*$/, "", line)
    n = length(line)
    for (i = 1; i <= n; i++) {
      c = substr(line, i, 1)
      if (c == "{") { brace_depth++; seen_open = 1 }
      else if (c == "}") { brace_depth-- }
    }
    if (line ~ /([[:space:];{]|^)break([[:space:];]|$)/) { found_break = 1 }
    if (seen_open && brace_depth <= 0) {
      if (found_break) {
        exit_code = 1
      }
      in_loop = 0
    }
    line = ""
  }
  END { if (exit_code != 0) exit 1; exit 0 }
' {} +; then
  pass "signal handlers avoid clippy::never_loop pattern"
else
  fail "found 'for ... in signals.forever()' loop with terminating break; use 'if let Some(...) = signals.forever().next()' to prevent clippy::never_loop on Unix"
fi

# 2. Check launch_prepared_apply parameter version suppression.
# On Unix, version is not passed to the bash updater; it must be suppressed or underscored to prevent -D unused-variables.
if grep -q "pub async fn launch_prepared_apply" src-tauri/src/lifecycle.rs 2>/dev/null; then
  if grep -A 10 "pub async fn launch_prepared_apply" src-tauri/src/lifecycle.rs | grep -qE "(([[:space:]]|,|^|\()_version:[[:space:]]*&str|([[:space:]]|^)let[[:space:]]+_[[:space:]]*=[[:space:]]*version;)"; then
    pass "launch_prepared_apply version parameter properly suppressed for Unix builds"
  else
    fail "launch_prepared_apply version parameter is unused on Unix and must be underscored (_version) or suppressed (let _ = version;)"
  fi
fi

# 3. Check E2E Settings spec section isolation.
# Any test searching for [data-testid='update-card'] must first navigate to the General section before looking up or waiting for the card.
if [ -f "e2e/tauri/specs/settings.e2e.ts" ]; then
  if awk '
    /displays the updates card with version and check control/ { in_test = 1; next }
    in_test && /^[[:space:]]*it\(/ { in_test = 0 }
    in_test {
      if (/General/) {
        if (!seen_card && !seen_wait) { seen_nav = 1 }
      }
      if (/update-card/) {
        if (!seen_nav) { failed = 1; exit 1 }
        seen_card = 1
      }
      if (/waitForDisplayed/) {
        if (!seen_nav) { failed = 1; exit 1 }
        seen_wait = 1
      }
    }
    END {
      if (seen_nav && (seen_card || seen_wait) && !failed) exit 0
      exit 1
    }
  ' e2e/tauri/specs/settings.e2e.ts; then
    pass "settings.e2e.ts updates card test explicitly selects General section before update-card lookup or wait"
  else
    fail "settings.e2e.ts updates card test must explicitly select General section before waiting for [data-testid='update-card']"
  fi
fi

if [ "$failures" -ne 0 ]; then
  echo "Cross-platform Unix compatibility guard FAILED" >&2
  exit 1
fi

echo "Cross-platform Unix compatibility guard passed"
