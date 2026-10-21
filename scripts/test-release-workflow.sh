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

expect_failure() {
  if bash "$verifier" "$1" "$2" "$3" >/dev/null 2>&1; then
    report FAIL "$4 fails closed"
  else
    report ok "$4 fails closed"
  fi
}

if contains "$release" 'uses:[[:space:]]*\./\.github/workflows/desktop-validation\.yml'; then
  report ok 'release caller invokes the reusable desktop validation workflow'
else
  report FAIL 'release caller invokes the reusable desktop validation workflow'
fi

# The E2E default is REAL audio; hosted runners do not guarantee an
# endpoint, so every release-path E2E gate must explicitly opt into the
# deterministic virtual backend. A future default flip must not silently
# change the release evidence path.
e2e_virtual_count="$(grep -Eci 'verify-tauri-e2e\.(ps1|sh).*virtual' "$validation" || true)"
if [ "$e2e_virtual_count" -eq 3 ]; then
  report ok 'desktop validation E2E gates opt into --virtual-audio (3/3)'
else
  report FAIL "desktop validation E2E gates opt into --virtual-audio" "found $e2e_virtual_count/3"
fi

preflight_line="$(grep -n '^  preflight:' "$release" | head -n 1 | cut -d: -f1 || true)"
validate_line="$(grep -n '^  validate:' "$release" | head -n 1 | cut -d: -f1 || true)"
if [[ -n "$preflight_line" && -n "$validate_line" && "$preflight_line" -lt "$validate_line" ]] && \
   contains "$release" 'needs:[[:space:]]*preflight'; then
  report ok 'release tag preflight gates the reusable validation matrix'
else
  report FAIL 'release tag preflight gates the reusable validation matrix'
fi

if contains "$release" 'needs:[[:space:]]*\[[^]]*preflight[^]]*validate[^]]*\]'; then
  report ok 'publish job requires preflight and validation'
else
  report FAIL 'publish job requires preflight and validation'
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

if contains "$release" 'EXPECTED_COMMIT_SHA:[[:space:]]*\$\{\{[[:space:]]*github\.sha[[:space:]]*\}\}' && \
   contains "$release" 'validated-\$\{platform\}-\$\{EXPECTED_COMMIT_SHA\}' && \
   contains "$validation" 'RELEASE_COMMIT_SHA:[[:space:]]*\$\{\{[[:space:]]*github\.sha[[:space:]]*\}\}' && \
   contains "$validation" 'validated-\$\{\{[[:space:]]*matrix\.platform[[:space:]]*\}\}-\$\{\{[[:space:]]*github\.sha[[:space:]]*\}\}'; then
  report ok 'publish and reusable validation remain bound to github.sha'
else
  report FAIL 'publish and reusable validation remain bound to github.sha'
fi

if contains "$release" 'tag_ref=' && \
   contains "$release" 'git/ref/tags' && \
   contains "$release" 'gh[[:space:]]+api[[:space:]]+"\$tag_ref"' && \
   contains "$release" 'EXPECTED_COMMIT_SHA:[[:space:]]*\$\{\{[[:space:]]*github\.sha[[:space:]]*\}\}' && \
   contains "$release" 'tag_sha.*EXPECTED_COMMIT_SHA'; then
  report ok 'release preflight binds the requested tag to github.sha'
else
  report FAIL 'release preflight binds the requested tag to github.sha'
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

if contains "$validation" 'find[[:space:]]+output/tauri-e2e.*junit.*-size[[:space:]]+\+0c' && \
   contains "$validation" 'find[[:space:]]+output/tauri-e2e.*manifest\.json.*-size[[:space:]]+\+0c' && \
   contains "$validation" 'find[[:space:]]+output/tauri-e2e.*timings\.json.*-size[[:space:]]+\+0c' && \
   contains "$validation" 'find[[:space:]]+output/tauri-e2e.*logs/.*-size[[:space:]]+\+0c' && \
   ! grep -Eq 'find[[:space:]]+output/tauri-e2e.*(junit|manifest\.json|timings\.json|logs/).*\|\|[[:space:]]*true' "$validation"; then
  report ok 'validation assembly requires non-empty JUnit, manifest, timings, and platform logs'
else
  report FAIL 'validation assembly requires non-empty JUnit, manifest, timings, and platform logs'
fi

