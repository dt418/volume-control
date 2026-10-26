#!/usr/bin/env pwsh
<#
.SYNOPSIS
  Watch the VolumeControl winget PR for merge and verify catalog publication.

.DESCRIPTION
  Queries microsoft/winget-pkgs PR 454508. While open, logs status and exits.
  Once merged, updates the winget source, checks the catalog for the expected
  version, and disables its scheduled task when the package is visible. A merge
  can precede catalog publication by up to about an hour, so a merged-but-not-
  yet-visible package keeps the task running.

.PARAMETER PrNumber
  Upstream PR number. Default: 454508.

.PARAMETER StateDir
  Status directory. Default: %LOCALAPPDATA%\VolumeControl\winget-merge.

.PARAMETER TaskName
  Scheduled task to disable when terminal. Default: VolumeControl-WingetMergeWatch.

.PARAMETER StopOnTerminal
  Disable the task when the package is published or the PR closes unmerged.

.OUTPUTS
  Exit 0: pending, merged pending catalog, or published. Exit 1: error.
#>
[CmdletBinding()]
param(
  [int]$PrNumber = 454508,
  [string]$StateDir = (Join-Path $env:LOCALAPPDATA "VolumeControl\winget-merge"),
  [string]$TaskName = "WingetMergeWatch",
  [string]$TaskPath = "\VolumeControl-Dev",
  [switch]$StopOnTerminal,
  [switch]$KeepTask
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
    if ($candidate -and (Test-Path $candidate -PathType Leaf)) { return $candidate }
  }
  $command = Get-Command gh.exe -ErrorAction SilentlyContinue
  if ($command) { return $command.Source }
  throw "GitHub CLI (gh.exe) not found"
}

# Terminal state removes the task instead of leaving a disabled entry behind:
# a developer task outlives the application uninstall, so nothing else would
# ever clean it up. -KeepTask keeps a disabled registration for inspection.
function Stop-WatchTask {
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

  # A registration that was renamed or moved still carries this task's name in
  # its own arguments, so exact-name lookups miss it and the task would keep
  # firing forever. Match the action arguments instead: they embed the
  # -TaskName this checker was started with.
  foreach ($task in (Get-ScheduledTask -ErrorAction SilentlyContinue)) {
    $key = "$($task.TaskPath)$($task.TaskName)"
    if ($seen.ContainsKey($key)) { continue }
    $arguments = [string]$task.Actions[0].Arguments
    # Two argument shapes occur: `-TaskName Name` (plain powershell) and
    # `"-TaskName" "Name"` (the wscript launcher quotes every token).
    $pattern = '-TaskName"?\s+"?' + [regex]::Escape($TaskName) + '"?'
    if ($arguments -notmatch $pattern) { continue }
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

function Get-ExpectedVersion {
  param($Pr)
  if ($Pr.title -match 'version\s+(\d+\.\d+\.\d+)') { return $Matches[1] }
  $localRoot = Join-Path (Split-Path $PSScriptRoot -Parent) "winget\dt418.VolumeControl"
  $latest = Get-ChildItem $localRoot -Directory -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -match '^\d+\.\d+\.\d+$' } |
    Sort-Object { [version]$_.Name } -Descending |
    Select-Object -First 1
  if ($latest) { return $latest.Name }
  return $null
}

try {
  $gh = Get-GhPath
  $errFile = [System.IO.Path]::GetTempFileName()
  try {
    $raw = & $gh pr view $PrNumber --repo microsoft/winget-pkgs `
      --json state,mergedAt,mergeStateStatus,url,title,labels 2>$errFile
    if ($LASTEXITCODE -ne 0) {
      $err = Get-Content $errFile -Raw -ErrorAction SilentlyContinue
      throw "gh pr view failed ($LASTEXITCODE): $err"
    }
  } finally {
    Remove-Item $errFile -Force -ErrorAction SilentlyContinue
  }
  $pr = ($raw -join "`n") | ConvertFrom-Json

  $version = Get-ExpectedVersion $pr
  $labels = @($pr.labels | ForEach-Object { $_.name })
  $state = "pending"
  $catalogVersion = $null
  $catalogOutput = ""

  if ($pr.state -eq "MERGED" -or $pr.mergedAt) {
    $state = "merged_pending_catalog"
    # Publish pipeline can lag the merge by up to an hour; refresh on each run.
    $updateOutput = & winget source update 2>&1 | Out-String
    $showExit = 1
    $showOutput = ""
    if ($version) {
      # Query the exact version instead of parsing the localized `Version:`
      # label; a successful exact show is the catalog visibility proof.
      $showOutput = & winget show --id dt418.VolumeControl --exact `
        --version $version --source winget `
        --accept-source-agreements --disable-interactivity 2>&1 | Out-String
      $showExit = $LASTEXITCODE
    }
    $catalogOutput = ($updateOutput + "`n" + $showOutput).Trim()
    if ($showExit -eq 0 -and $version) {
      $state = "published"
      $catalogVersion = $version
    }
  } elseif ($pr.state -eq "CLOSED") {
    $state = "closed_not_merged"
  }

  $snapshot = [ordered]@{
    checkedAt      = (Get-Date).ToUniversalTime().ToString("o")
    pr             = $PrNumber
    state          = $state
    expectedVersion = $version
    catalogVersion = $catalogVersion
    labels         = $labels
    mergeState     = $pr.mergeStateStatus
    mergedAt       = $pr.mergedAt
    url            = $pr.url
    title          = $pr.title
    catalogOutput  = $catalogOutput
  }
  $snapshot | ConvertTo-Json -Depth 6 | Set-Content -Path $jsonPath -Encoding UTF8
  Add-Content -Path $logPath -Value ("{0} {1} expected={2} catalog={3}" -f $snapshot.checkedAt, $state, $version, $catalogVersion)

  if ($state -in @("published", "closed_not_merged")) {
    Stop-WatchTask
  }
  exit 0
} catch {
  Add-Content -Path $logPath -Value "$(Get-Date -Format o) ERROR $($_.Exception.Message)"
  Write-Error $_
  exit 1
}
