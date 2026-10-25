#!/usr/bin/env bash
# Build and package the native egui host on Linux or macOS.
#
#   scripts/package-egui.sh [version] [output-root]
#
# Linux:  volumecontrol-<ver>-linux-amd64.tar.gz (binary + .desktop + icon)
# macOS:  volumecontrol-<ver>-macos.zip (.app bundle)
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

cargo build --release -p volumecontrol-egui
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
    tar -C "$stage" -czf "$out_root/volumecontrol-$version-linux-amd64.tar.gz" .
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
      echo "REQUIRE_SIGNING=true but MACOS_SIGN_IDENTITY is not set" >&2
      exit 1
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
    (cd "$stage" && zip -qry "../volumecontrol-$version-macos.zip" "VolumeControl.app")
    notary_profile=${MACOS_NOTARY_PROFILE:-}
    apple_id=${MACOS_APPLE_ID:-${APPLE_ID:-}}
    team_id=${MACOS_TEAM_ID:-${APPLE_TEAM_ID:-}}
    app_password=${MACOS_APP_PASSWORD:-${APPLE_APP_PASSWORD:-}}
    if [[ "$identity" != "-" ]] && { [[ -n "$notary_profile" ]] || \
        { [[ -n "$apple_id" && -n "$team_id" && -n "$app_password" ]]; }; }; then
      if [[ -n "$notary_profile" ]]; then
        xcrun notarytool submit "$out_root/volumecontrol-$version-macos.zip" \
          --keychain-profile "$notary_profile" --wait
      else
        xcrun notarytool submit "$out_root/volumecontrol-$version-macos.zip" \
          --apple-id "$apple_id" --team-id "$team_id" \
          --password "$app_password" --wait
      fi
      xcrun stapler staple "$app"
      rm -f "$out_root/volumecontrol-$version-macos.zip"
      (cd "$stage" && zip -qry "../volumecontrol-$version-macos.zip" "VolumeControl.app")
    elif [[ "$identity" != "-" ]]; then
      if [[ "${REQUIRE_SIGNING:-false}" == "true" ]]; then
        echo "REQUIRE_SIGNING=true but notarization credentials are not set" >&2
        exit 1
      fi
      echo "notarization skipped (MACOS_NOTARY_PROFILE or Apple credentials not set)" >&2
    fi
    ;;
  *)
    echo "unsupported platform: $(uname -s)" >&2
    exit 1
    ;;
esac

(
  cd "$out_root"
  rm -f SHA256SUMS.txt
  for f in volumecontrol-*.tar.gz volumecontrol-*.zip VolumeControl-*.zip; do
    [[ -e "$f" ]] || continue
    if command -v sha256sum >/dev/null 2>&1; then
      sha256sum "$f" >> SHA256SUMS.txt
    else
      shasum -a 256 "$f" >> SHA256SUMS.txt
    fi
  done
)

if [[ -n "${GPG_KEY_ID:-}" && -f "$out_root/SHA256SUMS.txt" ]]; then
  gpg --batch --yes --armor --detach-sign -u "$GPG_KEY_ID" "$out_root/SHA256SUMS.txt"
  echo "signed SHA256SUMS.txt with $GPG_KEY_ID"
fi

echo "artifacts in $out_root"
ls -l "$out_root"
