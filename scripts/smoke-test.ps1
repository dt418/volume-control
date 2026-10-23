<#
    smoke-test.ps1 - Cross-platform smoke test runner bridge for Windows PowerShell.
    Bridges to scripts/smoke-test.sh using Git for Windows bash.
#>
param()

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot

$sh = Get-Command sh -ErrorAction SilentlyContinue
if ($sh) {
    & $sh.Source "$repo/scripts/smoke-test.sh"
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

& $bash "$repo/scripts/smoke-test.sh"
exit $LASTEXITCODE
