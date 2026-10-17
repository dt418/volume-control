#!/usr/bin/env bash
#
# test-install-unix.sh - behavioural tests for packaging/install-macos.sh and
# packaging/install-linux.sh against a local fixture release (file:// base).
#
# Runs on any POSIX shell with curl, zip, unzip, tar, and gzip (Git Bash on
# Windows included): every case drives the real script, the real checksum
# verification, and the real extraction path against fixture assets.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
macos_installer="$repo/packaging/install-macos.sh"
linux_installer="$repo/packaging/install-linux.sh"
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

fixture_root="$(mktemp -d)"
trap 'rm -rf "$fixture_root"' EXIT

tag="v0.0.0"
version="0.0.0"
release_dir="$fixture_root/release/$tag"
mkdir -p "$release_dir"

# ── fixture assets ──────────────────────────────────────────────────────────
app_dir="$fixture_root/build/$version-macos"
mkdir -p "$app_dir/VolumeControl.app/Contents/MacOS"
printf '#!/bin/sh\necho harness-free macos stub\n' > "$app_dir/VolumeControl.app/Contents/MacOS/volumectl"
chmod +x "$app_dir/VolumeControl.app/Contents/MacOS/volumectl"
printf 'plist-stub' > "$app_dir/VolumeControl.app/Contents/Info.plist"
printf 'readme' > "$app_dir/README.md"

make_zip() { # srcdir dest
  if command -v zip >/dev/null 2>&1; then
    (cd "$1" && zip -qr "$2" VolumeControl.app README.md)
  elif command -v python3 >/dev/null 2>&1; then
    python3 - "$1" "$2" <<'PY'
import os, sys, zipfile
src, dest = sys.argv[1], sys.argv[2]
with zipfile.ZipFile(dest, "w", zipfile.ZIP_DEFLATED) as archive:
    for base, _dirs, files in os.walk(src):
        for name in files:
            path = os.path.join(base, name)
            archive.write(path, os.path.relpath(path, src).replace(os.sep, "/"))
PY
  else
    printf 'neither zip nor python3 is available for fixture creation\n' >&2
    exit 1
  fi
}

make_zip "$app_dir" "$release_dir/volumecontrol-$version-macos.zip"

linux_dir="$fixture_root/build/$version-ubuntu"
mkdir -p "$linux_dir"
printf '#!/bin/sh\necho harness-free linux stub\n' > "$linux_dir/volumectl"
chmod +x "$linux_dir/volumectl"
printf 'readme' > "$linux_dir/README.md"
(cd "$linux_dir" && tar czf "$release_dir/volumecontrol-$version-linux-amd64.tar.gz" volumectl README.md)

# Canonical manifest (sorted basenames, two spaces).
(
  cd "$release_dir"
  digest_of() {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi
  }
  for asset in $(ls -1 | LC_ALL=C sort); do digest_of "$asset"; done \
    | sed -E 's/^([0-9a-fA-F]{64}) [*] ?/\1  /' > SHA256SUMS.txt
)

# curl on Windows needs a drive-letter path inside file:// URLs; MSYS paths
# still work for the shell side.
fixture_root_url="$(cygpath -m "$fixture_root" 2>/dev/null || printf '%s' "$fixture_root")"
base="file://$fixture_root_url/release"

run_macos() { # action, root, extra env..., captures stdout
  # VC_VERSION_OVERRIDE lets a case exercise the malformed-version path.
  VOLUMECTL_RELEASE_BASE="$base" VOLUMECTL_VERSION="${VC_VERSION_OVERRIDE-$tag}" \
    VOLUMECTL_INSTALL_ROOT="$2" bash "$macos_installer" "$1" "${@:3}"
}

run_linux() {
  VOLUMECTL_RELEASE_BASE="$base" VOLUMECTL_VERSION="${VC_VERSION_OVERRIDE-$tag}" \
    VOLUMECTL_INSTALL_ROOT="$2" bash "$linux_installer" "$1" "${@:3}"
}

# ── macOS installer ─────────────────────────────────────────────────────────
mac_root="$fixture_root/macos-root"
if out="$(run_macos install "$mac_root" 2>&1)"; then
  case "$out" in *"Installed VolumeControl $version"*) report ok 'macos install copies the verified app bundle' ;;
                 *) report FAIL 'macos install copies the verified app bundle' ;; esac
