//! Windows global-hotkey backend via Win32 `RegisterHotKey`.
//!
//! Registers the app's custom combos against a caller-provided window handle.
//! `WM_HOTKEY` messages fired for those combos carry the hotkey id in
//! `WPARAM`; [`hotkey_action`] decodes an id into a [`HotkeyAction`]. The
//! app's window proc handles `WM_HOTKEY` directly — no secondary message
//! queue required.
//!
//! Media keys (`Volume_Up/Down/Mute`) are intentionally *not* registered here:
//! they keep the OS default (native flyout). The app polls the audio state to
//! stay in sync with them.

use windows_sys::Win32::{
    Foundation::{HWND, BOOL, FALSE},
    UI::Input::KeyboardAndMouse::{
        RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT,
        VIRTUAL_KEY, VK_DOWN, VK_M, VK_R, VK_UP, VK_V,
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

        let mb = |shifted: bool| modifier_bits(modifier, shifted);

        for (id, mods, vk) in [
            (ID_VOL_UP, mb(false), VK_UP),
            (ID_VOL_DOWN, mb(false), VK_DOWN),
            (ID_VOL_UP_LARGE, mb(true), VK_UP),
            (ID_VOL_DOWN_LARGE, mb(true), VK_DOWN),
            (ID_MUTE, mb(false), VK_M),
            (ID_RESET, mb(false), VK_R),
            (ID_MIXER, mb(false), VK_V),
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