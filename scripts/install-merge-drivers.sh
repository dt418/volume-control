#!/usr/bin/env bash
# Register the record-file merge drivers in this clone.
#
# .gitattributes marks claude-progress.md, session-handoff.md, and
# feature_list.json with the `records-md` / `records-json` merge drivers, but
# git only runs a driver that the local configuration declares. Run this once
# per clone (scripts/install-hooks.sh calls it too) and record-file edits made
# on top of an updated main merge without conflict markers.
#
# Usage: bash scripts/install-merge-drivers.sh
set -e

root="$(cd "$(dirname "$0")/.." && pwd)"

git -C "$root" config merge.records-json.name "semantic merge for feature_list.json"
git -C "$root" config merge.records-json.driver 'bash scripts/merge-records-json.sh %O %A %B'
git -C "$root" config merge.records-md.name "heading merge for the markdown record logs"
git -C "$root" config merge.records-md.driver 'bash scripts/merge-records-md.sh %O %A %B'

echo "Installed merge drivers: $(git -C "$root" config --get merge.records-json.driver)"
echo "                         $(git -C "$root" config --get merge.records-md.driver)"
