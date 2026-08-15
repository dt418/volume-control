//! System tray icon + context menu (Windows).
//!
//! Built on the Tauri ecosystem's `tray-icon` + `muda` crates. Menu (spec §9):
//! live volume label, then volume actions (mute check item, reset, open
//! mixer), surface actions (settings, help), configuration actions (reload,
//! open config file), and Exit — each group separated. Menu events are routed
//! through the Tauri runtime's global menu handler (see the host crate),
//! because Tauri owns the `muda` event channel on desktop.

use muda::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};

use crate::audio::VolumeState;

/// Commands the app can receive from the tray menu.
///
/// The host maps each command to a shared [`crate::ui::AppAction`] at the host
/// boundary and dispatches it through the central action handler; tray-origin
/// commands bypass the hotkey blacklist gate by design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCommand {
    ToggleMute,
    Reset50,
    OpenMixer,
    Help,
    Settings,
    EditConfig,
    ReloadConfig,
    Exit,
}

impl TrayCommand {
    /// Decode the stable ids assigned to the native tray menu items.
    ///
    /// Tauri installs a process-wide `muda` menu event handler, so tray events
    /// are delivered through `Builder::on_menu_event` rather than the
    /// `muda::MenuEvent::receiver` channel. Keeping the id mapping here gives
    /// both the Tauri callback and tests a single source of truth.
    pub fn from_menu_id(id: &str) -> Option<Self> {
        match id {
            "mute" => Some(Self::ToggleMute),
            "reset" => Some(Self::Reset50),
            "mixer" => Some(Self::OpenMixer),
            "help" => Some(Self::Help),
            "settings" => Some(Self::Settings),
            "edit" => Some(Self::EditConfig),
            "reload" => Some(Self::ReloadConfig),
            "exit" => Some(Self::Exit),
            _ => None,
        }
    }
}

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

/// A simple speaker glyph as RGBA pixels (32×32).
///
/// Body + cone in the app's blue, sound waves lighter. Generated in code so
/// no binary asset is needed.
fn tray_icon_rgba() -> Vec<u8> {
    const S: usize = 32;
    let mut px = vec![0u8; S * S * 4];
    for y in 0..S {
        for x in 0..S {
            let i = (y * S + x) * 4;
            let xi = x as i32;
            let yi = y as i32;

            // Speaker body: rectangle on the left.
            let in_body = (6..14).contains(&xi) && (10..22).contains(&yi);
            // Cone: triangle from the body edge widening right.
            let in_cone = {
                let dx = xi - 14;
                (14..24).contains(&xi) && yi >= (10 + dx) && yi <= (21 - dx)
            };
            // Sound waves: two arcs around the cone tip.
            let dx = xi as f64 - 27.0;
            let dy = yi as f64 - 16.0;
            let dist = (dx * dx + dy * dy).sqrt();
            let in_wave = (dist - 3.0).abs() < 2.0 || (dist - 8.0).abs() < 2.0;

            if in_body || in_cone {
                px[i] = 0x00;
                px[i + 1] = 0x78;
                px[i + 2] = 0xD4; // blue
                px[i + 3] = 255;
            } else if in_wave {
                px[i] = 0x6C;
                px[i + 1] = 0xC1;
                px[i + 2] = 0xFF; // light blue
                px[i + 3] = 255;
            }
            // else: transparent
        }
    }
    px
}

#[cfg(test)]
mod tests {
    use super::TrayCommand;

    #[test]
    fn every_native_menu_id_decodes_to_a_command() {
        let cases = [
            ("mute", TrayCommand::ToggleMute),
            ("reset", TrayCommand::Reset50),
            ("mixer", TrayCommand::OpenMixer),
            ("settings", TrayCommand::Settings),
            ("help", TrayCommand::Help),
            ("reload", TrayCommand::ReloadConfig),
            ("edit", TrayCommand::EditConfig),
            ("exit", TrayCommand::Exit),
        ];

        for (id, expected) in cases {
            assert_eq!(TrayCommand::from_menu_id(id), Some(expected));
        }
        assert_eq!(TrayCommand::from_menu_id("volume"), None);
        assert_eq!(TrayCommand::from_menu_id("unknown"), None);
    }
}
