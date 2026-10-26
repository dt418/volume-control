#!/usr/bin/env pwsh
<#
.SYNOPSIS
  Shared registry and lifecycle helpers for VolumeControl developer tasks.

.DESCRIPTION
  Developer watchers (winget merge/validation polls) are Windows scheduled tasks
  that run outside the application. They therefore survive an app uninstall and
  are invisible unless something enumerates them. This module is the single
  source of truth for which tasks exist, how they are configured, and how to
  remove them.

  Every task is:
    * registered under the \VolumeControl-Dev\ task folder (visible in the Task
      Scheduler UI as one group, never mixed with Windows or app tasks),
    * stamped with a description marker so the audit can find it even if the
      folder is changed,
    * created hidden at the process level (TASK_FLAG_HIDDEN) and with
      -WindowStyle Hidden -NonInteractive on the command line, so it never
      flashes a console window,
    * bounded in lifetime instead of a decade-long repetition,
    * unregistered (not merely disabled) when the watched condition is terminal.

  Dot-source it:
      . "$PSScriptRoot/lib/DevTask.ps1"
#>

$script:DevTaskFolder = '\VolumeControl-Dev'
# Get-ScheduledTask/Get-ScheduledTaskInfo only match a folder path with the
# trailing separator; registration accepts either form.
$script:DevTaskFolderQuery = '\VolumeControl-Dev\'
$script:DevTaskMarker = 'volume-control-dev-task'
$script:DevTaskCatalog = @(
  @{
    Id              = 'winget-merge'
    Name            = 'WingetMergeWatch'
    IntervalMinutes = 10
    TimeLimitMin    = 10
    LifetimeDays    = 14
    Checker         = 'check-winget-merge.ps1'
    CheckerArgs     = @('-StopOnTerminal')
    StateDir        = Join-Path $env:LOCALAPPDATA 'VolumeControl\winget-merge'
    Description     = 'Polls microsoft/winget-pkgs PR #454508 for merge and catalog publication.'
  }
  @{
    Id              = 'winget-validation'
    Name            = 'WingetValidation'
    IntervalMinutes = 5
    TimeLimitMin    = 5
    LifetimeDays    = 14
    Checker         = 'check-winget-validation.ps1'
    CheckerArgs     = @('-PingOnGreen', '-StopOnTerminal')
    StateDir        = Join-Path $env:LOCALAPPDATA 'VolumeControl\winget-validation'
    Description     = 'Polls microsoft/winget-pkgs PR #454508 validation and pings once when green.'
  }
)

# Leaf names used by the pre-namespace installers. Kept so the audit can report
# and remove tasks registered before the move into \VolumeControl-Dev\.
$script:DevTaskLegacyNames = @('VolumeControl-WingetMergeWatch', 'VolumeControl-WingetValidation')

function Get-DevTaskCatalog {
  <#
  .SYNOPSIS
    Return every known developer task definition.
  #>
  return $script:DevTaskCatalog
}

function Get-DevTaskDefinition {
  <#
  .SYNOPSIS
    Resolve one catalog entry by Id or task leaf name.
  #>
  param([Parameter(Mandatory)][string]$Id)

  foreach ($entry in $script:DevTaskCatalog) {
    if ($entry.Id -eq $Id -or $entry.Name -eq $Id) { return $entry }
  }
  throw "Unknown dev task '$Id'. Known: $((($script:DevTaskCatalog | ForEach-Object { $_.Id }) -join ', '))"
}

function Get-DevTaskDescription {
  <#
  .SYNOPSIS
    Build the marker description so an inventory can attribute a task's owner.
  #>
  param([Parameter(Mandatory)]$Definition)

  return "$($script:DevTaskMarker)|id=$($Definition.Id)|$($Definition.Description)"
}