else
  printf '%s\n' "$out" >&2
  report FAIL 'macos install copies the verified app bundle'
fi
[[ -x "$mac_root/VolumeControl.app/Contents/MacOS/volumectl" ]] \
  && report ok 'macos install preserves the bundle layout' \
  || report FAIL 'macos install preserves the bundle layout'

if run_macos install "$mac_root" >/dev/null 2>&1; then
  report FAIL 'macos install refuses to overwrite an existing install'
else
  report ok 'macos install refuses to overwrite an existing install'
fi

printf 'tampered' >> "$mac_root/VolumeControl.app/Contents/Info.plist"
if out="$(run_macos update "$mac_root" 2>&1)"; then
  case "$out" in *"Updated VolumeControl $version"*) report ok 'macos update replaces the installed app' ;;
                 *) report FAIL 'macos update replaces the installed app' ;; esac
else
  printf '%s\n' "$out" >&2
  report FAIL 'macos update replaces the installed app'
fi
grep -q 'plist-stub' "$mac_root/VolumeControl.app/Contents/Info.plist" \
  && report ok 'macos update restores pristine bundle bytes' \
  || report FAIL 'macos update restores pristine bundle bytes'

if out="$(run_macos uninstall "$mac_root" 2>&1)"; then
  case "$out" in *"Removed VolumeControl"*) report ok 'macos uninstall removes the app' ;;
                 *) report FAIL 'macos uninstall removes the app' ;; esac
else
  printf '%s\n' "$out" >&2
  report FAIL 'macos uninstall removes the app'
fi
[[ ! -e "$mac_root/VolumeControl.app" ]] \
  && report ok 'macos uninstall leaves no bundle behind' \
  || report FAIL 'macos uninstall leaves no bundle behind'
if out="$(run_macos uninstall "$mac_root" 2>&1)"; then
  case "$out" in *"nothing installed"*) report ok 'macos uninstall on a clean root reports nothing installed' ;;
                 *) report FAIL 'macos uninstall on a clean root reports nothing installed' ;; esac
else
  report FAIL 'macos uninstall on a clean root reports nothing installed'
fi
if run_macos update "$mac_root" >/dev/null 2>&1; then
  report FAIL 'macos update without an install fails closed'
else
  report ok 'macos update without an install fails closed'
fi

# Checksum tampering: flip a byte in the zip copy the installer will fetch.
cp "$release_dir/volumecontrol-$version-macos.zip" "$release_dir/keep.zip"
printf 'x' | dd of="$release_dir/volumecontrol-$version-macos.zip" bs=1 seek=10 conv=notrunc 2>/dev/null
if out="$(run_macos install "$fixture_root/macos-bad" 2>&1)"; then
  report FAIL 'macos install rejects a digest mismatch'
else
  case "$out" in *"SHA-256 mismatch"*) report ok 'macos install rejects a digest mismatch' ;;
                 *) report FAIL 'macos install rejects a digest mismatch' ;; esac
fi
mv "$release_dir/keep.zip" "$release_dir/volumecontrol-$version-macos.zip"

# Manifest with no entry for the asset.
cp "$release_dir/SHA256SUMS.txt" "$release_dir/SHA256SUMS.keep"
grep -v "macos.zip" "$release_dir/SHA256SUMS.keep" > "$release_dir/SHA256SUMS.txt"
if out="$(run_macos install "$fixture_root/macos-missing" 2>&1)"; then
  report FAIL 'macos install requires a canonical manifest entry'
else
  case "$out" in *"no canonical entry"*) report ok 'macos install requires a canonical manifest entry' ;;
                 *) report FAIL 'macos install requires a canonical manifest entry' ;; esac
fi
mv "$release_dir/SHA256SUMS.keep" "$release_dir/SHA256SUMS.txt"

# Vietnamese failure message via VOLUMECTL_LANG (reinstall against an
# installed root to hit the refusal path).
run_macos install "$fixture_root/macos-vi" >/dev/null 2>&1
if out="$(VOLUMECTL_LANG=vi run_macos install "$fixture_root/macos-vi" 2>&1)"; then
  report FAIL 'macos refuses a second install in Vietnamese too'
