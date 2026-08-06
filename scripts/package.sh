#!/usr/bin/env bash
# Package a release binary into dist/ with a versioned name and checksums.
#
# Usage: RELEASE_PLATFORM=<windows|macos|ubuntu> RELEASE_TAG=vX.Y.Z scripts/package.sh
#
# Produces, in dist/:
#   volumecontrol-<version>-<platform>.<zip|tar.gz>
#   Windows: volumectl.exe + README
#   macOS:   VolumeControl.app (ad-hoc signed bundle) + README
#   Ubuntu:  volumectl + README
#   SHA256SUMS.txt                                     (checksums)
set -euo pipefail

platform="${RELEASE_PLATFORM:?set RELEASE_PLATFORM (windows|macos|ubuntu)}"
tag="${RELEASE_TAG:?set RELEASE_TAG (e.g. v0.1.0)}"
version="${tag#v}"

case "$platform" in
  windows) bin="target/release/volumectl.exe" ; ext="zip" ;;
  macos)   bin="target/release/volumectl"     ; ext="zip" ;;
  ubuntu)  bin="target/release/volumectl"     ; ext="tar.gz" ;;
  *) echo "unknown platform: $platform" >&2 ; exit 1 ;;
esac

test -s "$bin" || { echo "release binary missing: $bin" >&2 ; exit 1; }

name="volumecontrol-${version}-${platform}.${ext}"
mkdir -p dist
rm -f "dist/$name" dist/SHA256SUMS.txt

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

# sha256sum is absent on macOS runners; shasum -a 256 is the portable form
# (same "hash  name" output format).
if command -v sha256sum >/dev/null 2>&1; then
  (cd dist && sha256sum "$name" > SHA256SUMS.txt)
else
  (cd dist && shasum -a 256 "$name" > SHA256SUMS.txt)
fi
echo "packaged: dist/$name"
ls -la "dist/$name"
