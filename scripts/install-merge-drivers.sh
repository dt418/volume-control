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
# Usage: bash scripts/install-merge-drivers.sh [repository]
# Defaults to the repository this script lives in.
set -e

script_dir="$(cd "$(dirname "$0")" && pwd)"
target="${1:-$(cd "$script_dir/.." && pwd)}"
target="$(cd "$target" && pwd)"

# The drivers live in the clone's git directory, not in the working tree: a
# topic branch that predates them (or a merge that is still in flight) has no
# scripts/merge-records-*.sh to run, and git reads merge attributes from the
# branch being merged into.
git_dir="$(git -C "$target" rev-parse --git-common-dir)"
case "$git_dir" in
  /*|[A-Za-z]:*) : ;;
  *) git_dir="$target/$git_dir" ;;
esac
driver_dir="$git_dir/records-merge"
mkdir -p "$driver_dir"
cp "$script_dir/merge-records-json.sh" "$driver_dir/merge-records-json.sh"
cp "$script_dir/merge-records-md.sh" "$driver_dir/merge-records-md.sh"

git -C "$target" config merge.records-json.name "semantic merge for feature_list.json"
git -C "$target" config merge.records-json.driver 'bash "$(git rev-parse --git-common-dir)/records-merge/merge-records-json.sh" %O %A %B'
git -C "$target" config merge.records-md.name "heading merge for the markdown record logs"
git -C "$target" config merge.records-md.driver 'bash "$(git rev-parse --git-common-dir)/records-merge/merge-records-md.sh" %O %A %B'

info_attributes="$(git -C "$target" rev-parse --git-path info/attributes)"
case "$info_attributes" in
  /*|[A-Za-z]:*) : ;;
  *) info_attributes="$target/$info_attributes" ;;
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

echo "Installed merge drivers in: $driver_dir"
echo "Attribute overrides in:     $info_attributes"
