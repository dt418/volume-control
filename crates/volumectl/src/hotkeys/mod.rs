//! Hotkey action definitions — shared across all platforms.
//!
//! Windows registers combos via Win32 `RegisterHotKey` and decodes them via
//! `hotkeys_win32::hotkey_action`. On macOS/Linux the dispatch layer may use
//! a poll- or callback-based approach; the platform-agnostic action type lives
//! here so the application core doesn't depend on backend internals.

/// Identifies an action triggered by a hotkey.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    VolumeUp,
    VolumeDown,
    VolumeUpLarge,
    VolumeDownLarge,
    ToggleMute,
    Reset50,
    OpenMixer,
}

impl HotkeyAction {
    pub fn label(&self) -> &'static str {
        match self {
            HotkeyAction::VolumeUp => "Volume +",
            HotkeyAction::VolumeDown => "Volume -",
            HotkeyAction::VolumeUpLarge => "Volume ++",
            HotkeyAction::VolumeDownLarge => "Volume --",
            HotkeyAction::ToggleMute => "Mute",
            HotkeyAction::Reset50 => "Reset 50%",
            HotkeyAction::OpenMixer => "Mixer",
        }
    }
}