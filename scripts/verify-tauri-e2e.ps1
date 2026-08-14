[CmdletBinding()]
param(
  [string]$Binary = (Join-Path $PSScriptRoot "..\target\debug\VolumeControl.exe"),
  [string]$OutputRoot = (Join-Path $PSScriptRoot "..\output\tauri-e2e"),
  [ValidateSet("all", "mixer", "runtime", "windows", "recovery", "settings", "help")]
  [string]$Surface = "all",
  [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$repo = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$e2e = Join-Path $repo "e2e\tauri"

if (-not (Get-Command node -ErrorAction SilentlyContinue)) { throw "node is required" }
if (-not (Test-Path (Join-Path $e2e "node_modules\@wdio\cli\bin\wdio.js"))) {
  throw "E2E dependencies are missing; run npm install --prefix e2e/tauri"
}

if (-not $SkipBuild) {
  & node (Join-Path $e2e "prepare-debug-frontend.mjs") -- node (Join-Path $e2e "prepare-debug-capabilities.mjs") --provider wdio -- cargo build -p volumecontrol-tauri --no-default-features --features e2e-wdio
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
if (-not (Test-Path $Binary)) { throw "debug E2E binary does not exist: $Binary" }

$env:TAURI_E2E_BINARY = (Resolve-Path $Binary).Path
$env:TAURI_E2E_OUTPUT = (Resolve-Path (New-Item -ItemType Directory -Force -Path $OutputRoot)).Path
try {
  & npm --prefix $e2e run test:e2e:debug -- --surface $Surface
  $code = $LASTEXITCODE
} finally {
  Remove-Item Env:TAURI_E2E_BINARY -ErrorAction SilentlyContinue
  Remove-Item Env:TAURI_E2E_OUTPUT -ErrorAction SilentlyContinue
}

if (Test-Path (Join-Path $repo "src-tauri\capabilities\e2e-wdio.json")) { throw "temporary WDIO capability was not restored" }
if (Test-Path (Join-Path $repo "frontend\dist\tauri-plugin.wdio.js")) { throw "temporary guest bridge was not restored" }
exit $code
