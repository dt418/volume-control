# Install the repository's versioned git hooks.
$root = Split-Path -Parent $PSScriptRoot
git -C $root config core.hooksPath .githooks
Write-Host "Installed hooks: $(git -C $root config --get core.hooksPath)"

# Register the record-file merge drivers, so record edits made on top of an
# updated main merge without conflict markers (see .gitattributes).
git -C $root config merge.records-json.name "semantic merge for feature_list.json"
git -C $root config merge.records-json.driver 'bash scripts/merge-records-json.sh %O %A %B'
git -C $root config merge.records-md.name "heading merge for the markdown record logs"
git -C $root config merge.records-md.driver 'bash scripts/merge-records-md.sh %O %A %B'
Write-Host "Installed merge drivers: $(git -C $root config --get merge.records-json.driver)"
