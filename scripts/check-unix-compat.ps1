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
                $inLineComment = $false
                $inBlockComment = $false
                $inString = $false
                $foundUnconditionalBreak = $false
                $hasContinue = $false
                $pendingLoop = $false
                $innerLoopStack = [System.Collections.Generic.Stack[int]]::new()

                while ($pos -lt $content.Length -and $depth -gt 0) {
                    $c = $content[$pos]
                    $nextC = if ($pos + 1 -lt $content.Length) { $content[$pos + 1] } else { "" }

                    if ($inLineComment) {
                        if ($c -eq "`n") { $inLineComment = $false }
                        $pos++
                        continue
                    }

                    if ($inBlockComment) {
                        if ($c -eq '*' -and $nextC -eq '/') {
                            $inBlockComment = $false
                            $pos += 2
                            continue
                        }
                        $pos++
                        continue
                    }

                    if ($inString) {
                        if ($c -eq '\') {
                            $pos += 2
                            continue
                        }
                        if ($c -eq '"') { $inString = $false }
                        $pos++
                        continue
                    }

                    if ($c -eq '/' -and $nextC -eq '/') {
                        $inLineComment = $true
                        $pos += 2
                        continue
                    }
                    if ($c -eq '/' -and $nextC -eq '*') {
                        $inBlockComment = $true
                        $pos += 2
                        continue
                    }

                    if ($c -eq '"') {
                        $inString = $true
                        $pos++
                        continue
                    }

                    if ($c -eq "'") {
                        if ($nextC -eq '\') { $pos += 4 }
                        else { $pos += 3 }
                        continue
                    }

                    $prevChar = if ($pos -gt 0) { $content[$pos - 1] } else { " " }

                    # Detect inner loop
                    if ($prevChar -match "\W") {
                        if ($pos + 4 -le $content.Length -and $content.Substring($pos, 4) -eq "for ") { $pendingLoop = $true }
                        elseif ($pos + 6 -le $content.Length -and $content.Substring($pos, 6) -eq "while ") { $pendingLoop = $true }
                        elseif ($pos + 5 -le $content.Length -and $content.Substring($pos, 5) -eq "loop ") { $pendingLoop = $true }
                        elseif ($pos + 5 -le $content.Length -and $content.Substring($pos, 5) -eq "loop{") { $pendingLoop = $true }
                    }

                    if ($c -eq "{") {
                        $depth++
                        if ($pendingLoop) {
                            $innerLoopStack.Push($depth)
                            $pendingLoop = $false
                        }
                        $pos++
                        continue
                    }
                    if ($c -eq "}") {
                        if ($innerLoopStack.Count -gt 0 -and $innerLoopStack.Peek() -eq $depth) {
                            [void]$innerLoopStack.Pop()
                        }
                        $depth--
                        $pos++
                        continue
                    }

                    # Detect continue targeting outer loop
                    if ($innerLoopStack.Count -eq 0 -and $pos + 8 -le $content.Length -and $content.Substring($pos, 8) -eq "continue") {
                        $nextChar = if ($pos + 8 -lt $content.Length) { $content[$pos + 8] } else { " " }
                        if ($prevChar -match "\W" -and $nextChar -match "\W") {
                            $hasContinue = $true
                        }
                    }

                    # Check for break token at outer loop depth without continue
                    if ($depth -eq 1 -and $pos + 5 -le $content.Length -and $content.Substring($pos, 5) -eq "break") {
                        $nextChar = if ($pos + 5 -lt $content.Length) { $content[$pos + 5] } else { " " }
                        if ($prevChar -match "\W" -and $nextChar -match "\W") {
                            if (-not $hasContinue) {
                                $foundUnconditionalBreak = $true
                                break
                            }
                        }
                    }

                    $pos++
                }

                if ($foundUnconditionalBreak) {
                    $signalHits += $f.FullName
                    break
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
    $testHeader = "displays the updates card with version and check control"
    $testIdx = $e2eContent.IndexOf($testHeader)
    if ($testIdx -ge 0) {
        $slice = $e2eContent.Substring($testIdx)
        $endIdx = $slice.IndexOf("it(")
        if ($endIdx -gt 0) { $slice = $slice.Substring(0, $endIdx) }

        # Strip line comments
        $slice = [regex]::Replace($slice, "(?m)^\s*//.*$", "")

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

        $preCard = if ($firstCardOrWait -ge 0) { $slice.Substring(0, $firstCardOrWait) } else { "" }

        $chainedPattern = '(?s)\$(?:\$)?\s*\([^;]*?(?:button[^;]*?General|General[^;]*?button)[^;]*?\)(?:\s*\.[a-zA-Z0-9_]+\([^)]*\))*\s*\.\s*click\s*\('
        $assignedPattern = '(?s)(?:const|let|var)\s+([a-zA-Z0-9_]+)\s*=\s*(?:await\s+)?\$(?:\$)?\s*\([^;]*?(?:button[^;]*?General|General[^;]*?button)[^;]*?\);?.*?await\s+\1\s*\.\s*click\s*\('

        if ($firstCardOrWait -ge 0 -and ($preCard -match $chainedPattern -or $preCard -match $assignedPattern)) {
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
