#!/usr/bin/env pwsh
param(
    [string]$Binary = 'target/debug/VolumeControl.exe',
    [string]$OutputRoot = 'output/desktop-launch',
    [int]$TimeoutSeconds = 20
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
if (-not $csc) { throw 'csc.exe not found; Windows .NET Framework is required.' }
$dllPath = Join-Path ([IO.Path]::GetTempPath()) "volume-control-launch-$PID.dll"
& $csc /nologo /target:library /out:$dllPath /reference:System.Drawing.dll $csFile 2>&1 | Write-Host
if ($LASTEXITCODE -ne 0) { throw 'Failed to compile the Win32 helper.' }
Add-Type -Path $dllPath

$configDir = Join-Path ([IO.Path]::GetTempPath()) "volume-control-launch-$PID"
New-Item -ItemType Directory -Force -Path $configDir | Out-Null
$started = [System.Collections.Generic.List[Diagnostics.Process]]::new()

function Start-Host {
    param([string[]]$Arguments = @())
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $binaryPath
    $start.WorkingDirectory = $repoRoot
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.Arguments = ($Arguments -join ' ')
    $start.Environment['VOLUMECTL_CONFIG_DIR'] = $configDir
    $process = [Diagnostics.Process]::Start($start)
    $script:started.Add($process)
    return $process
}

function Get-MixerWindow([int]$ProcessId) {
    foreach ($handle in [Win32]::GetProcessWindows($ProcessId)) {
        if ([Win32]::IsWindowVisible($handle) -and
            [Win32]::GetWindowTitle($handle) -eq 'Volume Mixer') {
            return $handle
        }
    }
    return [IntPtr]::Zero
}

function Wait-MixerWindow([Diagnostics.Process]$Process, [int]$TimeoutMs) {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    do {
        if ($Process.HasExited) { throw "Process exited early with code $($Process.ExitCode)." }
        $handle = Get-MixerWindow $Process.Id
        if ($handle -ne [IntPtr]::Zero) { return $handle }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "No visible Volume Mixer within $TimeoutMs ms."
}

function Wait-NoMixer([Diagnostics.Process]$Process, [int]$TimeoutMs) {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    do {
        if ($Process.HasExited) { throw "Background process exited with code $($Process.ExitCode)." }
        if ((Get-MixerWindow $Process.Id) -ne [IntPtr]::Zero) {
            throw 'Background launch showed the Volume Mixer.'
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
}

function Wait-MixerClosed([Diagnostics.Process]$Process, [int]$TimeoutMs) {
    $deadline = [DateTime]::UtcNow.AddMilliseconds($TimeoutMs)
    do {
        if ($Process.HasExited) { throw "Primary process exited while closing the mixer with code $($Process.ExitCode)." }
        if ((Get-MixerWindow $Process.Id) -eq [IntPtr]::Zero) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Visible Volume Mixer did not close within $TimeoutMs ms."
}


try {
    Get-Process VolumeControl -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -eq $binaryPath } |
        Stop-Process -Force

    $primary = Start-Host
    $mixer = Wait-MixerWindow $primary ($TimeoutSeconds * 1000)
    Write-Host 'PASS foreground launch opens the mixer'

    if (-not [Win32]::PostMessage($mixer, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)) {
        throw 'Failed to close the Volume Mixer.'
    }
    Wait-MixerClosed $primary 5000

    $second = Start-Host
    if (-not $second.WaitForExit(5000)) { throw 'Second foreground launch did not exit.' }
    if ($second.ExitCode -ne 0) { throw "Second foreground launch exited with code $($second.ExitCode)." }
    if ($primary.HasExited) { throw 'Primary exited after the second launch.' }
    [void](Wait-MixerWindow $primary 5000)
    Write-Host 'PASS second foreground launch keeps the primary and reopens the mixer'

    $primary | Stop-Process -Force
    $primary.WaitForExit(5000) | Out-Null

    $background = Start-Host @('--background')
    Wait-NoMixer $background 3000
    Write-Host 'PASS background launch stays windowless'

    $backgroundSecond = Start-Host @('--background')
    if (-not $backgroundSecond.WaitForExit(5000)) { throw 'Second background launch did not exit.' }
    if ($backgroundSecond.ExitCode -ne 0) { throw "Second background launch exited with code $($backgroundSecond.ExitCode)." }
    Wait-NoMixer $background 3000
    Write-Host 'PASS duplicate background launch stays windowless'

    $foreground = Start-Host
    if (-not $foreground.WaitForExit(5000)) { throw 'Foreground wake-up launch did not exit.' }
    if ($foreground.ExitCode -ne 0) { throw "Foreground wake-up launch exited with code $($foreground.ExitCode)." }
    [void](Wait-MixerWindow $background ($TimeoutSeconds * 1000))
    Write-Host 'PASS foreground launch wakes a background primary'
} finally {
    foreach ($process in $started) {
        if (-not $process.HasExited) { $process.Kill() }
        $process.Dispose()
    }
}
