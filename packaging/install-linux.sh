#!/usr/bin/env bash
#
# install-linux.sh - install, update, or uninstall the VolumeControl Linux
# CLI host from the published GitHub release, with SHA-256 verification
# against the release SHA256SUMS.txt before anything is unpacked.
#
# Usage:
#   curl -fsSL https://github.com/dt418/volume-control/releases/latest/download/install-linux.sh | bash -s -- install
#
# Actions: install | update | uninstall   (default: install)
# Flags:   --quiet
# Env:
#   VOLUMECTL_VERSION       exact release tag (vX.Y.Z); default: latest stable
#   VOLUMECTL_INSTALL_ROOT  user prefix; default: "$HOME/.local"
#                           (installs $prefix/bin/volumectl and
#                            $prefix/share/doc/volumecontrol/README.md)
#   VOLUMECTL_RELEASE_BASE  release base URL; default: the GitHub releases URL
#                           (a file:// path is accepted, which the test suite
#                           uses with a local fixture release)
#   VOLUMECTL_LANG          en | vi; default: from LANG/LC_ALL
#
# The binary links the system GTK/WebKitGTK stack only through the bundled
# Tauri host; the CLI path (volume get/set/mute) needs no extra libraries.
set -euo pipefail

RELEASE_BASE_DEFAULT="https://github.com/dt418/volume-control/releases"
RELEASES_API="https://api.github.com/repos/dt418/volume-control/releases/latest"
BIN_NAME="volumectl"

msg() {
  local id="$1"; shift
  local lang="${VOLUMECTL_LANG:-}"
  if [[ -z "$lang" ]]; then
    local locale="${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}"
    case "$locale" in vi*) lang=vi ;; *) lang=en ;; esac
  fi
  case "$lang" in vi) lang=vi ;; *) lang=en ;; esac
  local template
  if [[ "$lang" == vi ]]; then
    case "$id" in
      version.invalid) template="phiên bản phải có dạng vX.Y.Z: %s" ;;
      usage.unknown)   template="tham số không hợp lệ: %s" ;;
      asset.missing)   template="không tìm thấy %1\$s trong bản phát hành %2\$s" ;;
      sha.mismatch)    template="SHA-256 không khớp cho %s" ;;
      sha.missing)     template="SHA256SUMS.txt thiếu mục chính tắc cho %s" ;;
      download.failed) template="tải thất bại: %s" ;;
      install.refused) template="đã có bản cài tại %s — dùng 'update' thay vì 'install'" ;;
      update.missing)  template="chưa có bản cài tại %s — dùng 'install' trước" ;;
      installed)       template="Đã cài VolumeControl %s vào %s" ;;
      updated)         template="Đã cập nhật VolumeControl %s tại %s" ;;
      removed)         template="Đã gỡ VolumeControl khỏi %s" ;;
      not.installed)   template="Không có bản cài nào tại %s" ;;
      path.hint)       template="Thêm '%s' vào PATH nếu lệnh volumectl chưa có sẵn." ;;
      *)               template="$id" ;;
    esac
  else
    case "$id" in
      version.invalid) template="version must look like vX.Y.Z: %s" ;;
      usage.unknown)   template="unknown argument: %s" ;;
      asset.missing)   template="release %2\$s does not contain %1\$s" ;;
      sha.mismatch)    template="SHA-256 mismatch for %s" ;;
      sha.missing)     template="SHA256SUMS.txt has no canonical entry for %s" ;;
      download.failed) template="download failed: %s" ;;
      install.refused) template="already installed at %s — use 'update' instead of 'install'" ;;
      update.missing)  template="nothing installed at %s — run 'install' first" ;;
      installed)       template="Installed VolumeControl %s into %s" ;;
      updated)         template="Updated VolumeControl %s at %s" ;;
      removed)         template="Removed VolumeControl from %s" ;;
      not.installed)   template="nothing installed at %s" ;;
      path.hint)       template="Add '%s' to PATH if the volumectl command is not found yet." ;;
      *)               template="$id" ;;
    esac
  fi
  # shellcheck disable=SC2059  # the template is our controlled format string
  printf "$template" "$@"
}

say() { [[ "${QUIET:-0}" == 1 ]] || printf '%s\n' "$*"; }
die() { msg "$@" >&2; printf '\n' >&2; exit 1; }

ACTION="${1:-install}"
QUIET=0
case "${2:-}" in
  --quiet) QUIET=1 ;;
  "") ;;
  *) die "usage.unknown" "${2}" ;;
esac
case "$ACTION" in install|update|uninstall) ;; *) die "version.invalid" "$ACTION" ;; esac

RELEASE_BASE="${VOLUMECTL_RELEASE_BASE:-$RELEASE_BASE_DEFAULT}"
INSTALL_ROOT="${VOLUMECTL_INSTALL_ROOT:-$HOME/.local}"
TARGET_BIN="$INSTALL_ROOT/bin/$BIN_NAME"
TARGET_DOC="$INSTALL_ROOT/share/doc/volumecontrol/README.md"

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

