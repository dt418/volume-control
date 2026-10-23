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
# Flags:   --quiet, --prepared-dir <dir>, --parent-pid <pid>, --restart
# Env:
#   VOLUMECTL_VERSION       exact release tag (vX.Y.Z); default: latest stable
#   VOLUMECTL_INSTALL_ROOT  user prefix; default: "$HOME/.local"
#                           (installs $prefix/bin/volumectl and
#                            $prefix/share/doc/volumecontrol/README.md)
#   VOLUMECTL_RELEASE_BASE  release base URL; default: the GitHub releases URL
#                           (a file:// path is accepted, which the test suite
#                           uses with a local fixture release)
#   VOLUMECTL_LANG          en | vi; default: from LANG/LC_ALL
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
      no.downgrade)    template="phiên bản đã cài %s mới hơn %s; bỏ qua hạ cấp" ;;
      path.hint)       template="Thêm '%s' vào PATH nếu lệnh volumectl chưa có sẵn." ;;
      progress.downloading) template="[1/3] Đang tải %s..." ;;
      progress.verifying)   template="[2/3] Xác minh mã kiểm tra SHA-256..." ;;
      progress.installing)  template="[3/3] Đang cài đặt vào %s..." ;;
      component.updated)    template="Đã cập nhật thành phần: %s" ;;
      component.identical)  template="Thành phần %s không đổi (trùng khớp)" ;;
      lock.timeout)         template="không thể lấy khóa tại %s trong 30 giây" ;;
      stop.timeout)         template="không thể dừng tiến trình trong 15 giây" ;;
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
      no.downgrade)    template="installed version %s is newer than %s; skipping downgrade" ;;
      path.hint)       template="Add '%s' to PATH if the volumectl command is not found yet." ;;
      progress.downloading) template="[1/3] Downloading %s..." ;;
      progress.verifying)   template="[2/3] Verifying SHA-256 checksum..." ;;
      progress.installing)  template="[3/3] Installing into %s..." ;;
      component.updated)    template="Updated component: %s" ;;
      component.identical)  template="Component %s unchanged (matches current checksum)" ;;
      lock.timeout)         template="could not acquire lock at %s within 30s" ;;
      stop.timeout)         template="could not terminate process within 15s" ;;
      *)               template="$id" ;;
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
INSTALL_ROOT="${VOLUMECTL_INSTALL_ROOT:-$HOME/.local}"
TARGET_BIN="$INSTALL_ROOT/bin/$BIN_NAME"
DOC_DIR="$INSTALL_ROOT/share/doc/volumecontrol"
TARGET_DOC="$DOC_DIR/README.md"
RECEIPT_FILE="$DOC_DIR/install-receipt.txt"
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

# Offline uninstall must run before resolving release tag
if [[ "$ACTION" == uninstall ]]; then
  if [[ ! -e "$TARGET_BIN" && ! -e "$TARGET_DOC" && ! -e "$RECEIPT_FILE" ]]; then
    say "$(msg not.installed "$INSTALL_ROOT")"
    exit 0
  fi

  acquire_lock
  trap release_lock EXIT

  # Stop any running instance
  if [[ -f "$TARGET_BIN" ]]; then
    target_canon="$(readlink -f "$TARGET_BIN" 2>/dev/null || printf '%s' "$TARGET_BIN")"
    for p in /proc/[0-9]*/exe; do
      if [[ -L "$p" ]]; then
        exe_canon="$(readlink -f "$p" 2>/dev/null || true)"
        if [[ "$exe_canon" == "$target_canon" ]]; then
          pid="$(basename "$(dirname "$p")")"
          kill -TERM "$pid" 2>/dev/null || true
        fi
      fi
    done
  fi

  rm -f "$TARGET_BIN" "$TARGET_DOC" "$RECEIPT_FILE"
  rmdir "$DOC_DIR" 2>/dev/null || true
  rmdir "$INSTALL_ROOT/share/doc" 2>/dev/null || true
  rmdir "$INSTALL_ROOT/bin" 2>/dev/null || true
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
  mkdir -p "$DOC_DIR"
  printf 'volumecontrol-install-v1\n%s\n%s\n' "$1" "$2" > "$RECEIPT_FILE"
}

