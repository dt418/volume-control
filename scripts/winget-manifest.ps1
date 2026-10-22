#!/usr/bin/env pwsh
<#
.SYNOPSIS
  Run winget validate/show/install against the VolumeControl manifest.

.DESCRIPTION
  Resolves the newest (or pinned) manifest under
  winget/dt418.VolumeControl from the repo root, so the command works from any
  current directory.

.PARAMETER Action
  validate, show, install, or path. Default: validate.

.PARAMETER Version
  Package version, for example 0.1.9. Default: newest.

.EXAMPLE
  winget-manifest validate

.EXAMPLE
  winget-manifest show -Version 0.1.9

.EXAMPLE
  winget-manifest install
#>
[CmdletBinding()]
param(
  [Parameter(Position = 0)]
  [ValidateSet("validate", "show", "install", "path")]
  [string]$Action = "validate",
  [Parameter(Position = 1)]
  [string]$Version
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

if ($Action -eq "path") {
  $manifestPath
  exit 0
}

if ($Action -eq "install") {
  & (Join-Path $PSScriptRoot "install-local-winget.ps1") -Version (Split-Path $manifestPath -Leaf)
  exit $LASTEXITCODE
}

winget $Action --manifest $manifestPath
exit $LASTEXITCODE
