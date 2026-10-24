//! Blacklist draft operations (legacy `BlacklistEditor`/`RunningAppPicker`
//! semantics): every edit mutates the draft only; Save persists.

use volumectl::config::{self, HotkeyModifier};

/// Normalize one manual entry; returns `None` for blank input.
#[must_use]
pub fn normalize(entry: &str) -> Option<String> {
    let trimmed = entry.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(config::normalize_blacklist_entry(trimmed))
}

/// Add one entry. Returns true when the list changed (new, non-duplicate).
pub fn add(list: &mut Vec<String>, entry: &str) -> bool {
    let Some(entry) = normalize(entry) else {
        return false;
    };
    if list
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(&entry))
    {
        return false;
    }
    list.push(entry);
    true
}

/// Remove an entry (case-insensitive).
pub fn remove(list: &mut Vec<String>, entry: &str) -> bool {
    let before = list.len();
    list.retain(|existing| !existing.eq_ignore_ascii_case(entry));
    list.len() != before
}

/// Merge the recommended presets without removing or duplicating entries.
pub fn merge_recommended(list: &mut Vec<String>, modifier: HotkeyModifier) -> bool {
    let mut changed = false;
    for entry in config::recommended_blacklist(modifier) {
        changed |= add(list, &entry);
    }
    changed
}

/// Replace the draft with the recommended defaults.
pub fn replace_defaults(list: &mut Vec<String>, modifier: HotkeyModifier) -> bool {
    let defaults = config::recommended_blacklist(modifier);
    if *list == defaults {
        return false;
    }
    *list = defaults;
    true
}

/// Discard draft edits from the last saved list.
pub fn reload_saved(list: &mut Vec<String>, saved: &[String]) -> bool {
    if list.as_slice() == saved {
        return false;
    }
    *list = saved.to_vec();
    true
}

/// Remove every entry.
pub fn clear(list: &mut Vec<String>) -> bool {
    if list.is_empty() {
        return false;
    }
    list.clear();
    true
}

/// Toggle a running-app entry (checked = blocked).
pub fn toggle_selected(list: &mut Vec<String>, process: &str) -> bool {
    if list.iter().any(|entry| entry.eq_ignore_ascii_case(process)) {
        remove(list, process)
    } else {
        add(list, process)
    }
}

/// Case-insensitive membership check for picker checkboxes.
#[must_use]
pub fn is_blocked(list: &[String], process: &str) -> bool {
    list.iter().any(|entry| entry.eq_ignore_ascii_case(process))
}