function Get-DevTaskArguments {
  <#
  .SYNOPSIS
    Build the desired powershell.exe argument string for a definition.
  #>
  param([Parameter(Mandatory)]$Definition)

  $checker = Join-Path (Split-Path $PSScriptRoot -Parent) $Definition.Checker
  $extra = @($Definition.CheckerArgs) + @('-TaskName', $Definition.Name, '-TaskPath', $script:DevTaskFolder)
  return "-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File `"$checker`" $($extra -join ' ')"
}

function Get-DevTaskRegistration {
  <#
  .SYNOPSIS
    Find the registered task that belongs to a definition, if any.
  .DESCRIPTION
    Checks the namespaced folder first, then the legacy root registration.
    Returns $null when the task is not registered.
  #>
  param([Parameter(Mandatory)]$Definition)

  $task = Get-ScheduledTask -TaskName $Definition.Name -TaskPath $script:DevTaskFolderQuery -ErrorAction SilentlyContinue
  if ($task) { return $task }

  $legacy = Get-ScheduledTask -TaskName "VolumeControl-$($Definition.Name)" -TaskPath '\' -ErrorAction SilentlyContinue
  if ($legacy) { return $legacy }

  return $null
}

function Install-DevTask {
  <#
  .SYNOPSIS
    Register (or repair) a developer task so it matches the catalog.
  .DESCRIPTION
    Registration is idempotent and self-correcting: an existing task whose
    arguments, principal or hidden flag no longer match the catalog is
    re-registered. This closes the drift that let a stale, visibly-flashing
    registration keep running after the installer had been fixed.
  #>
  param(
    [Parameter(Mandatory)][string]$Id,
    [int]$IntervalMinutes = 0,
    [int]$LifetimeDays = 0
  )

  $definition = Get-DevTaskDefinition -Id $Id
  if ($IntervalMinutes -lt 1) { $IntervalMinutes = $definition.IntervalMinutes }
  if ($LifetimeDays -lt 1) { $LifetimeDays = $definition.LifetimeDays }

  $checker = Join-Path (Split-Path $PSScriptRoot -Parent) $definition.Checker
  if (-not (Test-Path $checker -PathType Leaf)) { throw "Checker not found: $checker" }

  $repo = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
  $powershell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
  $arguments = Get-DevTaskArguments -Definition $definition

  $action = New-ScheduledTaskAction -Execute $powershell -Argument $arguments -WorkingDirectory $repo
  $trigger = New-ScheduledTaskTrigger -Once -At ((Get-Date).AddMinutes(1)) `
    -RepetitionInterval (New-TimeSpan -Minutes $IntervalMinutes) `
    -RepetitionDuration (New-TimeSpan -Days $LifetimeDays)
  $settings = New-ScheduledTaskSettingsSet -StartWhenAvailable `
    -MultipleInstances IgnoreNew `
    -ExecutionTimeLimit (New-TimeSpan -Minutes $definition.TimeLimitMin) `
    -AllowStartIfOnBatteries `
    -DontStopIfGoingOnBatteries `
    -Hidden
  $principal = New-ScheduledTaskPrincipal -UserId "$env:USERDOMAIN\$env:USERNAME" `
    -LogonType Interactive -RunLevel Limited

  $existing = Get-DevTaskRegistration -Definition $definition
  $drift = @()
  if ($existing) {
    if ($existing.Actions[0].Execute -ne $powershell) { $drift += 'execute' }
    if ($existing.Actions[0].Arguments -ne $arguments) { $drift += 'arguments' }
    if (-not $existing.Settings.Hidden) { $drift += 'hidden' }
    if ($existing.TaskPath -ne $script:DevTaskFolderQuery) { $drift += 'taskpath' }
  }

  Register-ScheduledTask -TaskName $definition.Name `
    -TaskPath $script:DevTaskFolder `
    -Action $action `
    -Trigger $trigger `
    -Settings $settings `
    -Principal $principal `
    -Description (Get-DevTaskDescription -Definition $definition) `
    -Force | Out-Null

  # A legacy root registration would double-fire next to the namespaced one.
  foreach ($legacyName in $script:DevTaskLegacyNames) {
    $legacyTask = Get-ScheduledTask -TaskName $legacyName -TaskPath '\' -ErrorAction SilentlyContinue
    if ($legacyTask) {
      Unregister-ScheduledTask -TaskName $legacyTask.TaskName -TaskPath $legacyTask.TaskPath -Confirm:$false
      Write-Host "Removed legacy task: \$($legacyTask.TaskName)"
    }
  }

  $registered = Get-ScheduledTask -TaskName $definition.Name -TaskPath $script:DevTaskFolderQuery
  $info = Get-ScheduledTaskInfo -TaskName $definition.Name -TaskPath $script:DevTaskFolderQuery

  if ($existing -and $drift.Count -gt 0) {
    Write-Host "Re-registered (drift detected: $($drift -join ', ')): $($registered.TaskPath)$($registered.TaskName)"
  } else {
    Write-Host "Registered: $($registered.TaskPath)$($registered.TaskName) [$($registered.State)]"
  }
  Write-Host "Interval: every $IntervalMinutes min, lifetime $LifetimeDays day(s), hidden: $($registered.Settings.Hidden)"
  Write-Host "Next run: $($info.NextRunTime)"
  Write-Host "Log: $(Join-Path $definition.StateDir 'status.log')"
  Write-Host "Remove: powershell -ExecutionPolicy Bypass -File scripts/dev-tasks.ps1 -Unregister $($definition.Id) -Purge"

  return $registered
}

