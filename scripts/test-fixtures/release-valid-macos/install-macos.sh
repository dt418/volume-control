#!/usr/bin/env bash
#
# install-macos.sh - install, update, or uninstall VolumeControl.app from the
# published GitHub release, with SHA-256 verification against the release
# SHA256SUMS.txt before anything is unpacked.
#
# Usage:
#   curl -fsSL https://github.com/dt418/volume-control/releases/latest/download/install-macos.sh | bash -s -- install
#
# Actions: install | update | uninstall   (default: install)
# Flags:   --quiet
# Env:
#   VOLUMECTL_VERSION       exact release tag (vX.Y.Z); default: latest stable
#   VOLUMECTL_INSTALL_ROOT  destination root; default: "$HOME/Applications"
#   VOLUMECTL_RELEASE_BASE  release base URL; default: the GitHub releases URL
#                           (a file:// path is accepted, which the test suite
#                           uses with a local fixture release)
#   VOLUMECTL_LANG          en | vi; default: from LANG/LC_ALL
#
# The release assets are ad-hoc signed, not notarized: the script prints the
# Gatekeeper instruction instead of silently stripping quarantine.
set -euo pipefail

RELEASE_BASE_DEFAULT="https://github.com/dt418/volume-control/releases"
RELEASES_API="https://api.github.com/repos/dt418/volume-control/releases/latest"
APP_NAME="VolumeControl.app"

# ── messages (EN/VI) ────────────────────────────────────────────────────────
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
      version.invalid)      template="phiên bản phải có dạng vX.Y.Z: %s" ;;
      usage.unknown)        template="tham số không hợp lệ: %s" ;;
      asset.missing)        template="không tìm thấy %1\$s trong bản phát hành %2\$s" ;;
      sha.mismatch)         template="SHA-256 không khớp cho %s" ;;
      sha.missing)          template="SHA256SUMS.txt thiếu mục chính tắc cho %s" ;;
      download.failed)      template="tải thất bại: %s" ;;
      install.refused)      template="đã có bản cài tại %s — dùng 'update' thay vì 'install'" ;;
      update.missing)       template="chưa có bản cài tại %s — dùng 'install' trước" ;;
      installed)            template="Đã cài VolumeControl %s vào %s" ;;
      updated)              template="Đã cập nhật VolumeControl %s tại %s" ;;
      removed)              template="Đã gỡ VolumeControl khỏi %s" ;;
      not.installed)        template="Không có bản cài nào tại %s" ;;
      gatekeeper)           template="Bản phát hành chỉ được ký ad-hoc: lần mở đầu tiên cần chuột phải → Open, hoặc chạy: xattr -dr com.apple.quarantine '%s'" ;;
      *)                    template="$id" ;;
    esac
  else
    case "$id" in
      version.invalid)      template="version must look like vX.Y.Z: %s" ;;
      usage.unknown)        template="unknown argument: %s" ;;
      asset.missing)        template="release %2\$s does not contain %1\$s" ;;
      sha.mismatch)         template="SHA-256 mismatch for %s" ;;
      sha.missing)          template="SHA256SUMS.txt has no canonical entry for %s" ;;
      download.failed)      template="download failed: %s" ;;
      install.refused)      template="already installed at %s — use 'update' instead of 'install'" ;;
      update.missing)       template="nothing installed at %s — run 'install' first" ;;
      installed)            template="Installed VolumeControl %s into %s" ;;
      updated)              template="Updated VolumeControl %s at %s" ;;
      removed)              template="Removed VolumeControl from %s" ;;
      not.installed)        template="nothing installed at %s" ;;
      gatekeeper)           template="release is ad-hoc signed: first launch needs right-click → Open, or run: xattr -dr com.apple.quarantine '%s'" ;;
      *)                    template="$id" ;;
    esac
  fi
  # shellcheck disable=SC2059  # the template is our controlled format string
  printf "$template" "$@"
}

say() { [[ "${QUIET:-0}" == 1 ]] || printf '%s\n' "$*"; }

die() { msg "$@" >&2; printf '\n' >&2; exit 1; }

# ── arguments ───────────────────────────────────────────────────────────────
ACTION="${1:-install}"
QUIET=0
case "${2:-}" in
  --quiet) QUIET=1 ;;
  "") ;;
  *) die "usage.unknown" "${2}" ;;
