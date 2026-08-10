using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public struct RECT { public int Left, Top, Right, Bottom; }
public struct MONITORINFO {
    public int cbSize;
    public RECT rcMonitor;
    public RECT rcWork;
    public uint dwFlags;
}

public static class Win32 {
    public delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr lParam);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc cb, IntPtr lParam);
    [DllImport("user32.dll")] public static extern int GetWindowThreadProcessId(IntPtr hwnd, out int lpdwProcessId);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr hwnd, StringBuilder lpString, int nMaxCount);
    [DllImport("user32.dll")] public static extern int GetWindowTextLength(IntPtr hwnd);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr hwnd, StringBuilder lpClassName, int nMaxCount);
    [DllImport("user32.dll")] public static extern IntPtr FindWindow(string lpClassName, string lpWindowName);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out RECT lpRect);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hwnd, out RECT lpRect);
    [DllImport("user32.dll")] public static extern int GetSystemMetricsForDpi(int nIndex, uint dpi);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern uint GetDpiForSystem();
    [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr hwnd, int dwAttribute, out int pvAttribute, int cbAttribute);
    public const int DWMWA_SYSTEMBACKDROP_TYPE = 38;
    public const int DWMWA_MICA_EFFECT = 1029;
    [DllImport("user32.dll")] public static extern IntPtr MonitorFromWindow(IntPtr hwnd, uint dwFlags);
    [DllImport("user32.dll")] public static extern bool GetMonitorInfo(IntPtr hMon, ref MONITORINFO lpmi);
    public const uint MONITOR_DEFAULTTONEAREST = 2;
    [DllImport("user32.dll")] public static extern IntPtr GetDC(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern IntPtr ReleaseDC(IntPtr hwnd, IntPtr hdc);
    [DllImport("gdi32.dll")] public static extern uint GetPixel(IntPtr hdc, int x, int y);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint msg, IntPtr wParam, IntPtr lParam);
    [DllImport("user32.dll")] public static extern bool SystemParametersInfo(int uiAction, int uiParam, ref int pvParam, int fWinIni);
    [DllImport("user32.dll")] public static extern bool SystemParametersInfo(int uiAction, int uiParam, ref RECT pvParam, int fWinIni);
    public const int SPI_GETHIGHCONTRAST = 0x0042;
    public const int SPI_GETCLIENTAREAANIMATION = 0x1042;
    public const int SPIF_UPDATEINIFILE = 0x01;
    public const int SPIF_SENDCHANGE = 0x02;
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hwnd, int nCmdShow);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, System.UIntPtr dwExtraInfo);
    public const int SW_SHOW = 5;
    public const int SW_HIDE = 0;
    public const byte VK_CONTROL = 0x11;
    public const byte VK_MENU = 0x12;
    public const byte VK_SHIFT = 0x10;
    public const byte VK_UP = 0x26;
    public const byte VK_DOWN = 0x28;
    public const uint KEYEVENTF_KEYUP = 0x0002;

    public static List<IntPtr> GetProcessWindows(int pid) {
        var result = new List<IntPtr>();
        EnumWindows((hwnd, _) => {
            int windowPid;
            GetWindowThreadProcessId(hwnd, out windowPid);
            if (windowPid == pid) result.Add(hwnd);
            return true;
        }, IntPtr.Zero);
        return result;
    }

    public static string GetWindowClassName(IntPtr hwnd) {
        var sb = new StringBuilder(256);
        GetClassName(hwnd, sb, 256);
        return sb.ToString();
    }

    public static string GetWindowTitle(IntPtr hwnd) {
        int len = GetWindowTextLength(hwnd);
        if (len == 0) return string.Empty;
        var sb = new StringBuilder(len + 1);
        GetWindowText(hwnd, sb, sb.Capacity);
        return sb.ToString();
    }

    // PrintWindow for layered/transparent window capture
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hwnd, IntPtr hdcBlt, uint nFlags);
    public const uint PW_CLIENTONLY = 0x00000001;
    public const uint PW_RENDERFULLCONTENT = 0x00000002;
    [DllImport("gdi32.dll")] public static extern IntPtr CreateCompatibleDC(IntPtr hdc);
    [DllImport("gdi32.dll")] public static extern IntPtr CreateCompatibleBitmap(IntPtr hdc, int nWidth, int nHeight);
    [DllImport("gdi32.dll")] public static extern IntPtr SelectObject(IntPtr hdc, IntPtr hgdiobj);
    [DllImport("gdi32.dll")] public static extern bool DeleteDC(IntPtr hdc);
    [DllImport("gdi32.dll")] public static extern bool DeleteObject(IntPtr hObject);
    [DllImport("gdi32.dll")] public static extern bool BitBlt(IntPtr hdcDest, int xDest, int yDest, int wDest, int hDest, IntPtr hdcSrc, int xSrc, int ySrc, uint dwRop);
    public const uint SRCCOPY = 0x00CC0020;
    [DllImport("gdi32.dll")] public static extern IntPtr GetStockObject(int i);
    public const int WHITE_BRUSH = 0;
}
