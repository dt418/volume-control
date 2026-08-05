//! Hotkey action and status definitions shared across all platforms.

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
    /// Open the tray context menu (useful when the tray icon is hidden).
    OpenMenu,
}

/// The actions in the order shown by the Help surface.
pub const ALL_HOTKEY_ACTIONS: [HotkeyAction; 8] = [
    HotkeyAction::VolumeUp,
    HotkeyAction::VolumeDown,
    HotkeyAction::VolumeUpLarge,
    HotkeyAction::VolumeDownLarge,
    HotkeyAction::ToggleMute,
    HotkeyAction::Reset50,
    HotkeyAction::OpenMixer,
    HotkeyAction::OpenMenu,
];

/// Stable wire id used by the Windows mouse-wheel message bridge.
pub fn hotkey_id(action: HotkeyAction) -> i32 {
    match action {
        HotkeyAction::VolumeUp => 0x01,
        HotkeyAction::VolumeDown => 0x02,
        HotkeyAction::VolumeUpLarge => 0x03,
        HotkeyAction::VolumeDownLarge => 0x04,
        HotkeyAction::ToggleMute => 0x05,
        HotkeyAction::Reset50 => 0x06,
        HotkeyAction::OpenMixer => 0x07,
        HotkeyAction::OpenMenu => 0x08,
    }
}

/// Decode the stable id used by the Windows mouse-wheel bridge.
pub fn hotkey_from_id(id: i32) -> Option<HotkeyAction> {
    ALL_HOTKEY_ACTIONS
        .into_iter()
        .find(|&action| hotkey_id(action) == id)
}

/// Win32-independent status model consumed by the Help surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyRegResult {
    pub action: HotkeyAction,
    pub status: HotkeyRegStatus,
}

/// Availability of one global hotkey action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotkeyRegStatus {
    /// The action is handled by the global `rdev` listener.
    Registered,
    /// Kept for compatibility with older persisted/help models.
    Conflicted(HotkeyRegError),
    /// Kept for compatibility with older hook-backed configurations.
    HookRouted,
}

/// Platform-neutral description of a registration failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeyRegError {
    pub error_code: u32,
    pub message: String,
}

impl std::fmt::Display for HotkeyRegError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
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
            HotkeyAction::OpenMenu => "Open tray menu",
        }
    }
}
