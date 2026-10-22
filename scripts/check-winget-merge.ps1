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
  [string]$TaskName = "VolumeControl-WingetMergeWatch",
  [switch]$StopOnTerminal
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

function Disable-WatchTask {
  if (-not $StopOnTerminal) { return }
  try {
    $task = Get-ScheduledTask -TaskName $TaskName -ErrorAction Stop
    if ($task.State -ne "Disabled") {
      Disable-ScheduledTask -TaskName $TaskName | Out-Null
      Add-Content -Path $logPath -Value "$(Get-Date -Format o) disabled task $TaskName"
    }
  } catch {
    Add-Content -Path $logPath -Value "$(Get-Date -Format o) WARN could not disable task ${TaskName}: $($_.Exception.Message)"
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
  $raw = & $gh pr view $PrNumber --repo microsoft/winget-pkgs `
    --json state,mergedAt,mergeStateStatus,url,title,labels 2>&1
  if ($LASTEXITCODE -ne 0) { throw "gh pr view failed ($LASTEXITCODE): $raw" }
  $pr = $raw | ConvertFrom-Json

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
    Disable-WatchTask
  }
  exit 0
} catch {
  Add-Content -Path $logPath -Value "$(Get-Date -Format o) ERROR $($_.Exception.Message)"
  Write-Error $_
  exit 1
}
