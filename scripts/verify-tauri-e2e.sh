#!/usr/bin/env bash
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
binary="${TAURI_E2E_BINARY:-$repo/target/debug/VolumeControl}"
output_root="${TAURI_E2E_OUTPUT:-$repo/output/tauri-e2e}"
surface="all"
skip_build=0

while (($#)); do
  case "$1" in
    --binary) binary="$2"; shift 2 ;;
    --output-root) output_root="$2"; shift 2 ;;
    --surface) surface="$2"; shift 2 ;;
    --skip-build) skip_build=1; shift ;;
    *) echo "usage: $0 [--binary path] [--output-root dir] [--surface all|mixer|runtime|windows|recovery|settings|help] [--skip-build]" >&2; exit 2 ;;
  esac
done

if [[ "$skip_build" -eq 0 ]]; then
  node "$repo/e2e/tauri/prepare-debug-frontend.mjs" -- \
    node "$repo/e2e/tauri/prepare-debug-capabilities.mjs" --provider wdio -- \
    cargo build -p volumecontrol-tauri --no-default-features --features e2e-wdio
fi
[[ -x "$binary" ]] || { echo "debug E2E binary does not exist: $binary" >&2; exit 1; }
[[ -f "$repo/e2e/tauri/node_modules/@wdio/cli/bin/wdio.js" ]] || { echo "run npm install --prefix e2e/tauri first" >&2; exit 1; }
mkdir -p "$output_root"

export TAURI_E2E_BINARY="$binary"
export TAURI_E2E_OUTPUT="$output_root"
set +e
npm --prefix "$repo/e2e/tauri" run test:e2e:debug -- --surface "$surface"
code=$?
set -e
unset TAURI_E2E_BINARY TAURI_E2E_OUTPUT

[[ ! -e "$repo/src-tauri/capabilities/e2e-wdio.json" ]] || { echo "temporary WDIO capability was not restored" >&2; exit 1; }
[[ ! -e "$repo/frontend/dist/tauri-plugin.wdio.js" ]] || { echo "temporary guest bridge was not restored" >&2; exit 1; }
exit "$code"
