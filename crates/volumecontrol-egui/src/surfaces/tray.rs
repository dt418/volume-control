//! Native system tray surface and context menu.
//!
//! # Specification & Parity
//! - Spec: `docs/superpowers/specs/2026-10-24-egui-foundation-migration-design.md` §4.5
//! - Reference: `docs/references/legacy-app/feature-reference.md` §32
//! - Design: 10-item menu: disabled live volume header + 9 actionable items
//!   grouped with separators:
//!   - Group 1: Live header ("Volume Control — {pct}%")
//!   - (Separator)
//!   - Group 2: Volume actions (Tắt tiếng / Mute, Đặt lại 50% / Reset to 50%,
//!     Mở bộ trộn âm thanh / Open mixer)
//!   - (Separator)
//!   - Group 3: Surfaces & config (Cài đặt / Settings, Trợ giúp / Help,
//!     Kiểm tra cập nhật / Check for updates, Tải lại cấu hình / Reload configuration,
//!     Mở file cấu hình / Open config file)
//!   - (Separator)
//!   - Group 4: Exit (Thoát Điều khiển âm lượng / Exit Volume Control)
//!
//! # i18n Exception Note
//! `crates/volumecontrol-egui/src/i18n.rs` contains the 217 keys ported from
//! `frontend/src/lib/i18n.tsx`. Specific tray entries (e.g. `tray_reset`, `tray_mixer`,
//! `tray_check_updates`, `tray_reload`, `tray_edit`, `tray_exit`) were part of the
//! native host locale table rather than frontend web keys.
//! Per prompt instruction and spec §4.5, missing tray keys source their exact legacy strings
//! from `volumectl::locale::strings(language)` (matching `docs/references/legacy-app/feature-reference.md`),
//! preserving 100% bilingual parity without inventing ad-hoc strings.

use std::sync::mpsc::Sender;

use tray_icon::{TrayIcon, TrayIconBuilder};
use volumectl::config::Language;

/// Actions that can be triggered from the tray menu or tray icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrayAction {
    /// Non-clickable status header displaying live volume percentage.
    Header,
    /// Open the egui-rendered context menu (right click on the tray icon).
    OpenMenu,
    /// Toggle mute state.
    ToggleMute,
    /// Reset master volume to 50%.
    Reset50,
    /// Show or toggle the Volume Mixer surface.
    ShowMixer,
    /// Show the Settings window.
    ShowSettings,
    /// Show the Help window.
    ShowHelp,
    /// Trigger background update check.
    CheckUpdates,
    /// Reload configuration file from disk.
    ReloadConfig,
    /// Open configuration file in default editor.
    OpenConfig,
    /// Exit the application.
    Exit,
}

impl TrayAction {
    /// Returns `true` if this item is clickable and should dispatch an action.
    #[inline]
    pub fn is_actionable(&self) -> bool {
        !matches!(self, Self::Header | Self::OpenMenu)
    }

    /// Stable menu item identifier shared with cross-platform hosts.
    pub fn menu_id(&self) -> &'static str {
        match self {
            Self::Header => "volume",
            Self::OpenMenu => "open_menu",
            Self::ToggleMute => "mute",
            Self::Reset50 => "reset",
            Self::ShowMixer => "mixer",
            Self::ShowSettings => "settings",
            Self::ShowHelp => "help",
            Self::CheckUpdates => "check_updates",
            Self::ReloadConfig => "reload",
            Self::OpenConfig => "edit",
            Self::Exit => "exit",
        }
    }

    /// Decode a stable menu ID into a `TrayAction`.
    pub fn from_menu_id(id: &str) -> Option<Self> {
        match id {
            "volume" | "header" => Some(Self::Header),
            "open_menu" => Some(Self::OpenMenu),
            "mute" => Some(Self::ToggleMute),
            "reset" => Some(Self::Reset50),
            "mixer" => Some(Self::ShowMixer),
            "settings" => Some(Self::ShowSettings),
            "help" => Some(Self::ShowHelp),
            "check_updates" => Some(Self::CheckUpdates),
            "reload" => Some(Self::ReloadConfig),
            "edit" => Some(Self::OpenConfig),
            "exit" => Some(Self::Exit),
            _ => None,
        }
    }
}

/// Generate the ordered 10-item menu structure for the given language and volume percentage.
///
/// Returns `(action, localized_label)` pairs in exact legacy display order:
/// 1. Live Header ("Volume Control — {pct}%")
/// 2. Toggle Mute
/// 3. Reset 50%
/// 4. Open Mixer
/// 5. Settings
/// 6. Help
/// 7. Check for Updates
/// 8. Reload Configuration
/// 9. Open Config File
/// 10. Exit
pub fn menu_items(language: Language, volume_pct: u8) -> Vec<(TrayAction, String)> {
    let s = volumectl::locale::strings(language);
    vec![
        (
            TrayAction::Header,
            format!("Volume Control — {volume_pct}%"),
        ),
        (TrayAction::ToggleMute, s.tray_mute.to_string()),
        (TrayAction::Reset50, s.tray_reset.to_string()),
        (TrayAction::ShowMixer, s.tray_mixer.to_string()),
        (TrayAction::ShowSettings, s.tray_settings.to_string()),
        (TrayAction::ShowHelp, s.tray_help.to_string()),
        (TrayAction::CheckUpdates, s.tray_check_updates.to_string()),
        (TrayAction::ReloadConfig, s.tray_reload.to_string()),
        (TrayAction::OpenConfig, s.tray_edit.to_string()),
        (TrayAction::Exit, s.tray_exit.to_string()),
    ]
}

/// Load and decode the 32x32 application icon for the tray.
///
/// Path is relative to crate src: `../../../src-tauri/icons/32x32.png`.
pub fn build_icon() -> Option<tray_icon::Icon> {
    let bytes: &[u8] = include_bytes!("../../../../src-tauri/icons/32x32.png");
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer).ok()?;
    buffer.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buffer,
        png::ColorType::Rgb => {
            let (pixels, _) = buffer.as_chunks::<3>();
            pixels
                .iter()
                .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
                .collect()
        }
        _ => return None,
    };
    tray_icon::Icon::from_rgba(rgba, info.width, info.height).ok()
}

/// Managed system tray icon. The context menu is rendered by egui (see
/// [`super::tray_menu`]) so its colors, hit targets, and accessibility match
/// the app instead of the OS theme.
pub struct Tray {
    _tray: TrayIcon,
    _sender: Sender<TrayAction>,
}

impl Tray {
    /// Create and display the tray icon.
    pub fn new(sender: Sender<TrayAction>) -> Result<Self, String> {
        let icon = build_icon().ok_or_else(|| "Failed to build tray icon".to_string())?;
        let tray = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("Volume Control (egui)")
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            _tray: tray,
            _sender: sender,
        })
    }

    /// Drain pending tray icon events.
    ///
    /// Left click toggles the mixer; right click opens the egui context menu.
    pub fn poll(&self) -> Option<TrayAction> {
        while let Ok(event) = tray_icon::TrayIconEvent::receiver().try_recv() {
            if let tray_icon::TrayIconEvent::Click {
                button,
                button_state: tray_icon::MouseButtonState::Up,
                ..
            } = event
            {
                let action = match button {
                    tray_icon::MouseButton::Left => TrayAction::ShowMixer,
                    tray_icon::MouseButton::Right => TrayAction::OpenMenu,
                    _ => continue,
                };
                let _ = self._sender.send(action);
                return Some(action);
            }
        }
        None
    }
}
