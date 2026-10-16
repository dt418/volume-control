#!/usr/bin/env bash
# Verify that a desktop release artifact is bound to the expected commit and
# platform before it can be promoted to a GitHub release.
#
# build-metadata.json declares every staged file:
#   { commit_sha, platform, runner_arch, rustc, node,
#     artifacts: [{ name, sha256 }] }
# Every declared artifact must exist in the directory, match its metadata
# digest, and own exactly one canonical SHA256SUMS.txt entry.
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
import re
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
)
missing = [key for key in required if key not in value]
if missing:
    print("missing required fields: " + ", ".join(missing), file=sys.stderr)
    raise SystemExit(2)

scalars = []
for key in required:
    item = value[key]
    if not isinstance(item, str) or not item.strip():
        print(f"metadata field {key} must be a non-empty string", file=sys.stderr)
        raise SystemExit(2)
    scalars.append(item)

artifacts = value.get("artifacts")
if not isinstance(artifacts, list) or not artifacts:
    print("metadata field artifacts must be a non-empty array", file=sys.stderr)
    raise SystemExit(2)

seen = set()
rows = []
for entry in artifacts:
    if not isinstance(entry, dict):
        print("metadata artifact entry must be an object", file=sys.stderr)
        raise SystemExit(2)
    name = entry.get("name")
    digest = entry.get("sha256")
    if not isinstance(name, str) or not name:
        print("metadata artifact name must be a non-empty string", file=sys.stderr)
        raise SystemExit(2)
    if "/" in name or "\\" in name or ":" in name:
        print(f"artifact must be a file name, not a path: {name}", file=sys.stderr)
        raise SystemExit(2)
    if name.startswith("."):
        print(f"artifact must not be a hidden path: {name}", file=sys.stderr)
        raise SystemExit(2)
    if any(ord(char) < 0x20 or ord(char) == 0x7F for char in name):
        print(f"artifact name must not contain control characters: {name}", file=sys.stderr)
        raise SystemExit(2)
    if not isinstance(digest, str) or not re.fullmatch(r"[0-9A-Fa-f]{64}", digest):
        print(f"artifact sha256 is not a SHA-256 digest: {name}", file=sys.stderr)
        raise SystemExit(2)
    if name in seen:
        print(f"duplicate artifact name: {name}", file=sys.stderr)
        raise SystemExit(2)
    seen.add(name)
    rows.append((name, digest.lower()))

print("\t".join(scalars))
for name, digest in rows:
    print(f"{name}\t{digest}")
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
const required = ["commit_sha", "platform", "runner_arch", "rustc", "node"];
const missing = required.filter((key) => !(key in value));
if (missing.length) {
  console.error(`missing required fields: ${missing.join(", ")}`);
  process.exit(2);
}
const scalars = [];
for (const key of required) {
  if (typeof value[key] !== "string" || value[key].trim() === "") {
    console.error(`metadata field ${key} must be a non-empty string`);
    process.exit(2);
  }
  scalars.push(value[key]);
}
if (!Array.isArray(value.artifacts) || value.artifacts.length === 0) {
  console.error("metadata field artifacts must be a non-empty array");
  process.exit(2);
}
const seen = new Set();
const rows = [];
for (const entry of value.artifacts) {
  if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
    console.error("metadata artifact entry must be an object");
    process.exit(2);
  }
  const name = entry.name;
  const digest = entry.sha256;
  if (typeof name !== "string" || name === "") {
    console.error("metadata artifact name must be a non-empty string");
    process.exit(2);
  }
  if (name.includes("/") || name.includes("\\") || name.includes(":")) {
    console.error(`artifact must be a file name, not a path: ${name}`);
    process.exit(2);
  }
  if (name.startsWith(".")) {
    console.error(`artifact must not be a hidden path: ${name}`);
    process.exit(2);
  }
  if (/[\0-\x1F\x7F]/.test(name)) {
    console.error(`artifact name must not contain control characters: ${name}`);
    process.exit(2);
  }
  if (typeof digest !== "string" || !/^[0-9A-Fa-f]{64}$/.test(digest)) {
    console.error(`artifact sha256 is not a SHA-256 digest: ${name}`);
    process.exit(2);
  }
  if (seen.has(name)) {
    console.error(`duplicate artifact name: ${name}`);
    process.exit(2);
  }
  seen.add(name);
  rows.push(`${name}\t${digest.toLowerCase()}`);
}
process.stdout.write(`${scalars.join("\t")}\n${rows.join("\n")}\n`);
NODE
)" || fail "could not parse build metadata"
else
  fail "python3, python, or node is required to parse build metadata"
fi

