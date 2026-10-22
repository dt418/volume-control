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
  actual="${actual%$'\r'}"
  if [ "$actual" != "$version" ]; then
    echo "FAIL $label: expected $version, got ${actual:-<empty>}" >&2
    failures=$((failures + 1))
  fi
}
check_exactly_one() {
  local label="$1"
  shift
  if [ "$#" -ne 1 ]; then
    echo "FAIL $label: expected exactly one value, got $#" >&2
    failures=$((failures + 1))
    return
  fi
  check "$label" "$1"
}

cargo_lock_package_versions() {
  local package_name="$1"
  awk -v package_name="$package_name" '
    /^\[\[package\]\]$/ { in_target = 0; next }
    $0 == "name = \"" package_name "\"" { in_target = 1; next }
    in_target && /^version = "/ {
      sub(/^version = "/, "")
      sub(/"$/, "")
      print
      in_target = 0
    }
  ' "$root/Cargo.lock"
}

check "tauri.conf.json" "$(sed -n 's/^  "version": "\([^"]*\)",$/\1/p' "$root/src-tauri/tauri.conf.json" | head -n 1)"
check "workspace Cargo.toml" "$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\([^"]*\)"/\1/p' "$root/Cargo.toml" | head -n 1)"
check "crates/volumectl/Cargo.toml" "$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$root/crates/volumectl/Cargo.toml" | head -n 1)"
check "src-tauri/Cargo.toml" "$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$root/src-tauri/Cargo.toml" | head -n 1)"
check "frontend/package.json" "$(sed -n 's/^  "version": "\([^"]*\)",$/\1/p' "$root/frontend/package.json" | head -n 1)"
package_lock_top_level_versions=()
while IFS= read -r version_value; do
  package_lock_top_level_versions+=("$version_value")
done < <(
  sed -n 's/^  "version": "\([^"]*\)",$/\1/p' \
    "$root/frontend/package-lock.json"
)
package_lock_root_package_versions=()
while IFS= read -r version_value; do
  package_lock_root_package_versions+=("$version_value")
done < <(
  sed -n '/^    "": {$/,/^    },$/s/^      "version": "\([^"]*\)",$/\1/p' \
    "$root/frontend/package-lock.json"
)
cargo_lock_volumectl_versions=()
while IFS= read -r version_value; do
  cargo_lock_volumectl_versions+=("$version_value")
done < <(cargo_lock_package_versions volumectl)
cargo_lock_tauri_versions=()
while IFS= read -r version_value; do
  cargo_lock_tauri_versions+=("$version_value")
done < <(cargo_lock_package_versions volumecontrol-tauri)
check_exactly_one 'frontend/package-lock.json top-level version' \
  ${package_lock_top_level_versions[@]+"${package_lock_top_level_versions[@]}"}
check_exactly_one 'frontend/package-lock.json packages[""].version' \
  ${package_lock_root_package_versions[@]+"${package_lock_root_package_versions[@]}"}
check_exactly_one 'Cargo.lock volumectl version' \
  ${cargo_lock_volumectl_versions[@]+"${cargo_lock_volumectl_versions[@]}"}
check_exactly_one 'Cargo.lock volumecontrol-tauri version' \
  ${cargo_lock_tauri_versions[@]+"${cargo_lock_tauri_versions[@]}"}

if [ "$failures" -ne 0 ]; then
  exit 1
fi
echo "version sync ok: $version"
