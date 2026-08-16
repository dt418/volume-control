#!/usr/bin/env pwsh
<#
.SYNOPSIS
  Run the complete VolumeControl verification battery for the current
  platform. This script is the supported "test the whole app" entry point on
  Windows (macOS/Linux use scripts/verify-platform.sh).

.DESCRIPTION
  Runs, fail-closed, in order:
    1. The full format-lint gate (fmt, whitespace, clippy -D warnings,
       workspace tests) via scripts/format-lint.sh.
    2. Frontend Vitest suite + production build.
    3. The Tauri E2E evidence gate (WDIO surface matrix) via
       scripts/verify-tauri-e2e.ps1 -Surface all.
    4. Platform probes: Windows autostart registry verifier
       (verify-autostart.ps1). The interactive hotkey-latency probe is
       opt-in via -Interactive because it needs a real desktop session and
       sends global keys.
    5. The enforcement self-tests (records/format-lint/ship guards).

  Exit code 0 only when every step passed; the first failure aborts.

.PARAMETER SkipE2e
  Skip the WDIO surface matrix (faster local iterations).
.PARAMETER Interactive
  Also run the real hotkey-to-mixer latency probe (interactive desktop
  required; sends the configured shortcut and waits for the mixer).
.PARAMETER OutputRoot
  Where E2E/probe evidence is written. Defaults to output/platform/.
#>
param(
  [switch]$SkipE2e,
  [switch]$Interactive,
  [string]$OutputRoot = (Join-Path (Split-Path $PSScriptRoot -Parent) "output\platform")
)

$ErrorActionPreference = "Stop"
$repo = Split-Path $PSScriptRoot -Parent
$failures = @()

Write-Host "NOTE: the E2E step launches real app windows using the DEBUG" -ForegroundColor Yellow
Write-Host "virtual audio backend (VOLUMECTL_E2E_AUDIO=virtual). Their volume" -ForegroundColor Yellow
Write-Host "displays are simulated and NEVER change the real device; ignore them." -ForegroundColor Yellow

function Invoke-Step {
  param([string]$Name, [scriptblock]$Body)
  Write-Host "`n=== $Name ===" -ForegroundColor Cyan
  try {
    & $Body
    Write-Host "PASS: $Name" -ForegroundColor Green
  } catch {
    Write-Host "FAIL: $Name -- $($_.Exception.Message)" -ForegroundColor Red
    $script:failures += $Name
  }
}

Invoke-Step "format-lint gate (fmt, clippy -D warnings, workspace tests)" {
  bash (Join-Path $repo "scripts\format-lint.sh")
  if ($LASTEXITCODE -ne 0) { throw "format-lint.sh exited $LASTEXITCODE" }
}

Invoke-Step "frontend Vitest + production build" {
  npm test --prefix (Join-Path $repo "frontend")
  if ($LASTEXITCODE -ne 0) { throw "frontend tests exited $LASTEXITCODE" }
  npm run build --prefix (Join-Path $repo "frontend")
  if ($LASTEXITCODE -ne 0) { throw "frontend build exited $LASTEXITCODE" }
}

if (-not $SkipE2e) {
  Invoke-Step "Tauri E2E evidence gate (all surfaces)" {
    & (Join-Path $PSScriptRoot "verify-tauri-e2e.ps1") `
      -Surface all -OutputRoot (Join-Path $OutputRoot "tauri-e2e")
    if ($LASTEXITCODE -ne 0) { throw "E2E gate exited $LASTEXITCODE" }
  }
} else {
  Write-Host "SKIP: Tauri E2E evidence gate (-SkipE2e)" -ForegroundColor Yellow
}

Invoke-Step "Windows autostart registry verifier" {
  & (Join-Path $PSScriptRoot "verify-autostart.ps1")
  if ($LASTEXITCODE -ne 0) { throw "autostart verifier exited $LASTEXITCODE" }
}

if ($Interactive) {
  Invoke-Step "interactive hotkey latency probe" {
    & (Join-Path $PSScriptRoot "verify-hotkey-latency.ps1") `
      -Release -Iterations 10 -OutputRoot (Join-Path $OutputRoot "hotkey-latency")
    if ($LASTEXITCODE -ne 0) { throw "hotkey latency probe exited $LASTEXITCODE" }
  }
} else {
  Write-Host "SKIP: interactive hotkey latency probe (pass -Interactive on a real desktop)" -ForegroundColor Yellow
}

Invoke-Step "enforcement self-tests" {
  bash (Join-Path $repo "scripts\test-check-records.sh")
  if ($LASTEXITCODE -ne 0) { throw "test-check-records exited $LASTEXITCODE" }
  bash (Join-Path $repo "scripts\test-format-lint.sh")
  if ($LASTEXITCODE -ne 0) { throw "test-format-lint exited $LASTEXITCODE" }
  bash (Join-Path $repo "scripts\test-ship.sh")
  if ($LASTEXITCODE -ne 0) { throw "test-ship exited $LASTEXITCODE" }
}

if ($failures.Count -gt 0) {
  Write-Host "`nPLATFORM VERIFICATION FAILED: $($failures -join ', ')" -ForegroundColor Red
  exit 1
}
Write-Host "`nPLATFORM VERIFICATION PASSED (Windows)" -ForegroundColor Green
exit 0
