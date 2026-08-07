# Install the repository's versioned git hooks.
$root = Split-Path -Parent $PSScriptRoot
git -C $root config core.hooksPath .githooks
Write-Host "Installed hooks: $(git -C $root config --get core.hooksPath)"
