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
# Flags:   --quiet, --prepared-dir <dir>, --parent-pid <pid>, --restart
# Env:
#   VOLUMECTL_VERSION       exact release tag (vX.Y.Z); default: latest stable
#   VOLUMECTL_INSTALL_ROOT  destination root; default: "$HOME/Applications"
#   VOLUMECTL_RELEASE_BASE  release base URL; default: the GitHub releases URL
#                           (a file:// path is accepted, which the test suite
#                           uses with a local fixture release)
#   VOLUMECTL_LANG          en | vi; default: from LANG/LC_ALL
set -euo pipefail

RELEASE_BASE_DEFAULT="https://github.com/dt418/volume-control/releases"
RELEASES_API="https://api.github.com/repos/dt418/volume-control/releases/latest"
APP_NAME="VolumeControl.app"

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
      progress.downloading) template="[1/3] Đang tải %s..." ;;
      progress.verifying)   template="[2/3] Xác minh mã kiểm tra SHA-256..." ;;
      progress.installing)  template="[3/3] Đang cài đặt vào %s..." ;;
      component.updated)    template="Đã cập nhật thành phần: %s" ;;
      component.identical)  template="Thành phần %s không đổi (trùng khớp)" ;;
      lock.timeout)         template="không thể lấy khóa tại %s trong 30 giây" ;;
      stop.timeout)         template="không thể dừng tiến trình trong 15 giây" ;;
      arch.incompatible)    template="kiến trúc gói cài đặt %s không tương thích với máy" ;;
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
      progress.downloading) template="[1/3] Downloading %s..." ;;
      progress.verifying)   template="[2/3] Verifying SHA-256 checksum..." ;;
      progress.installing)  template="[3/3] Installing into %s..." ;;
      component.updated)    template="Updated component: %s" ;;
      component.identical)  template="Component %s unchanged (matches current checksum)" ;;
      lock.timeout)         template="could not acquire lock at %s within 30s" ;;
      stop.timeout)         template="could not terminate process within 15s" ;;
      arch.incompatible)    template="package architecture %s is not compatible with this system" ;;
      *)                    template="$id" ;;
    esac
  fi
  # shellcheck disable=SC2059
  printf "$template" "$@"
}

say() { [[ "${QUIET:-0}" == 1 ]] || printf '%s\n' "$*"; }
die() { msg "$@" >&2; printf '\n' >&2; exit 1; }

ACTION="${1:-install}"
shift || true
QUIET=0
PREPARED_DIR=""
PARENT_PID=0
RESTART=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --quiet) QUIET=1; shift ;;
    --prepared-dir) PREPARED_DIR="$2"; shift 2 ;;
    --parent-pid) PARENT_PID="$2"; shift 2 ;;
    --restart) RESTART=1; shift ;;
    "") shift ;;
    *) die "usage.unknown" "$1" ;;
  esac
done

case "$ACTION" in install|update|uninstall) ;; *) die "version.invalid" "$ACTION" ;; esac

RELEASE_BASE="${VOLUMECTL_RELEASE_BASE:-$RELEASE_BASE_DEFAULT}"
INSTALL_ROOT="${VOLUMECTL_INSTALL_ROOT:-$HOME/Applications}"
TARGET_APP="$INSTALL_ROOT/$APP_NAME"
RECEIPT_FILE="$INSTALL_ROOT/.volumecontrol-install-receipt"
LOCK_DIR="$INSTALL_ROOT/.volumecontrol.lock"

acquire_lock() {
  mkdir -p "$INSTALL_ROOT"
  local count=0
  while ! mkdir "$LOCK_DIR" 2>/dev/null; do
    if [[ -f "$LOCK_DIR/pid" ]]; then
      local lock_pid
      lock_pid="$(cat "$LOCK_DIR/pid" 2>/dev/null || true)"
      if [[ -n "$lock_pid" ]] && ! kill -0 "$lock_pid" 2>/dev/null; then
        rm -rf "$LOCK_DIR" 2>/dev/null || true
        continue
      fi
    fi
    count=$((count + 1))
    if [[ $count -ge 150 ]]; then
      die "lock.timeout" "$LOCK_DIR"
    fi
    sleep 0.2
  done
  printf '%s' "$$" > "$LOCK_DIR/pid"
}

release_lock() {
  rm -rf "$LOCK_DIR" 2>/dev/null || true
}

