# Query and drive VolumeControl through Windows UI Automation (UIA).
#
# Usage:
#   scripts/app-uia.ps1 -Windows                 # top-level windows for the process
#   scripts/app-uia.ps1 -Tree                    # accessible tree (first window)
#   scripts/app-uia.ps1 -Click "Đóng"            # click a control by accessible name
#   scripts/app-uia.ps1 -TrayMenu                # open the tray menu
#   scripts/app-uia.ps1 -TrayItem "Cài đặt"      # open the tray menu and invoke an item
#
# Exit code 1 means the requested element was not found or not invokable.

param(
    [string]$Process = 'VolumeControl-Egui',
    [switch]$Windows,
    [switch]$Tree,
    [switch]$TrayMenu,
    [string]$Click,
    [string]$TrayItem,
    [string]$TrayItemId,
    [int]$TrayIndex = 0,
    [int]$MaxDepth = 8
)

Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$AE = [System.Windows.Automation.AutomationElement]
$TS = [System.Windows.Automation.TreeScope]
$CT = [System.Windows.Automation.ControlType]
$InvokePatternType = [System.Windows.Automation.InvokePattern]

function Get-AppWindows {
    $procs = Get-Process -Name $Process -ErrorAction SilentlyContinue
    if (-not $procs) { throw "Process '$Process' is not running" }
    $found = @()
    foreach ($proc in $procs) {
        $cond = New-Object System.Windows.Automation.PropertyCondition(
            $AE::ProcessIdProperty, $proc.Id)
        foreach ($w in $AE::RootElement.FindAll($TS::Children, $cond)) {
            $found += $w
        }
    }
    return $found
}

function Show-Tree($element, $depth) {
    $rect = $element.Current.BoundingRectangle
    Write-Output ("{0}{1} [{2}] {3}@({4},{5})" -f `
            (' ' * ($depth * 2)),
        $element.Current.ControlType.ProgrammaticName,
        $element.Current.AutomationId,
        $element.Current.Name,
        [int]$rect.X, [int]$rect.Y)
    if ($depth -ge $MaxDepth) { return }
    foreach ($child in $element.FindAll($TS::Children, [System.Windows.Automation.Condition]::TrueCondition)) {
        Show-Tree $child ($depth + 1)
    }
}

function Find-Descendant($root, [string]$name) {
    $cond = New-Object System.Windows.Automation.PropertyCondition($AE::NameProperty, $name)
    return $root.FindFirst($TS::Descendants, $cond)
}

function Invoke-Element($element) {
    if ($null -eq $element) { return $false }
    $invoke = $null
    if ($element.TryGetCurrentPattern($InvokePatternType::Pattern, [ref]$invoke)) {
        $invoke.Invoke()
        return $true
    }
    # Menu items on Windows expose Invoke; tray icons may only be clickable.
    $rect = $element.Current.BoundingRectangle
    if ($rect.Width -gt 0 -and $rect.Height -gt 0) {
        Add-Type -AssemblyName System.Windows.Forms
        [System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point(
            [int]($rect.X + $rect.Width / 2), [int]($rect.Y + $rect.Height / 2))
        Start-Sleep -Milliseconds 80
        Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class UiaMouse {
    [DllImport("user32.dll")] public static extern void mouse_event(
        uint flags, uint dx, uint dy, uint data, UIntPtr extra);
}
'@ -ErrorAction SilentlyContinue
        [UiaMouse]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
        [UiaMouse]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
        return $true
    }
    return $false
}

$appWindows = Get-AppWindows
if ($Windows -or (-not $Tree -and -not $Click -and -not $TrayMenu -and -not $TrayItem -and -not $TrayItemId -and $TrayIndex -eq 0)) {
    foreach ($w in $appWindows) {
        $r = $w.Current.BoundingRectangle
        Write-Output ("window id={0} name='{1}' class='{2}' rect=({3},{4},{5},{6})" -f `
                $w.Current.NativeWindowHandle, $w.Current.Name, $w.Current.ClassName,
            [int]$r.X, [int]$r.Y, [int]$r.Width, [int]$r.Height)
    }
    exit 0
}

if ($Tree) {
    if (-not $appWindows) { throw 'No application window found' }
    Show-Tree $appWindows[0] 0
    exit 0
}

