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
# The correct pattern is `if let Some(sig) = signals.forever().next()`.
if grep -rn "for .* in signals\.forever()" src-tauri/src crates/volumectl/src 2>/dev/null | grep -q .; then
  fail "found 'for ... in signals.forever()' loop; use 'if let Some(...) = signals.forever().next()' to prevent clippy::never_loop on Unix"
else
  pass "signal handlers avoid clippy::never_loop pattern"
fi

# 2. Check launch_prepared_apply parameter version suppression.
# On Unix, version is not passed to the bash updater; it must be suppressed or underscored to prevent -D unused-variables.
if grep -q "pub async fn launch_prepared_apply" src-tauri/src/lifecycle.rs 2>/dev/null; then
  if grep -A 8 "pub async fn launch_prepared_apply" src-tauri/src/lifecycle.rs | grep -qE "(_version: &str|let _ = version;)"; then
    pass "launch_prepared_apply version parameter properly suppressed for Unix builds"
  else
    fail "launch_prepared_apply version parameter is unused on Unix and must be underscored (_version) or suppressed (let _ = version;)"
  fi
fi

# 3. Check E2E Settings spec section isolation.
# Any test searching for [data-testid='update-card'] must first navigate to the General section.
if [ -f "e2e/tauri/specs/settings.e2e.ts" ]; then
  if grep -A 5 "displays the updates card with version and check control" e2e/tauri/specs/settings.e2e.ts | grep -q "General"; then
    pass "settings.e2e.ts updates card test explicitly selects General section"
  else
    fail "settings.e2e.ts updates card test must explicitly select General section before waiting for [data-testid='update-card']"
  fi
fi

if [ "$failures" -ne 0 ]; then
  echo "Cross-platform Unix compatibility guard FAILED" >&2
  exit 1
fi

echo "Cross-platform Unix compatibility guard passed"
