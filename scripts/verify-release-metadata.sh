#!/usr/bin/env bash
# Verify that a desktop release artifact is bound to the expected commit and
# platform before it can be promoted to a GitHub release.
set -euo pipefail

usage() {
  echo "usage: $0 ARTIFACT_DIR EXPECTED_COMMIT_SHA PLATFORM" >&2
  echo "platform must be windows, macos, or ubuntu" >&2
}

fail() {
  echo "release metadata verification failed: $*" >&2
  exit 1
}

if [[ "$#" -ne 3 ]]; then
  usage
  exit 2
fi

artifact_dir="$1"
expected_sha="$2"
expected_platform="$3"

[[ -d "$artifact_dir" ]] || fail "artifact directory does not exist: $artifact_dir"
[[ "$expected_sha" =~ ^[[:xdigit:]]{7,64}$ ]] || fail "expected commit SHA is not hexadecimal: $expected_sha"
case "$expected_platform" in
  windows|macos|ubuntu) ;;
  *) fail "unsupported platform: $expected_platform" ;;
esac

metadata="$artifact_dir/build-metadata.json"
[[ -s "$metadata" ]] || fail "missing build metadata: $metadata"

metadata_values=""
python_cmd=""
for candidate in python3 python; do
  if command -v "$candidate" >/dev/null 2>&1 && "$candidate" -c 'import json' >/dev/null 2>&1; then
    python_cmd="$candidate"
    break
  fi
done

if [[ -n "$python_cmd" ]]; then
  metadata_values="$("$python_cmd" - "$metadata" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
try:
    value = json.loads(path.read_text(encoding="utf-8"))
except (OSError, ValueError) as error:
    print(f"invalid JSON: {error}", file=sys.stderr)
    raise SystemExit(2)

if not isinstance(value, dict):
    print("metadata root must be an object", file=sys.stderr)
    raise SystemExit(2)

required = (
    "commit_sha",
    "platform",
    "runner_arch",
    "rustc",
    "node",
    "artifact",
    "artifact_sha256",
)
missing = [key for key in required if key not in value]
if missing:
    print("missing required fields: " + ", ".join(missing), file=sys.stderr)
    raise SystemExit(2)

values = []
for key in required:
    item = value[key]
    if not isinstance(item, str) or not item.strip():
        print(f"metadata field {key} must be a non-empty string", file=sys.stderr)
        raise SystemExit(2)
    values.append(item)

print("\t".join(values))
PY
)" || fail "could not parse build metadata"
elif command -v node >/dev/null 2>&1; then
  metadata_values="$(node - "$metadata" <<'NODE'
const fs = require("node:fs");
const path = process.argv[2];
let value;
try {
  value = JSON.parse(fs.readFileSync(path, "utf8"));
} catch (error) {
  console.error(`invalid JSON: ${error.message}`);
  process.exit(2);
}
if (!value || typeof value !== "object" || Array.isArray(value)) {
  console.error("metadata root must be an object");
  process.exit(2);
}
const required = ["commit_sha", "platform", "runner_arch", "rustc", "node", "artifact", "artifact_sha256"];
const missing = required.filter((key) => !(key in value));
if (missing.length) {
  console.error(`missing required fields: ${missing.join(", ")}`);
  process.exit(2);
}
for (const key of required) {
  if (typeof value[key] !== "string" || value[key].trim() === "") {
    console.error(`metadata field ${key} must be a non-empty string`);
    process.exit(2);
  }
}
process.stdout.write(required.map((key) => value[key]).join("\t") + "\n");
NODE
)" || fail "could not parse build metadata"
else
  fail "python3, python, or node is required to parse build metadata"
fi

IFS=$'\t' read -r commit_sha platform runner_arch rustc node artifact artifact_sha256 <<< "$metadata_values"
[[ "$commit_sha" == "$expected_sha" ]] || fail "commit SHA mismatch (metadata=$commit_sha expected=$expected_sha)"
[[ "$platform" == "$expected_platform" ]] || fail "platform mismatch (metadata=$platform expected=$expected_platform)"
[[ "$artifact" != */* ]] || fail "artifact must be a file name, not a path: $artifact"
[[ "$artifact" != .* ]] || fail "artifact must not be a hidden path: $artifact"
[[ "$artifact_sha256" =~ ^[[:xdigit:]]{64}$ ]] || fail "artifact_sha256 is not a SHA-256 digest"

case "$expected_platform" in
  windows|macos)
    [[ "$artifact" == *.zip ]] || fail "expected a zip package for $expected_platform: $artifact"
    ;;
  ubuntu)
    [[ "$artifact" == *.tar.gz ]] || fail "expected a tar.gz package for ubuntu: $artifact"
    ;;
esac

package="$artifact_dir/$artifact"
[[ -f "$package" && -s "$package" ]] || fail "missing or empty package: $package"

checksums="$artifact_dir/SHA256SUMS.txt"
[[ -s "$checksums" ]] || fail "missing checksum input: $checksums"

if command -v sha256sum >/dev/null 2>&1; then
  actual_sha="$(sha256sum "$package" | awk '{print $1}')"
else
  shasum_cmd="$(command -v shasum || true)"
  [[ -n "$shasum_cmd" ]] || fail "sha256sum or shasum is required to verify the package"
  actual_sha="$(shasum -a 256 "$package" | awk '{print $1}')"
fi
[[ "$actual_sha" =~ ^[[:xdigit:]]{64}$ ]] || fail "could not calculate package SHA-256"
[[ "${actual_sha,,}" == "${artifact_sha256,,}" ]] || fail "metadata artifact_sha256 does not match package"

checksum_entry="$(awk -v name="$artifact" '$1 ~ /^[[:xdigit:]]{64}$/ && ($2 == name || $2 == "*" name) { print $1; exit }' "$checksums")"
[[ -n "$checksum_entry" ]] || fail "checksum input has no entry for $artifact"
[[ "${checksum_entry,,}" == "${actual_sha,,}" ]] || fail "checksum input does not match package"

echo "release metadata verified: platform=$platform commit=$commit_sha artifact=$artifact"
