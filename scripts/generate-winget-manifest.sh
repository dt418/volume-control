#!/usr/bin/env bash
#
# generate-winget-manifest.sh - emit a complete winget-pkgs manifest trio
# (version, installer, default locale) for a released NSIS installer.
#
# The output is submission-ready for microsoft/winget-pkgs:
#   winget/<id>/<version>/<id>.yaml
#   winget/<id>/<version>/<id>.installer.yaml
#   winget/<id>/<version>/<id>.locale.en-US.yaml
#
# Usage:
#   scripts/generate-winget-manifest.sh <vX.Y.Z> <installer.exe> [outdir]
#
# The installer file name must be VolumeControl_<version>_x64-setup.exe so the
# generated InstallerUrl matches the GitHub release asset layout the
# bootstrap resolves. InstallerSha256 is computed from the real bytes -
# never templated, never faked.
set -euo pipefail

package_id="dt418.VolumeControl"
repo_url="https://github.com/dt418/volume-control"
manifest_version="1.12.0"

usage() {
  echo "usage: $0 <vX.Y.Z> <installer.exe> [outdir]" >&2
  exit 2
}

[ "$#" -ge 2 ] && [ "$#" -le 3 ] || usage
tag="$1"
installer="$2"
outdir="${3:-winget}"

case "$tag" in
  v[0-9]*.[0-9]*.[0-9]*) ;;
  *) echo "version must look like vMAJOR.MINOR.PATCH: $tag" >&2; exit 1 ;;
esac
version="${tag#v}"

if ! printf '%s' "$version" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  echo "version must look like vMAJOR.MINOR.PATCH: $tag" >&2
  exit 1
fi

expected_name="VolumeControl_${version}_x64-setup.exe"
actual_name="$(basename "$installer")"
if [ "$actual_name" != "$expected_name" ]; then
  echo "installer must be named $expected_name, got: $actual_name" >&2
  exit 1
fi
[ -s "$installer" ] || { echo "installer missing or empty: $installer" >&2; exit 1; }

if command -v sha256sum >/dev/null 2>&1; then
  sha="$(sha256sum "$installer" | awk '{print $1}')"
else
  command -v shasum >/dev/null 2>&1 || { echo "sha256sum or shasum required" >&2; exit 1; }
  sha="$(shasum -a 256 "$installer" | awk '{print $1}')"
fi
# GNU tools prefix a backslash when the file name contains one.
sha="${sha#\\}"
sha="$(printf '%s' "$sha" | tr 'a-f' 'A-F')"
printf '%s' "$sha" | grep -Eq '^[0-9A-F]{64}$' || { echo "could not hash installer" >&2; exit 1; }

dest="$outdir/$package_id/$version"
mkdir -p "$dest"

cat > "$dest/$package_id.yaml" <<EOF
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.version.1.12.0.schema.json

PackageIdentifier: $package_id
PackageVersion: $version
DefaultLocale: en-US
ManifestType: version
ManifestVersion: $manifest_version
EOF

cat > "$dest/$package_id.installer.yaml" <<EOF
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.installer.1.12.0.schema.json

PackageIdentifier: $package_id
PackageVersion: $version
MinimumOSVersion: 10.0.17763.0
InstallerType: nullsoft
Scope: user
InstallModes:
  - interactive
  - silent
  - silentWithProgress
UpgradeBehavior: install
Installers:
  - Architecture: x64
    InstallerUrl: $repo_url/releases/download/$tag/$expected_name
    InstallerSha256: $sha
ManifestType: installer
ManifestVersion: $manifest_version
EOF

cat > "$dest/$package_id.locale.en-US.yaml" <<EOF
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.defaultLocale.1.12.0.schema.json

PackageIdentifier: $package_id
PackageVersion: $version
PackageLocale: en-US
Publisher: dt418
PackageName: VolumeControl
PackageUrl: $repo_url
License: MIT
LicenseUrl: $repo_url/blob/main/LICENSE
ShortDescription: Native-first volume controller with global hotkeys, system tray, volume overlay, and Mixer/Settings/Help surfaces.
Moniker: volumecontrol
ManifestType: defaultLocale
ManifestVersion: $manifest_version
EOF

echo "wrote winget manifests for $package_id $version:"
echo "  $dest/$package_id.yaml"
echo "  $dest/$package_id.installer.yaml"
echo "  $dest/$package_id.locale.en-US.yaml"
echo "InstallerSha256: $sha"
