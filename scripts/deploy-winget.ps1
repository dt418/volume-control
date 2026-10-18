[CmdletBinding()]
param(
  [Parameter(Mandatory=$true, Position=0)]
  [string]$Tag,
  [Parameter(Mandatory=$true, Position=1)]
  [string]$Installer,
  [Parameter(Position=2)]
  [string]$OutDir = (Join-Path $PSScriptRoot "..\output\winget")
)

$ErrorActionPreference = "Stop"
$scriptDir = $PSScriptRoot
$repoRoot = (Resolve-Path (Join-Path $scriptDir "..")).Path
$bash = "C:\Program Files\Git\bin\bash.exe"

if (-not (Test-Path $bash)) {
  $bash = (Get-Command bash -ErrorAction SilentlyContinue).Source
}

if (-not (Test-Path $bash)) {
  throw "Git Bash is required to run deploy-winget"
}

$deployScript = Join-Path $scriptDir "deploy-winget.sh"
& $bash $deployScript $Tag $Installer $OutDir
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}
