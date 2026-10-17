#!/usr/bin/env bash
# Register the record-file merge drivers in this clone.
#
# .gitattributes marks claude-progress.md, session-handoff.md, and
# feature_list.json with the `records-md` / `records-json` merge drivers, but
# git only runs a driver that the local configuration declares, and it reads
# the attributes from the branch being merged into. Registering the attributes
# in .git/info/attributes as well makes the drivers apply even when the branch
# predates the .gitattributes change, so a stale topic branch still merges the
# record files conflict-free.
#
# Usage: bash scripts/install-merge-drivers.sh
set -e

root="$(cd "$(dirname "$0")/.." && pwd)"

git -C "$root" config merge.records-json.name "semantic merge for feature_list.json"
git -C "$root" config merge.records-json.driver 'bash scripts/merge-records-json.sh %O %A %B'
git -C "$root" config merge.records-md.name "heading merge for the markdown record logs"
git -C "$root" config merge.records-md.driver 'bash scripts/merge-records-md.sh %O %A %B'

info_attributes="$(git -C "$root" rev-parse --git-path info/attributes)"
case "$info_attributes" in
  /*|[A-Za-z]:*) : ;;
  *) info_attributes="$root/$info_attributes" ;;
esac
mkdir -p "$(dirname "$info_attributes")"
touch "$info_attributes"
for entry in \
  'claude-progress.md merge=records-md' \
  'session-handoff.md merge=records-md' \
  'feature_list.json merge=records-json'
do
  if ! grep -qxF "$entry" "$info_attributes"; then
    printf '%s\n' "$entry" >> "$info_attributes"
  fi
done

echo "Installed merge drivers: $(git -C "$root" config --get merge.records-json.driver)"
echo "                         $(git -C "$root" config --get merge.records-md.driver)"
echo "Attribute overrides in:  $info_attributes"
