<##
.SYNOPSIS
    Measures the real Windows global shortcut-to-mixer latency.

.DESCRIPTION
    Starts one VolumeControl child process, sends the configured open-mixer
    shortcut with keybd_event, and polls the child-owned Volume Mixer window
    using Stopwatch. This is real Windows integration evidence; it is not a
    WDIO, Pilot, or synthetic Linux/macOS check.

.PARAMETER Release
    Use target\release\VolumeControl.exe. The default is the debug binary.

.PARAMETER AppPath
    Override the selected binary path.

.PARAMETER Iterations
    Number of shortcut measurements. Defaults to 10.

.PARAMETER OutputRoot
    Directory for hotkey-latency.json and hotkey-latency.txt.

.EXAMPLE
    pwsh -NoProfile -File scripts\verify-hotkey-latency.ps1 -Release -Iterations 10 -OutputRoot output\manual\hotkey-latency
##>
[CmdletBinding()]
param(
    [switch]$Release,
    [string]$AppPath,
    [ValidateRange(1, 1000)]
    [int]$Iterations = 10,
    [string]$OutputRoot,
    [ValidateRange(100, 30000)]
    [int]$TimeoutMs = 3000
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
    Write-Error 'verify-hotkey-latency.ps1 is Windows-only; no app was started.'
    exit 1
}

$repoRoot = Split-Path $PSScriptRoot -Parent
if (-not $OutputRoot) {
    $OutputRoot = Join-Path $repoRoot 'output\manual\hotkey-latency'
}
$OutputRoot = [IO.Path]::GetFullPath($OutputRoot)

if ($AppPath) {
    $appExe = [IO.Path]::GetFullPath($AppPath)
} elseif ($Release) {
    $appExe = Join-Path $repoRoot 'target\release\VolumeControl.exe'
} else {
    $appExe = Join-Path $repoRoot 'target\debug\VolumeControl.exe'
}
if (-not (Test-Path -LiteralPath $appExe -PathType Leaf)) {
    Write-Error "App not found at $appExe - build VolumeControl.exe first."
    exit 1
}

New-Item -ItemType Directory -Force -Path $OutputRoot | Out-Null

$script:childProcess = $null
$script:pinvokeDll = $null
$script:records = @()
$script:failures = @()
$script:appSha = (Get-FileHash -LiteralPath $appExe -Algorithm SHA256).Hash.ToLowerInvariant()
$script:osBuild = $null
$script:configuredShortcut = 'Ctrl+Alt+KeyV'
$runClock = [Diagnostics.Stopwatch]::StartNew()

function Get-OsBuild {
    try {
        $current = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
        $product = [string]$current.ProductName
        $display = [string]$current.DisplayVersion
        $build = [string]$current.CurrentBuild
        if ($current.UBR -ne $null) { $build = "$build.$($current.UBR)" }
        return "$product $display (build $build)"
    } catch {
        return [Environment]::OSVersion.Version.ToString()
    }
}

function Get-ConfigShortcut {
    $configPath = Join-Path $env:APPDATA 'volume-control\config.json'
    if (-not (Test-Path -LiteralPath $configPath -PathType Leaf)) { return 'Ctrl+Alt+KeyV' }
    try {
        $config = Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json
        $shortcut = [string]$config.hotkeys.open_mixer
        if ($shortcut) { return $shortcut }
    } catch {
        Write-Warning "Could not read configured shortcut from $configPath; using Ctrl+Alt+KeyV."
    }
    return 'Ctrl+Alt+KeyV'
}

function Get-WindowTitle {
    param([IntPtr]$Hwnd)
    $length = [Win32]::GetWindowTextLength($Hwnd)
    if ($length -le 0) { return '' }
    $builder = New-Object Text.StringBuilder ($length + 1)
    [Win32]::GetWindowText($Hwnd, $builder, $builder.Capacity) | Out-Null
    return $builder.ToString()
}

function Get-WindowClassName {
    param([IntPtr]$Hwnd)
    $builder = New-Object Text.StringBuilder 256
    [Win32]::GetClassName($Hwnd, $builder, $builder.Capacity) | Out-Null
    return $builder.ToString()
}

function Find-MixerWindow {
    param([int]$ProcessId)
    foreach ($hwnd in [Win32]::GetProcessWindows($ProcessId)) {
        if (-not [Win32]::IsWindowVisible($hwnd)) { continue }
        $title = Get-WindowTitle $hwnd
        $className = Get-WindowClassName $hwnd
        if ($title -eq 'Volume Mixer' -or $className -eq 'VolCtlMixer') {
            return $hwnd
        }
    }
    return [IntPtr]::Zero
}

