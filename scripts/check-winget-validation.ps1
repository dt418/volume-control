#!/usr/bin/env pwsh
<#
.SYNOPSIS
  Check the VolumeControl winget PR validation state.

.DESCRIPTION
  Queries microsoft/winget-pkgs PR labels and writes a status snapshot plus an
  append-only log under %LOCALAPPDATA%\VolumeControl\winget-validation\.
  Intended for Task Scheduler.
  With -PingOnGreen, posts one short moderator ping when Validation-Completed
  appears, then can disable its own scheduled task.

.PARAMETER PrNumber
  winget-pkgs pull request number. Default: 454508.

.PARAMETER PingOnGreen
  Post a single moderator ping when Validation-Completed is present.

.PARAMETER StopOnTerminal
  Disable the scheduled task after green or red. Requires -TaskName.

.PARAMETER TaskName
  Scheduled task to disable. Default: VolumeControl-WingetValidation.

.PARAMETER StateDir
  Status output directory. Default: %LOCALAPPDATA%\VolumeControl\winget-validation.

.OUTPUTS
  Exit 0: green or still pending. Exit 1: validation failure. Exit 2: error.
#>
[CmdletBinding()]
param(
  [int]$PrNumber = 454508,
  [switch]$PingOnGreen,
  [switch]$StopOnTerminal,
  [string]$TaskName = "WingetValidation",
  [string]$TaskPath = "\VolumeControl-Dev",
  [switch]$KeepTask,
  [string]$StateDir = (Join-Path $env:LOCALAPPDATA "VolumeControl\winget-validation")
)

$ErrorActionPreference = "Stop"

$stateDir = [System.IO.Path]::GetFullPath($StateDir)
New-Item -ItemType Directory -Force -Path $stateDir | Out-Null
$logPath = Join-Path $stateDir "status.log"
$jsonPath = Join-Path $stateDir "status.json"

function Get-GhPath {
  $candidates = @(
    (Join-Path $env:ProgramFiles "GitHub CLI\gh.exe"),
    (Join-Path $env:LOCALAPPDATA "Programs\GitHub CLI\gh.exe")
  )
  if (${env:ProgramFiles(x86)}) {
    $candidates += Join-Path ${env:ProgramFiles(x86)} "GitHub CLI\gh.exe"
  }
  foreach ($candidate in $candidates) {
    if ($candidate -and (Test-Path $candidate -PathType Leaf)) {
      return $candidate
    }
  }
  $command = Get-Command gh.exe -ErrorAction SilentlyContinue
  if ($command) { return $command.Source }
  throw "GitHub CLI (gh.exe) not found"
}

# Terminal state removes the task instead of leaving a disabled entry behind:
# a developer task outlives the application uninstall, so nothing else would
# ever clean it up. -KeepTask keeps a disabled registration for inspection.
function Stop-CheckTask {
  if (-not $StopOnTerminal) { return }

  # Task Scheduler only matches a folder path that ends with the separator.
  if (-not $TaskPath.EndsWith('\')) { $TaskPath = "$TaskPath\" }

  $candidates = New-Object System.Collections.Generic.List[object]
  $seen = @{}
  $paths = @(
    @{ Name = $TaskName; Path = $TaskPath },
    @{ Name = $TaskName; Path = '\' },
    @{ Name = "VolumeControl-$TaskName"; Path = '\' }
  )
  foreach ($candidate in $paths) {
    $task = Get-ScheduledTask -TaskName $candidate.Name -TaskPath $candidate.Path -ErrorAction SilentlyContinue
    if (-not $task) { continue }
    $key = "$($task.TaskPath)$($task.TaskName)"
    if ($seen.ContainsKey($key)) { continue }
    $seen[$key] = $true
    $candidates.Add($task)
  }

  foreach ($task in $candidates) {
    $key = "$($task.TaskPath)$($task.TaskName)"
    try {
      if ($KeepTask) {
        if ($task.State -ne "Disabled") {
          Disable-ScheduledTask -TaskName $task.TaskName -TaskPath $task.TaskPath | Out-Null
        }
        Add-Content -Path $logPath -Value "$(Get-Date -Format o) disabled task $key"
      } else {
        Unregister-ScheduledTask -TaskName $task.TaskName -TaskPath $task.TaskPath -Confirm:$false
        Add-Content -Path $logPath -Value "$(Get-Date -Format o) unregistered task $key"
      }
    } catch {
      Add-Content -Path $logPath -Value "$(Get-Date -Format o) WARN could not remove task ${key}: $($_.Exception.Message)"
    }
  }
}

try {
  $gh = Get-GhPath
  $errFile = [System.IO.Path]::GetTempFileName()
  try {
    $raw = & $gh pr view $PrNumber --repo microsoft/winget-pkgs `
      --json labels,updatedAt,mergeStateStatus,url,comments 2>$errFile
    if ($LASTEXITCODE -ne 0) {
      $err = Get-Content $errFile -Raw -ErrorAction SilentlyContinue
      throw "gh pr view failed ($LASTEXITCODE): $err"
    }
  } finally {
    Remove-Item $errFile -Force -ErrorAction SilentlyContinue
  }
  $pr = ($raw -join "`n") | ConvertFrom-Json

  $labels = @($pr.labels | ForEach-Object { $_.name })
  $failure = @($labels | Where-Object {
      $_ -match 'Error|Needs-|Blocking-Issue|Changes-Requested|Policy-Test|Validation-(Domain|Unapproved|Indirect|Unattended|Installation|Shell|Executable|Uninstall|Defender|Hash|MSIX|VCRuntime)'
    })

  if ($failure.Count -gt 0) {
    $state = "red"
  } elseif ($labels -contains "Validation-Completed") {
    $state = "green"
  } else {
    $state = "pending"
  }

  $snapshot = [ordered]@{
    checkedAt      = (Get-Date).ToUniversalTime().ToString("o")
    pr             = $PrNumber
    state          = $state
    labels         = $labels
    failureLabels  = $failure
    mergeState     = $pr.mergeStateStatus
    updatedAt      = $pr.updatedAt
    url            = $pr.url
  }
  $snapshot | ConvertTo-Json -Depth 5 | Set-Content -Path $jsonPath -Encoding UTF8
  Add-Content -Path $logPath -Value ("{0} {1} labels={2}" -f $snapshot.checkedAt, $state, ($labels -join ","))

  if ($state -eq "green" -and $PingOnGreen) {
    $comments = @($pr.comments | ForEach-Object { $_.body }) -join "`n"
    if ($comments -notmatch 'winget-review-ping') {
      $bodyPath = Join-Path $stateDir "ping.md"
      @"
<!-- winget-review-ping -->
@ImJoakim @Trenly PR #$PrNumber is ready for review: new package, validation green, and local ``winget install --manifest`` evidence is posted above. Could one of you take a look when you have a moment?
"@ | Set-Content -Path $bodyPath -Encoding UTF8
      & $gh pr comment $PrNumber --repo microsoft/winget-pkgs --body-file $bodyPath
      if ($LASTEXITCODE -ne 0) {
        throw "gh pr comment failed ($LASTEXITCODE)"
      }
      Add-Content -Path $logPath -Value "$(Get-Date -Format o) posted moderator ping"
    }
  }

  if ($state -ne "pending") {
    Stop-CheckTask
  }

  if ($state -eq "red") { exit 1 }
  exit 0
} catch {
  Add-Content -Path $logPath -Value "$(Get-Date -Format o) ERROR $($_.Exception.Message)"
  Write-Error $_
  exit 2
}
