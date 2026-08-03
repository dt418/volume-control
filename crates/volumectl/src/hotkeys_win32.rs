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
    Foundation::{
        GetLastError, BOOL, ERROR_HOTKEY_ALREADY_REGISTERED, FALSE, HWND, LPARAM, LRESULT, WPARAM,
    },
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
        // Resolved from the shared COMBOS table so hook routing can never
        // desync from the reported per-action status.
        let id = capslock_id(vk, shift);
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

/// Win32 error captured when `RegisterHotKey` rejects a combo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyRegError {
    /// The raw `GetLastError()` code from the failed `RegisterHotKey` call.
    pub error_code: u32,
    /// Human-readable description (includes the raw code).
    pub message: String,
}

impl HotkeyRegError {
    /// `ERROR_HOTKEY_ALREADY_REGISTERED`: the combo is already owned by
    /// another process — the "in use by another app" case the UI surfaces.
    pub fn already_registered(&self) -> bool {
        self.error_code == ERROR_HOTKEY_ALREADY_REGISTERED
    }
}

impl std::fmt::Display for HotkeyRegError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Outcome of attempting to register one hotkey combo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyRegResult {
    /// The action the combo triggers.
    pub action: HotkeyAction,
    /// Whether the combo is live, conflicted, or hook-routed.
    pub status: HotkeyRegStatus,
}

/// Per-action registration status, reported to the Settings/Help surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotkeyRegStatus {
    /// The combo is live (registered via `RegisterHotKey`).
    Registered,
    /// `RegisterHotKey` rejected the combo (e.g. in use by another app).
    /// Carries the Win32 error code + message so the UI can explain it.
    Conflicted(HotkeyRegError),
    /// Not registered via `RegisterHotKey` by design: the CapsLock modifier
    /// routes combos through the low-level keyboard hook.
    HookRouted,
}

/// The 8 hotkey combos as `(id, virtual key, action, shift)`.
///
/// `shift` is the Shift requirement: `Some(true)` = Shift must be held,
/// `Some(false)` = Shift must be absent, `None` = shift-agnostic. `None` is
/// used only for combos that have no Shift variant (Reset, Mixer): the original
/// CapsLock keyboard hook matched `VK_R`/`VK_V` without inspecting Shift, and
/// `RegisterHotKey` cannot express a shift-agnostic combo (it registers the
/// plain variant, exactly as the pre-Task-11 code did).
///
/// The single source of truth for every path: [`Win32Hotkeys::do_register`]
/// registers each combo, the CapsLock path's [`capslock_id`] resolves hook
/// input from the same table, and the reported per-action status is projected
/// from it — so hook routing and the UI status can never desync.
const COMBOS: [(i32, VIRTUAL_KEY, HotkeyAction, Option<bool>); 8] = [
    (ID_VOL_UP, VK_UP, HotkeyAction::VolumeUp, Some(false)),
    (ID_VOL_DOWN, VK_DOWN, HotkeyAction::VolumeDown, Some(false)),
    (
        ID_VOL_UP_LARGE,
        VK_UP,
        HotkeyAction::VolumeUpLarge,
        Some(true),
    ),
    (
        ID_VOL_DOWN_LARGE,
        VK_DOWN,
        HotkeyAction::VolumeDownLarge,
        Some(true),
    ),
    (ID_MUTE, VK_M, HotkeyAction::ToggleMute, Some(false)),
    (ID_RESET, VK_R, HotkeyAction::Reset50, None),
    (ID_MIXER, VK_V, HotkeyAction::OpenMixer, None),
    (ID_SHOW_MENU, VK_M, HotkeyAction::OpenMenu, Some(true)),
];

/// Resolve a CapsLock-hook `(vk, shifted)` into a hotkey id, or `-1` when the
/// key is not part of the combo set.
///
/// Driven by the shared [`COMBOS`] table (linear scan over 8 entries), so the
/// keyboard-hook path and the reported per-action status can never desync:
/// adding/removing/renaming a combo updates the hook routing automatically.
/// A combo with a `None` shift requirement matches regardless of Shift,
/// preserving the original hook behavior for Reset/Mixer. Returns `-1` for
/// keys outside the combo set; the caller still swallows those keys
/// (CapsLock-held keys never reach the focused app).
fn capslock_id(vk: u32, shifted: bool) -> i32 {
    COMBOS
        .into_iter()
        .find(|&(_, table_vk, _, shift)| {
            u32::from(table_vk) == vk && shift.is_none_or(|required| required == shifted)
        })
        .map(|(id, _, _, _)| id)
        .unwrap_or(-1)
}

