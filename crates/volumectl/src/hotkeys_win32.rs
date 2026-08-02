//! Windows global-hotkey backend via Win32 `RegisterHotKey` + a low-level
//! mouse hook for the scroll-wheel combos.
//!
//! Keyboard combos are registered against a caller-provided window handle;
//! `WM_HOTKEY` messages carry the hotkey id in `WPARAM` (decoded by
//! [`hotkey_action`]).
//!
//! `Mod+Scroll` cannot be expressed with `RegisterHotKey`, so a `WH_MOUSE_LL`
//! hook watches for `WM_MOUSEWHEEL` while the configured modifier is held and
//! posts an action id to the host window (which the app applies like a
//! hotkey). The wheel event is swallowed so it doesn't scroll the page.
//!
//! Media keys (`Volume_Up/Down/Mute`) are intentionally *not* registered here:
//! they keep the OS default (native flyout). The app polls the audio state to
//! stay in sync with them.

use std::sync::atomic::{AtomicI64, AtomicIsize, AtomicU8, Ordering};

use windows_sys::Win32::{
    Foundation::{BOOL, FALSE, HWND, LPARAM, LRESULT, WPARAM},
    System::LibraryLoader::GetModuleHandleW,
    UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
        MOD_SHIFT, VIRTUAL_KEY, VK_CAPITAL, VK_CONTROL, VK_DOWN, VK_M, VK_MENU, VK_R, VK_SHIFT,
        VK_UP, VK_V,
    },
    UI::WindowsAndMessaging::{
        CallNextHookEx, PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx, KBDLLHOOKSTRUCT,
        MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_APP, WM_KEYDOWN, WM_MOUSEWHEEL,
        WM_SYSKEYDOWN,
    },
};

use crate::config::HotkeyModifier;
use crate::hotkeys::HotkeyAction;

// Hotkey ids — internal to this module, encode "key + shifted" as separate ids.
pub const ID_VOL_UP: i32 = 0x01;
pub const ID_VOL_DOWN: i32 = 0x02;
pub const ID_VOL_UP_LARGE: i32 = 0x03;
pub const ID_VOL_DOWN_LARGE: i32 = 0x04;
pub const ID_MUTE: i32 = 0x05;
pub const ID_RESET: i32 = 0x06;
pub const ID_MIXER: i32 = 0x07;
pub const ID_SHOW_MENU: i32 = 0x08;

/// Custom message posted to the host window by the mouse-wheel hook.
/// `wparam` carries a [`HotkeyAction`] id, handled like `WM_HOTKEY`.
pub const WM_APP_WHEEL: u32 = WM_APP + 2;

const WHEEL_DELTA: i32 = 120;

/// Host window that receives the hook messages.
static HOOK_HOST_HWND: AtomicIsize = AtomicIsize::new(0);
/// The installed low-level mouse hook (0 = not installed).
static HOOK: AtomicIsize = AtomicIsize::new(0);
/// The installed low-level keyboard hook — CapsLock combos only (0 = not installed).
static KEYBOARD_HOOK: AtomicIsize = AtomicIsize::new(0);
/// The modifier the hook-based combos must match, kept in sync by
/// [`Win32Hotkeys::register`] so a config reload applies to the hooks too.
static ACTIVE_MODIFIER: AtomicU8 = AtomicU8::new(0);
/// Last handled (vk << 32 | time) for a CapsLock combo, for repeat suppression.
static LAST_COMBO_KEY: AtomicI64 = AtomicI64::new(0);

fn active_modifier() -> HotkeyModifier {
    match ACTIVE_MODIFIER.load(Ordering::SeqCst) {
        1 => HotkeyModifier::Alt,
        2 => HotkeyModifier::Ctrl,
        3 => HotkeyModifier::CapsLock,
        _ => HotkeyModifier::CtrlAlt,
    }
}

fn ctrl_held() -> bool {
    unsafe { (GetAsyncKeyState(VK_CONTROL as i32) as i16) < 0 }
}

fn alt_held() -> bool {
    unsafe { (GetAsyncKeyState(VK_MENU as i32) as i16) < 0 }
}

fn shift_held() -> bool {
    unsafe { (GetAsyncKeyState(VK_SHIFT as i32) as i16) < 0 }
}

fn caps_held() -> bool {
    unsafe { (GetAsyncKeyState(VK_CAPITAL as i32) as i16) < 0 }
}

/// Whether the physical state currently matches `modifier`.
fn modifier_held(m: HotkeyModifier) -> bool {
    match m {
        HotkeyModifier::CtrlAlt => ctrl_held() && alt_held(),
        HotkeyModifier::Ctrl => ctrl_held(),
        HotkeyModifier::Alt => alt_held(),
        // CapsLock as a held modifier (physical key down), like VolumePro.
        HotkeyModifier::CapsLock => caps_held(),
    }
}

