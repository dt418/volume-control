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
$signalHits = @()
$rsFiles = Get-ChildItem -Path "src-tauri/src", "crates/volumectl/src" -Recurse -Filter "*.rs" -ErrorAction SilentlyContinue
foreach ($f in $rsFiles) {
    $content = Get-Content $f.FullName -Raw
    $idx = 0
    while (($idx = $content.IndexOf("signals.forever()", $idx)) -ge 0) {
        $startOfLine = $content.LastIndexOf("`n", $idx)
        if ($startOfLine -lt 0) { $startOfLine = 0 }
        $prefix = $content.Substring($startOfLine, $idx - $startOfLine)
        if ($prefix -match "\bfor\s+") {
            $openBrace = $content.IndexOf("{", $idx)
            if ($openBrace -ge 0) {
                $depth = 1
                $pos = $openBrace + 1
                $bodyStart = $pos
                while ($pos -lt $content.Length -and $depth -gt 0) {
                    $c = $content[$pos]
                    if ($c -eq '{') { $depth++ }
                    elseif ($c -eq '}') { $depth-- }
                    $pos++
                }
                if ($depth -eq 0) {
                    $body = $content.Substring($bodyStart, $pos - $bodyStart - 1)
                    $bodyWithoutComments = [regex]::Replace($body, "//.*", "")
                    if ($bodyWithoutComments -match "(^|[;\s{])break([;\s]|$)") {
                        $signalHits += $f.FullName
                        break
                    }
                }
            }
        }
        $idx += "signals.forever()".Length
    }
}
if ($signalHits.Count -gt 0) {
    Fail "found 'for ... in signals.forever()' loop with terminating break; use 'if let Some(...) = signals.forever().next()' to prevent clippy::never_loop on Unix"
} else {
    Pass "signal handlers avoid clippy::never_loop pattern"
}

# 2. Check launch_prepared_apply parameter version suppression
if (Test-Path "src-tauri/src/lifecycle.rs") {
    $content = Get-Content "src-tauri/src/lifecycle.rs" -Raw
    if ($content -match "pub async fn launch_prepared_apply") {
        if ($content -match "((\b_version:\s*&str|[\s,(]_version:\s*&str)|(\blet\s+_ = version;))") {
            Pass "launch_prepared_apply version parameter properly suppressed for Unix builds"
        } else {
            Fail "launch_prepared_apply version parameter is unused on Unix and must be underscored (_version) or suppressed (let _ = version;)"
        }
    }
}

# 3. Check E2E Settings spec section isolation
if (Test-Path "e2e/tauri/specs/settings.e2e.ts") {
    $e2eContent = Get-Content "e2e/tauri/specs/settings.e2e.ts" -Raw
    $testIdx = $e2eContent.IndexOf("displays the updates card with version and check control")
    if ($testIdx -ge 0) {
        $slice = $e2eContent.Substring($testIdx)
        $endIdx = $slice.IndexOf("it(")
        if ($endIdx -gt 0) { $slice = $slice.Substring(0, $endIdx) }
        $navIdx = $slice.IndexOf("General")
        $cardIdx = $slice.IndexOf("update-card")
        $waitIdx = $slice.IndexOf("waitForDisplayed")
        $firstCardOrWait = -1
        if ($cardIdx -ge 0 -and $waitIdx -ge 0) {
            $firstCardOrWait = [Math]::Min($cardIdx, $waitIdx)
        } elseif ($cardIdx -ge 0) {
            $firstCardOrWait = $cardIdx
        } else {
            $firstCardOrWait = $waitIdx
        }
        if ($navIdx -ge 0 -and $firstCardOrWait -ge 0 -and $navIdx -lt $firstCardOrWait) {
            Pass "settings.e2e.ts updates card test explicitly selects General section before update-card lookup or wait"
        } else {
            Fail "settings.e2e.ts updates card test must explicitly select General section before waiting for [data-testid='update-card']"
        }
    } else {
        Fail "settings.e2e.ts missing updates card test"
    }
}

if ($failures -ne 0) {
    Write-Error "Cross-platform Unix compatibility guard FAILED"
    exit 1
}

Write-Host "Cross-platform Unix compatibility guard passed"
exit 0
