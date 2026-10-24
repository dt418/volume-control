//! Shortcut recorder state machine (legacy `ShortcutRecorder` parity).

use crate::i18n::{tr, Key};
use volumectl::config::HotkeyBindings;
use volumectl::config::Language;

/// One of the eight configurable actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecorderAction {
    VolumeUp,
    VolumeDown,
    VolumeUpLarge,
    VolumeDownLarge,
    ToggleMute,
    Reset50,
    OpenMixer,
    OpenMenu,
}

impl RecorderAction {
    pub const ALL: [RecorderAction; 8] = [
        RecorderAction::VolumeUp,
        RecorderAction::VolumeDown,
        RecorderAction::VolumeUpLarge,
        RecorderAction::VolumeDownLarge,
        RecorderAction::ToggleMute,
        RecorderAction::Reset50,
        RecorderAction::OpenMixer,
        RecorderAction::OpenMenu,
    ];

    /// Label key for the action.
    #[must_use]
    pub fn label(self) -> Key {
        match self {
            RecorderAction::VolumeUp => Key::HotkeyVolumeUp,
            RecorderAction::VolumeDown => Key::HotkeyVolumeDown,
            RecorderAction::VolumeUpLarge => Key::HotkeyVolumeUpLarge,
            RecorderAction::VolumeDownLarge => Key::HotkeyVolumeDownLarge,
            RecorderAction::ToggleMute => Key::HotkeyToggleMute,
            RecorderAction::Reset50 => Key::HotkeyReset50,
            RecorderAction::OpenMixer => Key::HotkeyOpenMixer,
            RecorderAction::OpenMenu => Key::HotkeyOpenMenu,
        }
    }

    /// Hint key for the action.
    #[must_use]
    pub fn help(self) -> Key {
        match self {
            RecorderAction::VolumeUp => Key::HotkeyVolumeUpHelp,
            RecorderAction::VolumeDown => Key::HotkeyVolumeDownHelp,
            RecorderAction::VolumeUpLarge => Key::HotkeyVolumeUpLargeHelp,
            RecorderAction::VolumeDownLarge => Key::HotkeyVolumeDownLargeHelp,
            RecorderAction::ToggleMute => Key::HotkeyToggleMuteHelp,
            RecorderAction::Reset50 => Key::HotkeyReset50Help,
            RecorderAction::OpenMixer => Key::HotkeyOpenMixerHelp,
            RecorderAction::OpenMenu => Key::HotkeyOpenMenuHelp,
        }
    }

    /// Current persisted binding.
    #[must_use]
    pub fn binding(self, bindings: &HotkeyBindings) -> &str {
        match self {
            RecorderAction::VolumeUp => &bindings.volume_up,
            RecorderAction::VolumeDown => &bindings.volume_down,
            RecorderAction::VolumeUpLarge => &bindings.volume_up_large,
            RecorderAction::VolumeDownLarge => &bindings.volume_down_large,
            RecorderAction::ToggleMute => &bindings.toggle_mute,
            RecorderAction::Reset50 => &bindings.reset_50,
            RecorderAction::OpenMixer => &bindings.open_mixer,
            RecorderAction::OpenMenu => &bindings.open_menu,
        }
    }

    /// Replace the persisted binding.
    pub fn set_binding(self, bindings: &mut HotkeyBindings, value: String) {
        match self {
            RecorderAction::VolumeUp => bindings.volume_up = value,
            RecorderAction::VolumeDown => bindings.volume_down = value,
            RecorderAction::VolumeUpLarge => bindings.volume_up_large = value,
            RecorderAction::VolumeDownLarge => bindings.volume_down_large = value,
            RecorderAction::ToggleMute => bindings.toggle_mute = value,
            RecorderAction::Reset50 => bindings.reset_50 = value,
            RecorderAction::OpenMixer => bindings.open_mixer = value,
            RecorderAction::OpenMenu => bindings.open_menu = value,
        }
    }
}

/// A keyboard event reduced to what the recorder reacts to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyEvent {
    /// A non-modifier key with the currently held modifiers.
    Key {
        code: String,
        ctrl: bool,
        alt: bool,
        shift: bool,
        meta: bool,
    },
    /// A modifier key pressed on its own; keep recording.
    ModifierOnly,
    Escape,
    Backspace,
}

/// What the recorder wants the surface to do next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecorderEffect {
    /// Store this portable `Ctrl+Alt+KeyX` string on the action.
    Captured {
        action: RecorderAction,
        binding: String,
    },
    /// Keep recording; show the modifier hint.
    NeedModifier,
    /// Keep recording silently (bare modifier press).
    Waiting,
    /// Clear the action's binding.
    Cleared { action: RecorderAction },
    /// Stop recording without a change.
    Cancelled,
}

/// Recorder state machine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recorder {
    active: Option<RecorderAction>,
    hint: Option<RecorderHint>,
}

/// Transient recorder hint (localized at render time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecorderHint {
    NeedModifier,
    Recorded,
    Cleared,
}

impl RecorderHint {
    #[must_use]
    pub fn text(self, language: Language) -> String {
        match self {
            RecorderHint::NeedModifier => tr(Key::RecorderNeedModifier, language, &[]).to_string(),
            RecorderHint::Recorded => tr(Key::RecorderRecorded, language, &[]).to_string(),
            RecorderHint::Cleared => tr(Key::RecorderCleared, language, &[]).to_string(),
        }
    }
}

impl Recorder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn active(&self) -> Option<RecorderAction> {
        self.active
    }

    #[must_use]
    pub fn hint(&self) -> Option<RecorderHint> {
        self.hint
    }

    pub fn start(&mut self, action: RecorderAction) {
        self.active = Some(action);
        self.hint = None;
    }

    pub fn cancel(&mut self) {
        self.active = None;
        self.hint = None;
    }

    /// Feed a key event; returns the effect when a binding changes.
    pub fn feed(&mut self, event: KeyEvent) -> RecorderEffect {
        let Some(action) = self.active else {
            return RecorderEffect::Cancelled;
        };
        match event {
            KeyEvent::Escape => {
                self.cancel();
                RecorderEffect::Cancelled
            }
            KeyEvent::Backspace => {
                self.active = None;
                self.hint = Some(RecorderHint::Cleared);
                RecorderEffect::Cleared { action }
            }
            KeyEvent::ModifierOnly => {
                self.hint = None;
                RecorderEffect::Waiting
            }
            KeyEvent::Key {
                code,
                ctrl,
                alt,
                shift,
                meta,
            } => {
                if !(ctrl || alt || shift || meta) {
                    self.hint = Some(RecorderHint::NeedModifier);
                    return RecorderEffect::NeedModifier;
                }
                let mut parts = Vec::new();
                if ctrl {
                    parts.push("Ctrl");
                }
                if alt {
                    parts.push("Alt");
                }
                if shift {
                    parts.push("Shift");
                }
                if meta {
                    parts.push("Cmd");
                }
                parts.push(&code);
                self.active = None;
                self.hint = Some(RecorderHint::Recorded);
                RecorderEffect::Captured {
                    action,
                    binding: parts.join("+"),
                }
            }
        }
    }
}
