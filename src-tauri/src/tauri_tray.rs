//! Tauri-managed system tray for macOS and Linux.
//!
//! Windows keeps its native `tray-icon`/`muda` tray (`crate::native_win32`);
//! this module is compiled only on non-Windows hosts. The menu mirrors the
//! Windows native tray structure (`crates/volumectl/src/tray.rs`) and reuses
//! the shared `TrayCommand` id mapping and generated icon from
//! `volumectl_lib::tray_common`.

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::AppHandle;

use volumectl_lib::audio::VolumeState;
use volumectl_lib::tray_common::{tray_icon_rgba, TrayCommand};

/// The Tauri-managed tray icon and its live menu items.
pub struct TauriTray {
    tray: TrayIcon,
    vol_label: MenuItem<tauri::Wry>,
    mute_item: CheckMenuItem<tauri::Wry>,
}

impl TauriTray {
    /// Build the tray menu, icon, and menu-event callback.
    ///
    /// Menu ids/texts mirror the Windows native tray in
    /// `crates/volumectl/src/tray.rs`; events route through the shared
    /// [`crate::dispatch_tray_command`] so both hosts dispatch identically.
    pub fn create(app: &AppHandle) -> Result<Self, String> {
        // Live volume label (non-clickable), then volume actions, surface
        // actions, configuration actions, and Exit — grouped per spec §9.
        let vol_label = MenuItem::with_id(app, "volume", "VolumeControl — --", false, None::<&str>)
            .map_err(|e| e.to_string())?;
        let sep1 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
        let mute_item = CheckMenuItem::with_id(app, "mute", "Mute", true, false, None::<&str>)
            .map_err(|e| e.to_string())?;
        let reset = MenuItem::with_id(app, "reset", "Reset to 50%", true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let mixer = MenuItem::with_id(app, "mixer", "Open mixer", true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let sep2 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
        let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let help = MenuItem::with_id(app, "help", "Help", true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let reload = MenuItem::with_id(app, "reload", "Reload configuration", true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let edit = MenuItem::with_id(app, "edit", "Open config file", true, None::<&str>)
            .map_err(|e| e.to_string())?;
        let sep3 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
        let exit = MenuItem::with_id(app, "exit", "Exit VolumeControl", true, None::<&str>)
            .map_err(|e| e.to_string())?;

        let menu = Menu::with_items(
            app,
            &[
                &vol_label, &sep1, &mute_item, &reset, &mixer, &sep2, &settings, &help, &reload,
                &edit, &sep3, &exit,
            ],
        )
        .map_err(|e| e.to_string())?;

        let icon = tauri::image::Image::new_owned(tray_icon_rgba(), 32, 32);
        let tray = TrayIconBuilder::with_id("main")
            .icon(icon)
            .menu(&menu)
            .tooltip("VolumeControl")
            .show_menu_on_left_click(true)
            .on_menu_event(|app, event| {
                let Some(command) = TrayCommand::from_menu_id(event.id().as_ref()) else {
                    return;
                };
                crate::dispatch_tray_command(app, command);
            })
            .build(app)
            .map_err(|e| e.to_string())?;

        Ok(Self {
            tray,
            vol_label,
            mute_item,
        })
    }

    /// Refresh the live volume label, mute check, and tooltip.
    pub fn set_volume(&self, state: &VolumeState) {
        let _ = self
            .vol_label
            .set_text(format!("VolumeControl — {}%", state.percent()));
        let _ = self.mute_item.set_checked(state.muted);
        // Tooltips are unsupported by the Linux appindicator backend; ignore.
        if let Err(error) = self
            .tray
            .set_tooltip(Some(format!("VolumeControl — {}%", state.percent())))
        {
            log::debug!("tray tooltip update unsupported: {error}");
        }
    }

    /// Pop the tray menu programmatically.
    ///
    /// Verified in the Tauri 2.11.5 source: `TrayIcon` has no `show_menu`
    /// API here (the inner `tray-icon` handle is not exposed, and
    /// `tray-icon`'s `show_menu` exists only on Windows/macOS impls). The
    /// menu opens on click on both macOS and Linux, so this is a documented
    /// no-op that keeps `EventSink::show_tray_menu` uniform across hosts.
    pub fn show_menu(&self) {
        log::debug!("tray menu opens on click; programmatic popup unsupported on this platform");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::menu::MenuId;

    #[test]
    fn menu_ids_match_the_shared_tray_command_mapping() {
        // Building a real tray needs a running app; assert the id contract
        // statically so the two sources cannot drift.
        for id in [
            "mute", "reset", "mixer", "settings", "help", "reload", "edit", "exit",
        ] {
            assert!(
                TrayCommand::from_menu_id(id).is_some(),
                "tray menu id {id:?} must map to a TrayCommand"
            );
            let _ = MenuId::new(id.to_string());
        }
    }
}