function Get-VkCode {
    param([string]$Token)
    $known = @{
        ArrowUp = [byte]0x26; ArrowDown = [byte]0x28; ArrowLeft = [byte]0x25; ArrowRight = [byte]0x27
        Escape = [byte]0x1B; Enter = [byte]0x0D; Space = [byte]0x20; Tab = [byte]0x09
    }
    if ($known.ContainsKey($Token)) { return $known[$Token] }
    if ($Token -match '^Key([A-Za-z])$') { return [byte][char]$Matches[1].ToUpperInvariant() }
    if ($Token -match '^Digit([0-9])$') { return [byte](0x30 + [int]$Matches[1]) }
    if ($Token -match '^[A-Za-z0-9]$') { return [byte][char]$Token.ToUpperInvariant() }
    throw "Unsupported configured shortcut key token: $Token"
}

function Send-ConfiguredShortcut {
    param([string]$Shortcut)
    $tokens = @($Shortcut -split '\+' | ForEach-Object { $_.Trim() } | Where-Object { $_ })
    if ($tokens.Count -lt 1) { throw 'Configured shortcut is empty.' }

    $modifierCodes = @{
        Ctrl = [byte][Win32]::VK_CONTROL; Control = [byte][Win32]::VK_CONTROL
        Alt = [byte][Win32]::VK_MENU; Shift = [byte][Win32]::VK_SHIFT
    }
    $pressed = @()
    $keyToken = $tokens[-1]
    $keyCode = Get-VkCode $keyToken
    try {
        if ($tokens.Count -gt 1) {
            foreach ($token in $tokens[0..($tokens.Count - 2)]) {
                if (-not $modifierCodes.ContainsKey($token)) { throw "Unsupported shortcut modifier: $token" }
                $code = $modifierCodes[$token]
                [Win32]::keybd_event($code, 0, 0, [UIntPtr]::Zero)
                $pressed += $code
            }
        }
        [Win32]::keybd_event($keyCode, 0, 0, [UIntPtr]::Zero)
        $pressed += $keyCode
        Start-Sleep -Milliseconds 20
    } finally {
        for ($index = $pressed.Count - 1; $index -ge 0; $index--) {
            [Win32]::keybd_event($pressed[$index], 0, [Win32]::KEYEVENTF_KEYUP, [UIntPtr]::Zero)
        }
    }
}

function Wait-ForMixerWindow {
    param([int]$ProcessId, [int]$Timeout)
    $clock = [Diagnostics.Stopwatch]::StartNew()
    while ($clock.ElapsedMilliseconds -lt $Timeout) {
        $hwnd = Find-MixerWindow $ProcessId
        if ($hwnd -ne [IntPtr]::Zero) {
            return [pscustomobject]@{
                Hwnd = $hwnd
                ElapsedMs = [math]::Round($clock.Elapsed.TotalMilliseconds, 3)
            }
        }
        Start-Sleep -Milliseconds 10
    }
    return $null
}

function Write-Reports {
    $deltas = @($script:records | Where-Object { $_.delta_ms -ne $null } | ForEach-Object { [double]$_.delta_ms } | Sort-Object)
    $p50 = $null
    $p95 = $null
    if ($deltas.Count -gt 0) {
        $p50 = [math]::Round($deltas[[math]::Max(0, [math]::Ceiling($deltas.Count * 0.50) - 1)], 3)
        $p95 = [math]::Round($deltas[[math]::Max(0, [math]::Ceiling($deltas.Count * 0.95) - 1)], 3)
    }
    $report = [ordered]@{
        schema_version = 1
        generated_at_utc = (Get-Date).ToUniversalTime().ToString('o')
        app_path = $appExe
        app_sha256 = $script:appSha
        os_build = $script:osBuild
        configured_shortcut = $script:configuredShortcut
        iterations_requested = $Iterations
        iterations = @($script:records)
        p50_ms = $p50
        p95_ms = $p95
        failures = @($script:failures)
    }
    $jsonPath = Join-Path $OutputRoot 'hotkey-latency.json'
    $textPath = Join-Path $OutputRoot 'hotkey-latency.txt'
    $report | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $jsonPath -Encoding UTF8

    $lines = @(
        'VolumeControl Windows hotkey latency evidence'
        "Generated UTC: $($report.generated_at_utc)"
        "App: $($report.app_path)"
        "App SHA-256: $($report.app_sha256)"
        "OS build: $($report.os_build)"
        "Configured shortcut: $($report.configured_shortcut)"
        "Iterations: $($report.iterations.Count)/$($report.iterations_requested)"
        "p50 (ms): $($report.p50_ms)"
        "p95 (ms): $($report.p95_ms)"
        ''
    )
    foreach ($record in $script:records) {
        $lines += "Iteration $($record.iteration): keydown=$($record.keydown_timestamp_utc), visible=$($record.surface_visible_timestamp_utc), delta_ms=$($record.delta_ms)"
    }
    if ($script:failures.Count -gt 0) {
        $lines += ''
        $lines += 'Failures:'
        $lines += $script:failures
    }
    $lines -join "`r`n" | Set-Content -LiteralPath $textPath -Encoding UTF8
    Write-Host "Wrote $jsonPath"
    Write-Host "Wrote $textPath"
}

