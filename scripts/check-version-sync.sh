#!/usr/bin/env bash
set -euo pipefail

tag="${1:?usage: $0 vX.Y.Z}"
if [[ ! "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "version must look like vMAJOR.MINOR.PATCH: $tag" >&2
  exit 2
fi
version="${tag#v}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
failures=0

check() {
  local label="$1" actual="$2"
  if [ "$actual" != "$version" ]; then
    echo "FAIL $label: expected $version, got ${actual:-<empty>}" >&2
    failures=$((failures + 1))
  fi
}

check "tauri.conf.json" "$(sed -n 's/^  "version": "\([^"]*\)",$/\1/p' "$root/src-tauri/tauri.conf.json" | head -n 1)"
check "workspace Cargo.toml" "$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\([^"]*\)"/\1/p' "$root/Cargo.toml" | head -n 1)"
check "crates/volumectl/Cargo.toml" "$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$root/crates/volumectl/Cargo.toml" | head -n 1)"
check "src-tauri/Cargo.toml" "$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$root/src-tauri/Cargo.toml" | head -n 1)"
check "frontend/package.json" "$(sed -n 's/^  "version": "\([^"]*\)",$/\1/p' "$root/frontend/package.json" | head -n 1)"
check "frontend/package-lock.json" "$(sed -n 's/^  "version": "\([^"]*\)",$/\1/p' "$root/frontend/package-lock.json" | head -n 1)"

if [ "$failures" -ne 0 ]; then
  exit 1
fi
echo "version sync ok: $version"
