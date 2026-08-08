#!/usr/bin/env bash
# CI diff gate: fail when the branch diff contains whitespace errors or touches
# paths the repository rules forbid staging (target/, .superpowers/,
# .claude/settings.local.json, .claude/worktrees/, runtime config, scratch
# files).
#
# Usage: bash scripts/ci-diff-check.sh [base-ref]
#   base-ref defaults to origin/master; the diff is taken against the merge
#   base of HEAD and that ref.
set -euo pipefail

base="${1:-origin/master}"
base_sha="$(git merge-base "$base" HEAD 2>/dev/null || echo "$base")"

if ! git diff --check "$base_sha" HEAD; then
  echo "diff gate FAILED: whitespace errors in the diff against $base:" >&2
  git diff --check "$base_sha" HEAD >&2
  exit 1
fi

if ! git diff --name-only "$base_sha" HEAD | grep -qE '^(target/|\.superpowers/|\.claude/(settings\.local\.json|worktrees/)|config\.json|.*\.log)'; then
  echo "diff gate: no forbidden paths in $(git rev-parse --short HEAD)"
  exit 0
fi

echo "diff gate FAILED: the diff against $base contains forbidden paths:" >&2
git diff --name-only "$base_sha" HEAD | grep -E '^(target/|\.superpowers/|\.claude/(settings\.local\.json|worktrees/)|config\.json|.*\.log)' >&2
exit 1
