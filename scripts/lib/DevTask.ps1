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
    * registered under the \VolumeControl-Dev\ task folder, with a description
      marker so it stays attributable even if the folder is renamed,
    * started through scripts/lib/run-hidden.vbs (a GUI-subsystem host) with
      window style 0, so no console window is ever created visibly,
    * bounded in lifetime instead of a decade-long repetition,
    * unregistered (not merely disabled) when the watched condition is terminal.

  The task itself stays VISIBLE in the Task Scheduler UI on purpose. Hiding it
  there is what makes background work undiscoverable; discoverability is the
  point of this module and of scripts/dev-tasks.ps1.

  Dot-source it:
      . "$PSScriptRoot/lib/DevTask.ps1"
#>

$script:DevTaskFolder = '\VolumeControl-Dev'
# Get-ScheduledTask/Get-ScheduledTaskInfo only match a folder path with the
# trailing separator; registration accepts either form.
$script:DevTaskFolderQuery = '\VolumeControl-Dev\'
$script:DevTaskMarker = 'volume-control-dev-task'
# Upper bound accepted by the installers. The inventory flags anything above it
# as `unbounded`, so the two cannot disagree about what a valid lifetime is.
$script:DevTaskMaxLifetimeDays = 30
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

function Get-DevTaskLegacyName {
  <#
  .SYNOPSIS
    Leaf name used by the pre-namespace installers for one definition.
  .DESCRIPTION
    Scoped to a single definition: installing one watcher must never remove the
    legacy registration of a different watcher.
  #>
  param([Parameter(Mandatory)]$Definition)
  return "VolumeControl-$($Definition.Name)"
}

function Get-DevTaskDescription {
  <#
  .SYNOPSIS
    Build the marker description so an inventory can attribute a task's owner.
  #>
  param([Parameter(Mandatory)]$Definition)

  return "$($script:DevTaskMarker)|id=$($Definition.Id)|$($Definition.Description)"
}

function Get-DevTaskLauncher {
  <#
  .SYNOPSIS
    Resolve the launcher for a task action.
  .DESCRIPTION
    Prefers scripts/lib/run-hidden.vbs, run by wscript.exe: that host has no
    console attached, so the child's console window is created already hidden
    and no frame is ever painted. `powershell.exe -WindowStyle Hidden` is the
    fallback when wscript.exe is unavailable (the window is created first and
    hidden once PowerShell parses its arguments, which can flash).
  #>
  $vbs = Join-Path $PSScriptRoot 'run-hidden.vbs'
  $wscript = Join-Path $env:SystemRoot 'System32\wscript.exe'
  $powershell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'

  if ((Test-Path $vbs -PathType Leaf) -and (Test-Path $wscript -PathType Leaf)) {
    return [pscustomobject]@{ Execute = $wscript; Kind = 'wscript' }
  }
  return [pscustomobject]@{ Execute = $powershell; Kind = 'powershell-windowstyle' }
}

