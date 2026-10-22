#!/usr/bin/env pwsh
<#
.SYNOPSIS
  Register a scheduled task that watches the winget PR merge and catalog.

.DESCRIPTION
  Runs scripts/check-winget-merge.ps1 every -IntervalMinutes. The checker logs
  under %LOCALAPPDATA%\VolumeControl\winget-merge and disables this task once
  the expected version appears in the winget catalog (or the PR closes).

.PARAMETER IntervalMinutes
  Check interval. Default: 10.

.PARAMETER TaskName
  Scheduled task name. Default: VolumeControl-WingetMergeWatch.

.PARAMETER Unregister
  Remove the scheduled task.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File .\scripts\install-winget-merge-task.ps1

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File .\scripts\install-winget-merge-task.ps1 -Unregister
#>
[CmdletBinding()]
param(
  [int]$IntervalMinutes = 10,
  [string]$TaskName = "VolumeControl-WingetMergeWatch",
  [switch]$Unregister
)

$ErrorActionPreference = "Stop"

$repo = Split-Path $PSScriptRoot -Parent
$checker = Join-Path $PSScriptRoot "check-winget-merge.ps1"

if ($Unregister) {
  Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false -ErrorAction SilentlyContinue
  Write-Host "Removed task: $TaskName"
  exit 0
}

if (-not (Test-Path $checker -PathType Leaf)) { throw "Checker not found: $checker" }
if ($IntervalMinutes -lt 1) { throw "IntervalMinutes must be at least 1" }

$powershell = Join-Path $env:SystemRoot "System32\WindowsPowerShell\v1.0\powershell.exe"
$arguments = "-NoProfile -ExecutionPolicy Bypass -File `"$checker`" -StopOnTerminal -TaskName `"$TaskName`""

$action = New-ScheduledTaskAction -Execute $powershell -Argument $arguments -WorkingDirectory $repo
$trigger = New-ScheduledTaskTrigger -Once -At ((Get-Date).AddMinutes(1)) `
  -RepetitionInterval (New-TimeSpan -Minutes $IntervalMinutes) `
  -RepetitionDuration (New-TimeSpan -Days 3650)
$settings = New-ScheduledTaskSettingsSet -StartWhenAvailable `
  -MultipleInstances IgnoreNew `
  -ExecutionTimeLimit (New-TimeSpan -Minutes 10) `
  -AllowStartIfOnBatteries `
  -DontStopIfGoingOnBatteries
$principal = New-ScheduledTaskPrincipal -UserId "$env:USERDOMAIN\$env:USERNAME" `
  -LogonType Interactive -RunLevel Limited

Register-ScheduledTask -TaskName $TaskName `
  -Action $action `
  -Trigger $trigger `
  -Settings $settings `
  -Principal $principal `
  -Description "Watches microsoft/winget-pkgs PR #454508 merge and winget catalog publication." `
  -Force | Out-Null

$task = Get-ScheduledTask -TaskName $TaskName
$info = Get-ScheduledTaskInfo -TaskName $TaskName
Write-Host "Registered: $($task.TaskName) [$($task.State)]"
Write-Host "Next run: $($info.NextRunTime)"
Write-Host "Log: $(Join-Path $env:LOCALAPPDATA 'VolumeControl\winget-merge\status.log')"
