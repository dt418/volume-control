#!/usr/bin/env bash
# VolumeControl project bootstrap — install deps, verify baseline, print start cmd.
# Adapted from the learn-harness-engineering init.sh template.
set -euo pipefail

INSTALL_CMD="${INSTALL_CMD:-cargo build}"
VERIFY_CMD="${VERIFY_CMD:-cargo test}"
START_CMD="${START_CMD:-cargo run}"

echo "== VolumeControl init =="
echo "1/3 Installing/building: $INSTALL_CMD"
$INSTALL_CMD

echo "2/3 Verifying baseline: $VERIFY_CMD"
$VERIFY_CMD

echo "3/3 To run: $START_CMD"
echo "Done. Baseline is green."
