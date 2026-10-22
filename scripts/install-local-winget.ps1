#!/usr/bin/env pwsh
<#
.SYNOPSIS
  Install the newest VolumeControl winget manifest with local manifests.

.DESCRIPTION
  Resolves the latest (or pinned) manifest under
  winget/dt418.VolumeControl, validates it, then runs
  `winget install --manifest <dir>`. Requires the local-manifest feature:
    winget settings --enable LocalManifestFiles   # elevated, one time

.PARAMETER Version
  Package version to install. Default: newest version directory.

.PARAMETER ValidateOnly
  Resolve and validate the manifest without installing.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/install-local-winget.ps1

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/install-local-winget.ps1 -Version 0.1.9 -ValidateOnly
#>
[CmdletBinding()]
param(
  [string]$Version,
  [switch]$ValidateOnly
)

$ErrorActionPreference = "Stop"

$repo = Split-Path $PSScriptRoot -Parent
$packageRoot = Join-Path $repo "winget\dt418.VolumeControl"

if ($Version) {
  $manifestPath = Join-Path $packageRoot $Version
  if (-not (Test-Path $manifestPath -PathType Container)) {
    throw "Manifest version not found: $manifestPath"
  }
} else {
  $versions = Get-ChildItem $packageRoot -Directory |
    Where-Object { $_.Name -match '^\d+\.\d+\.\d+$' } |
    Sort-Object { [version]$_.Name } -Descending
  if ($versions.Count -eq 0) {
    throw "No versioned manifests under $packageRoot"
  }
  $manifestPath = $versions[0].FullName
}

$manifestPath = (Resolve-Path $manifestPath).Path
$required = @(
  "dt418.VolumeControl.yaml",
  "dt418.VolumeControl.installer.yaml",
  "dt418.VolumeControl.locale.en-US.yaml"
)
foreach ($name in $required) {
  $file = Join-Path $manifestPath $name
  if (-not (Test-Path $file -PathType Leaf)) {
    throw "Incomplete manifest set: missing $file"
  }
}

Write-Host "Manifest: $manifestPath" -ForegroundColor Cyan
winget validate --manifest $manifestPath
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}
if ($ValidateOnly) {
  exit 0
}

winget install --manifest $manifestPath `
  --accept-package-agreements `
  --accept-source-agreements `
  --disable-interactivity
exit $LASTEXITCODE
