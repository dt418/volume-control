#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Verifies the three Tauri webview surfaces on a real Windows desktop.

.DESCRIPTION
    Starts one real VolumeControl binary per surface through the diagnostic
    VOLUMECTL_VERIFY_SURFACE environment variable, waits for a visible window,
    records process/window/monitor evidence, captures the client area, and
    fails on a crash, blank capture, or legacy geometry mismatch.

    Artifacts are intentionally written below output/ and are not tracked.
#>
param(
    [Parameter(Mandatory = $true)]
    [string]$Binary,
    [ValidateSet('window-mixer', 'window-settings', 'window-help')]
    [string[]]$Surface = @('window-mixer', 'window-settings', 'window-help'),
    [ValidateRange(2, 120)]
    [int]$TimeoutSeconds = 15,
    [string]$OutputRoot = 'output/tauri-surface-evidence/after'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$binaryPath = [IO.Path]::GetFullPath((Join-Path $repoRoot $Binary))
if (-not (Test-Path -LiteralPath $binaryPath -PathType Leaf)) {
    throw "Binary not found: $binaryPath"
}

$outputPath = [IO.Path]::GetFullPath((Join-Path $repoRoot $OutputRoot))
New-Item -ItemType Directory -Force -Path $outputPath | Out-Null

$csFile = Join-Path $PSScriptRoot 'win32_pinvoke.cs'
$cscCandidates = @(
    "$env:SystemRoot\Microsoft.NET\Framework64\v4.0.30319\csc.exe",
    "$env:SystemRoot\Microsoft.NET\Framework\v4.0.30319\csc.exe"
)
$csc = $cscCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
if (-not $csc) { throw 'csc.exe not found; Windows .NET Framework is required for capture evidence.' }
$dllPath = Join-Path ([IO.Path]::GetTempPath()) "volume-control-win32-$PID.dll"
& $csc /nologo /target:library /out:$dllPath /reference:System.Drawing.dll $csFile 2>&1 | Write-Host
if ($LASTEXITCODE -ne 0) { throw 'Failed to compile the Win32 capture helper.' }
Add-Type -Path $dllPath
Add-Type -AssemblyName System.Drawing

function Get-WindowGeometry {
    param([IntPtr]$Handle)
    $window = New-Object RECT
    $client = New-Object RECT
    if (-not [Win32]::GetWindowRect($Handle, [ref]$window)) { throw "GetWindowRect failed for $Handle" }
    if (-not [Win32]::GetClientRect($Handle, [ref]$client)) { throw "GetClientRect failed for $Handle" }
    [pscustomobject]@{
        left = $window.Left
        top = $window.Top
        right = $window.Right
        bottom = $window.Bottom
        width = $window.Right - $window.Left
        height = $window.Bottom - $window.Top
        client_width = $client.Right - $client.Left
        client_height = $client.Bottom - $client.Top
    }
}

function Find-ProcessWindow {
    param([int]$ProcessId)
    foreach ($handle in [Win32]::GetProcessWindows($ProcessId)) {
        if (-not [Win32]::IsWindowVisible($handle)) { continue }
        $geometry = Get-WindowGeometry $handle
        if ($geometry.client_width -gt 0 -and $geometry.client_height -gt 0) {
            return $handle
        }
    }
    return [IntPtr]::Zero
}

function Get-MonitorEvidence {
    param([IntPtr]$Handle)
    $monitor = [Win32]::MonitorFromWindow($Handle, [Win32]::MONITOR_DEFAULTTONEAREST)
    if ($monitor -eq [IntPtr]::Zero) { throw "MonitorFromWindow failed for $Handle" }
    $info = New-Object MONITORINFO
    $info.cbSize = [Runtime.InteropServices.Marshal]::SizeOf($info)
    if (-not [Win32]::GetMonitorInfo($monitor, [ref]$info)) { throw "GetMonitorInfo failed for $Handle" }
    $dpi = [Win32]::GetDpiForWindow($Handle)
    if ($dpi -eq 0) { $dpi = [Win32]::GetDpiForSystem() }
    [pscustomobject]@{
        monitor = @{ left = $info.rcMonitor.Left; top = $info.rcMonitor.Top; right = $info.rcMonitor.Right; bottom = $info.rcMonitor.Bottom }
        work_area = @{ left = $info.rcWork.Left; top = $info.rcWork.Top; right = $info.rcWork.Right; bottom = $info.rcWork.Bottom }
        dpi = [int]$dpi
        scale = [double]$dpi / 96.0
    }
}

function Get-ExpectedGeometry {
    param([string]$SurfaceName, [object]$Monitor)
    $scale = $Monitor.scale
    $wa = $Monitor.work_area
    switch ($SurfaceName) {
        'window-mixer' {
            $width = [math]::Round(400 * $scale)
            $height = [math]::Round(224 * $scale)
            $overlayHeight = 88 * $scale
            $right = $wa.right - 20
            $overlayTop = $wa.bottom - 40 - $overlayHeight
            $bottom = $overlayTop - 16
            $top = [math]::Max($wa.top, $bottom - $height)
            return @{ left = $right - $width; top = $top; right = $right; bottom = $top + $height; client_width = $width; client_height = $height; anchor = 'bottom-right-above-overlay' }
        }
        'window-settings' {
            $width = [math]::Round(760 * $scale)
            $height = [math]::Round(620 * $scale)
            $left = $wa.left + [math]::Max(0, [math]::Round(($wa.right - $wa.left - $width) / 2))
            $top = $wa.top + [math]::Max(0, [math]::Round(($wa.bottom - $wa.top - $height) / 2))
            return @{ left = $left; top = $top; right = $left + $width; bottom = $top + $height; client_width = $width; client_height = $height; anchor = 'centered' }
        }
        'window-help' {
            $width = [math]::Round(520 * $scale)
            $height = [math]::Round(500 * $scale)
            $left = $wa.right - 24 - $width
            $top = $wa.bottom - 48 - $height
            return @{ left = $left; top = $top; right = $left + $width; bottom = $top + $height; client_width = $width; client_height = $height; anchor = 'bottom-right' }
        }
    }
    throw "Unsupported surface $SurfaceName"
}

function Save-WindowCapture {
    param([IntPtr]$Handle, [string]$Path)
    $geometry = Get-WindowGeometry $Handle
    if ($geometry.client_width -le 0 -or $geometry.client_height -le 0) { throw 'Window client area is empty.' }
    $screenDc = [Win32]::GetDC([IntPtr]::Zero)
    $memoryDc = [Win32]::CreateCompatibleDC($screenDc)
    $bitmap = [Win32]::CreateCompatibleBitmap($screenDc, $geometry.client_width, $geometry.client_height)
    $old = [Win32]::SelectObject($memoryDc, $bitmap)
    try {
        $rendered = [Win32]::PrintWindow($Handle, $memoryDc, [Win32]::PW_RENDERFULLCONTENT)
        if (-not $rendered) {
            $rendered = [Win32]::PrintWindow($Handle, $memoryDc, [Win32]::PW_CLIENTONLY)
        }
        if (-not $rendered) { throw 'PrintWindow returned false.' }
        $dir = Split-Path -Parent $Path
        New-Item -ItemType Directory -Force -Path $dir | Out-Null
        $image = [System.Drawing.Bitmap]::FromHbitmap($bitmap)
        try { $image.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png) } finally { $image.Dispose() }
    } finally {
        [Win32]::SelectObject($memoryDc, $old) | Out-Null
        [Win32]::DeleteObject($bitmap) | Out-Null
        [Win32]::DeleteDC($memoryDc) | Out-Null
        [Win32]::ReleaseDC([IntPtr]::Zero, $screenDc) | Out-Null
    }
    if (-not (Test-Path -LiteralPath $Path)) { throw 'Screenshot file was not created.' }
    $bytes = [IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 128) { throw 'Screenshot file is too small.' }
    return $bytes.Length
}