# Task 3 contract: Windows validation must produce a real NSIS bundle,
# metadata must declare the artifacts array from the staged manifest, and
# publish must stage a flat immutable release-assets directory.
if contains "$validation" 'matrix\.platform == .windows.' && \
   contains "$validation" 'tauri build --bundles nsis --ci' && \
   contains "$validation" 'tauri build --bundles dmg --ci' && \
   contains "$validation" 'tauri build --bundles deb,rpm,appimage --ci'; then
  report ok 'validation builds nsis, dmg, and deb/rpm/appimage bundles per platform'
else
  report FAIL 'validation builds nsis, dmg, and deb/rpm/appimage bundles per platform'
fi

if contains "$validation" 'artifacts:[[:space:]]' && \
   ! contains "$validation" 'find dist -maxdepth 1'; then
  report ok 'build metadata declares artifacts array from the staged manifest'
else
  report FAIL 'build metadata declares artifacts array from the staged manifest'
fi

if contains "$release" 'release-assets' && \
   contains "$release" 'cd[[:space:]]+release-assets' && \
   contains "$release" 'release-assets/SHA256SUMS\.txt'; then
  report ok 'release stages flat assets and checksums inside the staging dir'
else
  report FAIL 'release stages flat assets and checksums inside the staging dir'
fi

if ! contains "$release" '[-]{2}clobber'; then
  report ok 'release upload never overwrites existing assets'
else
  report FAIL 'release upload never overwrites existing assets'
fi

if contains "$release" "name '\\*\\.exe'" && \
   contains "$release" "name '\\*\\.ps1'"; then
  report ok 'staged release assets must include exe installer and bootstrap script'
else
  report FAIL 'staged release assets must include exe installer and bootstrap script'
fi

# Task 5 contract: the NSIS installer terminates a running host before
# replacing its executable during an update.
hook="$repo/src-tauri/nsis/hooks.nsh"
if [[ -f "$hook" ]] && \
   contains "$repo/src-tauri/tauri.conf.json" \
     '"installerHooks"[[:space:]]*:[[:space:]]*"nsis/hooks\.nsh"' && \
   contains "$hook" '!macro[[:space:]]+NSIS_HOOK_PREINSTALL' && \
   contains "$hook" 'taskkill\.exe" /IM VolumeControl\.exe'; then
  report ok 'NSIS preinstall hook stops the running VolumeControl instance'
else
  report FAIL 'NSIS preinstall hook stops the running VolumeControl instance'
fi

# Task 4 contract: both READMEs document the safe local download-and-verify
# bootstrap sequence (never a pipe-into-shell command) and state the trust
# boundaries of unsigned, checksum-only release assets.
for readme in "$repo/README.md" "$repo/README.vi.md"; do
  readme_name="$(basename "$readme")"
  if contains "$readme" 'Install-VolumeControl\.ps1' && \
     contains "$readme" 'SHA256SUMS\.txt' && \
     contains "$readme" 'Invoke-WebRequest' && \
     contains "$readme" '[-]Action update' && \
     contains "$readme" '[-]Action uninstall' && \
     contains "$readme" '[-]Silent'; then
    report ok "$readme_name documents verified PowerShell bootstrap commands"
  else
    report FAIL "$readme_name documents verified PowerShell bootstrap commands"
  fi
  if contains "$readme" 'unsigned' && \
     contains "$readme" 'checksum' && \
     contains "$readme" 'SmartScreen'; then
    report ok "$readme_name states unsigned and checksum trust limits"
  else
    report FAIL "$readme_name states unsigned and checksum trust limits"
  fi
  if ! contains "$readme" 'Invoke-Expression' && \
     ! contains "$readme" '\|[[:space:]]+iex'; then
    report ok "$readme_name never pipes downloads into a shell"
  else
    report FAIL "$readme_name never pipes downloads into a shell"
  fi
done

# Remote install surface: the product page documents irm, curl, and winget
# flows without ever piping downloads into a shell, and the winget generator
# emits a complete manifest trio whose InstallerSha256 matches real bytes.
page="$repo/docs/index.html"
if [ -s "$page" ] && \
   contains "$page" 'Install-VolumeControl\.ps1' && \
   contains "$page" 'scriptblock\]::Create' && \
   contains "$page" 'curl\.exe' && \
   contains "$page" 'winget install --id dt418\.VolumeControl'; then
  report ok 'product page documents irm, curl, and winget remote commands'
else
  report FAIL 'product page documents irm, curl, and winget remote commands'
fi

