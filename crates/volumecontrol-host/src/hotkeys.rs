//! Global-hotkey adapter implementing the host `HotkeySource` seam.
//!
//! [`GlobalHotkeys`] already exposes `try_recv` / `listener_failure` /
//! `set_modifier`; this thin wrapper adapts it to the reducer-owned trait so
//! the Winit host stays free of hotkey plumbing details.

use volumectl::config::HotkeyModifier;
use volumectl::hotkeys_global::GlobalHotkeys;
use volumectl::linux_host_core::HotkeySource;

/// Adapter from [`GlobalHotkeys`] to the host hotkey seam.
pub struct GlobalHotkeySource(GlobalHotkeys);

impl GlobalHotkeySource {
    #[must_use]
    pub fn new(hotkeys: GlobalHotkeys) -> Self {
        Self(hotkeys)
    }
}

impl HotkeySource for GlobalHotkeySource {
    fn try_recv(&self) -> Option<volumectl::hotkeys::HotkeyAction> {
        self.0.try_recv()
    }

    fn listener_failure(&self) -> Option<String> {
        self.0.listener_failure()
    }

    fn set_modifier(&self, modifier: HotkeyModifier) {
        self.0.set_modifier(modifier);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use volumectl::config::HotkeyBindings;
    use volumectl::hotkeys::HotkeyAction;
    use volumectl::hotkeys_global::GlobalHotkeys;

    fn unavailable_source() -> GlobalHotkeySource {
        let bindings = HotkeyBindings::default();
        let hotkeys = GlobalHotkeys::unavailable(&bindings, "test: no listener");
        GlobalHotkeySource::new(hotkeys)
    }

    #[test]
    fn unavailable_source_reports_no_actions() {
        assert_eq!(unavailable_source().try_recv(), None);
    }

    #[test]
    fn unavailable_source_reports_listener_failure() {
        let failure = unavailable_source()
            .listener_failure()
            .expect("unavailable source must report a failure");
        assert!(failure.contains("no listener"));
    }

    #[test]
    fn set_modifier_is_accepted_without_panic() {
        unavailable_source().set_modifier(volumectl::config::HotkeyModifier::CtrlAlt);
    }

    #[test]
    fn hotkey_action_variant_exists_for_volume_up() {
        // Compile-time guard: the adapter's action type stays the shared one.
        let _tag = std::mem::discriminant(&HotkeyAction::VolumeUp);
    }
}
