# Build the native egui host and package it for Windows.
#
# Output (default output\egui):
#   VolumeControl_<version>_x64-setup.exe   NSIS installer (per-user; matches the updater contract)
#   VolumeControl-<version>-x64.zip         portable build
#   SHA256SUMS.txt
#
# Signing is intentionally out of scope: run signtool on the setup exe before
# publishing a real release.

param(
    [string]$Version,
    [string]$OutputRoot = "output/egui",
    [string]$Makensis,
    [string]$SignTool
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

Push-Location $root
try {
    if (-not $Version) {
        $conf = Get-Content 'src-tauri/tauri.conf.json' -Raw | ConvertFrom-Json
        $Version = $conf.version
    }
    if (-not $Version) { throw 'Version could not be resolved' }

    if (-not $Makensis) {
        $candidates = @(
            "$env:LOCALAPPDATA\tauri\NSIS\makensis.exe",
            "$env:LOCALAPPDATA\tauri\NSIS\Bin\makensis.exe",
            "$env:ProgramFiles\NSIS\makensis.exe"
        )
        $Makensis = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
    }
    if (-not $Makensis) { throw 'makensis.exe not found; install NSIS or pass -Makensis' }

    Write-Host "building volumecontrol-host $Version"
    cargo build --release -p volumecontrol-egui
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

    $outDir = Join-Path $root $OutputRoot
    New-Item -ItemType Directory -Force -Path $outDir | Out-Null
    $stage = Join-Path $outDir 'stage'
    Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $stage | Out-Null
    Copy-Item 'target/release/volumecontrol-host.exe' $stage
    $signScript = Join-Path $PSScriptRoot 'sign-windows.ps1'
    $inner = Join-Path $stage 'volumecontrol-host.exe'
    if ($SignTool) {
        & $signScript -Files $inner -SignTool $SignTool
    } else {
        & $signScript -Files $inner
    }
    if ($LASTEXITCODE -ne 0) { throw 'signing the host binary failed' }

    $zip = Join-Path $outDir "VolumeControl-$Version-x64.zip"
    Compress-Archive -Path (Join-Path $stage 'volumecontrol-host.exe') -DestinationPath $zip -Force

    $setup = Join-Path $outDir "VolumeControl_$Version`_x64-setup.exe"
    $icon = Join-Path $root 'src-tauri/icons/icon.ico'
    & $Makensis /V2 `
        "/DVERSION=$Version" `
        "/DSRC=$stage" `
        "/DOUT=$setup" `
        "/DICON=$icon" `
        (Join-Path $root 'packaging/egui/windows.nsi')
    if ($LASTEXITCODE -ne 0) { throw 'makensis failed' }
    if ($SignTool) {
        & $signScript -Files $setup -SignTool $SignTool
    } else {
        & $signScript -Files $setup
    }
    if ($LASTEXITCODE -ne 0) { throw 'signing the installer failed' }

    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        $lines = Get-ChildItem $outDir -File |
            Where-Object { $_.Extension -in '.exe', '.zip' } |
            ForEach-Object {
                $bytes = [System.IO.File]::ReadAllBytes($_.FullName)
                $hash = [BitConverter]::ToString($sha.ComputeHash($bytes)).
                    Replace('-', '').ToLowerInvariant()
                "$hash  $($_.Name)"
            }
    } finally {
        $sha.Dispose()
    }
    Set-Content -Path (Join-Path $outDir 'SHA256SUMS.txt') -Value $lines -Encoding ascii

    Write-Host "artifacts in $outDir"
    Get-ChildItem $outDir -File | Select-Object Name, Length
} finally {
    Pop-Location
}