if ! contains "$page" 'Invoke-Expression' && \
   ! contains "$page" '\|[[:space:]]+iex'; then
  report ok 'product page never pipes downloads into a shell'
else
  report FAIL 'product page never pipes downloads into a shell'
fi

winget_gen="$repo/scripts/generate-winget-manifest.sh"
fixture_exe="$repo/scripts/test-fixtures/release-windows-valid/VolumeControl_0.0.0_x64-setup.exe"
winget_out="$(mktemp -d "$repo/.winget-gen.XXXXXX")"
if command -v sha256sum >/dev/null 2>&1; then
  fixture_sha="$(sha256sum "$fixture_exe" | awk '{print toupper($1)}')"
else
  fixture_sha="$(shasum -a 256 "$fixture_exe" | awk '{print toupper($1)}')"
fi
# GNU sha256sum prefixes the digest when the file name contains a backslash.
fixture_sha="${fixture_sha#\\}"
if bash "$winget_gen" v0.0.0 "$fixture_exe" "$winget_out" >/dev/null 2>&1 && \
   [ -s "$winget_out/dt418.VolumeControl/0.0.0/dt418.VolumeControl.yaml" ] && \
   [ -s "$winget_out/dt418.VolumeControl/0.0.0/dt418.VolumeControl.installer.yaml" ] && \
   [ -s "$winget_out/dt418.VolumeControl/0.0.0/dt418.VolumeControl.locale.en-US.yaml" ] && \
   grep -q "InstallerSha256: $fixture_sha" \
     "$winget_out/dt418.VolumeControl/0.0.0/dt418.VolumeControl.installer.yaml" && \
   contains "$winget_out/dt418.VolumeControl/0.0.0/dt418.VolumeControl.installer.yaml" \
     'releases/download/v0\.0\.0/VolumeControl_0\.0\.0_x64-setup\.exe'; then
  report ok 'winget generator emits a complete hash-true manifest trio'
else
  report FAIL 'winget generator emits a complete hash-true manifest trio'
fi
rm -rf "$winget_out"

# macOS/Linux one-line installers ride the same release contract as the
# Windows bootstrap: staged per platform, declared in metadata, verified.
if contains "$repo/scripts/package.sh" 'install-macos\.sh' && \
   contains "$repo/scripts/package.sh" 'install-linux\.sh'; then
  report ok 'package.sh stages the macOS and Linux installers'
else
  report FAIL 'package.sh stages the macOS and Linux installers'
fi

if contains "$verifier" 'install-macos\.sh' && contains "$verifier" 'install-linux\.sh'; then
  report ok 'release verifier requires the platform installers'
else
  report FAIL 'release verifier requires the platform installers'
fi

if contains "$validation" 'bundles deb,rpm,appimage' && \
   contains "$validation" 'bundles dmg'; then
  report ok 'desktop validation bundles deb/rpm/AppImage and dmg per platform'
else
  report FAIL 'desktop validation bundles deb/rpm/AppImage and dmg per platform'
fi

if contains "$repo/scripts/package.sh" 'linux-amd64\.deb' && \
   contains "$repo/scripts/package.sh" 'linux-amd64\.rpm' && \
   contains "$repo/scripts/package.sh" 'linux-amd64\.AppImage' && \
   contains "$repo/scripts/package.sh" 'macos\.dmg'; then
  report ok 'package.sh stages the Linux and macOS bundles'
else
  report FAIL 'package.sh stages the Linux and macOS bundles'
fi

if contains "$repo/.github/workflows/ci.yml" 'tauri build --bundles dmg --ci'; then
  report ok 'ci macOS job bundles the dmg that package.sh stages'
else
  report FAIL 'ci macOS job bundles the dmg that package.sh stages'
fi

if contains "$repo/.github/workflows/ci.yml" 'tauri build --bundles deb,rpm,appimage --ci'; then
  report ok 'ci Ubuntu job bundles the Linux packages that package.sh stages'
else
  report FAIL 'ci Ubuntu job bundles the Linux packages that package.sh stages'
fi

# The AppImage bundler panics when no square icon is declared, and deb/rpm use
# the same list, so the bundle icon set must name a PNG.
if contains "$repo/src-tauri/tauri.conf.json" '"icons/[0-9]+x[0-9]+\.png"'; then
  report ok 'tauri.conf.json declares square bundle icons for the Linux packages'
else
  report FAIL 'tauri.conf.json declares square bundle icons for the Linux packages'
fi

