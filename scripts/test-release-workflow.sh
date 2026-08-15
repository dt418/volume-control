#!/usr/bin/env bash
# Static and fixture contract for the SHA-bound release workflow.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
release="$repo/.github/workflows/release.yml"
validation="$repo/.github/workflows/desktop-validation.yml"
verifier="$repo/scripts/verify-release-metadata.sh"
failures=0

report() {
  local status="$1" description="$2"
  if [[ "$status" == ok ]]; then
    printf 'ok   - %s\n' "$description"
  else
    printf 'FAIL - %s\n' "$description"
    failures=$((failures + 1))
  fi
}

contains() {
  local file="$1" pattern="$2"
  grep -Eq "$pattern" "$file"
}

if contains "$release" 'uses:[[:space:]]*\./\.github/workflows/desktop-validation\.yml'; then
  report ok 'release caller invokes the reusable desktop validation workflow'
else
  report FAIL 'release caller invokes the reusable desktop validation workflow'
fi

if contains "$release" 'needs:[[:space:]]*validate'; then
  report ok 'publish job requires validation'
else
  report FAIL 'publish job requires validation'
fi

if ! grep -Eq 'tauri[[:space:]]+build' "$release"; then
  report ok 'release caller contains no direct tauri build'
else
  report FAIL 'release caller contains no direct tauri build'
fi

if contains "$release" 'pattern:[[:space:]]*validated-\*-\$\{\{[[:space:]]*github\.sha[[:space:]]*\}\}'; then
  report ok 'release caller downloads only SHA-named validation artifacts'
else
  report FAIL 'release caller downloads only SHA-named validation artifacts'
fi

if contains "$validation" 'release_mode:[[:space:]]*'; then
  report ok 'reusable workflow declares release_mode input'
else
  report FAIL 'reusable workflow declares release_mode input'
fi
if contains "$validation" 'release_tag:[[:space:]]*'; then
  report ok 'reusable workflow declares release_tag input'
else
  report FAIL 'reusable workflow declares release_tag input'
fi
if contains "$validation" 'name:[[:space:]]*validated-\$\{\{[[:space:]]*matrix\.platform[[:space:]]*\}\}-\$\{\{[[:space:]]*github\.sha[[:space:]]*\}\}'; then
  report ok 'validation artifacts are named with platform and commit SHA'
else
  report FAIL 'validation artifacts are named with platform and commit SHA'
fi

verifier_line="$(grep -nF 'verify-release-metadata.sh' "$release" | head -n 1 | cut -d: -f1 || true)"
create_line="$(grep -nF 'gh release create' "$release" | head -n 1 | cut -d: -f1 || true)"
upload_line="$(grep -nF 'gh release upload' "$release" | head -n 1 | cut -d: -f1 || true)"
if [[ -n "$verifier_line" && -n "$create_line" && -n "$upload_line" && "$verifier_line" -lt "$create_line" && "$verifier_line" -lt "$upload_line" ]]; then
  report ok 'metadata verifier runs before release create/upload'
else
  report FAIL 'metadata verifier runs before release create/upload'
fi

expected_sha="$(git -C "$repo" rev-parse HEAD)"
valid="$repo/scripts/test-fixtures/release-valid"
mismatch="$repo/scripts/test-fixtures/release-mismatch"
wrong_platform="$repo/scripts/test-fixtures/release-wrong-platform"

# The committed valid fixture is anchored to the implementation baseline. On
# later commits, refresh only its copy so the test continues to exercise the
# verifier against the current commit without making a mutable SHA exception
# part of the production verifier.
tmp_root="$(mktemp -d)"
cleanup() { rm -rf "$tmp_root"; }
trap cleanup EXIT
valid_copy="$tmp_root/release-valid"
cp -R "$valid" "$valid_copy"
node - "$valid_copy/build-metadata.json" "$expected_sha" <<'NODE'
const fs = require("node:fs");
const path = process.argv[2];
const value = JSON.parse(fs.readFileSync(path, "utf8"));
value.commit_sha = process.argv[3];
fs.writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
NODE

if bash "$verifier" "$valid_copy" "$expected_sha" ubuntu >/dev/null; then
  report ok 'valid release metadata fixture passes'
else
  report FAIL 'valid release metadata fixture passes'
fi

if bash "$verifier" "$mismatch" "$expected_sha" ubuntu >/dev/null 2>&1; then
  report FAIL 'mismatched commit fixture fails closed'
else
  report ok 'mismatched commit fixture fails closed'
fi

if bash "$verifier" "$wrong_platform" "$expected_sha" ubuntu >/dev/null 2>&1; then
  report FAIL 'wrong-platform fixture fails closed'
else
  report ok 'wrong-platform fixture fails closed'
fi

if [[ "$failures" -eq 0 ]]; then
  echo "All release workflow contract checks passed."
  exit 0
fi
echo "$failures release workflow contract check(s) failed." >&2
exit 1
