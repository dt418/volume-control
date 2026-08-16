[CmdletBinding()]
param(
  [string]$Binary = (Join-Path $PSScriptRoot "..\target\debug\VolumeControl.exe"),
  [string]$OutputRoot = (Join-Path $PSScriptRoot "..\output\tauri-e2e"),
  [ValidateSet("all", "mixer", "runtime", "windows", "recovery", "settings", "help")]
  [string]$Surface = "all",
  [switch]$SkipBuild,
  [switch]$VirtualAudio
)

$ErrorActionPreference = "Stop"
$repo = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$e2e = Join-Path $repo "e2e\tauri"
$providerWasSet = [Environment]::GetEnvironmentVariable("E2E_DRIVER_PROVIDER", "Process")
$providerSetByWrapper = $false
if ([string]::IsNullOrWhiteSpace($providerWasSet)) {
  $env:E2E_DRIVER_PROVIDER = "embedded"
  $providerSetByWrapper = $true
}

if (-not (Get-Command node -ErrorAction SilentlyContinue)) { throw "node is required" }
if (-not (Test-Path (Join-Path $e2e "node_modules\@wdio\cli\bin\wdio.js"))) {
  throw "E2E dependencies are missing; run npm install --prefix e2e/tauri"
}

# Zombie cleanup: a force-killed previous run (or a crashed app) can leave a
# VolumeControl.exe holding the embedded WebDriver port 4445 and a stale HUD
# on the desktop. Kill leftovers before the run so the app under test always
# binds cleanly; the post-run cleanup below repeats it in the finally block.
Get-Process -Name "VolumeControl" -ErrorAction SilentlyContinue |
  Stop-Process -Force -ErrorAction SilentlyContinue

if (-not $SkipBuild) {
  & node (Join-Path $e2e "prepare-debug-frontend.mjs") -- node (Join-Path $e2e "prepare-debug-capabilities.mjs") --provider wdio -- cargo build -p volumecontrol-tauri --no-default-features --features e2e-wdio
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
if (-not (Test-Path $Binary)) { throw "debug E2E binary does not exist: $Binary" }

$env:TAURI_E2E_BINARY = (Resolve-Path $Binary).Path
$baseOutputRoot = (Resolve-Path (New-Item -ItemType Directory -Force -Path $OutputRoot)).Path
$runId = "run-{0}-{1}" -f (Get-Date -AsUTC -Format "yyyyMMddTHHmmssfffZ"), ([guid]::NewGuid().ToString("N"))
$runOutputRoot = (Resolve-Path (New-Item -ItemType Directory -Force -Path (Join-Path $baseOutputRoot $runId))).Path
$env:TAURI_E2E_OUTPUT = $runOutputRoot
$env:TAURI_E2E_RUN_ID = $runId
$expectedSpecs = switch ($Surface) {
  "all" { @("mixer.e2e.ts", "runtime.e2e.ts", "windows.e2e.ts", "recovery.e2e.ts", "settings.e2e.ts", "help.e2e.ts") }
  default { @("$Surface.e2e.ts") }
}
# PowerShell unwraps a single-output switch into a scalar; splatting a scalar
# string would enumerate its characters ("h","e","l",...) and break the
# evidence contract for single-surface runs.
$expectedSpecs = @($expectedSpecs)
$code = 1
$evidenceCode = 0
try {
  $e2eArgs = @("--surface", $Surface)
  if ($VirtualAudio) { $e2eArgs += "--virtual-audio" }
  # Re-kill immediately before the app launches: a long pre-build above can
  # race with a user-launched instance that would otherwise hold the embedded
  # WebDriver port 4445 (or the shared WebView2 state) when WDIO starts.
  Get-Process -Name "VolumeControl" -ErrorAction SilentlyContinue |
    Stop-Process -Force -ErrorAction SilentlyContinue
  & npm --prefix $e2e run test:e2e:debug -- @e2eArgs
  $code = $LASTEXITCODE
  if ($code -eq 0) {
    # Resolve tsx from the isolated E2E installation. The CI runner does not
    # install a repository-root node_modules, so `node --import tsx` fails
    # unless this check runs with the E2E package as its working directory.
    Push-Location $e2e
    try {
      & node --import tsx --input-type=module -e "import { assertE2eEvidence } from './support/artifacts.ts'; const [root, runId, ...expected] = process.argv.slice(1); await assertE2eEvidence(root, expected, runId);" $env:TAURI_E2E_OUTPUT $env:TAURI_E2E_RUN_ID @expectedSpecs
      $evidenceCode = $LASTEXITCODE
    } finally {
      Pop-Location
    }
  }
} finally {
  Get-Process -Name "VolumeControl" -ErrorAction SilentlyContinue |
    Stop-Process -Force -ErrorAction SilentlyContinue
  Remove-Item Env:TAURI_E2E_BINARY -ErrorAction SilentlyContinue
  Remove-Item Env:TAURI_E2E_OUTPUT -ErrorAction SilentlyContinue
  Remove-Item Env:TAURI_E2E_RUN_ID -ErrorAction SilentlyContinue
  if ($providerSetByWrapper) { Remove-Item Env:E2E_DRIVER_PROVIDER -ErrorAction SilentlyContinue }
}

if (Test-Path (Join-Path $repo "src-tauri\capabilities\e2e-wdio.json")) {
  Write-Error "temporary WDIO capability was not restored"
  $evidenceCode = 1
}
if (Test-Path (Join-Path $repo "frontend\dist\tauri-plugin.wdio.js")) {
  Write-Error "temporary guest bridge was not restored"
  $evidenceCode = 1
}
if ($code -eq 0 -and $evidenceCode -ne 0) { $code = $evidenceCode }
exit $code