function Uninstall-DevTask {
  <#
  .SYNOPSIS
    Remove a developer task, including any legacy root registration.
  .DESCRIPTION
    Purge deletes the task's state directory as well. The application uninstall
    never touched these paths, so removing the task is the only cleanup step.
  #>
  param(
    [Parameter(Mandatory)][string]$Id,
    [switch]$Purge,
    [switch]$KeepState
  )

  $definition = Get-DevTaskDefinition -Id $Id
  $removed = 0

  $task = Get-ScheduledTask -TaskName $definition.Name -TaskPath $script:DevTaskFolderQuery -ErrorAction SilentlyContinue
  if ($task) {
    Unregister-ScheduledTask -TaskName $task.TaskName -TaskPath $task.TaskPath -Confirm:$false
    Write-Host "Removed task: $($task.TaskPath)$($task.TaskName)"
    $removed++
  }

  foreach ($legacyName in $script:DevTaskLegacyNames) {
    $legacyTask = Get-ScheduledTask -TaskName $legacyName -TaskPath '\' -ErrorAction SilentlyContinue
    if ($legacyTask) {
      Unregister-ScheduledTask -TaskName $legacyTask.TaskName -TaskPath $legacyTask.TaskPath -Confirm:$false
      Write-Host "Removed legacy task: \$($legacyTask.TaskName)"
      $removed++
    }
  }

  if ($removed -eq 0) { Write-Host "No task registered for $Id" }

  if ($Purge -and -not $KeepState) {
    if (Test-Path $definition.StateDir) {
      $size = (Get-ChildItem $definition.StateDir -Recurse -File -ErrorAction SilentlyContinue |
        Measure-Object Length -Sum).Sum
      Remove-Item $definition.StateDir -Recurse -Force
      Write-Host "Purged state: $($definition.StateDir) ($([math]::Round($size / 1KB, 1)) KB)"
    }
  } elseif (-not $KeepState) {
    if (Test-Path $definition.StateDir) {
      Write-Host "State kept: $($definition.StateDir) (use -Purge to delete)"
    }
  }

  return $removed
}

