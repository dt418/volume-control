#!/usr/bin/env pwsh
<#
.SYNOPSIS
  Print the current winget validation status and recent cron log.

.PARAMETER Tail
  Number of log lines to show. Default: 20.

.EXAMPLE
  winget-status
#>
[CmdletBinding()]
param(
  [int]$Tail = 20
)

$ErrorActionPreference = "Stop"

$stateDir = Join-Path $env:LOCALAPPDATA "VolumeControl\winget-validation"
$logPath = Join-Path $stateDir "status.log"
$jsonPath = Join-Path $stateDir "status.json"

if (-not (Test-Path $logPath -PathType Leaf)) {
  throw "No validation log yet: $logPath"
}

if (Test-Path $jsonPath -PathType Leaf) {
  $status = Get-Content $jsonPath -Raw | ConvertFrom-Json
  Write-Host "State:   $($status.state)"
  Write-Host "PR:      #$($status.pr) $($status.url)"
  Write-Host "Checked: $($status.checkedAt)"
  Write-Host "Labels:  $($status.labels -join ', ')"
  if ($status.failureLabels.Count -gt 0) {
    Write-Host "Failure: $($status.failureLabels -join ', ')" -ForegroundColor Red
  }
  Write-Host ""
}

Get-Content $logPath -Tail $Tail
