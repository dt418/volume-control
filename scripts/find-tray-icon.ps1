# Locate the VolumeControl tray icon through UI Automation.
#
# Usage:
#   pwsh -File scripts/find-tray-icon.ps1            # print the icon bounds
#   pwsh -File scripts/find-tray-icon.ps1 -Open     # expand the overflow, then print
#   pwsh -File scripts/find-tray-icon.ps1 -Left     # left-click the icon
#   pwsh -File scripts/find-tray-icon.ps1 -Right    # open the context menu
#
# The tray tooltip is unique ("Volume Control (egui)") so the lookup never
# depends on screen coordinates or icon artwork.

param(
    [switch]$Open,
    [switch]$Left,
    [switch]$Right,
    [string]$Name = 'Volume Control (egui)'
)

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$automation = [System.Windows.Automation.AutomationElement]
$scope = [System.Windows.Automation.TreeScope]
$condition = New-Object System.Windows.Automation.PropertyCondition(
    $automation::NameProperty, $Name)

function Find-Icon {
    return $automation::RootElement.FindFirst($scope::Descendants, $condition)
}

$icon = Find-Icon
if (-not $icon -and $Open) {
    # Windows 11 stores inactive icons in the overflow flyout. Its button is
    # named "Show hidden icons" / "Hiển thị các biểu tượng ẩn".
    foreach ($label in @('Show Hidden Icons', 'Show hidden icons', 'Hiển thị các biểu tượng ẩn')) {
        $overflowCondition = New-Object System.Windows.Automation.PropertyCondition(
            $automation::NameProperty, $label)
        $overflow = $automation::RootElement.FindFirst($scope::Descendants, $overflowCondition)
        if ($overflow) {
            $pattern = $null
            if ($overflow.TryGetCurrentPattern(
                    [System.Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) {
                $pattern.Invoke()
                Start-Sleep -Milliseconds 400
                break
            }
        }
    }
    $icon = Find-Icon
}

if (-not $icon) {
    Write-Error "Tray icon '$Name' was not found. Run with -Open to expand the overflow area."
    exit 1
}

$rect = $icon.Current.BoundingRectangle
$x = [int]($rect.X + $rect.Width / 2)
$y = [int]($rect.Y + $rect.Height / 2)
Write-Output ("found={0} x={1} y={2} w={3} h={4}" -f `
        $icon.Current.Name, $x, $y, [int]$rect.Width, [int]$rect.Height)

if ($Left -or $Right) {
    Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class TrayMouse {
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(
        uint flags, uint dx, uint dy, uint data, UIntPtr extra);
    public const uint LEFTDOWN = 0x0002, LEFTUP = 0x0004;
    public const uint RIGHTDOWN = 0x0008, RIGHTUP = 0x0010;
    public static void Click(int x, int y, bool right) {
        SetCursorPos(x, y);
        System.Threading.Thread.Sleep(80);
        uint down = right ? RIGHTDOWN : LEFTDOWN;
        uint up = right ? RIGHTUP : LEFTUP;
        mouse_event(down, 0, 0, 0, UIntPtr.Zero);
        mouse_event(up, 0, 0, 0, UIntPtr.Zero);
    }
}
'@
    [TrayMouse]::Click($x, $y, [bool]$Right)
}