function Get-DevTaskInventory {
  <#
  .SYNOPSIS
    Enumerate every developer task and the on-disk state it owns.
  .DESCRIPTION
    Produces one row per catalog entry with the live registration (if any) plus
    an explicit flag list, so "what is running in the background" is a single
    command instead of a guess. Flags:
        legacy      registered in the task root instead of \VolumeControl-Dev\
        drift       arguments or hidden flag no longer match the catalog
        visible     window would be shown (not hidden, or no -WindowStyle Hidden)
        unbounded   repetition lifetime longer than the catalog value
        expired     repetition lifetime has elapsed; it will never fire again
        orphan      state directory exists but no task is registered
        missing     task registered but checker script is gone
  #>
  $rows = foreach ($definition in $script:DevTaskCatalog) {
    $task = Get-DevTaskRegistration -Definition $definition
    $info = $null
    $interval = $null
    $duration = $null
    $checker = Join-Path (Split-Path $PSScriptRoot -Parent) $definition.Checker
    $flags = New-Object System.Collections.Generic.List[string]

    if ($task) {
      $info = Get-ScheduledTaskInfo -TaskName $task.TaskName -TaskPath $task.TaskPath -ErrorAction SilentlyContinue
      $arguments = [string]$task.Actions[0].Arguments
      if ($task.TaskPath -ne $script:DevTaskFolderQuery) { $flags.Add('legacy') }
      if ($arguments -ne (Get-DevTaskArguments -Definition $definition)) { $flags.Add('drift') }
      if (-not $task.Settings.Hidden -or $arguments -notmatch '-WindowStyle\s+Hidden') { $flags.Add('visible') }
      if (-not (Test-Path $checker -PathType Leaf)) { $flags.Add('missing') }

      $interval = $null
      $duration = $null
      foreach ($trigger in $task.Triggers) {
        if ($trigger.Repetition -and $trigger.Repetition.Interval) { $interval = $trigger.Repetition.Interval }
        if ($trigger.Repetition -and $trigger.Repetition.Duration) { $duration = $trigger.Repetition.Duration }
      }
      if ($duration) {
        # Reject anything beyond the catalog's intent; ISO8601 duration strings
        # from the scheduler are normalized through TimeSpan.
        try {
          $span = [System.Xml.XmlConvert]::ToTimeSpan($duration)
          if ($span.TotalDays -gt $definition.LifetimeDays) { $flags.Add('unbounded') }
        } catch {
          $flags.Add('unbounded')
        }
      }
      if ($info -and $info.NextRunTime -eq [DateTime]::MinValue -and $task.State -ne 'Disabled') { $flags.Add('expired') }
    } elseif (Test-Path $definition.StateDir) {
      $flags.Add('orphan')
    }

    $stateDir = $definition.StateDir
    $stateSize = 0
    if (Test-Path $stateDir) {
      $stateSize = (Get-ChildItem $stateDir -Recurse -File -ErrorAction SilentlyContinue |
        Measure-Object Length -Sum).Sum
    }

    [pscustomobject]@{
      Id           = $definition.Id
      Task         = if ($task) { "$($task.TaskPath)$($task.TaskName)" } else { "(not registered)" }
      State        = if ($task) { [string]$task.State } else { '-' }
      Hidden       = if ($task) { [string]$task.Settings.Hidden } else { '-' }
      EveryMinutes = if ($interval) { $interval } else { '-' }
      Lifetime     = if ($duration) { $duration } else { '-' }
      LastRun      = if ($info) { $info.LastRunTime } else { '-' }
      NextRun      = if ($info) { $info.NextRunTime } else { '-' }
      StateKB      = [math]::Round($stateSize / 1KB, 1)
      Flags        = if ($flags.Count -gt 0) { $flags -join ',' } else { 'ok' }
    }
  }

  # Unattributed leftovers: root-folder tasks from the pre-namespace installers.
  foreach ($legacyName in $script:DevTaskLegacyNames) {
    $task = Get-ScheduledTask -TaskName $legacyName -TaskPath '\' -ErrorAction SilentlyContinue
    if ($task) {
      $info = Get-ScheduledTaskInfo -TaskName $task.TaskName -TaskPath $task.TaskPath -ErrorAction SilentlyContinue
      [pscustomobject]@{
        Id           = "(legacy) $legacyName"
        Task         = '\' + $task.TaskName
        State        = [string]$task.State
        Hidden       = [string]$task.Settings.Hidden
        EveryMinutes = '-'
        Lifetime     = '-'
        LastRun      = if ($info) { $info.LastRunTime } else { '-' }
        NextRun      = if ($info) { $info.NextRunTime } else { '-' }
        StateKB      = 0
        Flags        = 'legacy'
      }
    }
  }

  return $rows
}
