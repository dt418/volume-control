//! Windows-only mouse-wheel bridge host window.
//!
//! `volumectl::wheel_win32` installs a low-level mouse hook and posts
//! `WM_APP_WHEEL` to a window handle supplied at install time. Winit does not
//! expose a message-only window, so the egui host creates one here and
//! forwards decoded hotkey actions to the eframe loop.

use std::sync::mpsc;

use volumectl::hotkeys::HotkeyAction;

/// Owns the hidden message window and the wheel hook lifetime.
pub struct WheelBridge {
    #[cfg(target_os = "windows")]
    hwnd: windows_sys::Win32::Foundation::HWND,
}

/// Install the wheel hook and return the bridge plus the action receiver.
#[cfg(target_os = "windows")]
pub fn install() -> Result<(WheelBridge, mpsc::Receiver<HotkeyAction>), String> {
    use std::sync::OnceLock;

    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, RegisterClassW, CW_USEDEFAULT, HMENU, HWND_MESSAGE,
        WM_APP, WNDCLASSW, WS_OVERLAPPED,
    };

    static SENDER: OnceLock<std::sync::Mutex<mpsc::Sender<HotkeyAction>>> = OnceLock::new();

    unsafe extern "system" fn wnd_proc(
        hwnd: windows_sys::Win32::Foundation::HWND,
        msg: u32,
        wparam: windows_sys::Win32::Foundation::WPARAM,
        lparam: windows_sys::Win32::Foundation::LPARAM,
    ) -> windows_sys::Win32::Foundation::LRESULT {
        const WM_APP_WHEEL: u32 = WM_APP + 2;
        if msg == WM_APP_WHEEL {
            if let Some(sender) = SENDER.get() {
                if let Ok(guard) = sender.lock() {
                    if let Some(action) = volumectl::hotkeys::hotkey_from_id(wparam as i32) {
                        let _ = guard.send(action);
                    }
                }
            }
            return 0;
        }
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    let (sender, receiver) = mpsc::channel();
    let _ = SENDER.set(std::sync::Mutex::new(sender));

    let instance = unsafe { GetModuleHandleW(std::ptr::null()) };
    let class_name = windows_sys::core::w!("VolumeControlEguiWheelBridge");
    let class = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        hIcon: 0,
        hCursor: 0,
        hbrBackground: 0,
        lpszMenuName: std::ptr::null(),
        lpszClassName: class_name,
    };
    // SAFETY: the class name and callback outlive the window; registering the
    // same class twice is harmless because only one bridge is created.
    unsafe {
        RegisterClassW(&class);
    }
    // SAFETY: message-only window creation with a valid class name and no
    // parent (HWND_MESSAGE marks a message-only window).
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class_name,
            windows_sys::core::w!("VolumeControl Egui Wheel Bridge"),
            WS_OVERLAPPED,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            HWND_MESSAGE,
            0 as HMENU,
            instance,
            std::ptr::null_mut(),
        )
    };
    if hwnd == 0 {
        return Err("wheel bridge window creation failed".into());
    }
    volumectl::wheel_win32::install_wheel_hook(hwnd)?;
    Ok((WheelBridge { hwnd }, receiver))
}

/// Non-Windows: no wheel gesture exists, so no bridge is created.
#[cfg(not(target_os = "windows"))]
pub fn install() -> Result<(WheelBridge, mpsc::Receiver<HotkeyAction>), String> {
    Err("mouse-wheel bridge is Windows-only".into())
}

#[cfg(target_os = "windows")]
impl Drop for WheelBridge {
    fn drop(&mut self) {
        volumectl::wheel_win32::uninstall_wheel_hook();
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(self.hwnd);
        }
    }
}
