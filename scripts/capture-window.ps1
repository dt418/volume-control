# Capture a top-level window to PNG: -Hwnd 12345 -Out C:\temp\shot.png
param([Parameter(Mandatory = $true)][int64]$Hwnd,
    [Parameter(Mandatory = $true)][string]$Out)

Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public struct RECT { public int Left, Top, Right, Bottom; }
public static class WinRect {
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hwnd, int cmd);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(
        IntPtr hwnd, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern bool PrintWindow(
        IntPtr hwnd, IntPtr hdc, uint flags);
}
'@

$handle = [IntPtr]::new($Hwnd)
[WinRect]::ShowWindow($handle, 9) | Out-Null
# HWND_TOPMOST=(-1), SWP_NOMOVE|NOSIZE|SHOWWINDOW=0x0003|0x0001|0x0040.
[WinRect]::SetWindowPos($handle, [IntPtr](-1), 0, 0, 0, 0, 0x0043) | Out-Null
[WinRect]::SetForegroundWindow($handle) | Out-Null
Start-Sleep -Milliseconds 400

$rect = New-Object RECT
if (-not [WinRect]::GetWindowRect($handle, [ref]$rect)) { throw 'GetWindowRect failed' }
$width = $rect.Right - $rect.Left
$height = $rect.Bottom - $rect.Top
if ($width -le 0 -or $height -le 0) { throw "Window is not visible ($width x $height)" }

$bmp = New-Object System.Drawing.Bitmap $width, $height
$graphics = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $graphics.GetHdc()
# PW_RENDERFULLCONTENT captures DirectComposition/wgpu surfaces even when
# another window overlaps them.
$ok = [WinRect]::PrintWindow($handle, $hdc, 2)
$graphics.ReleaseHdc($hdc)
if (-not $ok) {
    $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bmp.Size)
}
$graphics.Dispose()
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
Write-Output "saved $Out"
