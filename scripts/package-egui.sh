#!/usr/bin/env bash
# Build and package the native egui host on Linux or macOS.
#
#   scripts/package-egui.sh [version] [output-root]
#
# Linux:  volumecontrol-<ver>-linux-<x86_64|arm64>.tar.gz/.deb/.rpm/.AppImage
# macOS:  volumecontrol-<ver>-macos-<x86_64|arm64>.dmg and .zip
# Names match the host updater's asset contract.
# plus SHA256SUMS.txt. Signing/notarization is out of scope.
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"

version=${1:-}
out_root=${2:-output/egui}

if [[ -z "$version" ]]; then
  version=$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' src-tauri/tauri.conf.json | head -n1)
fi
[[ -n "$version" ]] || { echo "cannot resolve version" >&2; exit 1; }

# SKIP_BUILD=1 reuses target/release (the workflow compiles without secrets).
if [[ -z "${SKIP_BUILD:-}" ]]; then
  cargo build --release -p volumecontrol-egui
fi
mkdir -p "$out_root"
stage="$out_root/stage"
rm -rf "$stage"
mkdir -p "$stage"

case "$(uname -s)" in
  Linux)
    install -m 0755 target/release/volumecontrol-host "$stage/volumecontrol-host"
    install -m 0644 packaging/egui/volumecontrol.desktop "$stage/volumecontrol.desktop"
    cp src-tauri/icons/128x128.png "$stage/volumecontrol.png"
    cat > "$stage/README.txt" <<'EOF'
Volume Control — egui host

Install (per user):
  install -Dm755 volumecontrol-host ~/.local/bin/volumecontrol-host
  install -Dm644 volumecontrol.desktop ~/.local/share/applications/volumecontrol.desktop
  install -Dm644 volumecontrol.png ~/.local/share/icons/hicolor/128x128/apps/volumecontrol.png

The host needs a compositor for translucency; blur is compositor-controlled
(KWin, GNOME Blur My Shell, Hyprland).
EOF
    namearch=x86_64
    [[ "$(uname -m)" == "aarch64" ]] && namearch=arm64
    tar -C "$stage" -czf "$out_root/volumecontrol-$version-linux-$namearch.tar.gz" .

    # Debian/Ubuntu package
    debarch=amd64
    [[ "$(uname -m)" == "aarch64" ]] && debarch=arm64
    debroot="$stage/deb"
    rm -rf "$debroot"
    mkdir -p "$debroot/DEBIAN" "$debroot/usr/bin" \
      "$debroot/usr/share/applications" \
      "$debroot/usr/share/icons/hicolor/128x128/apps"
    install -m 0755 target/release/volumecontrol-host "$debroot/usr/bin/volumecontrol-host"
    install -m 0644 packaging/egui/volumecontrol.desktop "$debroot/usr/share/applications/volumecontrol.desktop"
    install -m 0644 src-tauri/icons/128x128.png "$debroot/usr/share/icons/hicolor/128x128/apps/volumecontrol.png"
    cat > "$debroot/DEBIAN/control" <<VC_DEB
Package: volumecontrol
Version: $version
Section: sound
Priority: optional
Architecture: $debarch
Maintainer: VolumeControl <releases@volumecontrol.local>
Depends: libpulse0, libgtk-3-0, libayatana-appindicator3-1, libxkbcommon0, libxkbcommon-x11-0, libgl1
Description: Volume mixer, hotkeys, and on-screen volume HUD
 Native egui host with a system tray, global hotkeys, per-app mixer,
 volume overlay, and draft-based settings.
