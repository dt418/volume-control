//! Windows-only mouse-wheel bridge for the configured volume shortcuts.
//!
//! Keyboard hotkey combos are registered through the cross-platform
//! `global-hotkey` backend, which registers key combos only — it does not
//! observe mouse input. The wheel gesture therefore stays a small native
//! Windows hook that feeds the same action pipeline as the hotkeys.

use std::sync::atomic::{AtomicIsize, AtomicU8, Ordering};

use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    System::LibraryLoader::GetModuleHandleW,
    UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_MENU, VK_RCONTROL, VK_RMENU,
        VK_SHIFT,
    },
    UI::WindowsAndMessaging::{
        CallNextHookEx, PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx, MSLLHOOKSTRUCT,
        WH_MOUSE_LL, WM_APP, WM_MOUSEWHEEL,
    },
};

use crate::config::HotkeyModifier;
use crate::hotkeys::{hotkey_id, HotkeyAction};

/// Custom message posted to the Windows host for a wheel action.
pub const WM_APP_WHEEL: u32 = WM_APP + 2;

const WHEEL_DELTA: i32 = 120;

static HOST_HWND: AtomicIsize = AtomicIsize::new(0);
static HOOK: AtomicIsize = AtomicIsize::new(0);
static ACTIVE_MODIFIER: AtomicU8 = AtomicU8::new(0);

fn modifier_code(modifier: HotkeyModifier) -> u8 {
    match modifier {
        HotkeyModifier::CtrlAlt => 0,
        HotkeyModifier::Alt => 1,
        HotkeyModifier::Ctrl => 2,
        HotkeyModifier::CapsLock => 3,
    }
}

fn active_modifier() -> HotkeyModifier {
    match ACTIVE_MODIFIER.load(Ordering::Acquire) {
        1 => HotkeyModifier::Alt,
        2 => HotkeyModifier::Ctrl,
        3 => HotkeyModifier::CapsLock,
        _ => HotkeyModifier::CtrlAlt,
    }
}

/// Keep the wheel gesture in sync with config reloads.
pub fn set_modifier(modifier: HotkeyModifier) {
    ACTIVE_MODIFIER.store(modifier_code(modifier), Ordering::Release);
}

fn key_held(key: u16) -> bool {
    unsafe { (GetAsyncKeyState(key as i32) as i16) < 0 }
}

fn modifier_held(modifier: HotkeyModifier) -> bool {
    let ctrl = key_held(VK_CONTROL) || key_held(VK_LCONTROL) || key_held(VK_RCONTROL);
    let alt = key_held(VK_MENU) || key_held(VK_LMENU) || key_held(VK_RMENU);
    match modifier {
        HotkeyModifier::CtrlAlt => ctrl && alt,
        HotkeyModifier::Alt => alt,
        HotkeyModifier::Ctrl => ctrl,
        // CapsLock is a keyboard-only modifier in the origin project.
        HotkeyModifier::CapsLock => false,
    }
}

pub fn install_wheel_hook(hwnd: HWND) -> Result<(), String> {
    if HOOK.load(Ordering::Acquire) != 0 {
        return Ok(());
    }

    let hook = unsafe {
        SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(mouse_proc),
            GetModuleHandleW(std::ptr::null()),
            0,
        )
    };
    if hook == 0 {
        return Err("SetWindowsHookEx(WH_MOUSE_LL) failed".into());
    }
    HOST_HWND.store(hwnd, Ordering::Release);
    HOOK.store(hook as isize, Ordering::Release);
    log::debug!("mouse wheel hook installed");
    Ok(())
}

pub fn uninstall_wheel_hook() {
    let hook = HOOK.swap(0, Ordering::AcqRel);
    if hook != 0 {
        unsafe {
            UnhookWindowsHookEx(hook as _);
        }
    }
    HOST_HWND.store(0, Ordering::Release);
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && wparam as u32 == WM_MOUSEWHEEL && lparam != 0 {
        let state = &*(lparam as *const MSLLHOOKSTRUCT);
        let delta = ((state.mouseData >> 16) as u16 as i16 as i32) / WHEEL_DELTA;
        if delta != 0 && modifier_held(active_modifier()) {
            let shifted = key_held(VK_SHIFT);
            let action = match (delta > 0, shifted) {
                (true, false) => HotkeyAction::VolumeUp,
                (true, true) => HotkeyAction::VolumeUpLarge,
                (false, false) => HotkeyAction::VolumeDown,
                (false, true) => HotkeyAction::VolumeDownLarge,
            };
            let hwnd = HOST_HWND.load(Ordering::Acquire);
            if hwnd != 0 {
                PostMessageW(hwnd as HWND, WM_APP_WHEEL, hotkey_id(action) as usize, 0);
                return 1;
            }
        }
    }
    CallNextHookEx(0, code, wparam, lparam)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The modifier → wheel-encoding mapping shared by the startup path
    /// (AppCore::new) and every modifier-change path must round-trip.
    #[test]
    fn modifier_code_round_trips_every_variant() {
        for modifier in [
            HotkeyModifier::CtrlAlt,
            HotkeyModifier::Alt,
            HotkeyModifier::Ctrl,
            HotkeyModifier::CapsLock,
        ] {
            let code = modifier_code(modifier);
            assert_eq!(active_modifier_for(code), modifier, "code {code}");
        }
    }

    /// Encodings are stable and distinct (changing them would silently break
    /// the wheel gesture for a persisted config modifier).
    #[test]
    fn modifier_encodings_are_stable() {
        assert_eq!(modifier_code(HotkeyModifier::CtrlAlt), 0);
        assert_eq!(modifier_code(HotkeyModifier::Alt), 1);
        assert_eq!(modifier_code(HotkeyModifier::Ctrl), 2);
        assert_eq!(modifier_code(HotkeyModifier::CapsLock), 3);
    }

    #[test]
    fn set_modifier_updates_active_state() {
        set_modifier(HotkeyModifier::Alt);
        assert_eq!(active_modifier(), HotkeyModifier::Alt);
        set_modifier(HotkeyModifier::Ctrl);
        assert_eq!(active_modifier(), HotkeyModifier::Ctrl);
        set_modifier(HotkeyModifier::CtrlAlt);
        assert_eq!(active_modifier(), HotkeyModifier::CtrlAlt);
    }

    // Test-only helper mirroring `active_modifier()` with the code value
    // directly, so the round-trip test exercises both directions.
    fn active_modifier_for(code: u8) -> HotkeyModifier {
        match code {
            1 => HotkeyModifier::Alt,
            2 => HotkeyModifier::Ctrl,
            3 => HotkeyModifier::CapsLock,
            _ => HotkeyModifier::CtrlAlt,
        }
    }
}