fetch() {
  command -v curl >/dev/null 2>&1 || die "download.failed" "curl is required"
  if ! curl -fsSL "$1" -o "$2"; then die "download.failed" "$1"; fi
}

verify_against_manifest() { # file basename manifest_path
  local file="$1" basename="$2" manifest="$3"
  local expected actual
  expected="$(awk -v name="$basename" '$0 ~ /^[0-9A-Fa-f]{64}  [^ \/\\]+$/ && substr($0, 67) == name { count++; digest = substr($0, 1, 64) } END { if (count == 1) print digest }' "$manifest")"
  [[ -n "$expected" ]] || die "sha.missing" "$basename"
  actual="$(sha256_of "$file")"
  [[ "$(printf %s "$actual" | tr "[:upper:]" "[:lower:]")" == "$(printf %s "$expected" | tr "[:upper:]" "[:lower:]")" ]] \
    || die "sha.mismatch" "$basename"
}

resolve_tag() {
  if [[ -n "${VOLUMECTL_VERSION:-}" ]]; then
    case "$VOLUMECTL_VERSION" in v[0-9]*.[0-9]*.[0-9]*) ;; *) die "version.invalid" "$VOLUMECTL_VERSION" ;; esac
    printf '%s' "$VOLUMECTL_VERSION"
    return
  fi
  if [[ "$RELEASE_BASE" == file://* ]]; then
    local tag
    tag="$(basename "$(dirname "$(find "${RELEASE_BASE#file://}" -type f -name SHA256SUMS.txt | head -n 1)")")"
    [[ -n "$tag" ]] || die "download.failed" "no fixture release under $RELEASE_BASE"
    printf '%s' "$tag"
    return
  fi
  # Unauthenticated api.github.com calls share 60 requests/hour per IP, so a
  # shared NAT can answer 403/429. Try the JSON API first, then fall back to
  # the HTML redirect target of releases/latest (no API quota), and only then
  # fail with a message that names both the limit and the VOLUMECTL_VERSION
  # escape hatch.
  local tag api_body location
  api_body="$(curl -fsSL -H 'Accept: application/vnd.github+json' "$RELEASES_API" 2>/dev/null || true)"
  tag="$(printf '%s' "$api_body" \
    | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
  if [[ -z "$tag" ]]; then
    location="$(curl -fsSIL -o /dev/null -w '%{url_effective}' \
      "$RELEASE_BASE_DEFAULT/latest" 2>/dev/null || true)"
    case "$location" in */releases/tag/v[0-9]*.[0-9]*.[0-9]*) tag="${location##*/tag/}" ;; esac
  fi
  # A 403 body still parses to an empty tag and lands here.
  [[ -n "$tag" ]] || die "download.failed" "could not resolve the latest release (api.github.com may be rate-limited; retry later or pin VOLUMECTL_VERSION=vX.Y.Z)"
  printf '%s' "$tag"
}

tag="$(resolve_tag)"
version="${tag#v}"
tar_name="volumecontrol-${version}-linux-amd64.tar.gz"
asset_dir="$RELEASE_BASE/download/$tag"
[[ "$RELEASE_BASE" == file://* ]] && asset_dir="$RELEASE_BASE/$tag"

if [[ "$ACTION" == uninstall ]]; then
  if [[ -e "$TARGET_BIN" || -e "$TARGET_DOC" ]]; then
    rm -f "$TARGET_BIN" "$TARGET_DOC"
    rmdir "$(dirname "$TARGET_DOC")" 2>/dev/null || true
    rmdir "$INSTALL_ROOT/share/doc" 2>/dev/null || true
    say "$(msg removed "$INSTALL_ROOT")"
  else
    say "$(msg not.installed "$INSTALL_ROOT")"
  fi
  exit 0
fi

if [[ "$ACTION" == install && -e "$TARGET_BIN" ]]; then
  die "install.refused" "$TARGET_BIN"
fi
if [[ "$ACTION" == update && ! -e "$TARGET_BIN" ]]; then
  die "update.missing" "$TARGET_BIN"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
fetch "$asset_dir/$tar_name" "$tmp/$tar_name"
fetch "$asset_dir/SHA256SUMS.txt" "$tmp/SHA256SUMS.txt"
verify_against_manifest "$tmp/$tar_name" "$tar_name" "$tmp/SHA256SUMS.txt"

tar xzf "$tmp/$tar_name" -C "$tmp" "$BIN_NAME" README.md 2>/dev/null \
  || die "asset.missing" "$BIN_NAME" "$tag"
[[ -f "$tmp/$BIN_NAME" ]] || die "asset.missing" "$BIN_NAME" "$tag"

mkdir -p "$INSTALL_ROOT/bin" "$(dirname "$TARGET_DOC")"
install -m 0755 "$tmp/$BIN_NAME" "$TARGET_BIN"
install -m 0644 "$tmp/README.md" "$TARGET_DOC"

if [[ "$ACTION" == update ]]; then
  say "$(msg updated "$version" "$TARGET_BIN")"
else
  say "$(msg installed "$version" "$TARGET_BIN")"
  say "$(msg path.hint "$INSTALL_ROOT/bin")"
fi