# Stop running instance targeting TARGET_APP
stop_running_app() {
  if [[ ! -d "$TARGET_APP" ]]; then return 0; fi
  if command -v lsof >/dev/null 2>&1; then
    local pids
    pids="$(lsof -t "$TARGET_APP" 2>/dev/null || true)"
    if [[ -n "$pids" ]]; then
      for pid in $pids; do
        kill -TERM "$pid" 2>/dev/null || true
      done
      local count=0
      while lsof -t "$TARGET_APP" >/dev/null 2>&1; do
        sleep 0.1
        count=$((count + 1))
        if [[ $count -ge 150 ]]; then
          die "stop.timeout"
        fi
      done
    fi
  fi
}

# Offline uninstall before resolving release tag
if [[ "$ACTION" == uninstall ]]; then
  if [[ ! -e "$TARGET_APP" && ! -e "$RECEIPT_FILE" ]]; then
    say "$(msg not.installed "$INSTALL_ROOT")"
    exit 0
  fi

  acquire_lock
  trap release_lock EXIT

  stop_running_app
  rm -rf "$TARGET_APP" "$RECEIPT_FILE"
  say "$(msg removed "$INSTALL_ROOT")"
  exit 0
fi

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

write_receipt() { # tag sha256
  printf 'volumecontrol-install-v1\n%s\n%s\n' "$1" "$2" > "$RECEIPT_FILE"
}

validate_staged_app() { # staged_app
  local app="$1"
  local exe="$app/Contents/MacOS/volumectl"
  [[ -f "$exe" ]] || exe="$app/Contents/MacOS/VolumeControl"
  [[ -f "$exe" ]] || die "asset.missing" "executable in bundle" "macOS"

  # Validate architecture via lipo if available
  if command -v lipo >/dev/null 2>&1; then
    local archs
    archs="$(lipo -archs "$exe" 2>/dev/null || true)"
    if [[ -n "$archs" ]]; then
      local host_arch
      host_arch="$(uname -m)"
      # macOS arm64 vs x86_64
      if [[ "$host_arch" == "arm64" && "$archs" != *"arm64"* ]]; then
        die "arch.incompatible" "$archs"
      fi
    fi
  fi
}

# Handle offline prepared mode
if [[ -n "$PREPARED_DIR" ]]; then
  [[ -d "$PREPARED_DIR" ]] || die "download.failed" "prepared directory missing"
  acquire_lock
  trap release_lock EXIT

  printf 'ready\n' > "$PREPARED_DIR/handoff.ready"

  if [[ $PARENT_PID -gt 0 ]]; then
    count=0
    while kill -0 "$PARENT_PID" 2>/dev/null; do
      sleep 0.1
      count=$((count + 1))
      [[ $count -ge 150 ]] && break
    done
  fi

  manifest="$PREPARED_DIR/SHA256SUMS.txt"
  [[ -f "$manifest" ]] || die "sha.missing" "SHA256SUMS.txt"

  zip_pkg="$(find "$PREPARED_DIR" -maxdepth 1 -name 'volumecontrol-*-macos.zip' | head -n 1)"
  [[ -n "$zip_pkg" && -f "$zip_pkg" ]] || die "asset.missing" "zip" "prepared"

  pkg_base="$(basename "$zip_pkg")"
  verify_against_manifest "$zip_pkg" "$pkg_base" "$manifest"

  stage_dir="$INSTALL_ROOT/.volumecontrol.stage.$$"
  mkdir -p "$stage_dir"
  unzip -q -o "$zip_pkg" -d "$stage_dir" || die "asset.missing" "unzip failed" "zip"
  validate_staged_app "$stage_dir/$APP_NAME"

  # Atomic replacement
  backup_dir="$INSTALL_ROOT/.volumecontrol.bak.$$"
  [[ -d "$TARGET_APP" ]] && mv -f "$TARGET_APP" "$backup_dir" 2>/dev/null || true
  mv -f "$stage_dir/$APP_NAME" "$TARGET_APP" || {
    [[ -d "$backup_dir" ]] && mv -f "$backup_dir" "$TARGET_APP" 2>/dev/null || true
    rm -rf "$stage_dir" "$backup_dir"
    die "download.failed" "atomic swap failed"
  }
  rm -rf "$backup_dir" "$stage_dir"

  app_hash="$(sha256_of "$TARGET_APP/Contents/Info.plist" 2>/dev/null || true)"
  pkg_ver="${pkg_base#volumecontrol-}"
  pkg_ver="${pkg_ver%-macos.zip}"
  write_receipt "v$pkg_ver" "$app_hash"
  printf '0\n' > "$PREPARED_DIR/result.exit-code.tmp"
  mv -f "$PREPARED_DIR/result.exit-code.tmp" "$PREPARED_DIR/result.exit-code"

  if [[ $RESTART -eq 1 ]]; then
    open -a "$TARGET_APP" --args --background 2>/dev/null || true
  fi
  say "$(msg updated "prepared" "$TARGET_APP")"
  exit 0