# Resolve HEAD without assuming this shell's git can read this checkout: a
# worktree created by native Windows git stores a `gitdir: C:/...` pointer in
# `.git`, which a POSIX git (WSL) cannot follow. Fall back to the converted
# git dir before failing.
expected_sha="$(git -C "$repo" rev-parse HEAD 2>/dev/null || true)"
if [[ -z "$expected_sha" ]]; then
  gitdir="$(sed -n 's/^gitdir:[[:space:]]*//p' "$repo/.git" 2>/dev/null || true)"
  case "$gitdir" in
    [A-Za-z]:[\\/]*)
      if command -v wslpath >/dev/null 2>&1; then
        gitdir="$(wslpath -u "$gitdir" 2>/dev/null || true)"
      elif command -v cygpath >/dev/null 2>&1; then
        gitdir="$(cygpath -u "$gitdir" 2>/dev/null || true)"
      fi
      ;;
  esac
  if [[ -n "$gitdir" && -d "$gitdir" ]]; then
    expected_sha="$(git --git-dir="$gitdir" --work-tree="$repo" rev-parse HEAD 2>/dev/null || true)"
  fi
fi
if [[ -z "$expected_sha" ]]; then
  echo 'test-release-workflow: cannot resolve HEAD via git' >&2
  echo "  repo: $repo" >&2
  exit 1
fi
valid="$repo/scripts/test-fixtures/release-valid"
mismatch="$repo/scripts/test-fixtures/release-mismatch"
wrong_platform="$repo/scripts/test-fixtures/release-wrong-platform"
missing_package="$repo/scripts/test-fixtures/release-missing-package"
windows_valid="$repo/scripts/test-fixtures/release-windows-valid"
missing_nsis_fixture="$repo/scripts/test-fixtures/release-missing-nsis"
duplicate_checksum_fixture="$repo/scripts/test-fixtures/release-duplicate-checksum"
bootstrap_digest_mismatch_fixture="$repo/scripts/test-fixtures/release-bootstrap-mismatch"
valid_ubuntu="$repo/scripts/test-fixtures/release-valid-ubuntu"
valid_macos="$repo/scripts/test-fixtures/release-valid-macos"

# The committed valid fixture is anchored to the implementation baseline. On
# later commits, refresh only its copy so the test continues to exercise the
# verifier against the current commit without making a mutable SHA exception
# part of the production verifier.
tmp_root="$(mktemp -d "$repo/.release-workflow.XXXXXX")"
cleanup() { rm -rf "$tmp_root"; }
trap cleanup EXIT

node_bin=node
if ! command -v "$node_bin" >/dev/null 2>&1 && command -v node.exe >/dev/null 2>&1; then
  node_bin=node.exe
fi
command -v "$node_bin" >/dev/null 2>&1 || { echo 'node is required for release workflow fixtures' >&2; exit 1; }
node_path() {
  local value="$1"
  if [[ "$node_bin" == node.exe ]]; then
    if [[ "$value" =~ ^/mnt/([A-Za-z])/(.*)$ ]]; then
      printf '%s:/%s' "${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}"
      return
    fi
    if [[ "$value" =~ ^/([A-Za-z])/(.*)$ ]]; then
      printf '%s:/%s' "${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}"
      return
    fi
  fi
  printf '%s' "$value"
}

refresh_sha() {
  "$node_bin" - "$(node_path "$1")" "$expected_sha" <<'NODE'
const fs = require("node:fs");
const path = process.argv[2];
const value = JSON.parse(fs.readFileSync(path, "utf8"));
value.commit_sha = process.argv[3];
fs.writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
NODE
}

valid_copy="$tmp_root/release-valid"
cp -R "$valid" "$valid_copy"
refresh_sha "$valid_copy/build-metadata.json"

missing_copy="$tmp_root/release-missing-package"
cp -R "$missing_package" "$missing_copy"
refresh_sha "$missing_copy/build-metadata.json"

valid_ubuntu_copy="$tmp_root/release-valid-ubuntu"
cp -R "$valid_ubuntu" "$valid_ubuntu_copy"
refresh_sha "$valid_ubuntu_copy/build-metadata.json"

valid_macos_copy="$tmp_root/release-valid-macos"
cp -R "$valid_macos" "$valid_macos_copy"
refresh_sha "$valid_macos_copy/build-metadata.json"

windows_valid_copy="$tmp_root/release-windows-valid"
cp -R "$windows_valid" "$windows_valid_copy"
refresh_sha "$windows_valid_copy/build-metadata.json"