function Test-NonBlankImage {
    param([string]$Path)
    $image = [System.Drawing.Bitmap]::new($Path)
    try {
        $colors = [Collections.Generic.HashSet[string]]::new()
        $stepX = [math]::Max(1, [math]::Floor($image.Width / 12))
        $stepY = [math]::Max(1, [math]::Floor($image.Height / 12))
        for ($x = 0; $x -lt $image.Width; $x += $stepX) {
            for ($y = 0; $y -lt $image.Height; $y += $stepY) {
                $pixel = $image.GetPixel($x, $y)
                $null = $colors.Add("$($pixel.R),$($pixel.G),$($pixel.B),$($pixel.A)")
            }
        }
        return $colors.Count -ge 4
    } finally {
        $image.Dispose()
    }
}

function Wait-ForRenderedCapture {
    param([IntPtr]$Handle, [string]$Path, [int]$TimeoutMs)
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    $lastError = $null
    do {
        try {
            $bytes = Save-WindowCapture $Handle $Path
            if (Test-NonBlankImage $Path) { return $bytes }
        } catch {
            $lastError = $_.Exception.Message
        }
        Start-Sleep -Milliseconds 150
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($lastError) { throw "Window remained blank/un-capturable: $lastError" }
    throw 'Window remained a uniform/blank capture until timeout.'
}

function Wait-ForSurface {
    param([Diagnostics.Process]$Process, [int]$TimeoutMs)
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    do {
        if ($Process.HasExited) { throw "Process exited early with code $($Process.ExitCode)." }
        $handle = Find-ProcessWindow $Process.Id
        if ($handle -ne [IntPtr]::Zero) { return $handle }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "No visible non-empty surface window appeared within $TimeoutMs ms."
}

$failures = 0
foreach ($surfaceName in $Surface) {
    $surfaceDir = Join-Path $outputPath $surfaceName
    New-Item -ItemType Directory -Force -Path $surfaceDir | Out-Null
    $configDir = Join-Path ([IO.Path]::GetTempPath()) "volume-control-verify-$PID-$($surfaceName -replace '[^a-z-]', '')"
    New-Item -ItemType Directory -Force -Path $configDir | Out-Null
    $logPath = Join-Path $surfaceDir 'process.log'
    $jsonPath = Join-Path $surfaceDir 'evidence.json'
    $pngPath = Join-Path $surfaceDir 'surface.png'
    $process = $null
    try {
        $start = [Diagnostics.ProcessStartInfo]::new()
        $start.FileName = $binaryPath
        $start.WorkingDirectory = $repoRoot
        $start.UseShellExecute = $false
        $start.CreateNoWindow = $true
        $start.Environment['VOLUMECTL_VERIFY_SURFACE'] = $surfaceName
        $start.Environment['VOLUMECTL_CONFIG_DIR'] = $configDir
        $process = [Diagnostics.Process]::Start($start)
        $handle = Wait-ForSurface $process ($TimeoutSeconds * 1000)
        Start-Sleep -Milliseconds 250
        if ($process.HasExited) { throw "Process exited after showing the surface with code $($process.ExitCode)." }
        $geometry = Get-WindowGeometry $handle
        $monitor = Get-MonitorEvidence $handle
        $expected = Get-ExpectedGeometry $surfaceName $monitor
        $captureBytes = Wait-ForRenderedCapture $handle $pngPath ($TimeoutSeconds * 1000)

        $sizeOk = $geometry.client_width -eq $expected.client_width -and $geometry.client_height -eq $expected.client_height
        $positionTolerance = 8
        $positionOk = [math]::Abs($geometry.left - $expected.left) -le $positionTolerance -and [math]::Abs($geometry.top - $expected.top) -le $positionTolerance
        if (-not $sizeOk -or -not $positionOk) {
            throw "Geometry mismatch: actual client $($geometry.client_width)x$($geometry.client_height) at $($geometry.left),$($geometry.top); expected client $($expected.client_width)x$($expected.client_height) at $($expected.left),$($expected.top)."
        }

        $record = [ordered]@{
            surface = $surfaceName
            pid = $process.Id
            hwnd = ('0x{0:X}' -f $handle.ToInt64())
            title = [Win32]::GetWindowTitle($handle)
            process_alive_after_ready = -not $process.HasExited
            process_exit_code = $null
            actual = $geometry
            expected = $expected
            monitor = $monitor
            screenshot = (Resolve-Path -LiteralPath $pngPath).Path
            screenshot_bytes = $captureBytes
            captured_at = (Get-Date).ToUniversalTime().ToString('o')
        }
        $record | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $jsonPath -Encoding UTF8
        Write-Host "PASS $surfaceName -> $pngPath"
    } catch {
        $failures++
        $_ | Out-String | Set-Content -LiteralPath (Join-Path $surfaceDir 'failure.txt') -Encoding UTF8
        Write-Host "FAIL ${surfaceName}: $($_.Exception.Message)" -ForegroundColor Red
    } finally {
        if ($process) {
            $wasAlive = -not $process.HasExited
            try { if ($wasAlive) { $process.Kill(); $process.WaitForExit(5000) } } catch {}
            @(
                "PID: $($process.Id)"
                "Was alive at cleanup: $wasAlive"
                "Exited: $($process.HasExited)"
                "Exit code: $($process.ExitCode)"
            ) | Set-Content -LiteralPath $logPath -Encoding UTF8
            $process.Dispose()
        }
        Remove-Item -LiteralPath $configDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Remove-Item -LiteralPath $dllPath -Force -ErrorAction SilentlyContinue
if ($failures -gt 0) { exit 1 }
exit 0