/// Install the `WH_MOUSE_LL` hook (Mod+Scroll) and the `WH_KEYBOARD_LL` hook
/// (CapsLock combos). Both read [`ACTIVE_MODIFIER`], so they follow config.
/// The installing thread must pump messages (the app's message loop does).
pub fn install_wheel_hook(hwnd: HWND) -> Result<(), String> {
    if HOOK.load(Ordering::SeqCst) == 0 {
        unsafe {
            let hinst = GetModuleHandleW(std::ptr::null());
            let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), hinst, 0);
            if hook == 0 {
                return Err("SetWindowsHookEx(WH_MOUSE_LL) failed".into());
            }
            HOOK.store(hook, Ordering::SeqCst);
        }
    }
    if KEYBOARD_HOOK.load(Ordering::SeqCst) == 0 {
        unsafe {
            let hinst = GetModuleHandleW(std::ptr::null());
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(key_proc), hinst, 0);
            if hook == 0 {
                // Not fatal: only CapsLock combos depend on it.
                log::warn!("SetWindowsHookEx(WH_KEYBOARD_LL) failed");
            } else {
                KEYBOARD_HOOK.store(hook, Ordering::SeqCst);
            }
        }
    }
    HOOK_HOST_HWND.store(hwnd, Ordering::SeqCst);
    log::debug!("mouse wheel + keyboard hooks installed");
    Ok(())
}

/// Remove both hooks (idempotent).
pub fn uninstall_wheel_hook() {
    let hook = HOOK.swap(0, Ordering::SeqCst);
    if hook != 0 {
        unsafe {
            UnhookWindowsHookEx(hook);
        }
    }
    let khook = KEYBOARD_HOOK.swap(0, Ordering::SeqCst);
    if khook != 0 {
        unsafe {
            UnhookWindowsHookEx(khook);
        }
    }
    HOOK_HOST_HWND.store(0, Ordering::SeqCst);
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && wparam as u32 == WM_MOUSEWHEEL && lparam != 0 {
        let st = &*(lparam as *const MSLLHOOKSTRUCT);
        let delta = ((st.mouseData >> 16) as u16 as i16 as i32) / WHEEL_DELTA;
        if delta != 0 && modifier_held(active_modifier()) {
            // GetAsyncKeyState returns i16; the high bit (0x8000) means "held".
            let shift = shift_held();
            let id = if delta > 0 {
                if shift {
                    ID_VOL_UP_LARGE
                } else {
                    ID_VOL_UP
                }
            } else if shift {
                ID_VOL_DOWN_LARGE
            } else {
                ID_VOL_DOWN
            };
            let hwnd = HOOK_HOST_HWND.load(Ordering::SeqCst);
            if hwnd != 0 {
                PostMessageW(hwnd as HWND, WM_APP_WHEEL, id as usize, 0);
                // Swallow the wheel event so it doesn't also scroll.
                return 1;
            }
        }
    }
    CallNextHookEx(0, code, wparam, lparam)
}

/// Low-level keyboard hook: routes the CapsLock combos (RegisterHotKey can't
/// express a CapsLock modifier). While CapsLock is physically held, the target
/// key is swallowed and the action posted to the host window. Autorepeat is
/// suppressed (like the `MOD_NOREPEAT` flag on the keyboard combos).
unsafe extern "system" fn key_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0
        && active_modifier() == HotkeyModifier::CapsLock
        && caps_held()
        && (wparam as u32 == WM_KEYDOWN || wparam as u32 == WM_SYSKEYDOWN)
        && lparam != 0
    {
        let st = &*(lparam as *const KBDLLHOOKSTRUCT);
        let vk = st.vkCode as u32;
        let shift = shift_held();
        let id = match vk {
            v if v == VK_UP as u32 => {
                if shift {
                    ID_VOL_UP_LARGE
                } else {
                    ID_VOL_UP
                }
            }
            v if v == VK_DOWN as u32 => {
                if shift {
                    ID_VOL_DOWN_LARGE
                } else {
                    ID_VOL_DOWN
                }
            }
            v if v == VK_M as u32 => {
                if shift {
                    ID_SHOW_MENU
                } else {
                    ID_MUTE
                }
            }
            v if v == VK_R as u32 => ID_RESET,
            v if v == VK_V as u32 => ID_MIXER,
            _ => -1,
        };
        if id >= 0 {
            let now = st.time as i64;
            let prev = LAST_COMBO_KEY.load(Ordering::SeqCst);
            LAST_COMBO_KEY.store(((vk as i64) << 32) | now, Ordering::SeqCst);
            let same_key = (prev >> 32) == vk as i64;
            let repeat = same_key && (now - (prev & 0xFFFF_FFFF)) < 300;
            let hwnd = HOOK_HOST_HWND.load(Ordering::SeqCst);
            if !repeat && hwnd != 0 {
                PostMessageW(hwnd as HWND, WM_APP_WHEEL, id as usize, 0);
            }
            // Swallow the key so it never reaches the focused app.
            return 1;
        }
    }
    CallNextHookEx(0, code, wparam, lparam)
}

