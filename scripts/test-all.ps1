<#
    test-all.ps1 - Comprehensive end-to-end integration test runner for Windows PowerShell.
    Bridges to scripts/test-all.sh using Git for Windows bash.
#>
param(
    [switch]$Quick,
    [switch]$SkipSmoke,
    [switch]$Fix
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot

$argsList = @()
if ($Quick) { $argsList += '--quick' }
if ($SkipSmoke) { $argsList += '--skip-smoke' }
if ($Fix) { $argsList += '--fix' }

$sh = Get-Command sh -ErrorAction SilentlyContinue
if ($sh) {
    & $sh.Source "$repo/scripts/test-all.sh" @argsList
    exit $LASTEXITCODE
}

$bashCandidates = @(
    "$env:ProgramFiles\Git\bin\bash.exe",
    "${env:ProgramFiles(x86)}\Git\bin\bash.exe"
)
$bash = $bashCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
if (-not $bash) {
    throw "Neither sh nor Git bash found on PATH or standard install paths."
}

& $bash "$repo/scripts/test-all.sh" @argsList
exit $LASTEXITCODE
