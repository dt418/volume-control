#!/usr/bin/env pwsh
<#
.SYNOPSIS
  Register a Windows scheduled task that checks the winget PR validation.

.DESCRIPTION
  Registers the "winget-validation" developer task defined in
  scripts/lib/DevTask.ps1 and runs scripts/check-winget-validation.ps1 every
  -IntervalMinutes. The checker logs to
  %LOCALAPPDATA%\VolumeControl\winget-validation\, posts one ping when
  Validation-Completed appears, then unregisters this task.

  The task is created hidden at the process level and runs with
  -WindowStyle Hidden, so it never flashes a console window. Re-running this
  script repairs a stale registration instead of leaving it in place.

  List or remove every developer task with scripts/dev-tasks.ps1.

.PARAMETER IntervalMinutes
  Check interval. Default: 5.

.PARAMETER LifetimeDays
  How long the repetition may keep firing before giving up. Default: 14.

.PARAMETER Unregister
  Remove the scheduled task instead of creating it, including any legacy root
  registration.

.PARAMETER Purge
  With -Unregister, also delete the task's state directory.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/install-winget-validation-task.ps1

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/install-winget-validation-task.ps1 -Unregister -Purge
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
  Uninstall-DevTask -Id 'winget-validation' -Purge:$Purge | Out-Null
  exit 0
}

Install-DevTask -Id 'winget-validation' -IntervalMinutes $IntervalMinutes -LifetimeDays $LifetimeDays | Out-Null
