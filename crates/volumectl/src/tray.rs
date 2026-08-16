//! System tray icon + context menu (Windows native host).
//!
//! Built on the Tauri ecosystem's `tray-icon` + `muda` crates. Menu (spec §9):
//! live volume label, then volume actions (mute check item, reset, open
//! mixer), surface actions (settings, help), configuration actions (reload,
//! open config file), and Exit — each group separated. Menu events are routed
//! through the Tauri runtime's global menu handler (see the host crate),
//! because Tauri owns the `muda` event channel on desktop.
//!
//! The stable menu ids and the generated icon live in [`crate::tray_common`]
//! so the macOS/Linux Tauri tray shares the same contract.

use muda::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};

use crate::audio::VolumeState;
use crate::tray_common::{tray_icon_rgba, TrayCommand};

pub struct Tray {
    _tray: tray_icon::TrayIcon,
    vol_label: MenuItem,
    mute_item: CheckMenuItem,
}

impl Tray {
    pub fn new() -> Result<Tray, Box<dyn std::error::Error>> {
        let menu = Menu::new();

        // Live label (non-clickable), then volume actions, surface actions,
        // configuration actions, and Exit — grouped per spec §9.
        let vol_label = MenuItem::with_id("volume", "VolumeControl — --", false, None);
        let sep1 = PredefinedMenuItem::separator();
        let mute_item = CheckMenuItem::with_id("mute", "Mute", true, false, None);
        let reset = MenuItem::with_id("reset", "Reset to 50%", true, None);
        let mixer = MenuItem::with_id("mixer", "Open mixer", true, None);
        let sep2 = PredefinedMenuItem::separator();
        let settings = MenuItem::with_id("settings", "Settings", true, None);
        let help = MenuItem::with_id("help", "Help", true, None);
        let reload = MenuItem::with_id("reload", "Reload configuration", true, None);
        let edit = MenuItem::with_id("edit", "Open config file", true, None);
        let sep3 = PredefinedMenuItem::separator();
        let exit = MenuItem::with_id("exit", "Exit VolumeControl", true, None);

        menu.append_items(&[
            &vol_label, &sep1, &mute_item, &reset, &mixer, &sep2, &settings, &help, &reload, &edit,
            &sep3, &exit,
        ])?;

        let tray = tray_icon::TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(tray_icon::Icon::from_rgba(tray_icon_rgba(), 32, 32)?)
            .with_tooltip("VolumeControl")
            .build()?;

        Ok(Tray {
            _tray: tray,
            vol_label,
            mute_item,
        })
    }

    /// Refresh the volume label and mute check state.
    pub fn set_volume(&self, state: &VolumeState) {
        self.vol_label
            .set_text(format!("VolumeControl — {}%", state.percent()));
        self.mute_item.set_checked(state.muted);
    }

    /// Poll for a menu command for the legacy native host.
    ///
    /// The Tauri host must use its `on_menu_event` callback instead because
    /// Tauri takes ownership of the global `muda` event handler. The legacy
    /// host has no Tauri runtime and can continue consuming this channel.
    pub fn poll(&self) -> Option<TrayCommand> {
        let rx = MenuEvent::receiver();
        loop {
            match rx.try_recv() {
                Ok(event) => {
                    if let Some(command) = TrayCommand::from_menu_id(event.id.as_ref()) {
                        return Some(command);
                    }
                }
                Err(_) => return None,
            }
        }
    }

    /// Pop the context menu at the current cursor position.
    pub fn show_menu(&self) {
        self._tray.show_menu();
    }
}
