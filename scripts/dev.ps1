# Development loop: rebuild, restart, and debug
#
# Usage:
#   powershell -File scripts/dev.ps1                 # mixer + egui debug overlay
#   powershell -File scripts/dev.ps1 -Args '--surface=settings','--debug-ui'
#
# Hot reload: eframe keeps live GPU/UI state in-process, so a true in-process
# hot-swap is not supported. This script gives the practical equivalent: it
# watches the two UI crates and restarts the app (about one second) on change.

param(
    [string[]]$Args = @('--surface=mixer', '--debug-ui'),
    [string]$Exe = 'target/debug/VolumeControl-Egui.exe'
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$watch = @(
    (Join-Path $root 'crates/volumecontrol-egui/src'),
    (Join-Path $root 'crates/volumectl/src')
)
$env:RUST_BACKTRACE = '1'
$env:RUST_LOG = 'info,volumecontrol_egui=debug'

function Get-LatestWrite {
    $latest = [DateTime]::MinValue
    foreach ($dir in $watch) {
        $file = Get-ChildItem $dir -Recurse -File | Sort-Object LastWriteTime -Descending |
            Select-Object -First 1
        if ($file -and $file.LastWriteTime -gt $latest) {
            $latest = $file.LastWriteTime
        }
    }
    return $latest
}

$stamp = [DateTime]::MinValue
$app = $null

while ($true) {
    $latest = Get-LatestWrite
    if (-not $app -or $latest -gt $stamp) {
        if ($app -and -not $app.HasExited) {
            $app.Kill()
            $app.WaitForExit()
        }
        Write-Host 'building…'
        & cargo build -p volumecontrol-egui
        if ($LASTEXITCODE -ne 0) {
            Write-Host 'build failed; waiting for the next edit'
            $stamp = $latest
            Start-Sleep -Milliseconds 800
            continue
        }
        $app = Start-Process -FilePath (Join-Path $root $Exe) -ArgumentList $Args -PassThru
        Write-Host ("started pid {0}" -f $app.Id)
        $stamp = $latest
    }
    Start-Sleep -Milliseconds 600
}
