//! Cross-platform tray contracts shared by every host.
//!
//! The Windows native tray (`tray-icon` + `muda`, see [`crate::tray`]) and
//! the Tauri-managed tray on macOS/Linux (`src-tauri`) both use the same
//! stable menu ids and the same generated icon, so the id→command mapping
//! stays the single source of truth on every platform.

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
    /// are delivered through `Builder::on_menu_event` (Windows) or the
    /// `TrayIconBuilder::on_menu_event` callback (macOS/Linux) rather than the
    /// `muda::MenuEvent::receiver` channel. Keeping the id mapping here gives
    /// both hosts and tests a single source of truth.
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

/// A simple speaker glyph as RGBA pixels (32×32).
///
/// Body + cone in the app's blue, sound waves lighter. Generated in code so
/// no binary asset is needed; both tray backends (native Windows tray and
/// the Tauri tray) consume the same bytes.
pub fn tray_icon_rgba() -> Vec<u8> {
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
    use super::*;

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

    #[test]
    fn tray_icon_is_32x32_rgba() {
        let px = tray_icon_rgba();
        assert_eq!(px.len(), 32 * 32 * 4);
        // At least one opaque speaker pixel and one transparent pixel.
        assert!(px.chunks_exact(4).any(|p| p[3] == 255));
        assert!(px.chunks_exact(4).any(|p| p[3] == 0));
    }
}
