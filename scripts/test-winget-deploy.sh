#!/usr/bin/env bash
#
# test-winget-deploy.sh - unit & regression tests for deploy-winget
#
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"

echo "testing deploy-winget arguments..."
set +e
bash "$script_dir/deploy-winget.sh" >/dev/null 2>&1
code=$?
set -e
if [ "$code" -ne 2 ]; then
  echo "expected exit code 2 on missing arguments, got $code" >&2
  exit 1
fi

echo "testing deploy-winget invalid tag..."
set +e
bash "$script_dir/deploy-winget.sh" invalid-tag non-existent.exe >/dev/null 2>&1
code=$?
set -e
if [ "$code" -ne 1 ]; then
  echo "expected exit code 1 on invalid tag, got $code" >&2
  exit 1
fi

echo "testing deploy-winget missing installer..."
set +e
bash "$script_dir/deploy-winget.sh" v0.1.7 non-existent-installer.exe >/dev/null 2>&1
code=$?
set -e
if [ "$code" -ne 1 ]; then
  echo "expected exit code 1 on missing installer, got $code" >&2
  exit 1
fi

echo "deploy-winget smoke ok"
