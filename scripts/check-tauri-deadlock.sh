#!/usr/bin/env bash
set -euo pipefail

# Tauri deadlock guard (see .agents/skills/tauri-deadlock-guard/SKILL.md).
#
# Enforces the invariants that prevent the two production bugs from Session
# 077: release webviews resolving to the dev server, and surface commands
# blocking the Tauri main thread. Run before changing any src-tauri surface
# code and in CI.

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

# 1. tauri dependency must keep the custom-protocol feature so release builds
#    serve embedded assets instead of cfg(dev)/devUrl.
if grep -q 'custom-protocol' src-tauri/Cargo.toml; then
  pass "tauri features include custom-protocol"
else
  fail "src-tauri/Cargo.toml tauri features are missing custom-protocol"
fi

# 2. WebviewWindowBuilder may only live in window_manager.rs.
if grep -rl 'WebviewWindowBuilder' src-tauri/src --include='*.rs' | grep -v 'window_manager.rs' | grep -q .; then
  fail "WebviewWindowBuilder used outside src-tauri/src/window_manager.rs"
else
  pass "WebviewWindowBuilder confined to window_manager.rs"
fi

# 3. Surface commands must be async (sync commands run inside an async task
#    on the main thread and deadlock wry's message pump).
for cmd in open_surface close_surface surface_ready; do
  if grep -qE "pub async fn ${cmd}\(" src-tauri/src/commands.rs; then
    pass "command ${cmd} is async"
  else
    fail "command ${cmd} must be declared async"
  fi
done

# 4. WindowManager must marshal surface operations onto the main thread and
#    every public wrapper must route through it.
if grep -q 'fn on_main' src-tauri/src/window_manager.rs; then
  pass "WindowManager provides on_main marshalling"
else
  fail "WindowManager is missing on_main marshalling"
fi
for wrapper in 'pub fn open(' 'pub fn close(' 'pub fn surface_ready('; do
  if grep -qF "${wrapper}" src-tauri/src/window_manager.rs; then
    pass "wrapper ${wrapper} present"
  else
    fail "missing WindowManager wrapper ${wrapper}"
  fi
done

# 5. Commands must marshal through run_on_main_thread with an async channel,
#    not block synchronously.
if grep -q 'run_on_main_thread' src-tauri/src/commands.rs &&
  grep -q 'async_runtime::channel' src-tauri/src/commands.rs; then
  pass "commands marshal via run_on_main_thread + async channel"
else
  fail "surface commands must use run_on_main_thread + async_runtime::channel"
fi

if [[ "$failures" -ne 0 ]]; then
  echo "Tauri deadlock guard FAILED" >&2
  exit 1
fi
echo "Tauri deadlock guard passed"
