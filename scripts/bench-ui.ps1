param(
  [string]$Exe,
  [string]$Arguments = "",
  [int]$IdleSeconds = 10
)

$ErrorActionPreference = "Stop"
Get-Process VolumeControl-Egui -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 300

$sw = [Diagnostics.Stopwatch]::StartNew()
$proc = Start-Process -FilePath $Exe -ArgumentList $Arguments -PassThru
$window = 0
while ($sw.Elapsed.TotalSeconds -lt 20) {
  $proc.Refresh()
  if ($proc.MainWindowHandle -ne 0) { $window = $sw.Elapsed.TotalMilliseconds; break }
  Start-Sleep -Milliseconds 25
}
if ($window -eq 0) { Write-Output "startup_ms=timeout"; $proc | Stop-Process -Force; exit 1 }
Start-Sleep -Seconds $IdleSeconds
$proc.Refresh()
$rss = [math]::Round($proc.WorkingSet64 / 1MB, 1)
$cpu1 = $proc.TotalProcessorTime.TotalMilliseconds
Start-Sleep -Seconds $IdleSeconds
$proc.Refresh()
$cpu2 = $proc.TotalProcessorTime.TotalMilliseconds
$idle = [math]::Round((($cpu2 - $cpu1) / ($IdleSeconds * 1000)) * 100, 2)
$rss2 = [math]::Round($proc.WorkingSet64 / 1MB, 1)
Write-Output ("startup_ms={0:N0} rss_mb={1} rss_after_mb={2} idle_cpu_pct={3}" -f $window, $rss, $rss2, $idle)
$proc | Stop-Process -Force