/// Registers/unregisters hotkeys against a window handle.
pub struct Win32Hotkeys {
    hwnd: HWND,
    registered: Vec<i32>,
    /// Status of every combo from the most recent [`Win32Hotkeys::register`]
    /// call, read via [`Win32Hotkeys::status`] so the UI can report conflicts.
    last_status: Vec<HotkeyRegResult>,
}

impl Win32Hotkeys {
    pub fn new(hwnd: HWND, modifier: HotkeyModifier) -> Result<Self, String> {
        let mut s = Self {
            hwnd,
            registered: Vec::new(),
            last_status: Vec::new(),
        };
        s.register(modifier)?;
        Ok(s)
    }

    /// Per-action status of every combo from the last (re)registration.
    ///
    /// Mirrors the 8 combos implied by the modifier; see [`HotkeyRegStatus`]
    /// for the possible states. Empty before the first [`Win32Hotkeys::register`].
    pub fn status(&self) -> &[HotkeyRegResult] {
        &self.last_status
    }

    fn reg(&mut self, id: i32, mods: u32, vk: VIRTUAL_KEY) -> Result<(), HotkeyRegError> {
        unsafe {
            // MOD_NOREPEAT: holding a combo shouldn't spam-apply.
            let ok: BOOL = RegisterHotKey(self.hwnd, id, mods | MOD_NOREPEAT, vk as u32);
            if ok == FALSE {
                let error_code = GetLastError();
                return Err(HotkeyRegError {
                    error_code,
                    message: format!(
                        "RegisterHotKey(id={id}, vk={vk:#x}) failed — maybe in use \
                         (error {error_code:#x})"
                    ),
                });
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
    /// Each combo's outcome is recorded on `self` and readable via
    /// [`Win32Hotkeys::status`].
    pub fn register(&mut self, modifier: HotkeyModifier) -> Result<(), String> {
        self.last_status = self.do_register(modifier);
        Ok(())
    }

    fn do_register(&mut self, modifier: HotkeyModifier) -> Vec<HotkeyRegResult> {
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
            return COMBOS
                .into_iter()
                .map(|(_, _, action, _)| HotkeyRegResult {
                    action,
                    status: HotkeyRegStatus::HookRouted,
                })
                .collect();
        }

        let mb = |shifted: bool| modifier_bits(modifier, shifted);
        let mut results = Vec::with_capacity(COMBOS.len());
        for (id, vk, action, shift) in COMBOS {
            // A `None` shift requirement registers the plain variant (the
            // pre-Task-11 behavior for Reset/Mixer).
            match self.reg(id, mb(shift.unwrap_or(false)), vk) {
                Ok(()) => results.push(HotkeyRegResult {
                    action,
                    status: HotkeyRegStatus::Registered,
                }),
                Err(e) => {
                    log::warn!("{e} — skipping this hotkey");
                    results.push(HotkeyRegResult {
                        action,
                        status: HotkeyRegStatus::Conflicted(e),
                    });
                }
            }
        }
        results
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_combo_table_id_decodes_to_the_action_it_claims() {
        // Locks the 8-combo table: each id must round-trip through the
        // WM_HOTKEY decoder to the action the status map reports.
        for &(id, _, action, _) in &COMBOS {
            assert_eq!(hotkey_action(id), Some(action));
        }
        // And the ids are distinct, so each action is covered exactly once.
        let ids: std::collections::HashSet<i32> = COMBOS.iter().map(|&(id, _, _, _)| id).collect();
        assert_eq!(ids.len(), COMBOS.len());
    }

    #[test]
    fn unknown_hotkey_id_decodes_to_none() {
        assert_eq!(hotkey_action(0), None);
        assert_eq!(hotkey_action(0x7FFF), None);
    }

    #[test]
    fn already_registered_detects_the_in_use_error_code() {
        let in_use = HotkeyRegError {
            error_code: ERROR_HOTKEY_ALREADY_REGISTERED,
            message: "in use".into(),
        };
        assert!(in_use.already_registered());

        let other = HotkeyRegError {
            error_code: 5,
            message: "access denied".into(),
        };
        assert!(!other.already_registered());
    }

    #[test]
    fn caps_lock_path_reports_every_combo_as_hook_routed() {
        // The CapsLock path reports all 8 combos as hook-routed (registered via
        // the keyboard hook, not RegisterHotKey); the UI can rely on status
        // always covering every combo for the active modifier.
        let expected: Vec<HotkeyRegResult> = COMBOS
            .into_iter()
            .map(|(_, _, action, _)| HotkeyRegResult {
                action,
                status: HotkeyRegStatus::HookRouted,
            })
            .collect();
        let mut hk = Win32Hotkeys {
            hwnd: 0,
            registered: Vec::new(),
            last_status: Vec::new(),
        };
        hk.register(HotkeyModifier::CapsLock)
            .expect("caps register");
        assert_eq!(hk.status(), expected.as_slice());
    }

    #[test]
    fn capslock_id_matches_the_combos_table_entry_for_every_combo() {
        // Pins the CapsLock hook path to the shared COMBOS table: every combo's
        // (vk, shift) must resolve to its own id and decode to its action, so
        // hook routing and the reported status can never silently desync.
        for &(id, vk, action, shift) in &COMBOS {
            match shift {
                Some(required) => {
                    assert_eq!(
                        capslock_id(u32::from(vk), required),
                        id,
                        "capslock_id({vk}, shifted={required}) must resolve to id {id:#x}"
                    );
                }
                None => {
                    // Shift-agnostic combos match with or without Shift.
                    assert_eq!(capslock_id(u32::from(vk), false), id);
                    assert_eq!(capslock_id(u32::from(vk), true), id);
                }
            }
            assert_eq!(hotkey_action(id), Some(action));
        }
    }

    #[test]
    fn capslock_reset_and_mixer_are_shift_agnostic_like_the_original_hook() {
        // The pre-Task-11 key_proc matched VK_R and VK_V without inspecting
        // Shift (only the arrow keys and M are shift-sensitive). The COMBOS
        // table records these as `None` and capslock_id honors that, so the
        // refactor is behavior-preserving.
        assert_eq!(capslock_id(u32::from(VK_R), false), ID_RESET);
        assert_eq!(capslock_id(u32::from(VK_R), true), ID_RESET);
        assert_eq!(capslock_id(u32::from(VK_V), false), ID_MIXER);
        assert_eq!(capslock_id(u32::from(VK_V), true), ID_MIXER);
    }

    #[test]
    fn capslock_inputs_never_match_two_combos() {
        // capslock_id does a linear find; no two COMBOS entries may match the
        // same (vk, shifted) input, or the find would be order-dependent.
        let combos = &COMBOS;
        for &(_, vk, _, _) in combos {
            for shift in [false, true] {
                let matches = combos
                    .iter()
                    .filter(|&&(_, table_vk, _, requirement)| {
                        u32::from(table_vk) == u32::from(vk)
                            && requirement.map_or(true, |r| r == shift)
                    })
                    .count();
                assert!(
                    matches <= 1,
                    "vk={vk:#x} shifted={shift} matched by {matches} combos"
                );
            }
        }
    }

    #[test]
    fn capslock_id_returns_negative_for_keys_outside_the_combo_set() {
        // Keys not in the table resolve to -1 (the hook swallows them without
        // posting), and the combo set is exactly the 8 table entries.
        let mut combo_keys: Vec<u16> = COMBOS.iter().map(|&(_, vk, _, _)| vk).collect();
        combo_keys.sort_unstable();
        combo_keys.dedup();
        for vk in [
            0x41u16, /* VK_A */
            0x50,    /* VK_P */
            0x1B,    /* VK_ESCAPE */
        ] {
            assert_eq!(
                capslock_id(u32::from(vk), false),
                -1,
                "vk={vk:#x} without shift"
            );
            assert_eq!(
                capslock_id(u32::from(vk), true),
                -1,
                "vk={vk:#x} with shift"
            );
            assert!(
                !combo_keys.contains(&vk),
                "vk={vk:#x} must not be in COMBOS"
            );
        }
    }
}
