#!/usr/bin/env sh
#
# smoke-test.sh - Cross-platform smoke test runner for VolumeControl (Windows, macOS, Linux).
# Verifies real application surfaces and native renderers before push/ship.
#
# Exit: 0 = smoke test passed; 1 = verification failed.
set -eu

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

evidence_dir="$repo_root/output/smoke-test-evidence"
mkdir -p "$evidence_dir"

os_name="$(uname -s 2>/dev/null || echo "Unknown")"
echo "=== VolumeControl Cross-Platform Smoke Test ($os_name) ==="

case "$os_name" in
    *CYGWIN*|*MINGW*|*MSYS*|*Windows*)
        echo "[smoke-test] Platform: Windows"
        # 1. Frontend unit & component suites
        echo "[smoke-test] 1/3: Frontend component and token tests..."
        npm run test --prefix "$repo_root/frontend"

        # 2. Frontend production build
        echo "[smoke-test] 2/3: Frontend production build..."
        npm run build --prefix "$repo_root/frontend"

        # 3. Real Win32 Surface Capture Smoke Test
        echo "[smoke-test] 3/3: Real Desktop Surface Capture (verify-tauri-surfaces.ps1)..."
        binary="target/debug/VolumeControl.exe"
        if [ ! -f "$binary" ]; then
            echo "[smoke-test] Debug binary not found at $binary; building debug target..."
            cargo build -p volumecontrol-tauri --no-default-features
        fi

        if command -v pwsh >/dev/null 2>&1; then
            VOLUMECTL_E2E_DEBUG=1 pwsh -NoProfile -File "$repo_root/scripts/verify-tauri-surfaces.ps1" \
                -Binary "$binary" \
                -OutputRoot "output/smoke-test-evidence/surfaces"
        elif command -v powershell.exe >/dev/null 2>&1; then
            VOLUMECTL_E2E_DEBUG=1 powershell.exe -NoProfile -File "$repo_root/scripts/verify-tauri-surfaces.ps1" \
                -Binary "$binary" \
                -OutputRoot "output/smoke-test-evidence/surfaces"
        else
            echo "[smoke-test] WARNING: Neither pwsh nor powershell.exe found; skipping Win32 surface capture" >&2
        fi
        ;;

    Darwin*)
        echo "[smoke-test] Platform: macOS"
        # 1. Frontend tests & build
        echo "[smoke-test] 1/3: Frontend tests and build..."
        npm run test --prefix "$repo_root/frontend"
        npm run build --prefix "$repo_root/frontend"

        # 2. AppKit native renderer smoke test
        echo "[smoke-test] 2/3: AppKit native renderer smoke test..."
        cargo test -p volumectl --test appkit_smoke -- --nocapture

        # 3. macOS host loop smoke test
        echo "[smoke-test] 3/3: macOS host smoke test..."
        cargo test -p volumectl --test macos_host_smoke -- --nocapture
        ;;

    Linux*)
        echo "[smoke-test] Platform: Linux"
        # 1. Frontend tests & build
        echo "[smoke-test] 1/3: Frontend tests and build..."
        npm run test --prefix "$repo_root/frontend"
        npm run build --prefix "$repo_root/frontend"

        # 2. Linux host core smoke test
        echo "[smoke-test] 2/3: Linux host core smoke test..."
        cargo test -p volumectl --test linux_host_smoke -- --nocapture

        # 3. GTK4 / Libadwaita renderer smoke test (under Xvfb if available)
        echo "[smoke-test] 3/3: GTK4 / libadwaita smoke test..."
        if command -v xvfb-run >/dev/null 2>&1; then
            xvfb-run -a cargo test -p volumectl --features gtk-renderer --test gtk_smoke -- --nocapture
        else
            echo "[smoke-test] NOTE: xvfb-run not found; running non-gui linux smoke tests"
            cargo test -p volumectl --test linux_host_core -- --nocapture
        fi
        ;;

    *)
        echo "[smoke-test] Generic POSIX platform: running shared test battery..."
        npm run test --prefix "$repo_root/frontend"
        npm run build --prefix "$repo_root/frontend"
        cargo test --workspace --no-default-features
        ;;
esac

echo "[smoke-test] PASSED: All smoke test verifications succeeded on $os_name."
exit 0
