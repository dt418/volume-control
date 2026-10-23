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

# 2. Rejection of signals.forever() loop with break
tmp_file="src-tauri/src/__test_sig_leak.rs"
lifecycle_file="src-tauri/src/lifecycle.rs"
lifecycle_bak="${lifecycle_file}.testbak"
e2e_file="e2e/tauri/specs/settings.e2e.ts"
e2e_bak="${e2e_file}.testbak"

cleanup() {
  rm -f "$tmp_file"
  if [ -f "$lifecycle_bak" ]; then
    mv -f "$lifecycle_bak" "$lifecycle_file"
  fi
  if [ -f "$e2e_bak" ]; then
    mv -f "$e2e_bak" "$e2e_file"
  fi
}
trap cleanup EXIT

echo 'fn dummy(signals: &signal_hook::iterator::Signals) { for s in signals.forever() { break; } }' > "$tmp_file"
if bash scripts/check-unix-compat.sh >/dev/null 2>&1; then
  fail "check-unix-compat.sh should have rejected single-line for .. in signals.forever() with break"
else
  pass "check-unix-compat.sh catches single-line for .. in signals.forever() with break"
fi
rm -f "$tmp_file"

# 3. Rejection of multiline signals.forever() loop with break
printf 'fn dummy(signals: &signal_hook::iterator::Signals) {\n    for s in signals.forever() {\n        log::info!("sig");\n        break;\n    }\n}\n' > "$tmp_file"
if bash scripts/check-unix-compat.sh >/dev/null 2>&1; then
  fail "check-unix-compat.sh should have rejected multiline for .. in signals.forever() with break"
else
  pass "check-unix-compat.sh catches multiline for .. in signals.forever() with break"
fi
rm -f "$tmp_file"

# 4. Acceptance of signals.forever() loop without break (processes multiple signals)
printf 'fn dummy(signals: &signal_hook::iterator::Signals) {\n    for s in signals.forever() {\n        log::info!("sig: {s}");\n        handle(s);\n    }\n}\n' > "$tmp_file"
if bash scripts/check-unix-compat.sh >/dev/null 2>&1; then
  pass "check-unix-compat.sh allows for .. in signals.forever() without terminating break"
else
  fail "check-unix-compat.sh rejected for .. in signals.forever() without break"
fi
rm -f "$tmp_file"

# 5. Parameter suppression: reject expected_version substring when version unsuppressed
cp "$lifecycle_file" "$lifecycle_bak"
sed -e 's/version: &str/expected_version: \&str, version: \&str/' \
    -e 's/let _ = version;/let _ = other;/' "$lifecycle_bak" > "$lifecycle_file"
if bash scripts/check-unix-compat.sh >/dev/null 2>&1; then
  fail "check-unix-compat.sh should have rejected unsuppressed version when expected_version is present"
else
  pass "check-unix-compat.sh binds suppression check to exact version parameter"
fi
mv -f "$lifecycle_bak" "$lifecycle_file"

# 6. Section isolation: reject update-card wait occurring before General navigation
cp "$e2e_file" "$e2e_bak"
awk '
  /displays the updates card with version and check control/ { in_test = 1 }
  in_test && /General/ { nav = $0; next }
  in_test && /updateCard.waitForDisplayed/ {
    print $0
    print nav
    in_test = 0
    next
  }
  { print }
' "$e2e_bak" > "$e2e_file"
if bash scripts/check-unix-compat.sh >/dev/null 2>&1; then
  fail "check-unix-compat.sh should have rejected updateCard wait before General navigation"
else
  pass "check-unix-compat.sh enforces General navigation before updateCard lookup/wait"
fi
mv -f "$e2e_bak" "$e2e_file"

# 7. Acceptance of signals.forever() loop with conditional break
printf 'fn dummy(signals: &signal_hook::iterator::Signals) {\n    for s in signals.forever() {\n        if s == 15 {\n            break;\n        }\n    }\n}\n' > "$tmp_file"
if bash scripts/check-unix-compat.sh >/dev/null 2>&1; then
  pass "check-unix-compat.sh allows for .. in signals.forever() with conditional break"
else
  fail "check-unix-compat.sh rejected for .. in signals.forever() with conditional break"
fi
rm -f "$tmp_file"

# 8. Acceptance of signals.forever() loop with break in nested loop
printf 'fn dummy(signals: &signal_hook::iterator::Signals) {\n    for s in signals.forever() {\n        for x in 0..10 {\n            break;\n        }\n    }\n}\n' > "$tmp_file"
if bash scripts/check-unix-compat.sh >/dev/null 2>&1; then
  pass "check-unix-compat.sh allows break in nested loop inside signals.forever()"
else
  fail "check-unix-compat.sh rejected break in nested loop inside signals.forever()"
fi
rm -f "$tmp_file"

# 9. Section isolation: reject General token without actual navigation click
cp "$e2e_file" "$e2e_bak"
sed -e 's/await.*General.*click.*/const targetSection = "General";/' "$e2e_bak" > "$e2e_file"
if bash scripts/check-unix-compat.sh >/dev/null 2>&1; then
  fail "check-unix-compat.sh should have rejected General variable assignment without click"
else
  pass "check-unix-compat.sh requires actual General navigation click"
fi
mv -f "$e2e_bak" "$e2e_file"

# 10. Acceptance of signals.forever() loop with continue before outer-depth break
printf 'fn dummy(signals: &signal_hook::iterator::Signals) {\n    for s in signals.forever() {\n        if s != 15 {\n            continue;\n        }\n        break;\n    }\n}\n' > "$tmp_file"
if bash scripts/check-unix-compat.sh >/dev/null 2>&1; then
  pass "check-unix-compat.sh allows for .. in signals.forever() with continue path before break"
else
  fail "check-unix-compat.sh rejected for .. in signals.forever() with continue path before break"
fi
rm -f "$tmp_file"

echo "All check-unix-compat tests passed."
