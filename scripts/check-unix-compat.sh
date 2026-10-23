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
  FNR == 1 { in_loop = 0; brace_depth = 0; seen_open = 0; in_comment = 0; in_string = 0; has_continue = 0; inner_loop_sp = 0; pending_loop = 0 }
  {
    line = $0
    idx = match(line, /for[[:space:]]+.*[[:space:]]+in[[:space:]]+.*signals\.forever\(\)/)
    if (idx > 0) {
      in_loop = 1
      brace_depth = 0
      seen_open = 0
      has_continue = 0
      inner_loop_sp = 0
      pending_loop = 0
      line = substr(line, idx)
    }
    if (!in_loop) next

    n = length(line)
    for (i = 1; i <= n; i++) {
      c = substr(line, i, 1)
      next_c = (i < n) ? substr(line, i + 1, 1) : ""

      if (in_comment) {
        if (c == "*" && next_c == "/") { in_comment = 0; i++ }
        continue
      }

      if (in_string) {
        if (c == "\\") { i++; continue }
        if (c == "\"") { in_string = 0 }
        continue
      }

      if (c == "/" && next_c == "/") {
        break
      }

      if (c == "/" && next_c == "*") {
        in_comment = 1
        i++
        continue
      }

      if (c == "\"") {
        in_string = 1
        continue
      }

      if (c == "\047") {
        if (next_c == "\\") { i += 3 } else { i += 2 }
        continue
      }

      prev_c = (i == 1) ? " " : substr(line, i - 1, 1)

      # Detect inner loop keyword
      if (seen_open && prev_c ~ /[^a-zA-Z0-9_]/) {
        if (substr(line, i, 4) == "for " || substr(line, i, 6) == "while " || substr(line, i, 5) == "loop ") {
          pending_loop = 1
        } else if (substr(line, i, 5) == "loop{") {
          pending_loop = 1
        }
      }

      if (c == "{") {
        brace_depth++
        seen_open = 1
        if (pending_loop) {
          inner_loop_sp++
          inner_loop_stack[inner_loop_sp] = brace_depth
          pending_loop = 0
        }
        continue
      }

      if (c == "}") {
        if (inner_loop_sp > 0 && inner_loop_stack[inner_loop_sp] == brace_depth) {
          inner_loop_sp--
        }
        brace_depth--
        if (seen_open && brace_depth <= 0) {
          in_loop = 0
        }
        continue
      }

      # Detect continue keyword targeting outer loop
      if (seen_open && inner_loop_sp == 0 && substr(line, i, 8) == "continue") {
        after_c = (i + 8 <= n) ? substr(line, i + 8, 1) : " "
        if (prev_c ~ /[^a-zA-Z0-9_]/ && after_c ~ /[^a-zA-Z0-9_]/) {
          has_continue = 1
        }
      }

      # Unconditional break: outer loop depth (brace_depth == 1) without a prior continue path
      if (brace_depth == 1 && substr(line, i, 5) == "break") {
        after_c = (i + 5 <= n) ? substr(line, i + 5, 1) : " "
        if (prev_c ~ /[^a-zA-Z0-9_]/ && after_c ~ /[^a-zA-Z0-9_]/) {
          if (!has_continue) {
            exit_code = 1
            in_loop = 0
            break
          }
        }
      }
    }
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
      line = $0
      if (line ~ /^[[:space:]]*\/\//) next
      test_body = test_body "\n" line
    }
    END {
      idx_card = index(test_body, "update-card")
      idx_wait = index(test_body, "waitForDisplayed")
      first_stop = -1
      if (idx_card > 0 && idx_wait > 0) {
        first_stop = (idx_card < idx_wait) ? idx_card : idx_wait
      } else if (idx_card > 0) {
        first_stop = idx_card
      } else {
        first_stop = idx_wait
      }
      if (first_stop <= 0) exit 1

      pre_card = substr(test_body, 1, first_stop - 1)

      # 1. Chained lookup and click: $(...button...General...).click()
      if (pre_card ~ /\$([[:space:]]|\$)*\([^;]*(button[^;]*General|General[^;]*button)[^;]*\)[^;]*[.][[:space:]]*click[[:space:]]*[(]/) {
        exit 0
      }

      # 2. Variable assignment then .click()
      if (match(pre_card, /(const|let|var)[[:space:]]+[a-zA-Z0-9_]+[[:space:]]*=[^;]*\$([[:space:]]|\$)*\([^;]*(button[^;]*General|General[^;]*button)[^;]*\)/)) {
        matched = substr(pre_card, RSTART, RLENGTH)
        sub(/^[[:space:]]*(const|let|var)[[:space:]]+/, "", matched)
        sub(/[[:space:]]*=.*$/, "", matched)
        varname = matched
        if (varname != "") {
          pattern = varname "[[:space:]]*[.][[:space:]]*click[[:space:]]*[(]"
          if (pre_card ~ pattern) {
            exit 0
          }
        }
      }

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
