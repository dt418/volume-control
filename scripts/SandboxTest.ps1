#!/usr/bin/env pwsh
<#
.SYNOPSIS
  Test the VolumeControl winget manifest in Windows Sandbox.

.DESCRIPTION
  Resolves the newest (or a pinned) manifest under
  winget/dt418.VolumeControl, then runs the upstream winget-pkgs
  SandboxTest.ps1 against it. The upstream script downloads WinGet into an
  isolated Windows Sandbox, installs the manifest, and reports the result.
  No app is installed on the host.

.PARAMETER Manifest
  Manifest directory or file to test. Overrides version discovery.

.PARAMETER Version
  Package version to test, for example 0.1.9. Default: newest directory.

.PARAMETER Prerelease
  Use the WinGet preview build inside the sandbox.

.PARAMETER Clean
  Re-download the upstream SandboxTest.ps1 and force WinGet re-download.

.PARAMETER SkipManifestValidation
  Skip the upstream `winget validate` step (validation already run).

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/SandboxTest.ps1

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/SandboxTest.ps1 -Version 0.1.9
#>
[CmdletBinding()]
param(
  [string]$Manifest,
  [string]$Version,
  [switch]$Prerelease,
  [switch]$Clean,
  [switch]$SkipManifestValidation
)

$ErrorActionPreference = "Stop"

$repo = Split-Path $PSScriptRoot -Parent
$packageRoot = Join-Path $repo "winget\dt418.VolumeControl"

if (-not (Test-Path $packageRoot -PathType Container)) {
  throw "Package manifest root not found: $packageRoot"
}

if ($Manifest) {
  $manifestPath = (Resolve-Path $Manifest).Path
  if (Test-Path $manifestPath -PathType Leaf) {
    $manifestPath = Split-Path $manifestPath -Parent
  }
} elseif ($Version) {
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

$sandboxExe = Join-Path $env:SystemRoot "System32\WindowsSandbox.exe"
if (-not (Test-Path $sandboxExe)) {
  throw @"
Windows Sandbox is not installed. On Windows Pro/Enterprise/Education:
  Enable-WindowsOptionalFeature -FeatureName Containers-DisposableClientVM -All -Online
Then reboot. Windows Home does not support Windows Sandbox.
"@
}

$cacheDir = Join-Path $repo "output\sandbox"
New-Item -ItemType Directory -Force -Path $cacheDir | Out-Null
$upstream = Join-Path $cacheDir "upstream-SandboxTest.ps1"
$upstreamUrl = "https://raw.githubusercontent.com/microsoft/winget-pkgs/master/Tools/SandboxTest.ps1"

if ($Clean -or -not (Test-Path $upstream -PathType Leaf)) {
  Write-Host "Fetching upstream SandboxTest.ps1..." -ForegroundColor Cyan
  Invoke-WebRequest -Uri $upstreamUrl -OutFile $upstream -UseBasicParsing
}
if (-not (Select-String -Path $upstream -Pattern 'param\s*\(' -Quiet)) {
  throw "Upstream SandboxTest.ps1 download is invalid: $upstream"
}

# The upstream script uses APIs unavailable in Windows PowerShell 5.1 when a
# GitHub token is supplied. Prefer PowerShell 7 and fail clearly for
# authenticated runs if it is missing.
$pwsh = Get-Command pwsh.exe -ErrorAction SilentlyContinue
if ($pwsh) {
  $runner = $pwsh.Source
} elseif ($env:WINGET_PKGS_GITHUB_TOKEN) {
  throw "PowerShell 7 (pwsh.exe) is required for authenticated SandboxTest runs; install it or unset WINGET_PKGS_GITHUB_TOKEN."
} else {
  $runner = (Get-Command powershell.exe -ErrorAction Stop).Source
}

# Map only a disposable directory into Windows Sandbox. The upstream script
# copies the manifest into its own test-data folder; mapping the whole repo
# would give the sandbox write access to scripts and git hooks.
$mapDir = Join-Path $env:TEMP ("VolumeControl-Sandbox-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $mapDir | Out-Null

$upstreamArgs = @(
  "-NoProfile",
  "-ExecutionPolicy", "Bypass",
  "-File", $upstream,
  "-Manifest", $manifestPath,
  "-MapFolder", $mapDir
)
if ($Prerelease) { $upstreamArgs += "-Prerelease" }
if ($Clean) { $upstreamArgs += "-Clean" }
if ($SkipManifestValidation) { $upstreamArgs += "-SkipManifestValidation" }

Write-Host "Launching Windows Sandbox (install is isolated from the host)..." -ForegroundColor Cyan
$exitCode = 0
try {
  & $runner @upstreamArgs
  $exitCode = $LASTEXITCODE
} finally {
  Remove-Item -LiteralPath $mapDir -Recurse -Force -ErrorAction SilentlyContinue
}
exit $exitCode