esac
case "$ACTION" in install|update|uninstall) ;; *) die "version.invalid" "$ACTION" ;; esac

RELEASE_BASE="${VOLUMECTL_RELEASE_BASE:-$RELEASE_BASE_DEFAULT}"
INSTALL_ROOT="${VOLUMECTL_INSTALL_ROOT:-$HOME/Applications}"
TARGET_APP="$INSTALL_ROOT/$APP_NAME"

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

fetch() { # url destination
  command -v curl >/dev/null 2>&1 || die "download.failed" "curl is required"
  if ! curl -fsSL "$1" -o "$2"; then die "download.failed" "$1"; fi
}

verify_against_manifest() { # file basename manifest_path
  local file="$1" basename="$2" manifest="$3"
  local expected actual
  expected="$(awk -v name="$basename" '$0 ~ /^[0-9A-Fa-f]{64}  [^ \/\\]+$/ && substr($0, 67) == name { count++; digest = substr($0, 1, 64) } END { if (count == 1) print digest }' "$manifest")"
  [[ -n "$expected" ]] || die "sha.missing" "$basename"
  actual="$(sha256_of "$file")"
  [[ "${actual,,}" == "${expected,,}" ]] || die "sha.mismatch" "$basename"
}

resolve_tag() {
  if [[ -n "${VOLUMECTL_VERSION:-}" ]]; then
    case "$VOLUMECTL_VERSION" in v[0-9]*.[0-9]*.[0-9]*) ;; *) die "version.invalid" "$VOLUMECTL_VERSION" ;; esac
    printf '%s' "$VOLUMECTL_VERSION"
    return
  fi
  if [[ "$RELEASE_BASE" == file://* ]]; then
    # Fixture layout: <base>/<tag>/ holds the release assets.
    local tag_dir tag
    tag_dir="$(dirname "$(find "${RELEASE_BASE#file://}" -maxdepth 2 -name SHA256SUMS.txt -print -quit)")"
    tag="$(basename "$tag_dir")"
    [[ -n "$tag" ]] || die "download.failed" "no fixture release under $RELEASE_BASE"
    printf '%s' "$tag"
    return
  fi
  local tag
  tag="$(curl -fsSL -H 'Accept: application/vnd.github+json' "$RELEASES_API" \
    | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
  [[ -n "$tag" ]] || die "download.failed" "could not resolve the latest release"
  printf '%s' "$tag"
}

tag="$(resolve_tag)"
version="${tag#v}"
zip_name="volumecontrol-${version}-macos.zip"
asset_dir="$RELEASE_BASE/download/$tag"
[[ "$RELEASE_BASE" == file://* ]] && asset_dir="$RELEASE_BASE/$tag"

if [[ "$ACTION" == uninstall ]]; then
  if [[ -d "$TARGET_APP" ]]; then
    rm -rf "$TARGET_APP"
    rmdir "$INSTALL_ROOT" 2>/dev/null || true
    say "$(msg removed "$TARGET_APP")"
  else
    say "$(msg not.installed "$TARGET_APP")"
  fi
  exit 0
fi

if [[ "$ACTION" == install && -d "$TARGET_APP" ]]; then
  die "install.refused" "$TARGET_APP"
fi
if [[ "$ACTION" == update && ! -d "$TARGET_APP" ]]; then
  die "update.missing" "$TARGET_APP"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
fetch "$asset_dir/$zip_name" "$tmp/$zip_name"
fetch "$asset_dir/SHA256SUMS.txt" "$tmp/SHA256SUMS.txt"
verify_against_manifest "$tmp/$zip_name" "$zip_name" "$tmp/SHA256SUMS.txt"

(unzip -q "$tmp/$zip_name" -d "$tmp/unpack") || die "download.failed" "unzip failed"
[[ -d "$tmp/unpack/$APP_NAME" ]] || die "asset.missing" "$APP_NAME" "$tag"

mkdir -p "$INSTALL_ROOT"
rm -rf "$TARGET_APP"
cp -R "$tmp/unpack/$APP_NAME" "$TARGET_APP"

if [[ "$ACTION" == update ]]; then
  say "$(msg updated "$version" "$TARGET_APP")"
else
  say "$(msg installed "$version" "$TARGET_APP")"
fi
say "$(msg gatekeeper "$TARGET_APP")"
