#!/usr/bin/env sh
# Install the repository's versioned git hooks.
set -e

root="$(cd "$(dirname "$0")/.." && pwd)"
git -C "$root" config core.hooksPath .githooks
echo "Installed hooks: $(git -C "$root" config --get core.hooksPath)"

# Register the record-file merge drivers, so record edits made on top of an
# updated main merge without conflict markers (see .gitattributes).
sh "$root/scripts/install-merge-drivers.sh"