read_receipt_tag() {
  if [[ -f "$RECEIPT_FILE" ]]; then
    head -n 2 "$RECEIPT_FILE" | tail -n 1
  else
    printf ''
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

  tar_pkg="$(find "$PREPARED_DIR" -maxdepth 1 -name 'volumecontrol-*-linux-amd64.tar.gz' | head -n 1)"
  [[ -n "$tar_pkg" && -f "$tar_pkg" ]] || die "asset.missing" "tarball" "prepared"

  pkg_base="$(basename "$tar_pkg")"
  verify_against_manifest "$tar_pkg" "$pkg_base" "$manifest"

  stage_dir="$INSTALL_ROOT/.volumecontrol.stage.$$"
  mkdir -p "$stage_dir"
  tar xzf "$tar_pkg" -C "$stage_dir" "$BIN_NAME" README.md 2>/dev/null || die "asset.missing" "$BIN_NAME" "tar"

  mkdir -p "$INSTALL_ROOT/bin" "$DOC_DIR"
  install -m 0755 "$stage_dir/$BIN_NAME" "$TARGET_BIN"
  install -m 0644 "$stage_dir/README.md" "$TARGET_DOC"
  bin_hash="$(sha256_of "$TARGET_BIN")"
  pkg_ver="${pkg_base#volumecontrol-}"
  pkg_ver="${pkg_ver%-linux-amd64.tar.gz}"
  write_receipt "v$pkg_ver" "$bin_hash"
  rm -rf "$stage_dir"
  printf '0\n' > "$PREPARED_DIR/result.exit-code.tmp"
  mv -f "$PREPARED_DIR/result.exit-code.tmp" "$PREPARED_DIR/result.exit-code"

  if [[ $RESTART -eq 1 && -x "$TARGET_BIN" ]]; then
    nohup "$TARGET_BIN" --background >/dev/null 2>&1 &
  fi
  say "$(msg updated "prepared" "$TARGET_BIN")"
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
tar_name="volumecontrol-${version}-linux-amd64.tar.gz"
asset_dir="$RELEASE_BASE/download/$tag"
[[ "$RELEASE_BASE" == file://* ]] && asset_dir="$RELEASE_BASE/$tag"

acquire_lock
trap release_lock EXIT

# Check existing installation for install action
if [[ "$ACTION" == install && -f "$TARGET_BIN" ]]; then
  installed_tag="$(read_receipt_tag)"
  receipt_hash="$(head -n 3 "$RECEIPT_FILE" 2>/dev/null | tail -n 1)"
  current_hash="$(sha256_of "$TARGET_BIN")"
  if [[ "$installed_tag" == "$tag" && "$receipt_hash" == "$current_hash" && -f "$TARGET_DOC" ]]; then
    say "$(msg component.identical "$BIN_NAME")"
    exit 0
  fi
fi

# Stop running instance if present
if [[ -f "$TARGET_BIN" ]]; then
  target_canon="$(readlink -f "$TARGET_BIN" 2>/dev/null || printf '%s' "$TARGET_BIN")"
  for p in /proc/[0-9]*/exe; do
    if [[ -L "$p" ]]; then
      exe_canon="$(readlink -f "$p" 2>/dev/null || true)"
      if [[ "$exe_canon" == "$target_canon" ]]; then
        pid="$(basename "$(dirname "$p")")"
        kill -TERM "$pid" 2>/dev/null || true
      fi
    fi
  done
fi

tmp="$(mktemp -d)"
defer_rm() { rm -rf "$tmp"; release_lock; }
trap defer_rm EXIT

say "$(msg progress.downloading "$tar_name")"
fetch "$asset_dir/$tar_name" "$tmp/$tar_name"
fetch "$asset_dir/SHA256SUMS.txt" "$tmp/SHA256SUMS.txt"
say "$(msg progress.verifying)"
verify_against_manifest "$tmp/$tar_name" "$tar_name" "$tmp/SHA256SUMS.txt"

tar xzf "$tmp/$tar_name" -C "$tmp" "$BIN_NAME" README.md 2>/dev/null \
  || die "asset.missing" "$BIN_NAME" "$tag"
[[ -f "$tmp/$BIN_NAME" ]] || die "asset.missing" "$BIN_NAME" "$tag"

say "$(msg progress.installing "$INSTALL_ROOT")"
mkdir -p "$INSTALL_ROOT/bin" "$DOC_DIR"

# Transactional install / update
backup_bin="$TARGET_BIN.bak.$$"
backup_doc="$TARGET_DOC.bak.$$"
[[ -f "$TARGET_BIN" ]] && cp -f "$TARGET_BIN" "$backup_bin" 2>/dev/null || true
[[ -f "$TARGET_DOC" ]] && cp -f "$TARGET_DOC" "$backup_doc" 2>/dev/null || true

rollback() {
  [[ -f "$backup_bin" ]] && mv -f "$backup_bin" "$TARGET_BIN" 2>/dev/null || true
  [[ -f "$backup_doc" ]] && mv -f "$backup_doc" "$TARGET_DOC" 2>/dev/null || true
  rm -f "$backup_bin" "$backup_doc" 2>/dev/null || true
}

install -m 0755 "$tmp/$BIN_NAME" "$TARGET_BIN" || { rollback; die "download.failed" "install binary failed"; }
install -m 0644 "$tmp/README.md" "$TARGET_DOC" || { rollback; die "download.failed" "install doc failed"; }

bin_sha="$(sha256_of "$TARGET_BIN")"
write_receipt "$tag" "$bin_sha"

rm -f "$backup_bin" "$backup_doc" 2>/dev/null || true

if [[ "$ACTION" == update ]]; then
  say "$(msg updated "$version" "$TARGET_BIN")"
else
  say "$(msg installed "$version" "$TARGET_BIN")"
  say "$(msg path.hint "$INSTALL_ROOT/bin")"
fi
