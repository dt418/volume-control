#!/usr/bin/env pwsh
<#
.SYNOPSIS
  Show, verify, or remove every VolumeControl developer task.

.DESCRIPTION
  Developer watchers run as Windows scheduled tasks, outside the application.
  They survive an app uninstall and never appear in the app UI, so this is the
  one command that answers "what is this repository leaving running on my
  machine?".

  Default action is an inventory. Nothing is changed unless -Unregister is
  passed.

.PARAMETER Id
  Catalog id or task leaf name: winget-merge, winget-validation. Omit to act on
  every catalog entry.

.PARAMETER Verify
  Exit 1 when any task is flagged (legacy, drift, visible, unbounded, expired,
  orphan, missing). Use this in a release checklist or CI job.

.PARAMETER Unregister
  Remove the matching task(s), including any legacy root registration.

.PARAMETER Purge
  With -Unregister, also delete the task's state directory
  (%LOCALAPPDATA%\VolumeControl\<id>).

.PARAMETER Json
  Emit the inventory as JSON instead of a table.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/dev-tasks.ps1

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/dev-tasks.ps1 -Verify

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/dev-tasks.ps1 -Unregister -Purge
#>
[CmdletBinding()]
param(
  [string]$Id,
  [switch]$Verify,
  [switch]$Unregister,
  [switch]$Purge,
  [switch]$Json
)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'lib/DevTask.ps1')

function Resolve-DevTaskIds {
  if ($Id) { return @((Get-DevTaskDefinition -Id $Id).Id) }
  return @(Get-DevTaskCatalog | ForEach-Object { $_.Id })
}

if ($Unregister) {
  $removed = 0
  foreach ($taskId in (Resolve-DevTaskIds)) {
    $removed += Uninstall-DevTask -Id $taskId -Purge:$Purge
  }
  Write-Host "Removed $removed task registration(s)."
  if (-not $Purge) {
    Write-Host 'State directories are kept on purpose; add -Purge to delete them.'
  }
  exit 0
}

$inventory = @(Get-DevTaskInventory)
$flagged = @($inventory | Where-Object { $_.Flags -ne 'ok' })

if ($Json) {
  $inventory | ConvertTo-Json -Depth 4
} else {
  $inventory | Format-Table Id, Task, State, Hidden, EveryMinutes, Lifetime, Flags -AutoSize -Wrap

  $registered = @($inventory | Where-Object { $_.Task -ne '(not registered)' })
  Write-Host ("Tasks registered: {0} of {1} catalog entries. Flagged: {2}." -f
    $registered.Count, $inventory.Count, $flagged.Count)

  if ($flagged.Count -gt 0) {
    Write-Host ''
    Write-Host 'Flags:'
    foreach ($row in $flagged) {
      Write-Host ("  {0,-18} {1}" -f $row.Id, $row.Flags)
    }
    Write-Host ''
    Write-Host 'Remediate: powershell -ExecutionPolicy Bypass -File scripts/dev-tasks.ps1 -Unregister -Purge'
    Write-Host 'Re-register when needed: see scripts/install-winget-merge-task.ps1 -?'
  }
}

if ($Verify -and $flagged.Count -gt 0) { exit 1 }
exit 0