missing_nsis_copy="$tmp_root/release-missing-nsis"
cp -R "$missing_nsis_fixture" "$missing_nsis_copy"
refresh_sha "$missing_nsis_copy/build-metadata.json"

duplicate_checksum_copy="$tmp_root/release-duplicate-checksum"
cp -R "$duplicate_checksum_fixture" "$duplicate_checksum_copy"
refresh_sha "$duplicate_checksum_copy/build-metadata.json"

bootstrap_mismatch_copy="$tmp_root/release-bootstrap-mismatch"
cp -R "$bootstrap_digest_mismatch_fixture" "$bootstrap_mismatch_copy"
refresh_sha "$bootstrap_mismatch_copy/build-metadata.json"

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

if bash "$verifier" "$missing_copy" "$expected_sha" ubuntu >/dev/null 2>&1; then
  report FAIL 'missing-package fixture fails closed'
else
  report ok 'missing-package fixture fails closed'
fi

if bash "$verifier" "$valid_ubuntu_copy" "$expected_sha" ubuntu >/dev/null; then
  report ok 'ubuntu artifact-array fixture passes'
else
  report FAIL 'ubuntu artifact-array fixture passes'
fi

if bash "$verifier" "$valid_macos_copy" "$expected_sha" macos >/dev/null; then
  report ok 'macos artifact-array fixture passes'
else
  report FAIL 'macos artifact-array fixture passes'
fi

if bash "$verifier" "$windows_valid_copy" "$expected_sha" windows >/dev/null; then
  report ok 'windows artifact-array fixture passes'
else
  report FAIL 'windows artifact-array fixture passes'
fi

expect_failure "$missing_nsis_copy" "$expected_sha" windows 'missing NSIS fixture'
expect_failure "$duplicate_checksum_copy" "$expected_sha" windows 'duplicate checksum fixture'
expect_failure "$bootstrap_mismatch_copy" "$expected_sha" windows 'bootstrap digest mismatch fixture'

current_tag="v$(sed -n 's/^  "version": "\([^"]*\)",$/\1/p' "$repo/src-tauri/tauri.conf.json" | head -n 1)"
if bash "$repo/scripts/check-version-sync.sh" "$current_tag" >/dev/null 2>&1; then
  report ok 'version sync accepts the checked-in app version'
else
  report FAIL 'version sync accepts the checked-in app version'
fi
if bash "$repo/scripts/check-version-sync.sh" v0.0.0 >/dev/null 2>&1; then
  report FAIL 'version sync rejects a mismatched tag'
else
  report ok 'version sync rejects a mismatched tag'
fi

version_sync_fixture="$tmp_root/version-sync"
mkdir -p "$version_sync_fixture/scripts"
for version_file in \
  Cargo.toml \
  crates/volumectl/Cargo.toml \
  src-tauri/Cargo.toml \
  src-tauri/tauri.conf.json \
  frontend/package.json \
  frontend/package-lock.json; do
  mkdir -p "$(dirname "$version_sync_fixture/$version_file")"
  cp "$repo/$version_file" "$version_sync_fixture/$version_file"
done
cp "$repo/scripts/check-version-sync.sh" \
  "$version_sync_fixture/scripts/check-version-sync.sh"

if bash "$version_sync_fixture/scripts/check-version-sync.sh" "$current_tag" >/dev/null 2>&1; then
  report ok 'version sync fixture accepts matching nested lockfile version'
else
  report FAIL 'version sync fixture accepts matching nested lockfile version'
fi

awk '
  /^    "": \{$/ { in_root_package = 1 }
  in_root_package && /^      "version": / {
    sub(/"[^"]*"/, "\"0.0.0\"")
    in_root_package = 0
  }
  { print }
' "$version_sync_fixture/frontend/package-lock.json" \
  > "$version_sync_fixture/frontend/package-lock.json.mutated"
mv "$version_sync_fixture/frontend/package-lock.json.mutated" \
  "$version_sync_fixture/frontend/package-lock.json"

if bash "$version_sync_fixture/scripts/check-version-sync.sh" "$current_tag" >/dev/null 2>&1; then
  report FAIL 'version sync rejects nested lockfile version drift'
else
  report ok 'version sync rejects nested lockfile version drift'
fi

if [[ "$failures" -eq 0 ]]; then
  echo "All release workflow contract checks passed."
  exit 0
fi
echo "$failures release workflow contract check(s) failed." >&2
exit 1