try {
    $script:osBuild = Get-OsBuild
    $script:configuredShortcut = Get-ConfigShortcut

    # Compile the shared P/Invoke declarations used by verify-vol011.ps1.
    $csFile = Join-Path $PSScriptRoot 'win32_pinvoke.cs'
    $cscCandidates = @(
        "$env:SystemRoot\Microsoft.NET\Framework\v4.0.30319\csc.exe",
        "$env:SystemRoot\Microsoft.NET\Framework64\v4.0.30319\csc.exe"
    )
    $csc = $cscCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
    if (-not $csc) { throw 'csc.exe not found; install the .NET Framework developer tools.' }
    $script:pinvokeDll = Join-Path $env:TEMP ("volume-control-pinvoke-$([guid]::NewGuid()).dll")
    & $csc /nologo /target:library /out:$script:pinvokeDll $csFile 2>&1 | Out-Host
    if ($LASTEXITCODE -ne 0) { throw 'win32_pinvoke.cs compilation failed.' }
    Add-Type -Path $script:pinvokeDll

    $script:childProcess = Start-Process -FilePath $appExe -WorkingDirectory $repoRoot -PassThru -WindowStyle Hidden
    Start-Sleep -Milliseconds 500
    if ($script:childProcess.HasExited) { throw "Started app child exited with code $($script:childProcess.ExitCode)." }

    for ($iteration = 1; $iteration -le $Iterations; $iteration++) {
        if ($script:childProcess.HasExited) { throw 'The app child exited before all iterations completed.' }
        $existing = Find-MixerWindow $script:childProcess.Id
        if ($existing -ne [IntPtr]::Zero) {
            [Win32]::ShowWindow($existing, [Win32]::SW_HIDE) | Out-Null
            Start-Sleep -Milliseconds 50
        }

        $keydownClockMs = [math]::Round($runClock.Elapsed.TotalMilliseconds, 3)
        $keydownUtc = (Get-Date).ToUniversalTime().ToString('o')
        Send-ConfiguredShortcut $script:configuredShortcut
        $visible = Wait-ForMixerWindow $script:childProcess.Id $TimeoutMs
        if ($null -eq $visible) {
            $script:records += [pscustomobject]@{
                iteration = $iteration
                keydown_timestamp_utc = $keydownUtc
                surface_visible_timestamp_utc = $null
                keydown_monotonic_ms = $keydownClockMs
                surface_visible_monotonic_ms = $null
                delta_ms = $null
            }
            $script:failures += "Iteration ${iteration}: Volume Mixer was not visible within ${TimeoutMs}ms."
            continue
        }
        $visibleClockMs = [math]::Round($keydownClockMs + $visible.ElapsedMs, 3)
        $script:records += [pscustomobject]@{
            iteration = $iteration
            keydown_timestamp_utc = $keydownUtc
            surface_visible_timestamp_utc = (Get-Date).ToUniversalTime().ToString('o')
            keydown_monotonic_ms = $keydownClockMs
            surface_visible_monotonic_ms = $visibleClockMs
            delta_ms = [math]::Round($visible.ElapsedMs, 3)
        }
        [Win32]::ShowWindow($visible.Hwnd, [Win32]::SW_HIDE) | Out-Null
        Start-Sleep -Milliseconds 50
    }
    Write-Reports
} catch {
    $script:failures += $_.Exception.Message
    try { Write-Reports } catch { Write-Warning "Could not write failure report: $($_.Exception.Message)" }
    $script:exitCode = 1
} finally {
    # Only terminate the child process created above; never enumerate or kill
    # another user's/existing VolumeControl process.
    if ($null -ne $script:childProcess) {
        try {
            if (-not $script:childProcess.HasExited) {
                Stop-Process -Id $script:childProcess.Id -Force -ErrorAction SilentlyContinue
            }
        } catch { }
    }
    if ($script:pinvokeDll -and (Test-Path -LiteralPath $script:pinvokeDll)) {
        Remove-Item -LiteralPath $script:pinvokeDll -Force -ErrorAction SilentlyContinue
    }
}

if ($script:failures.Count -gt 0) { exit 1 }
exit 0
