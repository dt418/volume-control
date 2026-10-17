#!/usr/bin/env bash
# Package a release binary into dist/ with a versioned name and checksums.
#
# Usage: RELEASE_PLATFORM=<windows|macos|ubuntu> RELEASE_TAG=vX.Y.Z scripts/package.sh
#
# Produces, in dist/:
#   volumecontrol-<version>-<platform>.<zip|tar.gz>
#   Windows: VolumeControl.exe + README
#   macOS:   VolumeControl.app (ad-hoc signed bundle) + README
#   Ubuntu:  volumectl + README
#   SHA256SUMS.txt                                     (checksums)
set -euo pipefail

platform="${RELEASE_PLATFORM:?set RELEASE_PLATFORM (windows|macos|ubuntu)}"
tag="${RELEASE_TAG:?set RELEASE_TAG (e.g. v0.1.0)}"
version="${tag#v}"

case "$platform" in
  windows) bin="target/release/VolumeControl.exe" ; ext="zip" ;;
  macos)   bin="target/release/VolumeControl"     ; ext="zip" ;;
  ubuntu)  bin="target/release/VolumeControl"     ; ext="tar.gz" ;;
  *) echo "unknown platform: $platform" >&2 ; exit 1 ;;
esac

test -s "$bin" || { echo "release binary missing: $bin" >&2 ; exit 1; }

name="volumecontrol-${version}-${platform}.${ext}"
# A release artifact directory is promoted as a unit. Recreate it so stale
# packages, checksums, or metadata from an earlier local run cannot leak in
# (portable across GNU and BSD userlands).
rm -rf dist
mkdir -p dist

case "$ext" in
  zip)
    if [ "$platform" = "windows" ]; then
      # Windows: Compress-Archive is present on every Windows install.
      # Double the backslashes for the PowerShell -Path list; forward slashes
      # work too but the absolute path is the reliable form.
      win_bin="$(cygpath -w -a "$bin" 2>/dev/null || echo "$bin")"
      win_readme="$(cygpath -w -a README.md 2>/dev/null || echo "README.md")"
      powershell.exe -NoProfile -Command \
        "Compress-Archive -Force -Path '$win_bin','$win_readme' -DestinationPath '$(cygpath -w -a dist)/$name'" \
        || { echo "Compress-Archive failed" >&2; exit 1; }
    else
      # macOS: build a proper .app bundle. A plain Mach-O binary cannot be
      # added to the Accessibility permission list reliably and Gatekeeper
      # blocks unsigned downloads; the bundle (ad-hoc signed) gives the OS an
      # app identity so the permission prompt and System Settings entry work.
      app_tmp="dist/app-build-$$"
      mkdir -p "$app_tmp/VolumeControl.app/Contents/MacOS" \
               "$app_tmp/VolumeControl.app/Contents/Resources"
      cp "$bin" "$app_tmp/VolumeControl.app/Contents/MacOS/volumectl"
      sed "s/@VERSION@/$version/g" packaging/macos/Info.plist \
        > "$app_tmp/VolumeControl.app/Contents/Info.plist"
      cp README.md "$app_tmp/"
      codesign --force --sign - "$app_tmp/VolumeControl.app"
      (cd "$app_tmp" && zip -r -y "../$name" VolumeControl.app README.md)
      rm -rf "$app_tmp"
    fi
    ;;
  tar.gz)
    tmp="dist/pkg-$$"
    mkdir -p "$tmp"
    cp "$bin" "$tmp/volumectl"
    cp README.md "$tmp/"
    tar czf "dist/$name" -C "$tmp" volumectl README.md
    rm -rf "$tmp"
    ;;
esac
if [ "$platform" = "windows" ]; then
  nsis="target/release/bundle/nsis/VolumeControl_${version}_x64-setup.exe"
  test -s "$nsis" || { echo "release NSIS installer missing: $nsis" >&2; exit 1; }
  cp "$nsis" "dist/$(basename "$nsis")"
  cp scripts/Install-VolumeControl.ps1 dist/Install-VolumeControl.ps1
fi

# macOS/Linux ship their own one-line installers next to the package so the
# product-page curl commands can verify and install from the same release.
case "$platform" in
  macos)
    test -s packaging/install-macos.sh || { echo "missing packaging/install-macos.sh" >&2; exit 1; }
    cp packaging/install-macos.sh dist/install-macos.sh
    dmg="$(find target/release/bundle/dmg -type f -name '*.dmg' 2>/dev/null | head -n 1)"
    test -n "$dmg" && test -s "$dmg" || { echo "release dmg missing under target/release/bundle/dmg" >&2; exit 1; }
    cp "$dmg" "dist/volumecontrol-${version}-macos.dmg"
    ;;
  ubuntu)
    test -s packaging/install-linux.sh || { echo "missing packaging/install-linux.sh" >&2; exit 1; }
    cp packaging/install-linux.sh dist/install-linux.sh
    deb="$(find target/release/bundle/deb -type f -name '*.deb' 2>/dev/null | head -n 1)"
    rpm="$(find target/release/bundle/rpm -type f -name '*.rpm' 2>/dev/null | head -n 1)"
    appimage="$(find target/release/bundle/appimage -type f -name '*.AppImage' 2>/dev/null | head -n 1)"
    test -n "$deb" && test -s "$deb" || { echo "release deb missing under target/release/bundle/deb" >&2; exit 1; }
    test -n "$rpm" && test -s "$rpm" || { echo "release rpm missing under target/release/bundle/rpm" >&2; exit 1; }
    test -n "$appimage" && test -s "$appimage" || { echo "release appimage missing under target/release/bundle/appimage" >&2; exit 1; }
    cp "$deb" "dist/volumecontrol-${version}-ubuntu.deb"
    cp "$rpm" "dist/volumecontrol-${version}-ubuntu.rpm"
    cp "$appimage" "dist/volumecontrol-${version}-ubuntu.AppImage"
    ;;
esac

# sha256sum is absent on macOS runners; shasum -a 256 is the portable form.
# The manifest covers every staged file with sorted canonical entries shaped
# "<64 hex><two spaces><basename>": no directory prefixes, no binary-mode
# markers, and SHA256SUMS.txt itself is never a manifest input.
rm -f dist/SHA256SUMS.txt
if command -v sha256sum >/dev/null 2>&1; then
  checksum() { sha256sum "$@" | sed -E 's/^([0-9a-fA-F]{64}) [*] ?/\1  /'; }
else
  checksum() { shasum -a 256 "$@" | sed -E 's/^([0-9a-fA-F]{64}) [*] ?/\1  /'; }
fi
(cd dist && checksum $(ls -1 | LC_ALL=C sort) > SHA256SUMS.txt)
echo "packaged: dist/$name"
echo "artifact: $name"
ls -la "dist/$name"