VC_DEB
    if command -v dpkg-deb >/dev/null 2>&1; then
      dpkg-deb --build --root-owner-group "$debroot" \
        "$out_root/volumecontrol-$version-linux-$namearch.deb" >/dev/null
    else
      echo "dpkg-deb not found; skipping .deb" >&2
    fi

    # Fedora/RHEL package
    if command -v rpmbuild >/dev/null 2>&1; then
      rpmbase=${version%%-*}
      rpmrel=${version#*-}
      [[ "$rpmrel" == "$version" ]] && rpmrel=1
      rpmarch=$(uname -m)
      top="$root/$out_root/stage/rpmbuild"
      rm -rf "$top"
      mkdir -p "$top"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
      cp target/release/volumecontrol-host "$top/SOURCES/"
      cp packaging/egui/volumecontrol.desktop "$top/SOURCES/"
      cp src-tauri/icons/128x128.png "$top/SOURCES/volumecontrol.png"
      cat > "$top/SPECS/volumecontrol.spec" <<VC_RPM
%global debug_package %{nil}
Name: volumecontrol
Version: $rpmbase
Release: $rpmrel
Summary: Volume mixer, hotkeys, and on-screen volume HUD
License: MIT
Requires: pulseaudio-libs, gtk3, libayatana-appindicator-gtk3, libxkbcommon
Source0: volumecontrol-host
Source1: volumecontrol.desktop
Source2: volumecontrol.png
%description
Native egui host with a system tray, global hotkeys, per-app mixer,
volume overlay, and draft-based settings.
%install
mkdir -p %{buildroot}/usr/bin %{buildroot}/usr/share/applications \
  %{buildroot}/usr/share/icons/hicolor/128x128/apps
install -m 0755 %{_sourcedir}/volumecontrol-host %{buildroot}/usr/bin/volumecontrol-host
install -m 0644 %{_sourcedir}/volumecontrol.desktop %{buildroot}/usr/share/applications/volumecontrol.desktop
install -m 0644 %{_sourcedir}/volumecontrol.png %{buildroot}/usr/share/icons/hicolor/128x128/apps/volumecontrol.png
%files
/usr/bin/volumecontrol-host
/usr/share/applications/volumecontrol.desktop
/usr/share/icons/hicolor/128x128/apps/volumecontrol.png
VC_RPM
      rpmbuild -bb --define "_topdir $top" --target "$rpmarch" \
        "$top/SPECS/volumecontrol.spec" >/dev/null
      find "$top/RPMS" -name '*.rpm' -exec cp {} "$out_root/volumecontrol-$version-linux-$namearch.rpm" \;
    else
      echo "rpmbuild not found; skipping .rpm" >&2
    fi

    # Portable AppImage
    if command -v appimagetool >/dev/null 2>&1; then
      appdir="$stage/VolumeControl.AppDir"
      rm -rf "$appdir"
      mkdir -p "$appdir/usr/bin" "$appdir/usr/share/applications" \
        "$appdir/usr/share/icons/hicolor/128x128/apps"
      install -m 0755 target/release/volumecontrol-host "$appdir/usr/bin/volumecontrol-host"
      install -m 0644 packaging/egui/volumecontrol.desktop "$appdir/volumecontrol.desktop"
      install -m 0644 packaging/egui/volumecontrol.desktop "$appdir/usr/share/applications/volumecontrol.desktop"
      install -m 0644 src-tauri/icons/128x128.png "$appdir/volumecontrol.png"
      cp src-tauri/icons/128x128.png "$appdir/.DirIcon"
      install -m 0644 src-tauri/icons/128x128.png \
        "$appdir/usr/share/icons/hicolor/128x128/apps/volumecontrol.png"
      printf '%s\n' '#!/bin/sh' 'exec "$APPDIR/usr/bin/volumecontrol-host" "$@"' > "$appdir/AppRun"
      chmod 0755 "$appdir/AppRun"
      apparch=x86_64
      [[ "$(uname -m)" == "aarch64" ]] && apparch=aarch64
      ARCH="$apparch" APPIMAGE_EXTRACT_AND_RUN=1 appimagetool "$appdir" \
        "$out_root/volumecontrol-$version-linux-$namearch.AppImage" >/dev/null
    else
      echo "appimagetool not found; skipping .AppImage" >&2
    fi
    ;;
  Darwin)
    arch=$(uname -m)
    app="$stage/VolumeControl.app"
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    install -m 0755 target/release/volumecontrol-host "$app/Contents/MacOS/volumecontrol-host"
    cp src-tauri/icons/icon.icns "$app/Contents/Resources/icon.icns"
    sed -e "s/@VERSION@/$version/g" \
        -e 's#<string>volumectl</string>#<string>volumecontrol-host</string>#' \
        packaging/macos/Info.plist > "$app/Contents/Info.plist"
    identity=${MACOS_SIGN_IDENTITY:-${APPLE_SIGNING_IDENTITY:--}}
    if [[ "${REQUIRE_SIGNING:-false}" == "true" && "$identity" == "-" ]]; then
      if [[ "${MACOS_SIGNING_DISABLED:-false}" == "true" ]]; then
        echo "WARNING: macOS signing/notarization temporarily disabled; shipping ad-hoc build" >&2
      else
        echo "REQUIRE_SIGNING=true but MACOS_SIGN_IDENTITY is not set" >&2
        exit 1
      fi
    fi
    keychain_dir=""
    cleanup_keychain() {
      if [[ -n "$keychain_dir" ]]; then
        security delete-keychain "$keychain_dir/signing.keychain-db" 2>/dev/null || true
        rm -rf "$keychain_dir"
      fi
    }
    trap cleanup_keychain EXIT
    if [[ -n "${MACOS_CERT_P12_BASE64:-}" ]]; then
      keychain_dir=$(mktemp -d)
      keychain="$keychain_dir/signing.keychain-db"
      printf '%s' "$MACOS_CERT_P12_BASE64" | base64 -D > "$keychain_dir/cert.p12"
      security create-keychain -p "$MACOS_CERT_PASSWORD" "$keychain"
      security import "$keychain_dir/cert.p12" -k "$keychain" -P "$MACOS_CERT_PASSWORD" \
        -T /usr/bin/codesign
      security set-key-partition-list -S apple-tool:,apple: -s \
        -k "$MACOS_CERT_PASSWORD" "$keychain" >/dev/null
      existing=()
      while IFS= read -r line; do
        existing+=("$line")
      done < <(security list-keychains -d user | sed 's/^[[:space:]]*//; s/"//g')
      security list-keychains -d user -s "$keychain" "${existing[@]}"
    fi
    if command -v codesign >/dev/null 2>&1; then
      if [[ "$identity" != "-" ]]; then
        codesign --force --deep --options runtime --timestamp --sign "$identity" "$app"
      else
        codesign --force --deep --sign - "$app" || echo "ad-hoc signing failed" >&2
      fi
    fi
    namearch=x86_64
    [[ "$(uname -m)" == "arm64" || "$(uname -m)" == "aarch64" ]] && namearch=arm64
    dmg="$out_root/volumecontrol-$version-macos-$namearch.dmg"
    zip="$out_root/volumecontrol-$version-macos-$namearch.zip"
    rm -f "$dmg"
    hdiutil create -volname "Volume Control" -srcfolder "$app" \
      -ov -format UDZO "$dmg" >/dev/null
    notary_profile=${MACOS_NOTARY_PROFILE:-}
    apple_id=${MACOS_APPLE_ID:-${APPLE_ID:-}}
    team_id=${MACOS_TEAM_ID:-${APPLE_TEAM_ID:-}}
    app_password=${MACOS_APP_PASSWORD:-${APPLE_APP_PASSWORD:-}}
    if [[ "$identity" != "-" ]] && { [[ -n "$notary_profile" ]] || \
        { [[ -n "$apple_id" && -n "$team_id" && -n "$app_password" ]]; }; }; then
      if [[ -n "$notary_profile" ]]; then
        xcrun notarytool submit "$dmg" --keychain-profile "$notary_profile" --wait
      else
        xcrun notarytool submit "$dmg" --apple-id "$apple_id" --team-id "$team_id" \
          --password "$app_password" --wait
      fi
      xcrun stapler staple "$dmg"
      rm -f "$zip"
    elif [[ "$identity" != "-" ]]; then
      if [[ "${REQUIRE_SIGNING:-false}" == "true" ]]; then
        echo "REQUIRE_SIGNING=true but notarization credentials are not set" >&2
        exit 1
      fi
      echo "notarization skipped (MACOS_NOTARY_PROFILE or Apple credentials not set)" >&2
    fi
    (cd "$stage" && zip -qry "../$(basename "$zip")" "VolumeControl.app")
    ;;
  *)
    echo "unsupported platform: $(uname -s)" >&2
    exit 1
    ;;
esac

(
  cd "$out_root"
  rm -f SHA256SUMS.txt
  for f in volumecontrol-*.tar.gz volumecontrol-*.dmg volumecontrol-*.zip volumecontrol-*.deb volumecontrol-*.rpm volumecontrol-*.AppImage; do
    [[ -e "$f" ]] || continue
    if command -v sha256sum >/dev/null 2>&1; then
      sha256sum "$f" >> SHA256SUMS.txt
    else
      shasum -a 256 "$f" >> SHA256SUMS.txt
    fi
  done
)

if [[ "$(uname -s)" == "Linux" && "${REQUIRE_SIGNING:-false}" == "true" && -z "${GPG_KEY_ID:-}" ]]; then
  echo "REQUIRE_SIGNING=true but GPG_KEY_ID is not set" >&2
  exit 1
fi
if [[ "$(uname -s)" == "Linux" && -n "${GPG_KEY_ID:-}" && -f "$out_root/SHA256SUMS.txt" ]]; then
  gpg --batch --yes --armor --detach-sign -u "$GPG_KEY_ID" "$out_root/SHA256SUMS.txt"
  echo "signed SHA256SUMS.txt with $GPG_KEY_ID"
fi

echo "artifacts in $out_root"
ls -l "$out_root"
