//! Host integration for native system tray actions and context menu dispatch.

use crate::tray_common::TrayCommand;
use crate::ui::renderer::HostHandle;

/// Bridges system tray menu selections into host action queue dispatches.
pub struct TrayDispatcher {
    host: HostHandle,
}

impl TrayDispatcher {
    /// Create a new tray dispatcher bound to the host handle.
    pub fn new(host: HostHandle) -> Self {
        Self { host }
    }

    /// Dispatch a tray command through the host handle.
    pub fn dispatch_command(&self, cmd: TrayCommand) {
        self.host.enqueue(cmd.to_app_action());
    }

    /// Handle a raw menu item ID from the system tray menu.
    pub fn handle_menu_id(&self, menu_id: &str) -> bool {
        if let Some(cmd) = TrayCommand::from_menu_id(menu_id) {
            self.dispatch_command(cmd);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::model::{AppAction, SurfaceId};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn tray_command_to_app_action_mapping() {
        assert_eq!(
            TrayCommand::ToggleMute.to_app_action(),
            AppAction::ToggleMute
        );
        assert_eq!(TrayCommand::Reset50.to_app_action(), AppAction::ResetVolume);
        assert_eq!(
            TrayCommand::OpenMixer.to_app_action(),
            AppAction::ShowSurface(SurfaceId::Mixer)
        );
        assert_eq!(
            TrayCommand::Settings.to_app_action(),
            AppAction::ShowSurface(SurfaceId::Settings)
        );
        assert_eq!(
            TrayCommand::Help.to_app_action(),
            AppAction::ShowSurface(SurfaceId::Help)
        );
        assert_eq!(
            TrayCommand::EditConfig.to_app_action(),
            AppAction::OpenConfigLocation
        );
        assert_eq!(
            TrayCommand::ReloadConfig.to_app_action(),
            AppAction::ReloadConfig
        );
        assert_eq!(
            TrayCommand::CheckUpdates.to_app_action(),
            AppAction::CheckUpdates
        );
        assert_eq!(TrayCommand::Exit.to_app_action(), AppAction::Exit);
    }

    #[test]
    fn tray_dispatcher_routes_menu_id_to_host() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let count_clone = Arc::clone(&call_count);

        let host = HostHandle::new(move |action| {
            if action == AppAction::ShowSurface(SurfaceId::Mixer) {
                count_clone.fetch_add(1, Ordering::SeqCst);
            }
        });

        let dispatcher = TrayDispatcher::new(host);
        assert!(dispatcher.handle_menu_id("mixer"));
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        assert!(!dispatcher.handle_menu_id("unknown_id"));
    }
}
