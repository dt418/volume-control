#!/usr/bin/env bash
# Run a cargo command in WSL against the repo on the Windows drive.
# Output goes to stdout/stderr (redirected by the caller to a Windows file).
set -e
export PATH="$HOME/.cargo/bin:$PATH"
cd /mnt/d/Projects/volume-control
echo "== cargo $* =="
cargo "$@"
echo "CARGO_EXIT_OK"
