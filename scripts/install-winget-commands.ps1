#!/usr/bin/env pwsh
<#
.SYNOPSIS
  Install `winget-manifest` and `winget-status` commands for the current user.

.DESCRIPTION
  Writes small .cmd shims into %LOCALAPPDATA%\Microsoft\WindowsApps (already on
  PATH) that call this repo's scripts by absolute path. The commands then work
  from any current directory. Re-run this script if the repo moves.

.PARAMETER Uninstall
  Remove the two shims.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File .\scripts\install-winget-commands.ps1

.EXAMPLE
  winget-manifest validate
  winget-status
#>
[CmdletBinding()]
param(
  [switch]$Uninstall
)

$ErrorActionPreference = "Stop"

$repo = Split-Path $PSScriptRoot -Parent
$targetDir = Join-Path $env:LOCALAPPDATA "Microsoft\WindowsApps"
$commands = @{
  "winget-manifest.cmd" = "winget-manifest.ps1"
  "winget-status.cmd"   = "show-winget-validation.ps1"
}

if ($Uninstall) {
  foreach ($name in $commands.Keys) {
    $path = Join-Path $targetDir $name
    if (Test-Path $path) {
      Remove-Item $path -Force
      Write-Host "Removed: $path"
    }
  }
  exit 0
}

if (-not (Test-Path $targetDir -PathType Container)) {
  throw "WindowsApps directory not found: $targetDir"
}

$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
foreach ($name in $commands.Keys) {
  $scriptPath = Join-Path $repo ("scripts\" + $commands[$name])
  if (-not (Test-Path $scriptPath -PathType Leaf)) {
    throw "Script not found: $scriptPath"
  }
  $content = "@echo off`r`npowershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$scriptPath`" %*`r`n"
  [System.IO.File]::WriteAllText((Join-Path $targetDir $name), $content, $utf8NoBom)
  Write-Host "Installed: $(Join-Path $targetDir $name)"
}

$pathEntries = $env:PATH -split ";"
if ($pathEntries -notcontains $targetDir) {
  Write-Warning "$targetDir is not on PATH in this process. Open a new terminal after checking the user PATH."
}

Write-Host ""
Write-Host "Try from any directory:"
Write-Host "  winget-manifest validate"
Write-Host "  winget-status"
