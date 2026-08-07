//! Hotkey action and status definitions shared across all platforms.

use crate::config::HotkeyModifier;

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

/// A known clash between a configured modifier and an OS or common-app
/// shortcut that uses one of the app's fixed keys (↑/↓, M, R, V).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortcutConflict {
    /// The affected key of the app's fixed key set (e.g. `↑/↓`, `M`, `R`).
    pub key: &'static str,
    /// Why the combo clashes (OS/desktop/app behaviour).
    pub note: &'static str,
    /// True when the OS or desktop environment reserves the combo
    /// system-wide (e.g. GNOME workspace switching).
    pub os_reserved: bool,
}

/// The configured modifier as a compact label (`Ctrl+Alt`, `Alt`, `Ctrl`,
/// `CapsLock`).
pub fn modifier_prefix(modifier: HotkeyModifier) -> &'static str {
    match modifier {
        HotkeyModifier::CtrlAlt => "Ctrl+Alt",
        HotkeyModifier::Alt => "Alt",
        HotkeyModifier::Ctrl => "Ctrl",
        HotkeyModifier::CapsLock => "CapsLock",
    }
}

/// One human-readable combo for a conflict, e.g. `Ctrl+Alt+↑/↓`.
pub fn combo_label(modifier: HotkeyModifier, key: &str) -> String {
    format!("{}+{key}", modifier_prefix(modifier))
}

/// Known OS/common-app shortcuts that clash with the configured modifier on
/// the platform the binary runs on. The app's keys are fixed, so the whole
/// conflict surface is `modifier × key`.
pub fn conflicts_for(modifier: HotkeyModifier) -> Vec<ShortcutConflict> {
    #[cfg(target_os = "windows")]
    {
        windows_conflicts(modifier)
    }
    #[cfg(target_os = "macos")]
    {
        macos_conflicts(modifier)
    }
    #[cfg(target_os = "linux")]
    {
        linux_conflicts(modifier)
    }
}

#[cfg(target_os = "windows")]
fn windows_conflicts(modifier: HotkeyModifier) -> Vec<ShortcutConflict> {
    match modifier {
        HotkeyModifier::CtrlAlt => vec![ShortcutConflict {
            key: "↑/↓",
            note: "the screen-rotation shortcut on many laptops (Intel/AMD graphics)",
            os_reserved: false,
        }],
        HotkeyModifier::Alt => vec![
            ShortcutConflict {
                key: "↑/↓",
                note: "move-line in editors (VS Code, JetBrains)",
                os_reserved: false,
            },
            ShortcutConflict {
                key: "M/R/V",
                note: "menu mnemonics in most apps",
                os_reserved: false,
            },
        ],
        HotkeyModifier::Ctrl => vec![
            ShortcutConflict {
                key: "R",
                note: "reload in browsers and editors",
                os_reserved: false,
            },
            ShortcutConflict {
                key: "V",
                note: "paste in most apps",
                os_reserved: false,
            },
        ],
        HotkeyModifier::CapsLock => Vec::new(),
    }
}

#[cfg(target_os = "macos")]
fn macos_conflicts(modifier: HotkeyModifier) -> Vec<ShortcutConflict> {
    match modifier {
        // The Ctrl config accepts both ⌃ and ⌘ as the primary key, so the
        // resulting combos collide with common ⌘ shortcuts.
        HotkeyModifier::Ctrl => vec![
            ShortcutConflict {
                key: "M",
                note: "Minimize (⌘+M)",
                os_reserved: false,
            },
            ShortcutConflict {
                key: "V",
                note: "Paste (⌘+V)",
                os_reserved: false,
            },
            ShortcutConflict {
                key: "R",
                note: "Reload in many apps (⌘+R)",
                os_reserved: false,
            },
            ShortcutConflict {
                key: "↑/↓",
                note: "Home/End in text fields (⌘+↑/↓)",
                os_reserved: false,
            },
        ],
        HotkeyModifier::CtrlAlt => vec![ShortcutConflict {
            key: "M",
            note: "Minimize All in some apps (⌘/⌃+⌥+M)",
            os_reserved: false,
        }],
        HotkeyModifier::Alt | HotkeyModifier::CapsLock => Vec::new(),
    }
}

#[cfg(target_os = "linux")]
fn linux_conflicts(modifier: HotkeyModifier) -> Vec<ShortcutConflict> {
    match modifier {
        HotkeyModifier::CtrlAlt => vec![ShortcutConflict {
            key: "↑/↓",
            note: "workspace switching in GNOME and KDE",
            os_reserved: true,
        }],
        HotkeyModifier::Alt => vec![
            ShortcutConflict {
                key: "↑/↓",
                note: "move-line in editors (VS Code, JetBrains)",
                os_reserved: false,
            },
            ShortcutConflict {
                key: "M/R/V",
                note: "menu mnemonics in most apps",
                os_reserved: false,
            },
        ],
        HotkeyModifier::Ctrl => vec![
            ShortcutConflict {
                key: "R",
                note: "reload in browsers and editors",
                os_reserved: false,
            },
            ShortcutConflict {
                key: "V",
                note: "paste in most apps",
                os_reserved: false,
            },
        ],
        HotkeyModifier::CapsLock => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifier_prefix_renders_all_four_modifiers() {
        assert_eq!(modifier_prefix(HotkeyModifier::CtrlAlt), "Ctrl+Alt");
        assert_eq!(modifier_prefix(HotkeyModifier::Alt), "Alt");
        assert_eq!(modifier_prefix(HotkeyModifier::Ctrl), "Ctrl");
        assert_eq!(modifier_prefix(HotkeyModifier::CapsLock), "CapsLock");
    }

    #[test]
    fn combo_label_joins_prefix_and_key() {
        assert_eq!(combo_label(HotkeyModifier::CtrlAlt, "↑/↓"), "Ctrl+Alt+↑/↓");
        assert_eq!(combo_label(HotkeyModifier::CapsLock, "M"), "CapsLock+M");
    }

    #[test]
    fn capslock_has_no_known_conflicts_on_any_platform() {
        assert!(conflicts_for(HotkeyModifier::CapsLock).is_empty());
    }

    #[test]
    fn the_platform_default_modifier_is_the_least_conflicting_choice() {
        #[cfg(target_os = "linux")]
        assert_eq!(HotkeyModifier::default(), HotkeyModifier::CapsLock);
        #[cfg(not(target_os = "linux"))]
        assert_eq!(HotkeyModifier::default(), HotkeyModifier::CtrlAlt);

        // On Linux the default (CapsLock) is entirely conflict-free.
        #[cfg(target_os = "linux")]
        assert!(conflicts_for(HotkeyModifier::default()).is_empty());
    }

    #[test]
    fn conflict_table_covers_the_alternative_modifiers() {
        // CtrlAlt clashes with something on every platform (screen rotation /
        // workspace switching / Minimize All) — that is why the platform
        // default avoids it where possible.
        assert!(!conflicts_for(HotkeyModifier::CtrlAlt).is_empty());

        #[cfg(target_os = "linux")]
        assert!(
            conflicts_for(HotkeyModifier::CtrlAlt)
                .iter()
                .any(|c| c.os_reserved),
            "GNOME/KDE workspace switching is reserved by the desktop"
        );

        // Ctrl also collides with common app shortcuts on every platform.
        assert!(!conflicts_for(HotkeyModifier::Ctrl).is_empty());
    }
}
