//! CLI verbs module for headless commands (`get`, `set <0-100>`, `mute [on|off]`).
//!
//! Exposes argument parsing, host action mapping, and output formatting.

use volumectl::audio::VolumeState;
use volumectl::host_core::AppCore;
use volumectl::ui::AppAction;

/// One CLI command supported by the headless verb interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    /// Read the current system volume percentage.
    Get,
    /// Set the master volume to an exact percentage (0–100).
    Set(u8),
    /// Set or toggle mute state.
    ///
    /// `None` indicates toggle; `Some(true)` mutes; `Some(false)` unmutes.
    Mute(Option<bool>),
}

impl Verb {
    /// Convert this verb into a host [`AppAction`], or `None` if the verb
    /// is a read query ([`Verb::Get`]).
    pub fn to_app_action(self) -> Option<AppAction> {
        match self {
            Verb::Get => None,
            Verb::Set(percent) => Some(AppAction::SetVolumePercent {
                percent: percent as u16,
            }),
            Verb::Mute(Some(muted)) => Some(AppAction::SetMute { muted }),
            Verb::Mute(None) => Some(AppAction::ToggleMute),
        }
    }
}

/// Parse command-line arguments into a [`Verb`].
///
/// Accepts:
/// - `get`
/// - `set <0-100>`
/// - `mute`
/// - `mute on`
/// - `mute off`
///
/// Parsing is case-insensitive. Out-of-range percentages (e.g. >100) or
/// malformed/extra arguments return `None`.
pub fn parse(args: &[String]) -> Option<Verb> {
    parse_slice(args)
}

/// Generic slice parser accepting any `AsRef<str>` sequence.
pub fn parse_slice<S: AsRef<str>>(args: &[S]) -> Option<Verb> {
    match args {
        [verb] if verb.as_ref().eq_ignore_ascii_case("get") => Some(Verb::Get),
        [verb, val] if verb.as_ref().eq_ignore_ascii_case("set") => {
            let val = val.as_ref();
            if val.is_empty() || !val.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let n: u8 = val.parse().ok()?;
            if n <= 100 {
                Some(Verb::Set(n))
            } else {
                None
            }
        }
        [verb] if verb.as_ref().eq_ignore_ascii_case("mute") => Some(Verb::Mute(None)),
        [verb, state] if verb.as_ref().eq_ignore_ascii_case("mute") => {
            let state = state.as_ref();
            if state.eq_ignore_ascii_case("on") {
                Some(Verb::Mute(Some(true)))
            } else if state.eq_ignore_ascii_case("off") {
                Some(Verb::Mute(Some(false)))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Apply a verb to the core application state.
///
/// Maps [`Verb::Set`] and [`Verb::Mute`] to their corresponding [`AppAction`]
/// handlers, and reads [`AppCore::last_state`] for [`Verb::Get`].
#[allow(dead_code)]
pub fn apply(core: &mut AppCore, verb: Verb) -> Result<(), String> {
    match verb {
        Verb::Get => {
            let _ = core.last_state().percent();
        }
        Verb::Set(pct) => {
            if pct > 100 {
                return Err(format!("volume {pct}% exceeds 100%"));
            }
            core.handle_action(AppAction::SetVolumePercent {
                percent: pct as u16,
            });
        }
        Verb::Mute(Some(muted)) => {
            core.handle_action(AppAction::SetMute { muted });
        }
        Verb::Mute(None) => {
            core.handle_action(AppAction::ToggleMute);
        }
    }
    Ok(())
}

/// Format a volume percentage as a plain string for stdout (e.g. `"42"`).
pub fn render_get(percent: u8) -> String {
    percent.to_string()
}

/// Helper to render the volume percentage directly from [`AppCore`].
#[allow(dead_code)]
pub fn render_get_core(core: &AppCore) -> String {
    render_get(core.last_state().percent())
}

/// Helper to render the volume percentage directly from [`VolumeState`].
#[allow(dead_code)]
pub fn render_get_state(state: &VolumeState) -> String {
    render_get(state.percent())
}
