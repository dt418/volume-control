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
.PARAMETER VirtualAudio
  Use the debug virtual audio backend for the E2E step (hosted-CI style).
  Default is REAL audio: the E2E drives the actual OS endpoint.
#>
param(
  [switch]$SkipE2e,
  [switch]$Interactive,
  [switch]$VirtualAudio,
  [string]$OutputRoot = (Join-Path (Split-Path $PSScriptRoot -Parent) "output\platform")
)

$ErrorActionPreference = "Stop"
$repo = Split-Path $PSScriptRoot -Parent
$failures = @()
New-Item -ItemType Directory -Force -Path $OutputRoot | Out-Null

Write-Host "NOTE: the E2E step uses REAL audio by default — the app drives the" -ForegroundColor Yellow
Write-Host "actual OS endpoint (volume/mute round-trips affect the device and are" -ForegroundColor Yellow
Write-Host "restored afterwards). Pass -VirtualAudio for hosted-CI-style simulated" -ForegroundColor Yellow
Write-Host "audio, whose displays never touch the real device." -ForegroundColor Yellow

function Invoke-Step {
  param([string]$Name, [scriptblock]$Body)
  Write-Host "`n=== $Name ===" -ForegroundColor Cyan
  $stepLog = Join-Path $OutputRoot "step-$($Name -replace '[^A-Za-z0-9]+','-').log"
  try {
    & $Body *>&1 | Tee-Object -FilePath $stepLog
    if ($LASTEXITCODE -ne 0) { throw "step exited $LASTEXITCODE" }
    Write-Host "PASS: $Name" -ForegroundColor Green
  } catch {
    Write-Host "FAIL: $Name -- $($_.Exception.Message)" -ForegroundColor Red
    if (Test-Path $stepLog) {
      Write-Host "--- last 15 lines of $stepLog ---" -ForegroundColor Yellow
      Get-Content $stepLog -Tail 15 | ForEach-Object { Write-Host $_ -ForegroundColor Gray }
    }
    $script:failures += $Name
  }
}

function Invoke-RepoBash {
  param([string]$Script)
  Push-Location $repo
  try {
    bash $Script
    if ($LASTEXITCODE -ne 0) { throw "$Script exited $LASTEXITCODE" }
  } finally {
    Pop-Location
  }
}

Invoke-Step "format-lint gate (fmt, clippy -D warnings, workspace tests)" {
  Invoke-RepoBash "scripts/format-lint.sh"
}

Invoke-Step "frontend Vitest + production build" {
  npm test --prefix (Join-Path $repo "frontend")
  if ($LASTEXITCODE -ne 0) { throw "frontend tests exited $LASTEXITCODE" }
  npm run build --prefix (Join-Path $repo "frontend")
  if ($LASTEXITCODE -ne 0) { throw "frontend build exited $LASTEXITCODE" }
}

if (-not $SkipE2e) {
  Invoke-Step "Tauri E2E evidence gate (all surfaces)" {
    $e2eParams = @{
      Surface = "all"
      OutputRoot = (Join-Path $OutputRoot "tauri-e2e")
      VirtualAudio = [bool]$VirtualAudio
    }
    & (Join-Path $PSScriptRoot "verify-tauri-e2e.ps1") @e2eParams
    if ($LASTEXITCODE -ne 0) { throw "E2E gate exited $LASTEXITCODE" }
  }
} else {
  Write-Host "SKIP: Tauri E2E evidence gate (-SkipE2e)" -ForegroundColor Yellow
}

$releaseExe = Join-Path $repo "target\release\VolumeControl.exe"
if (Test-Path $releaseExe) {
  Invoke-Step "Windows autostart registry verifier" {
    & (Join-Path $PSScriptRoot "verify-autostart.ps1") -Binary $releaseExe
    if ($LASTEXITCODE -ne 0) { throw "autostart verifier exited $LASTEXITCODE" }
  }
} else {
  Write-Host "SKIP: autostart verifier (release binary missing: $releaseExe)" -ForegroundColor Yellow
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
  Invoke-RepoBash "scripts/test-check-records.sh"
  Invoke-RepoBash "scripts/test-format-lint.sh"
  Invoke-RepoBash "scripts/test-ship.sh"
}

if ($failures.Count -gt 0) {
  Write-Host "`nPLATFORM VERIFICATION FAILED: $($failures -join ', ')" -ForegroundColor Red
  exit 1
}
Write-Host "`nPLATFORM VERIFICATION PASSED (Windows)" -ForegroundColor Green
exit 0