if ($Click) {
    foreach ($w in $appWindows) {
        $target = Find-Descendant $w $Click
        if ($target -and (Invoke-Element $target)) {
            Write-Output "clicked '$Click'"
            exit 0
        }
    }
    Write-Error "No invokable element named '$Click'"
    exit 1
}

# Tray: resolve the unique tooltip, expand the overflow when needed.
function Get-TrayIcon {
    $cond = New-Object System.Windows.Automation.PropertyCondition(
        $AE::NameProperty, 'Volume Control (egui)')
    $icon = $AE::RootElement.FindFirst($TS::Descendants, $cond)
    if (-not $icon) {
        # Reuse the standalone locator, which knows how to open the overflow.
        & "$PSScriptRoot/find-tray-icon.ps1" -Open | Out-Null
    }
    if (-not $icon) {
        foreach ($label in @('Show Hidden Icons', 'Show hidden icons', 'Hiển thị các biểu tượng ẩn')) {
            $overflow = $AE::RootElement.FindFirst($TS::Descendants,
                (New-Object System.Windows.Automation.PropertyCondition($AE::NameProperty, $label)))
            if ($overflow) {
                Invoke-Element $overflow | Out-Null
                Start-Sleep -Milliseconds 400
                break
            }
        }
        $icon = $AE::RootElement.FindFirst($TS::Descendants, $cond)
    }
    return $icon
}

function Open-TrayMenu {
    $icon = Get-TrayIcon
    if (-not $icon) { throw 'Tray icon not found' }
    $rect = $icon.Current.BoundingRectangle
    Add-Type -AssemblyName System.Windows.Forms
    [System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point(
        [int]($rect.X + $rect.Width / 2), [int]($rect.Y + $rect.Height / 2))
    Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class TrayRightClick {
    [DllImport("user32.dll")] public static extern void mouse_event(
        uint flags, uint dx, uint dy, uint data, UIntPtr extra);
}
'@ -ErrorAction SilentlyContinue
    [TrayRightClick]::mouse_event(0x0008, 0, 0, 0, [UIntPtr]::Zero)
    [TrayRightClick]::mouse_event(0x0010, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 500
    Write-Output ("tray at ({0},{1})" -f [int]($rect.X + $rect.Width / 2), [int]($rect.Y + $rect.Height / 2))
}

if ($TrayMenu) {
    Open-TrayMenu
    exit 0
}

if ($TrayIndex -gt 0) {
    Open-TrayMenu
    Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class UiaKeys {
    [DllImport("user32.dll")] public static extern void keybd_event(
        byte vk, byte scan, uint flags, UIntPtr extra);
    public static void Tap(byte vk) {
        keybd_event(vk, 0, 0, UIntPtr.Zero);
        keybd_event(vk, 0, 2, UIntPtr.Zero);
        System.Threading.Thread.Sleep(60);
    }
}
'@ -ErrorAction SilentlyContinue
    # Header is disabled, so Down skips it: 1..9 = mute, reset, mixer,
    # settings, help, check-updates, reload, open-config, exit.
    for ($i = 0; $i -lt $TrayIndex; $i++) { [UiaKeys]::Tap(0x28) }
    [UiaKeys]::Tap(0x0D)
    Write-Output "invoked tray index $TrayIndex"
    exit 0
}

if ($TrayItem -or $TrayItemId) {
    Open-TrayMenu
    $menuItem = $null
    if ($TrayItemId) {
        $cond = New-Object System.Windows.Automation.PropertyCondition(
            $AE::AutomationIdProperty, $TrayItemId)
    } else {
        $cond = New-Object System.Windows.Automation.PropertyCondition(
            $AE::NameProperty, $TrayItem)
    }
    $all = $AE::RootElement.FindAll($TS::Descendants, $cond)
    foreach ($element in $all) {
        if ($element.Current.ControlType -eq $CT::MenuItem) { $menuItem = $element; break }
    }
    if (-not $menuItem) {
        $wanted = if ($TrayItemId) { $TrayItemId } else { $TrayItem }
        throw "Tray menu item '$wanted' not found"
    }
    if (-not (Invoke-Element $menuItem)) { throw 'Tray menu item is not invokable' }
    Write-Output ("invoked {0}" -f $menuItem.Current.Name)
    exit 0
}