function Get-DevTaskAction {
  <#
  .SYNOPSIS
    Build the scheduled task action for a definition.
  #>
  param([Parameter(Mandatory)]$Definition, [string]$WorkingDirectory = '')

  $checker = Join-Path (Split-Path $PSScriptRoot -Parent) $Definition.Checker
  $extra = @($Definition.CheckerArgs) + @('-TaskName', $Definition.Name, '-TaskPath', $script:DevTaskFolder)
  $launcher = Get-DevTaskLauncher

  # The launcher arguments are quoted for wscript, so the checker path and the
  # checker switches travel as separate quoted argv entries.
  if ($launcher.Kind -eq 'wscript') {
    $vbs = Join-Path $PSScriptRoot 'run-hidden.vbs'
    $powershell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
    $quoted = @($vbs, $powershell, '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $checker) + $extra
    $argument = '//nologo ' + (($quoted | ForEach-Object { "`"$_`"" }) -join ' ')
  } else {
    $argument = "-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File `"$checker`" $($extra -join ' ')"
  }

  $params = @{ Execute = $launcher.Execute; Argument = $argument }
  if ($WorkingDirectory) { $params.WorkingDirectory = $WorkingDirectory }
  return New-ScheduledTaskAction @params
}

function Find-DevTaskRegistrations {
  <#
  .SYNOPSIS
    Every registration this repository owns, wherever it currently lives.
  .DESCRIPTION
    Enumerates all scheduled tasks and matches on (a) the description marker,
    (b) the namespaced name, and (c) the legacy root name. The marker is what
    keeps a task attributable after a folder rename or a hand-moved task; an
    exact-name lookup alone would report "not registered" and leave it running.
  #>
  param([Parameter(Mandatory)]$Definition)

  $marker = "$($script:DevTaskMarker)|id=$($Definition.Id)"
  $wanted = @($Definition.Name, (Get-DevTaskLegacyName -Definition $Definition))

  $results = New-Object System.Collections.Generic.List[object]
  $seen = @{}

  $all = Get-ScheduledTask -ErrorAction SilentlyContinue
  foreach ($task in $all) {
    $description = [string]$task.Description
    $match = $description.StartsWith($marker, [System.StringComparison]::Ordinal) -or
      ($wanted -contains $task.TaskName -and ($task.TaskPath -eq $script:DevTaskFolderQuery -or $task.TaskPath -eq '\'))
    if (-not $match) { continue }

    $key = "$($task.TaskPath)$($task.TaskName)"
    if ($seen.ContainsKey($key)) { continue }
    $seen[$key] = $true
    $results.Add($task)
  }

  return $results
}

function Get-DevTaskRegistration {
  <#
  .SYNOPSIS
    The single registration that owns a definition, preferring the namespaced one.
  #>
  param([Parameter(Mandatory)]$Definition)

  $all = @(Find-DevTaskRegistrations -Definition $Definition)
  if ($all.Count -eq 0) { return $null }
  foreach ($task in $all) {
    if ($task.TaskPath -eq $script:DevTaskFolderQuery) { return $task }
  }
  return $all[0]
}

function Remove-DevTaskRegistration {
  <#
  .SYNOPSIS
    Remove every registration of one definition, namespaced and legacy.
  #>
  param([Parameter(Mandatory)]$Definition)

  $removed = 0
  foreach ($task in (Find-DevTaskRegistrations -Definition $Definition)) {
    Unregister-ScheduledTask -TaskName $task.TaskName -TaskPath $task.TaskPath -Confirm:$false
    Write-Host "Removed task: $($task.TaskPath)$($task.TaskName)"
    $removed++
  }
  return $removed
}

function Install-DevTask {
  <#
  .SYNOPSIS
    Register (or repair) a developer task so it matches the catalog.
  .DESCRIPTION
    Registration is idempotent and self-correcting: an existing task whose
    launcher, arguments, hidden flag or folder no longer match the catalog is
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
  if ($LifetimeDays -gt $script:DevTaskMaxLifetimeDays) {
    throw "LifetimeDays $LifetimeDays exceeds the supported maximum of $($script:DevTaskMaxLifetimeDays) days; a watcher must not outlive a release cycle. Use -LifetimeDays $($script:DevTaskMaxLifetimeDays) or less."
  }

  $checker = Join-Path (Split-Path $PSScriptRoot -Parent) $definition.Checker
  if (-not (Test-Path $checker -PathType Leaf)) { throw "Checker not found: $checker" }

  $repo = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
  $desiredAction = Get-DevTaskAction -Definition $definition -WorkingDirectory $repo

  $trigger = New-ScheduledTaskTrigger -Once -At ((Get-Date).AddMinutes(1)) `
    -RepetitionInterval (New-TimeSpan -Minutes $IntervalMinutes) `
    -RepetitionDuration (New-TimeSpan -Days $LifetimeDays)
  # No -Hidden: the task stays visible in the Task Scheduler UI so a developer
  # can find it. The console window is suppressed by the launcher instead.
  $settings = New-ScheduledTaskSettingsSet -StartWhenAvailable `
    -MultipleInstances IgnoreNew `
    -ExecutionTimeLimit (New-TimeSpan -Minutes $definition.TimeLimitMin) `
    -AllowStartIfOnBatteries `
    -DontStopIfGoingOnBatteries
  $principal = New-ScheduledTaskPrincipal -UserId "$env:USERDOMAIN\$env:USERNAME" `
    -LogonType Interactive -RunLevel Limited

  $existing = Get-DevTaskRegistration -Definition $definition
  $drift = @()
  if ($existing) {
    if ($existing.Actions[0].Execute -ne $desiredAction.Execute) { $drift += 'launcher' }
    if ($existing.Actions[0].Arguments -ne $desiredAction.Arguments) { $drift += 'arguments' }
    if ($existing.Settings.Hidden) { $drift += 'task hidden in UI' }
    if ($existing.TaskPath -ne $script:DevTaskFolderQuery) { $drift += 'taskpath' }
  }

  Register-ScheduledTask -TaskName $definition.Name `
    -TaskPath $script:DevTaskFolder `
    -Action $desiredAction `
    -Trigger $trigger `
    -Settings $settings `
    -Principal $principal `
    -Description (Get-DevTaskDescription -Definition $definition) `
    -Force | Out-Null

  # Only this definition's legacy registration: removing another watcher's
  # legacy task would silently stop that watcher's polling.
  $legacyName = Get-DevTaskLegacyName -Definition $definition
  $legacyTask = Get-ScheduledTask -TaskName $legacyName -TaskPath '\' -ErrorAction SilentlyContinue
  if ($legacyTask) {
    Unregister-ScheduledTask -TaskName $legacyTask.TaskName -TaskPath $legacyTask.TaskPath -Confirm:$false
    Write-Host "Removed legacy task: \$($legacyTask.TaskName)"
  }

  $registered = Get-ScheduledTask -TaskName $definition.Name -TaskPath $script:DevTaskFolderQuery
  $info = Get-ScheduledTaskInfo -TaskName $definition.Name -TaskPath $script:DevTaskFolderQuery

  if ($existing -and $drift.Count -gt 0) {
    Write-Host "Re-registered (drift detected: $($drift -join ', ')): $($registered.TaskPath)$($registered.TaskName)"
  } else {
    Write-Host "Registered: $($registered.TaskPath)$($registered.TaskName) [$($registered.State)]"
  }
  Write-Host "Every $IntervalMinutes min, lifetime $LifetimeDays day(s), launcher: $(if ((Get-DevTaskLauncher).Kind -eq 'wscript') { 'wscript (no console window)' } else { 'powershell -WindowStyle Hidden' })"
  Write-Host "Next run: $($info.NextRunTime)"
  Write-Host "Log: $(Join-Path $definition.StateDir 'status.log')"
  Write-Host "Inspect: powershell -ExecutionPolicy Bypass -File scripts/dev-tasks.ps1"
  Write-Host "Remove:  powershell -ExecutionPolicy Bypass -File scripts/dev-tasks.ps1 -Unregister $($definition.Id) -Purge"

  return $registered
}

function Uninstall-DevTask {
  <#
  .SYNOPSIS
    Remove a developer task, including any legacy or moved registration.
  .DESCRIPTION
    Only the selected definition is touched. Purge deletes the task's state
    directory as well; the application uninstall never touched these paths, so
    removing the task is the only cleanup step.
  #>
  param(
    [Parameter(Mandatory)][string]$Id,
    [switch]$Purge,
    [switch]$KeepState
  )

  $definition = Get-DevTaskDefinition -Id $Id
  $removed = Remove-DevTaskRegistration -Definition $definition
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
    Produces one row per registration (never the same physical task twice) plus
    one row per catalog entry with no registration but leftover state. Flags:
        legacy      registered in the task root or a stale folder
        drift       launcher/arguments no longer match the catalog
        visible     the launch mechanism would show a console window
        unbounded   repetition lifetime missing or longer than the catalog value
        expired     repetition lifetime elapsed; it will never fire again
        orphan      state directory exists but no task is registered
        missing     task registered but the checker script is gone
  #>
  $rows = New-Object System.Collections.Generic.List[object]
  $assigned = @{}

  foreach ($definition in $script:DevTaskCatalog) {
    $checker = Join-Path (Split-Path $PSScriptRoot -Parent) $definition.Checker
    $desired = Get-DevTaskAction -Definition $definition
    $registrations = @(Find-DevTaskRegistrations -Definition $definition)

    foreach ($task in $registrations) {
      $key = "$($task.TaskPath)$($task.TaskName)"
      $assigned[$key] = $true

      $info = Get-ScheduledTaskInfo -TaskName $task.TaskName -TaskPath $task.TaskPath -ErrorAction SilentlyContinue
      $arguments = [string]$task.Actions[0].Arguments
      $execute = [string]$task.Actions[0].Execute
      $flags = New-Object System.Collections.Generic.List[string]

      if ($task.TaskPath -ne $script:DevTaskFolderQuery) { $flags.Add('legacy') }
      if ($execute -ne $desired.Execute) { $flags.Add('drift') }
      elseif ($arguments -ne $desired.Arguments) { $flags.Add('drift') }
      # Visible means: the launcher is plain powershell without -WindowStyle
      # Hidden, or anything else that allocates a console it will show.
      $hiddenLauncher = ($execute -match '(?i)wscript\.exe$') -or ($arguments -match '-WindowStyle\s+Hidden')
      if (-not $hiddenLauncher) { $flags.Add('visible') }
      # A registration hidden in the Task Scheduler UI is undiscoverable by
      # design-breaking drift, even though the launcher still suppresses the
      # console window.
      if ($task.Settings.Hidden) { $flags.Add('hiddenUI') }
      if (-not (Test-Path $checker -PathType Leaf)) { $flags.Add('missing') }

      $interval = $null
      $durationFound = $false
      $unbounded = $false
      foreach ($trigger in $task.Triggers) {
        if (-not $trigger.Repetition) { continue }
        if ($trigger.Repetition.Interval) { $interval = $trigger.Repetition.Interval }
        if ($trigger.Repetition.Duration) {
          $durationFound = $true
          try {
            $span = [System.Xml.XmlConvert]::ToTimeSpan([string]$trigger.Repetition.Duration)
            if ($span.TotalDays -gt $script:DevTaskMaxLifetimeDays) { $unbounded = $true }
          } catch {
            $unbounded = $true
          }
        }
      }
      # Task Scheduler treats an omitted repetition duration as "repeat
      # indefinitely", so a missing duration on a repeating trigger is exactly
      # the unbounded case this audit exists to catch.
      if ($interval -and -not $durationFound) { $unbounded = $true }
      if ($unbounded) { $flags.Add('unbounded') }
      if ($info -and $info.NextRunTime -eq [DateTime]::MinValue -and $task.State -ne 'Disabled') { $flags.Add('expired') }

      $stateSize = 0
      if (Test-Path $definition.StateDir) {
        $stateSize = (Get-ChildItem $definition.StateDir -Recurse -File -ErrorAction SilentlyContinue |
          Measure-Object Length -Sum).Sum
      }

      $rows.Add([pscustomobject]@{
        Id           = $definition.Id
        Task         = "$($task.TaskPath)$($task.TaskName)"
        State        = [string]$task.State
        HiddenInUI   = [string]$task.Settings.Hidden
        EveryMinutes = if ($interval) { $interval } else { '-' }
        LastRun      = if ($info) { $info.LastRunTime } else { '-' }
        NextRun      = if ($info) { $info.NextRunTime } else { '-' }
        StateKB      = [math]::Round($stateSize / 1KB, 1)
        Flags        = if ($flags.Count -gt 0) { $flags -join ',' } else { 'ok' }
      })
    }

    if ($registrations.Count -eq 0 -and (Test-Path $definition.StateDir)) {
      $stateSize = (Get-ChildItem $definition.StateDir -Recurse -File -ErrorAction SilentlyContinue |
        Measure-Object Length -Sum).Sum
      $rows.Add([pscustomobject]@{
        Id           = $definition.Id
        Task         = '(not registered)'
        State        = '-'
        HiddenInUI   = '-'
        EveryMinutes = '-'
        LastRun      = '-'
        NextRun      = '-'
        StateKB      = [math]::Round($stateSize / 1KB, 1)
        Flags        = 'orphan'
      })
    } elseif ($registrations.Count -eq 0) {
      $rows.Add([pscustomobject]@{
        Id           = $definition.Id
        Task         = '(not registered)'
        State        = '-'
        HiddenInUI   = '-'
        EveryMinutes = '-'
        LastRun      = '-'
        NextRun      = '-'
        StateKB      = 0
        Flags        = 'ok'
      })
    }
  }

  # Marked tasks that no catalog entry claimed (renamed id, moved folder, or a
  # definition that was removed from the catalog while its task kept running).
  foreach ($task in (Get-ScheduledTask -ErrorAction SilentlyContinue)) {
    if (-not ([string]$task.Description).StartsWith($script:DevTaskMarker, [System.StringComparison]::Ordinal)) { continue }
    $key = "$($task.TaskPath)$($task.TaskName)"
    if ($assigned.ContainsKey($key)) { continue }

    $info = Get-ScheduledTaskInfo -TaskName $task.TaskName -TaskPath $task.TaskPath -ErrorAction SilentlyContinue
    $rows.Add([pscustomobject]@{
      Id           = '(unclaimed marker)'
      Task         = $key
      State        = [string]$task.State
      HiddenInUI   = [string]$task.Settings.Hidden
      EveryMinutes = '-'
      LastRun      = if ($info) { $info.LastRunTime } else { '-' }
      NextRun      = if ($info) { $info.NextRunTime } else { '-' }
      StateKB      = 0
      Flags        = 'unclaimed'
    })
  }

  return $rows
}
