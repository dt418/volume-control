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
use crate::config::Language;
use crate::locale;
use crate::tray_common::{tray_icon_rgba, TrayCommand};

pub struct Tray {
    _tray: tray_icon::TrayIcon,
    vol_label: MenuItem,
    mute_item: CheckMenuItem,
    reset_item: MenuItem,
    mixer_item: MenuItem,
    settings_item: MenuItem,
    help_item: MenuItem,
    check_updates_item: MenuItem,
    reload_item: MenuItem,
    edit_item: MenuItem,
    exit_item: MenuItem,
}

impl Tray {
    pub fn new(lang: Language) -> Result<Tray, Box<dyn std::error::Error>> {
        let menu = Menu::new();
        let s = locale::strings(lang);

        // Live label (non-clickable), then volume actions, surface actions,
        // configuration actions, and Exit — grouped per spec §9. Menu ids are
        // stable and untranslated; only labels come from the locale table.
        let vol_label = MenuItem::with_id("volume", "Volume Control — --", false, None);
        let sep1 = PredefinedMenuItem::separator();
        let mute_item = CheckMenuItem::with_id("mute", s.tray_mute, true, false, None);
        let reset_item = MenuItem::with_id("reset", s.tray_reset, true, None);
        let mixer_item = MenuItem::with_id("mixer", s.tray_mixer, true, None);
        let sep2 = PredefinedMenuItem::separator();
        let settings_item = MenuItem::with_id("settings", s.tray_settings, true, None);
        let help_item = MenuItem::with_id("help", s.tray_help, true, None);
        let check_updates_item =
            MenuItem::with_id("check_updates", s.tray_check_updates, true, None);
        let reload_item = MenuItem::with_id("reload", s.tray_reload, true, None);
        let edit_item = MenuItem::with_id("edit", s.tray_edit, true, None);
        let sep3 = PredefinedMenuItem::separator();
        let exit_item = MenuItem::with_id("exit", s.tray_exit, true, None);

        menu.append_items(&[
            &vol_label,
            &sep1,
            &mute_item,
            &reset_item,
            &mixer_item,
            &sep2,
            &settings_item,
            &help_item,
            &check_updates_item,
            &reload_item,
            &edit_item,
            &sep3,
            &exit_item,
        ])?;

        let tray = tray_icon::TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_icon(tray_icon::Icon::from_rgba(tray_icon_rgba(), 32, 32)?)
            .with_tooltip("Volume Control")
            .build()?;

        Ok(Tray {
            _tray: tray,
            vol_label,
            mute_item,
            reset_item,
            mixer_item,
            settings_item,
            help_item,
            check_updates_item,
            reload_item,
            edit_item,
            exit_item,
        })
    }

    /// Relabel every menu entry for `lang` (used after a config reload or a
    /// Settings save changes the language; menu ids never change).
    pub fn apply_language(&self, lang: Language) {
        let s = locale::strings(lang);
        self.mute_item.set_text(s.tray_mute);
        self.reset_item.set_text(s.tray_reset);
        self.mixer_item.set_text(s.tray_mixer);
        self.settings_item.set_text(s.tray_settings);
        self.help_item.set_text(s.tray_help);
        self.check_updates_item.set_text(s.tray_check_updates);
        self.reload_item.set_text(s.tray_reload);
        self.edit_item.set_text(s.tray_edit);
        self.exit_item.set_text(s.tray_exit);
    }

    /// Refresh the volume label and mute check state.
    pub fn set_volume(&self, state: &VolumeState) {
        self.vol_label
            .set_text(format!("Volume Control — {}%", state.percent()));
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
