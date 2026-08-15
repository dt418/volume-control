[CmdletBinding()]
param()

# Tauri deadlock guard (see .agents/skills/tauri-deadlock-guard/SKILL.md).
# Windows twin of scripts/check-tauri-deadlock.sh.

$ErrorActionPreference = "Stop"
$repo = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$failures = 0
function Fail([string]$Message) {
  Write-Error "FAIL: $Message"
  $script:failures = 1
}
function Ok([string]$Message) {
  Write-Output "ok: $Message"
}

$cargoToml = Get-Content (Join-Path $repo "src-tauri\Cargo.toml") -Raw
if ($cargoToml -match "custom-protocol") {
  Ok "tauri features include custom-protocol"
} else {
  Fail "src-tauri/Cargo.toml tauri features are missing custom-protocol"
}

$builderFiles = Get-ChildItem (Join-Path $repo "src-tauri\src") -Filter *.rs -Recurse |
  Where-Object { $_.Name -ne "window_manager.rs" } |
  Select-String -Pattern "WebviewWindowBuilder" -List
if ($builderFiles) {
  Fail "WebviewWindowBuilder used outside src-tauri/src/window_manager.rs"
} else {
  Ok "WebviewWindowBuilder confined to window_manager.rs"
}

$commands = Get-Content (Join-Path $repo "src-tauri\src\commands.rs") -Raw
foreach ($cmd in @("open_surface", "close_surface", "surface_ready")) {
  if ($commands -match "pub async fn ${cmd}\(") {
    Ok "command ${cmd} is async"
  } else {
    Fail "command ${cmd} must be declared async"
  }
}

$windowManager = Get-Content (Join-Path $repo "src-tauri\src\window_manager.rs") -Raw
if ($windowManager -match "fn on_main") {
  Ok "WindowManager provides on_main marshalling"
} else {
  Fail "WindowManager is missing on_main marshalling"
}
foreach ($wrapper in @("pub fn open(", "pub fn close(", "pub fn surface_ready(")) {
  if ($windowManager -match [regex]::Escape($wrapper)) {
    Ok "wrapper $wrapper present"
  } else {
    Fail "missing WindowManager wrapper $wrapper"
  }
}

if ($commands -match "run_on_main_thread" -and $commands -match "async_runtime::channel") {
  Ok "commands marshal via run_on_main_thread + async channel"
} else {
  Fail "surface commands must use run_on_main_thread + async_runtime::channel"
}

if ($failures -ne 0) {
  Write-Error "Tauri deadlock guard FAILED"
  exit 1
}
Write-Output "Tauri deadlock guard passed"
