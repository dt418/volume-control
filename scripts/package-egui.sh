#!/usr/bin/env bash
# Build and package the native egui host on Linux or macOS.
#
#   scripts/package-egui.sh [version] [output-root]
#
# Linux:  VolumeControl-<ver>-linux-x64.tar.gz (binary + .desktop + icon)
# macOS:  VolumeControl-<ver>-macos-<arch>.zip (.app bundle)
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
    tar -C "$stage" -czf "$out_root/VolumeControl-$version-linux-$(uname -m).tar.gz" .
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
    if command -v codesign >/dev/null 2>&1; then
      codesign --force --deep --sign - "$app" || echo "ad-hoc signing failed" >&2
    fi
    (cd "$stage" && zip -qry "../VolumeControl-$version-macos-$arch.zip" "VolumeControl.app")
    ;;
  *)
    echo "unsupported platform: $(uname -s)" >&2
    exit 1
    ;;
esac

(
  cd "$out_root"
  rm -f SHA256SUMS.txt
  for f in VolumeControl-*.tar.gz VolumeControl-*.zip; do
    [[ -e "$f" ]] || continue
    if command -v sha256sum >/dev/null 2>&1; then
      sha256sum "$f" >> SHA256SUMS.txt
    else
      shasum -a 256 "$f" >> SHA256SUMS.txt
    fi
  done
)

echo "artifacts in $out_root"
ls -l "$out_root"
