#!/usr/bin/env bash
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
binary="${TAURI_E2E_BINARY:-$repo/target/debug/VolumeControl}"
output_root="${TAURI_E2E_OUTPUT:-$repo/output/tauri-e2e}"
surface="all"
skip_build=0
provider_set_by_wrapper=0
if [[ -z "${E2E_DRIVER_PROVIDER:-}" ]]; then
  export E2E_DRIVER_PROVIDER=embedded
  provider_set_by_wrapper=1
fi

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
run_output_root="$(mktemp -d "$output_root/run-XXXXXX")"
run_id="$(basename "$run_output_root")"

case "$surface" in
  all) expected_specs=(mixer.e2e.ts runtime.e2e.ts windows.e2e.ts recovery.e2e.ts settings.e2e.ts help.e2e.ts) ;;
  mixer|runtime|windows|recovery|settings|help) expected_specs=("$surface.e2e.ts") ;;
  *) echo "invalid surface: $surface" >&2; exit 2 ;;
esac

export TAURI_E2E_BINARY="$binary"
export TAURI_E2E_OUTPUT="$run_output_root"
export TAURI_E2E_RUN_ID="$run_id"
set +e
npm --prefix "$repo/e2e/tauri" run test:e2e:debug -- --surface "$surface"
code=$?
set -e
if [[ "$code" -eq 0 ]]; then
  set +e
  node --import tsx --input-type=module -e '
    import { assertE2eEvidence } from "./e2e/tauri/support/artifacts.ts";
    const [root, runId, ...expected] = process.argv.slice(1);
    await assertE2eEvidence(root, expected, runId);
  ' "$run_output_root" "$run_id" "${expected_specs[@]}"
  evidence_code=$?
  set -e
else
  evidence_code=0
fi
unset TAURI_E2E_BINARY TAURI_E2E_OUTPUT TAURI_E2E_RUN_ID
if [[ "$provider_set_by_wrapper" -eq 1 ]]; then unset E2E_DRIVER_PROVIDER; fi

[[ ! -e "$repo/src-tauri/capabilities/e2e-wdio.json" ]] || { echo "temporary WDIO capability was not restored" >&2; evidence_code=1; }
[[ ! -e "$repo/frontend/dist/tauri-plugin.wdio.js" ]] || { echo "temporary guest bridge was not restored" >&2; evidence_code=1; }
if [[ "$code" -eq 0 && "$evidence_code" -ne 0 ]]; then code="$evidence_code"; fi
exit "$code"
