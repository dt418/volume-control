<#
.SYNOPSIS
    Ensure the pkg-config cross-check stub exists and works.
.DESCRIPTION
    Environment shim for cross-target cargo checks from a Windows host
    (libpulse-sys's build script needs pkg-config). Creates
    %TEMP%\rtk-stub-bin\pkg-config.cmd if missing and verifies it answers
    --modversion. Writes UTF-8 no-BOM (never Set-Content).
.EXAMPLE
    powershell -ExecutionPolicy Bypass -File .agents/skills/windows-host/scripts/ensure-pkg-config-stub.ps1
#>

$ErrorActionPreference = 'Stop'

$stubDir = Join-Path $env:TEMP 'rtk-stub-bin'
$stubPath = Join-Path $stubDir 'pkg-config.cmd'
# A pure exit-0 stub panics inside pkg-config-0.3.33 parse_modversion
# (Option::unwrap on empty --modversion output), so it must print a version.
$stubContent = "@echo off`r`nfor %%a in (%*) do if ""%%a""==""--modversion"" echo 1.0`r`nexit /b 0`r`n"

if (-not (Test-Path -LiteralPath $stubDir)) {
    New-Item -ItemType Directory -Path $stubDir | Out-Null
}

$needWrite = $true
if (Test-Path -LiteralPath $stubPath) {
    $existing = [System.IO.File]::ReadAllText($stubPath)
    if ($existing -eq $stubContent) {
        $needWrite = $false
    } else {
        Write-Host "Existing stub differs; overwriting: $stubPath"
    }
}

if ($needWrite) {
    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText($stubPath, $stubContent, $utf8NoBom)
    Write-Host "Created stub: $stubPath"
}

if (-not (Test-Path -LiteralPath $stubPath)) {
    Write-Host "ERROR: stub was not created at $stubPath" -ForegroundColor Red
    exit 1
}
$out = & $stubPath --modversion 2>$null
if ($LASTEXITCODE -ne 0 -or "$out".Trim() -ne '1.0') {
    Write-Host "ERROR: stub does not answer --modversion correctly" -ForegroundColor Red
    exit 1
}

Write-Host "Stub ready: $stubPath"
Write-Host "Set for cross-target checks:"
Write-Host "  `$env:PKG_CONFIG = `"$stubPath`""
Write-Host "  `$env:PKG_CONFIG_ALLOW_CROSS = '1'"
exit 0
