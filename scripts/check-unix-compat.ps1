[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

$repo = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $repo

$failures = 0

function Fail([string]$msg) {
    Write-Host "FAIL: $msg" -ForegroundColor Red
    $script:failures = 1
}

function Pass([string]$msg) {
    Write-Host "ok: $msg"
}

# 1. Signal hook never_loop check
$signalHits = Get-ChildItem -Path "src-tauri/src", "crates/volumectl/src" -Recurse -Filter "*.rs" -ErrorAction SilentlyContinue |
    Select-String -Pattern "for .* in signals\.forever\(\)"
if ($signalHits) {
    Fail "found 'for ... in signals.forever()' loop; use 'if let Some(...) = signals.forever().next()' to prevent clippy::never_loop on Unix"
} else {
    Pass "signal handlers avoid clippy::never_loop pattern"
}

# 2. Check launch_prepared_apply parameter version suppression
if (Test-Path "src-tauri/src/lifecycle.rs") {
    $content = Get-Content "src-tauri/src/lifecycle.rs" -Raw
    if ($content -match "pub async fn launch_prepared_apply") {
        if ($content -match "(_version:\s*&str|let _ = version;)") {
            Pass "launch_prepared_apply version parameter properly suppressed for Unix builds"
        } else {
            Fail "launch_prepared_apply version parameter is unused on Unix and must be underscored (_version) or suppressed (let _ = version;)"
        }
    }
}

# 3. Check E2E Settings spec section isolation
if (Test-Path "e2e/tauri/specs/settings.e2e.ts") {
    $e2eContent = Get-Content "e2e/tauri/specs/settings.e2e.ts" -Raw
    if ($e2eContent -match "displays the updates card with version and check control[\s\S]*?General[\s\S]*?update-card") {
        Pass "settings.e2e.ts updates card test explicitly selects General section"
    } else {
        Fail "settings.e2e.ts updates card test must explicitly select General section before waiting for [data-testid='update-card']"
    }
}

if ($failures -ne 0) {
    Write-Error "Cross-platform Unix compatibility guard FAILED"
    exit 1
}

Write-Host "Cross-platform Unix compatibility guard passed"
exit 0