else
  case "$out" in *"dùng 'update'"*) report ok 'macos localizes failure messages (vi)' ;;
                 *) report FAIL 'macos localizes failure messages (vi)' ;; esac
fi

if out="$(VC_VERSION_OVERRIDE=1.2.3 run_macos install "$fixture_root/macos-v" 2>&1)"; then
  report FAIL 'macos rejects a malformed version'
else
  case "$out" in *"vX.Y.Z"*) report ok 'macos rejects a malformed version' ;;
                 *) report FAIL 'macos rejects a malformed version' ;; esac
fi

# ── Linux installer ─────────────────────────────────────────────────────────
linux_root="$fixture_root/linux-root"
if out="$(run_linux install "$linux_root" 2>&1)"; then
  case "$out" in *"Installed VolumeControl $version"*) report ok 'linux install places the verified binary' ;;
                 *) report FAIL 'linux install places the verified binary' ;; esac
else
  printf '%s\n' "$out" >&2
  report FAIL 'linux install places the verified binary'
fi
[[ -x "$linux_root/bin/volumectl" ]] \
  && report ok 'linux install marks the binary executable' \
  || report FAIL 'linux install marks the binary executable'
[[ -f "$linux_root/share/doc/volumecontrol/README.md" ]] \
  && report ok 'linux install ships the release README' \
  || report FAIL 'linux install ships the release README'

if run_linux install "$linux_root" >/dev/null 2>&1; then
  report FAIL 'linux install refuses to overwrite an existing install'
else
  report ok 'linux install refuses to overwrite an existing install'
fi

if out="$(run_linux update "$linux_root" 2>&1)"; then
  case "$out" in *"Updated VolumeControl $version"*) report ok 'linux update reinstalls the verified binary' ;;
                 *) report FAIL 'linux update reinstalls the verified binary' ;; esac
else
  printf '%s\n' "$out" >&2
  report FAIL 'linux update reinstalls the verified binary'
fi

cp "$release_dir/volumecontrol-$version-linux-amd64.tar.gz" "$release_dir/keep.tar.gz"
printf 'x' | dd of="$release_dir/volumecontrol-$version-linux-amd64.tar.gz" bs=1 seek=10 conv=notrunc 2>/dev/null
if out="$(run_linux install "$fixture_root/linux-bad" 2>&1)"; then
  report FAIL 'linux install rejects a digest mismatch'
else
  case "$out" in *"SHA-256 mismatch"*) report ok 'linux install rejects a digest mismatch' ;;
                 *) report FAIL 'linux install rejects a digest mismatch' ;; esac
fi
mv "$release_dir/keep.tar.gz" "$release_dir/volumecontrol-$version-linux-amd64.tar.gz"

if out="$(run_linux uninstall "$linux_root" 2>&1)"; then
  case "$out" in *"Removed VolumeControl"*) report ok 'linux uninstall removes the installed files' ;;
                 *) report FAIL 'linux uninstall removes the installed files' ;; esac
else
  printf '%s\n' "$out" >&2
  report FAIL 'linux uninstall removes the installed files'
fi
[[ ! -e "$linux_root/bin/volumectl" ]] \
  && report ok 'linux uninstall leaves no binary behind' \
  || report FAIL 'linux uninstall leaves no binary behind'
if out="$(run_linux uninstall "$linux_root" 2>&1)"; then
  case "$out" in *"nothing installed"*) report ok 'linux uninstall on a clean root reports nothing installed' ;;
                 *) report FAIL 'linux uninstall on a clean root reports nothing installed' ;; esac
else
  report FAIL 'linux uninstall on a clean root reports nothing installed'
fi

# Latest-release resolution against the fixture layout.
if out="$(VC_VERSION_OVERRIDE= run_macos install "$fixture_root/macos-latest" 2>&1)"; then
  case "$out" in *"Installed VolumeControl $version"*) report ok 'macos resolves the latest tag from the release layout' ;;
                 *) report FAIL 'macos resolves the latest tag from the release layout' ;; esac
else
  report FAIL 'macos resolves the latest tag from the release layout'
fi

printf '\n'
if (( failures == 0 )); then
  printf 'All install-script checks passed.\n'
else
  printf '%d install-script check(s) failed.\n' "$failures"
  exit 1
fi
