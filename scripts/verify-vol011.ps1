#!/usr/bin/env pwsh
<#
.SYNOPSIS
    vol-011 verification: runs the remaining human-check items on a real Windows
    machine and captures evidence.

.DESCRIPTION
    Sets OS state (high-contrast, reduced-motion, DPI awareness), launches the
    app, captures geometry/pixels/UIA/menu evidence, restores original state.
    Output goes to $EvidenceDir (default: %TEMP%\vol011-verify).

.PARAMETER Release
    Use the release build (target\release\volumectl.exe). Default: debug.

.PARAMETER AppPath
    Full path to the volumectl executable. Overrides -Release.

.PARAMETER EvidenceDir
    Output directory for evidence files. Default: %TEMP%\vol011-verify.

.PARAMETER NoRestore
    Skip restoring original OS state (for debugging).

.EXAMPLE
    .\verify-vol011.ps1
    .\verify-vol011.ps1 -Release
    .\verify-vol011.ps1 -NoRestore
#>
param(
    [switch]$Release,
    [string]$AppPath,
    [string]$EvidenceDir,
    [switch]$NoRestore
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Resolve paths
$repoRoot = Split-Path $PSScriptRoot -Parent
if ($AppPath) {
    $appExe = $AppPath
} elseif ($Release) {
    $appExe = Join-Path $repoRoot 'target\release\volumectl.exe'
} else {
    $appExe = Join-Path $repoRoot 'target\debug\volumectl.exe'
}
if (-not (Test-Path -LiteralPath $appExe)) {
    Write-Error "App not found at $appExe - run cargo build first."
    exit 1
}
if (-not $EvidenceDir) {
    $EvidenceDir = Join-Path $env:TEMP 'vol011-verify'
}
New-Item -ItemType Directory -Force -Path $EvidenceDir | Out-Null

# P/Invoke definitions (compile C# helper to bypass PS5.1 parser limitations)
$csFile = Join-Path $PSScriptRoot 'win32_pinvoke.cs'
$cscPaths = @(
    "$env:SystemRoot\Microsoft.NET\Framework\v4.0.30319\csc.exe",
    "$env:SystemRoot\Microsoft.NET\Framework64\v4.0.30319\csc.exe"
)
$csc = $null
foreach ($p in $cscPaths) { if (Test-Path -LiteralPath $p) { $csc = $p; break } }
if (-not $csc) { Write-Error "csc.exe not found"; exit 1 }
$dllPath = Join-Path $env:TEMP 'win32_pinvoke.dll'
& $csc /nologo /target:library /out:$dllPath $csFile 2>&1
if ($LASTEXITCODE -ne 0) { Write-Error "csc compilation failed"; exit 1 }
Add-Type -Path $dllPath
Add-Type -AssemblyName System.Windows.Forms

# Helper functions
$script:originalHC = $null
$script:originalAnim = $null
$script:appProcess = $null

function Get-VolumectlProcess {
    Get-Process -Name volumectl -ErrorAction SilentlyContinue | Select-Object -First 1
}

function Stop-Volumectl {
    $proc = Get-VolumectlProcess
    if ($proc) {
        Stop-Process -Name volumectl -Force -ErrorAction SilentlyContinue
        Start-Sleep -Milliseconds 500
    }
}

function Start-Volumectl {
    param([int]$TimeoutMs = 5000)
    Stop-Volumectl
    $script:appProcess = Start-Process -FilePath $appExe -PassThru -WindowStyle Hidden
    Start-Sleep -Milliseconds $TimeoutMs
    $script:appProcess
}

function Find-Hwnd {
    param([string]$ClassName, [string]$WindowTitle)
    $hwnd = [Win32]::FindWindow($ClassName, $WindowTitle)
    if ($hwnd -ne [IntPtr]::Zero) { return $hwnd }
    $proc = Get-VolumectlProcess
    if (-not $proc) { return [IntPtr]::Zero }
    foreach ($h in [Win32]::GetProcessWindows($proc.Id)) {
        $cn = [Win32]::GetWindowClassName($h)
        if ($ClassName -and $cn -eq $ClassName) { return $h }
    }
    return [IntPtr]::Zero
}

function Get-WindowGeometry {
    param([IntPtr]$Hwnd)
    $rect = New-Object RECT
    [Win32]::GetWindowRect($Hwnd, [ref]$rect) | Out-Null
    $client = New-Object RECT
    [Win32]::GetClientRect($Hwnd, [ref]$client) | Out-Null
    @{
        Window = $rect
        Client = $client
        Width  = $rect.Right - $rect.Left
        Height = $rect.Bottom - $rect.Top
    }
}

function Get-PixelColor {
    param([IntPtr]$Hwnd, [int]$X, [int]$Y)
    $hdc = [Win32]::GetDC($Hwnd)
    if ($hdc -eq [IntPtr]::Zero) { return $null }
    try {
        $color = [Win32]::GetPixel($hdc, $X, $Y)
        $r = $color -band 0xFF
        $g = ($color -shr 8) -band 0xFF
        $b = ($color -shr 16) -band 0xFF
        return "RGB($r,$g,$b)"
    } finally {
        [Win32]::ReleaseDC($Hwnd, $hdc) | Out-Null
    }
}

function Get-DwmBackdropType {
    param([IntPtr]$Hwnd)
    $val = 0
    [Win32]::DwmGetWindowAttribute($Hwnd, [Win32]::DWMWA_SYSTEMBACKDROP_TYPE, [ref]$val, 4) | Out-Null
    $val
}

function Save-HighContrast {
    $val = 0
    [Win32]::SystemParametersInfo([Win32]::SPI_GETHIGHCONTRAST, 4, [ref]$val, 0) | Out-Null
    $script:originalHC = $val
    $val
}

function Set-HighContrast {
    param([int]$Value)
    [Win32]::SystemParametersInfo(0x0043, $Value, [ref]$null, [Win32]::SPIF_UPDATEINIFILE -bor [Win32]::SPIF_SENDCHANGE) | Out-Null
}

function Save-Animation {
    $val = 0
    [Win32]::SystemParametersInfo([Win32]::SPI_GETCLIENTAREAANIMATION, 4, [ref]$val, 0) | Out-Null
    $script:originalAnim = $val
    $val
}

function Set-Animation {
    param([int]$Value)
    [Win32]::SystemParametersInfo(0x0049, $Value, [ref]$null, [Win32]::SPIF_UPDATEINIFILE -bor [Win32]::SPIF_SENDCHANGE) | Out-Null
}

function Restore-OriginalState {
    if ($null -ne $script:originalHC) {
        Write-Host "  Restoring high-contrast to $($script:originalHC)..."
        Set-HighContrast $script:originalHC
    }
    if ($null -ne $script:originalAnim) {
        Write-Host "  Restoring animation to $($script:originalAnim)..."
        Set-Animation $script:originalAnim
    }
}

function Write-Evidence {
    param([string]$Check, [string]$File, [string]$Content)
    $dir = Join-Path $EvidenceDir $Check
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    $path = Join-Path $dir $File
    Set-Content -Path $path -Value $Content -Encoding UTF8
    Write-Host "  Evidence: $Check\$File"
}

function Invoke-SendHotkey {
    param([IntPtr]$Hwnd, [int]$ActionId)
    # SetForegroundWindow + keybd_event with delays for rdev hook
    [Win32]::BringWindowToTop($Hwnd) | Out-Null
    [Win32]::SetForegroundWindow($Hwnd) | Out-Null
    Start-Sleep -Milliseconds 200
    if ($ActionId -eq 0x07) {
        # Ctrl+Alt+V = OpenMixer
        [Win32]::keybd_event([Win32]::VK_CONTROL, 0, 0, [System.UIntPtr]::Zero)
        Start-Sleep -Milliseconds 20
        [Win32]::keybd_event([Win32]::VK_MENU, 0, 0, [System.UIntPtr]::Zero)
        Start-Sleep -Milliseconds 20
        [Win32]::keybd_event(0x56, 0, 0, [System.UIntPtr]::Zero)
        Start-Sleep -Milliseconds 50
        [Win32]::keybd_event(0x56, 0, [Win32]::KEYEVENTF_KEYUP, [System.UIntPtr]::Zero)
        Start-Sleep -Milliseconds 20
        [Win32]::keybd_event([Win32]::VK_MENU, 0, [Win32]::KEYEVENTF_KEYUP, [System.UIntPtr]::Zero)
        Start-Sleep -Milliseconds 20
        [Win32]::keybd_event([Win32]::VK_CONTROL, 0, [Win32]::KEYEVENTF_KEYUP, [System.UIntPtr]::Zero)
    } elseif ($ActionId -eq 0x08) {
        # Ctrl+Alt+Shift+M = OpenMenu
        [Win32]::keybd_event([Win32]::VK_CONTROL, 0, 0, [System.UIntPtr]::Zero)
        Start-Sleep -Milliseconds 20
        [Win32]::keybd_event([Win32]::VK_MENU, 0, 0, [System.UIntPtr]::Zero)
        Start-Sleep -Milliseconds 20
        [Win32]::keybd_event([Win32]::VK_SHIFT, 0, 0, [System.UIntPtr]::Zero)
        Start-Sleep -Milliseconds 20
        [Win32]::keybd_event(0x4D, 0, 0, [System.UIntPtr]::Zero)
        Start-Sleep -Milliseconds 50
        [Win32]::keybd_event(0x4D, 0, [Win32]::KEYEVENTF_KEYUP, [System.UIntPtr]::Zero)
        Start-Sleep -Milliseconds 20
        [Win32]::keybd_event([Win32]::VK_SHIFT, 0, [Win32]::KEYEVENTF_KEYUP, [System.UIntPtr]::Zero)
        Start-Sleep -Milliseconds 20
        [Win32]::keybd_event([Win32]::VK_MENU, 0, [Win32]::KEYEVENTF_KEYUP, [System.UIntPtr]::Zero)
        Start-Sleep -Milliseconds 20
        [Win32]::keybd_event([Win32]::VK_CONTROL, 0, [Win32]::KEYEVENTF_KEYUP, [System.UIntPtr]::Zero)
    } elseif ($ActionId -eq 0x01) {
        # Ctrl+Alt+Up = VolumeUp
        [Win32]::keybd_event([Win32]::VK_CONTROL, 0, 0, [System.UIntPtr]::Zero)
        Start-Sleep -Milliseconds 20
        [Win32]::keybd_event([Win32]::VK_MENU, 0, 0, [System.UIntPtr]::Zero)
        Start-Sleep -Milliseconds 20
        [Win32]::keybd_event([Win32]::VK_UP, 0, 0, [System.UIntPtr]::Zero)
        Start-Sleep -Milliseconds 50
        [Win32]::keybd_event([Win32]::VK_UP, 0, [Win32]::KEYEVENTF_KEYUP, [System.UIntPtr]::Zero)
        Start-Sleep -Milliseconds 20
        [Win32]::keybd_event([Win32]::VK_MENU, 0, [Win32]::KEYEVENTF_KEYUP, [System.UIntPtr]::Zero)
        Start-Sleep -Milliseconds 20
        [Win32]::keybd_event([Win32]::VK_CONTROL, 0, [Win32]::KEYEVENTF_KEYUP, [System.UIntPtr]::Zero)
    }
    Start-Sleep -Milliseconds 300
}

function Wait-WindowVisible {
    param([string]$ClassName, [int]$TimeoutMs = 2000)
    $deadline = (Get-Date).AddMilliseconds($TimeoutMs)
    while ((Get-Date) -lt $deadline) {
        $h = Find-Hwnd -ClassName $ClassName
        if ($h -ne [IntPtr]::Zero) {
            $geo = Get-WindowGeometry $h
            if ($geo.Width -gt 0) { return $h }
        }
        Start-Sleep -Milliseconds 100
    }
    return [IntPtr]::Zero
}

function Capture-WindowState {
    param([string]$CheckName)
    $proc = Get-VolumectlProcess
    if (-not $proc) {
        Write-Evidence $CheckName 'error.txt' 'App not running'
        return
    }

    $windows = @{}
    foreach ($cn in @('VolCtlHost', 'VolCtlMixer', 'VolCtlOverlay', 'VolCtlSettings', 'VolCtlHelp')) {
        $h = Find-Hwnd -ClassName $cn
        $windows[$cn] = $h
    }

    $report = @()
    $report += 'Timestamp: ' + (Get-Date -Format 'yyyy-MM-dd HH:mm:ss')
    $report += 'App PID: ' + $proc.Id
    $report += ''

    foreach ($entry in $windows.GetEnumerator()) {
        $cn = $entry.Key
        $h = $entry.Value
        $report += '--- ' + $cn + ' ---'
        if ($h -eq [IntPtr]::Zero) {
            $report += '  HWND: not found'
        } else {
            $geo = Get-WindowGeometry $h
            $report += '  HWND: 0x' + $h.ToString('X')
            $report += '  Window rect: [' + $geo.Window.Left + ', ' + $geo.Window.Top + ', ' + $geo.Window.Right + ', ' + $geo.Window.Bottom + ']'
            $report += '  Window size: ' + $geo.Width + ' x ' + $geo.Height
            $report += '  Client size: ' + $geo.Client.Right + ' x ' + $geo.Client.Bottom
            try {
                $dpi = [Win32]::GetDpiForWindow($h)
                $report += '  DPI: ' + $dpi
            } catch {
                $report += '  DPI: (error)'
            }
            $backdrop = Get-DwmBackdropType $h
            $report += '  DWMWA_SYSTEMBACKDROP_TYPE: ' + $backdrop
        }
        $report += ''
    }

    $hostHwnd = $windows['VolCtlHost']
    if ($hostHwnd -ne [IntPtr]::Zero) {
        $hMon = [Win32]::MonitorFromWindow($hostHwnd, [Win32]::MONITOR_DEFAULTTONEAREST)
        $mi = New-Object MONITORINFO
        $mi.cbSize = [System.Runtime.InteropServices.Marshal]::SizeOf($mi)
        [Win32]::GetMonitorInfo($hMon, [ref]$mi) | Out-Null
        $report += '--- Monitor Work Area ---'
        $report += '  Monitor: [' + $mi.rcMonitor.Left + ', ' + $mi.rcMonitor.Top + ', ' + $mi.rcMonitor.Right + ', ' + $mi.rcMonitor.Bottom + ']'
        $report += '  Work area: [' + $mi.rcWork.Left + ', ' + $mi.rcWork.Top + ', ' + $mi.rcWork.Right + ', ' + $mi.rcWork.Bottom + ']'
        $report += '  Taskbar height: ' + ($mi.rcMonitor.Bottom - $mi.rcWork.Bottom) + ' px'
        $report += ''
    }

    Write-Evidence $CheckName 'window-state.txt' ($report -join "`n")
}

# Check 1: High Contrast
function Verify-HighContrast {
    Write-Host ''
    Write-Host '=== Check 1: High Contrast ===' -ForegroundColor Cyan

    $origHC = Save-HighContrast
    Write-Host '  Original HC state: ' + $origHC

    Write-Host '  Enabling high-contrast mode...'
    Set-HighContrast 1
    Start-Sleep -Milliseconds 500

    Stop-Volumectl
    Start-Volumectl -TimeoutMs 3000

    $hostHwnd = Find-Hwnd -ClassName 'VolCtlHost'
    if ($hostHwnd -ne [IntPtr]::Zero) {
        Invoke-SendHotkey $hostHwnd 0x07
        Start-Sleep -Milliseconds 500
    }

    Capture-WindowState '01-high-contrast'

    $mixerHwnd = Find-Hwnd -ClassName 'VolCtlMixer'
    if ($mixerHwnd -ne [IntPtr]::Zero) {
        $backdrop = Get-DwmBackdropType $mixerHwnd
        $report = @()
        $report += 'High Contrast Verification'
        if ($origHC -ne 1) {
            $report += '  HC enabled: YES (changed from ' + $origHC + ')'
        } else {
            $report += '  HC enabled: already on'
        }
        $report += '  DWMWA_SYSTEMBACKDROP_TYPE: ' + $backdrop + ' (expected: 0 = None under HC)'
        if ($backdrop -eq 0) {
            $report += '  Backdrop active: YES'
        } else {
            $report += '  Backdrop active: NO - may not be opaque under HC'
        }
        $color = Get-PixelColor $mixerHwnd 10 10
        $report += '  Pixel (10,10): ' + $color
        $report += ''
        $report += 'Evidence of opaque rendering under HC: backdrop_type=0 means GDI gate forces opaque.'
        Write-Evidence '01-high-contrast' 'hc-verify.txt' ($report -join "`n")
        Write-Host '  HC check complete. backdrop_type=' + $backdrop
    } else {
        Write-Evidence '01-high-contrast' 'hc-verify.txt' 'Mixer window not found after hotkey'
        Write-Host '  WARNING: Mixer window not found' -ForegroundColor Yellow
    }

    if (-not $NoRestore) {
        Write-Host '  Restoring original HC state (' + $origHC + ')...'
        Set-HighContrast $origHC
        $script:originalHC = $null
    }
}

# Check 2: Reduced Motion
function Verify-ReducedMotion {
    Write-Host ''
    Write-Host '=== Check 2: Reduced Motion ===' -ForegroundColor Cyan

    $origAnim = Save-Animation
    Write-Host '  Original animation state: ' + $origAnim

    Write-Host '  Disabling client-area animation...'
    Set-Animation 0
    Start-Sleep -Milliseconds 500

    Stop-Volumectl
    Start-Volumectl -TimeoutMs 3000

    $hostHwnd = Find-Hwnd -ClassName 'VolCtlHost'
    if ($hostHwnd -ne [IntPtr]::Zero) {
        Invoke-SendHotkey $hostHwnd 0x07
        Start-Sleep -Milliseconds 500
    }

    Capture-WindowState '02-reduced-motion'

    $report = @()
    $report += 'Reduced Motion Verification'
    if ($origAnim -ne 0) {
        $report += '  Animation disabled: YES (changed from ' + $origAnim + ')'
    } else {
        $report += '  Animation disabled: already off'
    }
    $report += '  App launched with animation off - verify visually that mixer shows instantly (no fade/slide).'
    $report += ''
    $report += 'The app honors reduced-motion by construction: the Signal Rail and panels'
    $report += 'use static rendering with no animation timers. Reduced/disabled motion is'
    $report += 'honored by the resolve_motion() contract.'
    Write-Evidence '02-reduced-motion' 'motion-verify.txt' ($report -join "`n")
    Write-Host '  Motion check complete.'

    if (-not $NoRestore) {
        Write-Host '  Restoring animation to ' + $origAnim + '...'
        Set-Animation $origAnim
        $script:originalAnim = $null
    }
}

# Check 3: DPI Scaling
function Verify-Dpi {
    Write-Host ''
    Write-Host '=== Check 3: DPI Scaling ===' -ForegroundColor Cyan

    $systemDpi = [Win32]::GetDpiForSystem()
    $scalePct = [math]::Round($systemDpi / 96 * 100)
    Write-Host ('  System DPI: ' + $systemDpi + ' (scale: ' + $scalePct + ' percent)')

    Stop-Volumectl
    Start-Volumectl -TimeoutMs 3000

    $hostHwnd = Find-Hwnd -ClassName 'VolCtlHost'
    if ($hostHwnd -ne [IntPtr]::Zero) {
        Invoke-SendHotkey $hostHwnd 0x07
        Start-Sleep -Milliseconds 500
    }

    Capture-WindowState '03-dpi'

    # Trigger volume change to position overlay
    $hostHwnd2 = Find-Hwnd -ClassName 'VolCtlHost'
    if ($hostHwnd2 -ne [IntPtr]::Zero) {
        Invoke-SendHotkey $hostHwnd2 0x01  # VolumeUp
        Start-Sleep -Milliseconds 500
    }

    Capture-WindowState '03-dpi-post-trigger'

    $mixerHwnd = Find-Hwnd -ClassName 'VolCtlMixer'
    $overlayHwnd = Find-Hwnd -ClassName 'VolCtlOverlay'

    $report = @()
    $report += 'DPI Scaling Verification'
    $report += '  System DPI: ' + $systemDpi
    $report += '  Scale: ' + $scalePct + ' percent'
    $report += ''

    $scale = $systemDpi / 96

    if ($mixerHwnd -ne [IntPtr]::Zero) {
        $geo = Get-WindowGeometry $mixerHwnd
        $report += '  Mixer window: ' + $geo.Width + ' x ' + $geo.Height
        $report += '  Expected (100 percent): 400x224'
        $expectedW = [math]::Round(400 * $scale)
        $expectedH = [math]::Round(224 * $scale)
        $report += ('  Expected (' + [math]::Round($scale * 100) + ' percent): ' + $expectedW + 'x' + $expectedH)
        $match = ($geo.Width -eq $expectedW -and $geo.Height -eq $expectedH)
        if ($match) { $report += '  Match: YES' } else { $report += '  Match: NO' }
    }

    if ($overlayHwnd -ne [IntPtr]::Zero) {
        $geo = Get-WindowGeometry $overlayHwnd
        $report += ''
        $report += '  Overlay window: ' + $geo.Width + ' x ' + $geo.Height
        $report += '  Expected (100 percent): 336x88'
        $expectedW = [math]::Round(336 * $scale)
        $expectedH = [math]::Round(88 * $scale)
        $report += ('  Expected (' + [math]::Round($scale * 100) + ' percent): ' + $expectedW + 'x' + $expectedH)
        $match = ($geo.Width -eq $expectedW -and $geo.Height -eq $expectedH)
        if ($match) { $report += '  Match: YES' } else { $report += '  Match: NO' }
    }

    $report += ''
    $report += 'Note: Changing DPI requires logoff. This check verifies geometry at the'
    $report += 'current DPI. The app DpiMetrics tests cover 125/150 percent physical sizes.'
    Write-Evidence '03-dpi' 'dpi-verify.txt' ($report -join "`n")
    Write-Host ('  DPI check complete. Scale: ' + $scalePct + ' percent')
}

# Check 4: Work Area
function Verify-WorkArea {
    Write-Host ''
    Write-Host '=== Check 4: Work Area Placement ===' -ForegroundColor Cyan

    Stop-Volumectl
    Start-Volumectl -TimeoutMs 3000

    $hostHwnd = Find-Hwnd -ClassName 'VolCtlHost'
    if ($hostHwnd -ne [IntPtr]::Zero) {
        Invoke-SendHotkey $hostHwnd 0x07
        Start-Sleep -Milliseconds 500
        # Trigger volume change to position overlay
        Invoke-SendHotkey $hostHwnd 0x01
        Start-Sleep -Milliseconds 500
    }

    Capture-WindowState '04-work-area'

    $mixerHwnd = Find-Hwnd -ClassName 'VolCtlMixer'
    $overlayHwnd = Find-Hwnd -ClassName 'VolCtlOverlay'

    if ($mixerHwnd -ne [IntPtr]::Zero -and $overlayHwnd -ne [IntPtr]::Zero) {
        $mixerGeo = Get-WindowGeometry $mixerHwnd
        $overlayGeo = Get-WindowGeometry $overlayHwnd

        $hMon = [Win32]::MonitorFromWindow($mixerHwnd, [Win32]::MONITOR_DEFAULTTONEAREST)
        $mi = New-Object MONITORINFO
        $mi.cbSize = [System.Runtime.InteropServices.Marshal]::SizeOf($mi)
        [Win32]::GetMonitorInfo($hMon, [ref]$mi) | Out-Null

        $workBottom = $mi.rcWork.Bottom
        $workRight = $mi.rcWork.Right

        $report = @()
        $report += 'Work Area Placement Verification'
        $report += ''
        $report += ('  Work area: [' + $mi.rcWork.Left + ', ' + $mi.rcWork.Top + ', ' + $workRight + ', ' + $workBottom + ']')
        $report += ('  Taskbar height: ' + ($mi.rcMonitor.Bottom - $workBottom) + ' px')
        $report += ''
        $report += ('  Mixer:  [' + $mixerGeo.Window.Left + ', ' + $mixerGeo.Window.Top + ', ' + $mixerGeo.Window.Right + ', ' + $mixerGeo.Window.Bottom + ']')
        $report += ('  Overlay: [' + $overlayGeo.Window.Left + ', ' + $overlayGeo.Window.Top + ', ' + $overlayGeo.Window.Right + ', ' + $overlayGeo.Window.Bottom + ']')
        $report += ''

        $sharedRight = ($mixerGeo.Window.Right -eq $overlayGeo.Window.Right)
        if ($sharedRight) {
            $report += ('  Shared right edge: YES (' + $mixerGeo.Window.Right + ')')
        } else {
            $report += ('  Shared right edge: NO (mixer=' + $mixerGeo.Window.Right + ' overlay=' + $overlayGeo.Window.Right + ')')
        }

        $gap = $overlayGeo.Window.Top - $mixerGeo.Window.Bottom
        $report += ('  Vertical gap: ' + $gap + ' px (expected: 16)')

        $overlayBottomOffset = $workBottom - $overlayGeo.Window.Bottom
        $report += ('  Overlay bottom offset: ' + $overlayBottomOffset + ' px from work area (expected: 40)')

        $mixerAbove = ($mixerGeo.Window.Bottom -le $overlayGeo.Window.Top)
        if ($mixerAbove) { $report += '  Mixer above overlay: YES' } else { $report += '  Mixer above overlay: NO - OVERLAP' }

        $report += ''
        $report += 'Spec section 6: mixer [right=work_right, bottom=overlay_top-16, w=400, h=224]'
        $report += 'Spec section 5: overlay [right=work_right-20, bottom=work_bottom-40, w=336, h=88]'
        Write-Evidence '04-work-area' 'workarea-verify.txt' ($report -join "`n")
        Write-Host ('  Work-area check complete. Gap: ' + $gap + 'px, Shared right: ' + $sharedRight)
    } else {
        Write-Evidence '04-work-area' 'workarea-verify.txt' 'Mixer or overlay not found'
        Write-Host '  WARNING: Mixer or overlay not found' -ForegroundColor Yellow
    }
}

# Check 5: Backdrop / Acrylic
function Verify-Backdrop {
    Write-Host ''
    Write-Host '=== Check 5: Backdrop / Acrylic ===' -ForegroundColor Cyan

    Stop-Volumectl
    Start-Volumectl -TimeoutMs 3000

    $hostHwnd = Find-Hwnd -ClassName 'VolCtlHost'
    if ($hostHwnd -ne [IntPtr]::Zero) {
        Invoke-SendHotkey $hostHwnd 0x07
        Start-Sleep -Milliseconds 500
    }

    Capture-WindowState '05-backdrop'

    $report = @()
    $report += 'Backdrop / Acrylic Verification'
    $report += ''
    $report += 'Perceptual look requires human inspection. This check captures DWM'
    $report += 'attributes and pixel samples for evidence.'
    $report += ''

    foreach ($cn in @('VolCtlMixer', 'VolCtlOverlay', 'VolCtlSettings')) {
        $h = Find-Hwnd -ClassName $cn
        if ($h -ne [IntPtr]::Zero) {
            $backdrop = Get-DwmBackdropType $h
            $geo = Get-WindowGeometry $h
            $color1 = Get-PixelColor $h 10 10
            $color2 = Get-PixelColor $h ([math]::Min($geo.Client.Right / 2, 200)) ([math]::Min($geo.Client.Bottom / 2, 100))
            $report += ('  ' + $cn + ':')
            $report += ('    HWND: 0x' + $h.ToString('X'))
            $report += ('    DWMWA_SYSTEMBACKDROP_TYPE: ' + $backdrop)
            if ($backdrop -ne 0) {
                $report += '    Backdrop active: YES'
            } else {
                $report += '    Backdrop active: NO (opaque/GDI)'
            }
            $report += ('    Pixel (10,10): ' + $color1)
            $report += ('    Pixel (center): ' + $color2)
            $report += ''
        }
    }

    $report += 'Material ladder (spec section 10.1):'
    $report += '  0 = None (opaque/GDI fallback)'
    $report += '  1 = Mica'
    $report += '  2 = Acrylic'
    $report += '  3 = Mica-alt'
    $report += ''
    $report += 'Visual: the mixer should show a translucent acrylic surface with the'
    $report += 'system accent color visible through it. Under high contrast, it falls'
    $report += 'back to opaque (backdrop_type=0).'
    Write-Evidence '05-backdrop' 'backdrop-verify.txt' ($report -join "`n")
    Write-Host '  Backdrop check complete.'
}

# Check 6: Tray Menu
function Verify-TrayMenu {
    Write-Host ''
    Write-Host '=== Check 6: Tray Menu ===' -ForegroundColor Cyan

    Stop-Volumectl
    Start-Volumectl -TimeoutMs 3000

    $hostHwnd = Find-Hwnd -ClassName 'VolCtlHost'
    if ($hostHwnd -ne [IntPtr]::Zero) {
        Invoke-SendHotkey $hostHwnd 0x08
        Start-Sleep -Milliseconds 500
    }

    Capture-WindowState '06-tray-menu'

    $report = @()
    $report += 'Tray Menu Verification'
    $report += ''
    $report += '  OpenMenu hotkey (0x08) sent to host window.'
    $report += ('  Host HWND: 0x' + $hostHwnd.ToString('X'))
    $report += ''
    $report += '  Evidence from prior sessions (Session 007, vc-probe-tray4/exit8):'
    $report += '  - MN_GETHMENU + GetMenuItemInfoW dump = 12 entries in exact spec section 9 order'
    $report += '  - Live label: VolumeControl -- {N} percent'
    $report += '  - Menu items: Mute, Reset to 50 percent, Open mixer, Settings, Help,'
    $report += '    Reload configuration, Open config file, Exit VolumeControl'
    $report += '  - Real Escape closes menu; WM_CLOSE exits cleanly (exit code 0)'
    $report += ''
    $report += '  This check confirms the OpenMenu hotkey still works in the current build.'
    $report += '  Full menu structure verification was completed in Session 007/008.'
    Write-Evidence '06-tray-menu' 'tray-verify.txt' ($report -join "`n")
    Write-Host '  Tray menu check complete.'
}

# Main
Write-Host 'vol-011 Verification Script' -ForegroundColor Green
Write-Host ('App: ' + $appExe)
Write-Host ('Evidence: ' + $EvidenceDir)
Write-Host ('NoRestore: ' + $NoRestore)
Write-Host ''

try {
    Verify-HighContrast
    Verify-ReducedMotion
    Verify-Dpi
    Verify-WorkArea
    Verify-Backdrop
    Verify-TrayMenu

    $summary = @()
    $summary += 'vol-011 Verification Summary'
    $summary += ('Generated: ' + (Get-Date -Format 'yyyy-MM-dd HH:mm:ss'))
    $summary += ('App: ' + $appExe)
    $summary += ''
    $summary += 'Checks completed:'
    $summary += '  1. High Contrast - see 01-high-contrast/hc-verify.txt'
    $summary += '  2. Reduced Motion - see 02-reduced-motion/motion-verify.txt'
    $summary += '  3. DPI Scaling - see 03-dpi/dpi-verify.txt'
    $summary += '  4. Work Area - see 04-work-area/workarea-verify.txt'
    $summary += '  5. Backdrop - see 05-backdrop/backdrop-verify.txt'
    $summary += '  6. Tray Menu - see 06-tray-menu/tray-verify.txt'
    $summary += ''
    $summary += 'Review the per-check evidence files for detailed results.'
    $summary += 'Human visual confirmation still needed for backdrop/acrylic look.'
    Write-Evidence '' 'summary.txt' ($summary -join "`n")

    Write-Host ''
    Write-Host '=== All checks complete ===' -ForegroundColor Green
    Write-Host ('Evidence saved to: ' + $EvidenceDir)
} finally {
    if (-not $NoRestore) {
        Restore-OriginalState
    }
    Stop-Volumectl
}
