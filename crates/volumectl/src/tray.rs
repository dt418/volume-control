//! System tray icon + context menu (Windows).
//!
//! Built on the Tauri ecosystem's `tray-icon` + `muda` crates. Menu items:
//! live volume label, mute toggle (check item), reset to 50%, and Exit.
//! Menu events are drained from the crate's global receiver with `try_recv`
//! inside the app's 150 ms poll — no extra thread or event loop needed.

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
    EditConfig,
    ReloadConfig,
    ApplyBlacklist,
    Exit,
}

pub struct Tray {
    _tray: tray_icon::TrayIcon,
    vol_label: MenuItem,
    mute_item: CheckMenuItem,
}

impl Tray {
    pub fn new() -> Result<Tray, Box<dyn std::error::Error>> {
        let menu = Menu::new();

        let vol_label = MenuItem::with_id("volume", "Volume: --", false, None);
        let mixer = MenuItem::with_id("mixer", "Volume Mixer", true, None);
        let mute_item = CheckMenuItem::with_id("mute", "Mute / Unmute", true, false, None);
        let help = MenuItem::with_id("help", "Help / Hotkeys", true, None);
        let sep1 = PredefinedMenuItem::separator();
        let edit = MenuItem::with_id("edit", "Edit Config", true, None);
        let reload = MenuItem::with_id("reload", "Reload Config", true, None);
        let sep2 = PredefinedMenuItem::separator();
        let blacklist = MenuItem::with_id("blacklist", "Apply Recommended Blacklist", true, None);
        let sep3 = PredefinedMenuItem::separator();
        let reset = MenuItem::with_id("reset", "Reset to 50%", true, None);
        let exit = MenuItem::with_id("exit", "Exit", true, None);

        menu.append_items(&[
            &vol_label, &mixer, &mute_item, &help, &sep1, &edit, &reload, &sep2, &blacklist, &sep3,
            &reset, &exit,
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

    /// Poll for a menu command (non-blocking). Call from the 150 ms timer.
    pub fn poll(&self) -> Option<TrayCommand> {
        let rx = MenuEvent::receiver();
        loop {
            match rx.try_recv() {
                Ok(ev) => {
                    let cmd = match ev.id.as_ref() {
                        "mute" => Some(TrayCommand::ToggleMute),
                        "reset" => Some(TrayCommand::Reset50),
                        "mixer" => Some(TrayCommand::OpenMixer),
                        "help" => Some(TrayCommand::Help),
                        "edit" => Some(TrayCommand::EditConfig),
                        "reload" => Some(TrayCommand::ReloadConfig),
                        "blacklist" => Some(TrayCommand::ApplyBlacklist),
                        "exit" => Some(TrayCommand::Exit),
                        _ => None,
                    };
                    if cmd.is_some() {
                        return cmd;
                    }
                }
                Err(_) => return None, // Empty or Disconnected
            }
        }
    }

    /// Refresh the volume label and mute check state.
    pub fn set_volume(&self, state: &VolumeState) {
        self.vol_label
            .set_text(&format!("Volume: {}%", state.percent()));
        self.mute_item.set_checked(state.muted);
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