fi

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
  local tag api_body location
  api_body="$(curl -fsSL -H 'Accept: application/vnd.github+json' "$RELEASES_API" 2>/dev/null || true)"
  tag="$(printf '%s' "$api_body" \
    | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)"
  if [[ -z "$tag" ]]; then
    location="$(curl -fsSIL -o /dev/null -w '%{url_effective}' \
      "$RELEASE_BASE_DEFAULT/latest" 2>/dev/null || true)"
    case "$location" in */releases/tag/v[0-9]*.[0-9]*.[0-9]*) tag="${location##*/tag/}" ;; esac
  fi
  [[ -n "$tag" ]] || die "download.failed" "could not resolve the latest release (api.github.com may be rate-limited; retry later or pin VOLUMECTL_VERSION=vX.Y.Z)"
  printf '%s' "$tag"
}

tag="$(resolve_tag)"
version="${tag#v}"
zip_name="volumecontrol-${version}-macos.zip"
asset_dir="$RELEASE_BASE/download/$tag"
[[ "$RELEASE_BASE" == file://* ]] && asset_dir="$RELEASE_BASE/$tag"

acquire_lock
trap release_lock EXIT

# Check intact same-version installation for install action
if [[ "$ACTION" == install && -d "$TARGET_APP" && -f "$RECEIPT_FILE" ]]; then
  receipt_tag="$(head -n 2 "$RECEIPT_FILE" | tail -n 1)"
  receipt_hash="$(head -n 3 "$RECEIPT_FILE" 2>/dev/null | tail -n 1)"
  current_hash="$(sha256_of "$TARGET_APP/Contents/Info.plist" 2>/dev/null || true)"
  if [[ "$receipt_tag" == "$tag" && "$receipt_hash" == "$current_hash" ]]; then
    say "$(msg component.identical "$APP_NAME")"
    exit 0
  fi
fi

stop_running_app

tmp="$(mktemp -d)"
defer_rm() { rm -rf "$tmp"; release_lock; }
trap defer_rm EXIT

say "$(msg progress.downloading "$zip_name")"
fetch "$asset_dir/$zip_name" "$tmp/$zip_name"
fetch "$asset_dir/SHA256SUMS.txt" "$tmp/SHA256SUMS.txt"
say "$(msg progress.verifying)"
verify_against_manifest "$tmp/$zip_name" "$zip_name" "$tmp/SHA256SUMS.txt"

unzip -q -o "$tmp/$zip_name" -d "$tmp" || die "asset.missing" "$APP_NAME" "$tag"
[[ -d "$tmp/$APP_NAME" ]] || die "asset.missing" "$APP_NAME" "$tag"
validate_staged_app "$tmp/$APP_NAME"

say "$(msg progress.installing "$INSTALL_ROOT")"
mkdir -p "$INSTALL_ROOT"

backup_dir="$INSTALL_ROOT/.volumecontrol.bak.$$"
[[ -d "$TARGET_APP" ]] && mv -f "$TARGET_APP" "$backup_dir" 2>/dev/null || true

if ! mv -f "$tmp/$APP_NAME" "$TARGET_APP"; then
  [[ -d "$backup_dir" ]] && mv -f "$backup_dir" "$TARGET_APP" 2>/dev/null || true
  rm -rf "$backup_dir"
  die "download.failed" "install failed"
fi
rm -rf "$backup_dir"

plist_hash="$(sha256_of "$TARGET_APP/Contents/Info.plist" 2>/dev/null || true)"
write_receipt "$tag" "$plist_hash"

if [[ "$ACTION" == update ]]; then
  say "$(msg updated "$version" "$TARGET_APP")"
else
  say "$(msg installed "$version" "$TARGET_APP")"
  say "$(msg gatekeeper "$TARGET_APP")"
fi
