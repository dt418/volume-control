#!/usr/bin/env bash
#
# Run the complete VolumeControl verification battery for the current
# platform (macOS or Linux; Windows uses scripts/verify-platform.ps1).
#
# Fail-closed, in order:
#   1. format-lint gate (fmt, whitespace, clippy -D warnings, workspace tests)
#   2. frontend Vitest suite + production build
#   3. Tauri E2E evidence gate (all surfaces; Linux runs under xvfb-run)
#   4. native probes:
#        Linux  - GTK4/libadwaita smoke (cargo test --features gtk-renderer,
#                 under Xvfb); Pulse server presence is recorded as SKIP with
#                 a reason when absent (never a fake PASS).
#        macOS  - AppKit surface smoke + OS/architecture record; codesign/
#                 plutil inspection only when a built .app is present.
#   5. enforcement self-tests (records/format-lint/ship guards)
#
# Usage:
#   bash scripts/verify-platform.sh [--platform linux|macos|auto]
#                                   [--skip-e2e] [--skip-native] [--output-root DIR]
#
# Exit 0 only when every required step passed.

set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
platform="auto"
skip_e2e=0
skip_native=0
output_root="$repo/output/platform"

while (($#)); do
  case "$1" in
    --platform) platform="$2"; shift 2 ;;
    --skip-e2e) skip_e2e=1; shift ;;
    --skip-native) skip_native=1; shift ;;
    --output-root) output_root="$2"; shift 2 ;;
    *) echo "usage: $0 [--platform linux|macos|auto] [--skip-e2e] [--skip-native] [--output-root dir]" >&2; exit 2 ;;
  esac
done

if [ "$platform" = "auto" ]; then
  case "$(uname -s)" in
    Darwin) platform="macos" ;;
    Linux) platform="linux" ;;
    *) echo "unsupported platform: $(uname -s)" >&2; exit 2 ;;
  esac
fi

mkdir -p "$output_root"
failures=()

echo "NOTE: the E2E step launches real app windows using the DEBUG virtual"
echo "audio backend (VOLUMECTL_E2E_AUDIO=virtual). Their volume displays are"
echo "simulated and NEVER change the real device; ignore them."

step() {
  local name="$1"; shift
  echo
  echo "=== $name ==="
  if "$@"; then
    echo "PASS: $name"
  else
    echo "FAIL: $name" >&2
    failures+=("$name")
  fi
}

step "format-lint gate (fmt, clippy -D warnings, workspace tests)" \
  bash "$repo/scripts/format-lint.sh"

step "frontend Vitest + production build" bash -c "
  npm test --prefix '$repo/frontend' &&
  npm run build --prefix '$repo/frontend'"

if [ "$skip_e2e" -eq 0 ]; then
  if [ "$platform" = "linux" ]; then
    step "Tauri E2E evidence gate (all surfaces, Xvfb)" bash -c "
      cd '$repo' &&
      xvfb-run -a bash scripts/verify-tauri-e2e.sh \
        --surface all --output-root '$output_root/tauri-e2e'"
  else
    step "Tauri E2E evidence gate (all surfaces)" bash -c "
      cd '$repo' &&
      bash scripts/verify-tauri-e2e.sh \
        --surface all --output-root '$output_root/tauri-e2e'"
  fi
else
  echo "SKIP: Tauri E2E evidence gate (--skip-e2e)"
fi

if [ "$skip_native" -eq 1 ]; then
  echo "SKIP: native platform probes (--skip-native)"
elif [ "$platform" = "linux" ]; then
  step "GTK4/libadwaita renderer smoke (Xvfb)" bash -c "
    cd '$repo' &&
    xvfb-run -a cargo test -p volumectl --features gtk-renderer \
      --test gtk_smoke -- --nocapture"
  if command -v pactl >/dev/null 2>&1 && pactl info >/dev/null 2>&1; then
    echo "PULSE: server present — per-app sessions can be exercised manually"
  else
    echo "SKIP: Pulse server absent — per-app audio needs a real Pulse/PipeWire session (not a fake pass)"
  fi
elif [ "$platform" = "macos" ]; then
  step "AppKit renderer smoke" bash -c "
    cd '$repo' &&
    cargo test -p volumectl --test appkit_smoke -- --nocapture"
  step "OS + architecture record" bash -c "
    sw_vers && uname -m"
  if [ -d "$repo/target/release/VolumeControl.app" ]; then
    step "codesign + plutil package inspection" bash -c "
      codesign --verify --strict --verbose=2 '$repo/target/release/VolumeControl.app' &&
      plutil -lint '$repo/target/release/VolumeControl.app/Contents/Info.plist'"
  else
    echo "SKIP: no built .app under target/release — package inspection skipped (not a fake pass)"
  fi
fi

step "enforcement self-tests" bash -c "
  bash '$repo/scripts/test-check-records.sh' &&
  bash '$repo/scripts/test-format-lint.sh' &&
  bash '$repo/scripts/test-ship.sh'"

if ((${#failures[@]})); then
  echo
  echo "PLATFORM VERIFICATION FAILED: ${failures[*]}" >&2
  exit 1
fi
echo
echo "PLATFORM VERIFICATION PASSED ($platform)"
exit 0