commit_sha=""
platform=""
names=()
digests=()
line_no=0
while IFS=$'\t' read -r col1 col2 col3 col4 col5; do
  # A Windows Python interpreter emits CRLF through the pipe; strip the
  # carriage return so digest comparisons stay canonical.
  col1="${col1%$'\r'}"
  col2="${col2%$'\r'}"
  col3="${col3%$'\r'}"
  col4="${col4%$'\r'}"
  col5="${col5%$'\r'}"
  line_no=$((line_no + 1))
  if [[ "$line_no" -eq 1 ]]; then
    commit_sha="$col1"
    platform="$col2"
    [[ -n "$commit_sha" && -n "$platform" && -n "$col3" && -n "$col4" && -n "$col5" ]] \
      || fail "could not parse build metadata"
  else
    [[ -n "$col1" && -n "$col2" && -z "${col3:-}" && -z "${col4:-}" && -z "${col5:-}" ]] \
      || fail "could not parse build metadata"
    names+=("$col1")
    digests+=("$col2")
  fi
done <<< "$metadata_values"

[[ "$commit_sha" == "$expected_sha" ]] || fail "commit SHA mismatch (metadata=$commit_sha expected=$expected_sha)"
[[ "$platform" == "$expected_platform" ]] || fail "platform mismatch (metadata=$platform expected=$expected_platform)"
((${#names[@]} > 0)) || fail "metadata declares no artifacts"

case "$expected_platform" in
  macos)
    ((${#names[@]} == 1)) || fail "macOS metadata must declare exactly one artifact"
    [[ "${names[0]}" == *.zip ]] || fail "expected a zip package for macos: ${names[0]}"
    ;;
  ubuntu)
    ((${#names[@]} == 1)) || fail "Ubuntu metadata must declare exactly one artifact"
    [[ "${names[0]}" == *.tar.gz ]] || fail "expected a tar.gz package for ubuntu: ${names[0]}"
    ;;
  windows)
    ((${#names[@]} == 3)) || fail "Windows metadata must declare exactly three artifacts"
    zips=0
    setups=0
    scripts=0
    for name in "${names[@]}"; do
      [[ "$name" == volumecontrol-*-windows.zip ]] && zips=$((zips + 1))
      [[ "$name" == VolumeControl_*_x64-setup.exe ]] && setups=$((setups + 1))
      [[ "$name" == Install-VolumeControl.ps1 ]] && scripts=$((scripts + 1))
    done
    [[ "$zips" -eq 1 && "$setups" -eq 1 && "$scripts" -eq 1 ]] \
      || fail "Windows metadata must declare one windows zip, one setup exe, and Install-VolumeControl.ps1"
    ;;
esac

checksums="$artifact_dir/SHA256SUMS.txt"
[[ -s "$checksums" ]] || fail "missing checksum input: $checksums"

for index in "${!names[@]}"; do
  name="${names[$index]}"
  expected_digest="${digests[$index]}"

  package="$artifact_dir/$name"
  [[ -f "$package" && -s "$package" ]] || fail "missing or empty artifact: $package"

  if command -v sha256sum >/dev/null 2>&1; then
    actual_sha="$(sha256sum "$package" | awk '{print $1}')"
  else
    shasum_cmd="$(command -v shasum || true)"
    [[ -n "$shasum_cmd" ]] || fail "sha256sum or shasum is required to verify the package"
    actual_sha="$(shasum -a 256 "$package" | awk '{print $1}')"
  fi
  # GNU sha256sum prefixes the digest with a backslash when the file name
  # contains a backslash or newline (Windows-style repo paths hit this).
  actual_sha="${actual_sha#\\}"
  [[ "$actual_sha" =~ ^[[:xdigit:]]{64}$ ]] || fail "could not calculate SHA-256 for $name"
  [[ "${actual_sha,,}" == "${expected_digest,,}" ]] || fail "metadata sha256 does not match $name"

  # Canonical manifest lines are "<64 hex><two spaces><basename>"; the
  # artifact must own exactly one of them.
  entry_digest="$(awk -v name="$name" \
    '$0 ~ /^[0-9A-Fa-f]{64}  [^ \/\\]+$/ && substr($0, 67) == name { count++; digest=substr($0, 1, 64) } \
     END { if (count == 1) print digest }' "$checksums")"
  [[ -n "$entry_digest" ]] || fail "checksum input must contain exactly one canonical entry for $name"
  [[ "${entry_digest,,}" == "${actual_sha,,}" ]] || fail "checksum input does not match $name"
done

echo "release metadata verified: platform=$platform commit=$commit_sha artifacts=${#names[@]}"