/// Map `HotkeyModifier` → Win32 modifier bits (HOT_KEY_MODIFIERS = u32).
fn modifier_bits(m: HotkeyModifier, shifted: bool) -> u32 {
    let base: u32 = match m {
        HotkeyModifier::CtrlAlt => MOD_CONTROL | MOD_ALT,
        HotkeyModifier::Ctrl => MOD_CONTROL,
        HotkeyModifier::Alt => MOD_ALT,
        HotkeyModifier::CapsLock => 0,
    };
    if shifted {
        base | MOD_SHIFT
    } else {
        base
    }
}

/// Decode a hotkey id (WPARAM of `WM_HOTKEY`) into an action.
pub fn hotkey_action(id: i32) -> Option<HotkeyAction> {
    use HotkeyAction as A;
    Some(match id {
        ID_VOL_UP => A::VolumeUp,
        ID_VOL_DOWN => A::VolumeDown,
        ID_VOL_UP_LARGE => A::VolumeUpLarge,
        ID_VOL_DOWN_LARGE => A::VolumeDownLarge,
        ID_MUTE => A::ToggleMute,
        ID_RESET => A::Reset50,
        ID_MIXER => A::OpenMixer,
        ID_SHOW_MENU => A::OpenMenu,
        _ => return None,
    })
}

/// Registers/unregisters hotkeys against a window handle.
pub struct Win32Hotkeys {
    hwnd: HWND,
    registered: Vec<i32>,
}

impl Win32Hotkeys {
    pub fn new(hwnd: HWND, modifier: HotkeyModifier) -> Result<Self, String> {
        let mut s = Self {
            hwnd,
            registered: Vec::new(),
        };
        s.register(modifier)?;
        Ok(s)
    }

    fn reg(&mut self, id: i32, mods: u32, vk: VIRTUAL_KEY) -> Result<(), String> {
        unsafe {
            // MOD_NOREPEAT: holding a combo shouldn't spam-apply.
            let ok: BOOL = RegisterHotKey(self.hwnd, id, mods | MOD_NOREPEAT, vk as u32);
            if ok == FALSE {
                return Err(format!(
                    "RegisterHotKey(id={id}, vk={vk:#x}) failed — maybe in use"
                ));
            }
            self.registered.push(id);
            Ok(())
        }
    }

    /// (Re)register all combos implied by `modifier`. Old registrations are
    /// torn down first, so this is safe to call after a config reload.
    ///
    /// A combo that is already held by another application is *not* fatal:
    /// the conflict is logged and registration continues with the rest, so
    /// the app still starts and the user can change the modifier in config.
    pub fn register(&mut self, modifier: HotkeyModifier) -> Result<(), String> {
        for &id in &self.registered {
            unsafe {
                UnregisterHotKey(self.hwnd, id);
            }
        }
        self.registered.clear();

        // The wheel + keyboard hooks need the current modifier to match.
        ACTIVE_MODIFIER.store(modifier as u8, Ordering::SeqCst);

        if modifier == HotkeyModifier::CapsLock {
            // RegisterHotKey cannot express a CapsLock modifier; the combos are
            // routed through the keyboard hook (key_proc). Registering the bare
            // keys here (mods = 0) would hijack Up/Down/M/R/V globally.
            log::debug!("CapsLock modifier: combos routed via keyboard hook");
            return Ok(());
        }

        let mb = |shifted: bool| modifier_bits(modifier, shifted);

        for (id, mods, vk) in [
            (ID_VOL_UP, mb(false), VK_UP),
            (ID_VOL_DOWN, mb(false), VK_DOWN),
            (ID_VOL_UP_LARGE, mb(true), VK_UP),
            (ID_VOL_DOWN_LARGE, mb(true), VK_DOWN),
            (ID_MUTE, mb(false), VK_M),
            (ID_RESET, mb(false), VK_R),
            (ID_MIXER, mb(false), VK_V),
            (ID_SHOW_MENU, mb(true), VK_M),
        ] {
            if let Err(e) = self.reg(id, mods, vk) {
                log::warn!("{e} — skipping this hotkey");
            }
        }
        Ok(())
    }
}

impl Drop for Win32Hotkeys {
    fn drop(&mut self) {
        for &id in &self.registered {
            unsafe {
                UnregisterHotKey(self.hwnd, id);
            }
        }
    }
}
