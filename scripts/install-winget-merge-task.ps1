#!/usr/bin/env pwsh
<#
.SYNOPSIS
  Register a scheduled task that watches the winget PR merge and catalog.

.DESCRIPTION
  Registers the "winget-merge" developer task defined in scripts/lib/DevTask.ps1
  and runs scripts/check-winget-merge.ps1 every -IntervalMinutes. The checker
  logs under %LOCALAPPDATA%\VolumeControl\winget-merge and unregisters this task
  once the expected version appears in the winget catalog (or the PR closes).

  The task is created hidden at the process level and runs with
  -WindowStyle Hidden, so it never flashes a console window. Re-running this
  script repairs a stale registration instead of leaving it in place.

  List or remove every developer task with scripts/dev-tasks.ps1.

.PARAMETER IntervalMinutes
  Check interval. Default: 10.

.PARAMETER LifetimeDays
  How long the repetition may keep firing before giving up. Default: 14.

.PARAMETER Unregister
  Remove the scheduled task instead of creating it, including any legacy root
  registration.

.PARAMETER Purge
  With -Unregister, also delete the task's state directory.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File .\scripts\install-winget-merge-task.ps1

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File .\scripts\install-winget-merge-task.ps1 -Unregister -Purge
#>
[CmdletBinding()]
param(
  [int]$IntervalMinutes = 0,
  [int]$LifetimeDays = 0,
  [switch]$Unregister,
  [switch]$Purge
)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'lib/DevTask.ps1')

if ($Unregister) {
  Uninstall-DevTask -Id 'winget-merge' -Purge:$Purge | Out-Null
  exit 0
}

Install-DevTask -Id 'winget-merge' -IntervalMinutes $IntervalMinutes -LifetimeDays $LifetimeDays | Out-Null
