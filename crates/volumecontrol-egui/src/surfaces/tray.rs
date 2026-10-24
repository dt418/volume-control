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

use muda::{Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};
use volumectl::config::Language;

/// Actions that can be triggered from the tray menu or tray icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrayAction {
    /// Non-clickable status header displaying live volume percentage.
    Header,
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
        !matches!(self, Self::Header)
    }

    /// Stable menu item identifier shared with cross-platform hosts.
    pub fn menu_id(&self) -> &'static str {
        match self {
            Self::Header => "volume",
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

// Global event routing state for `muda` and `tray-icon`.
// Handlers are installed once and forward events directly to the active Tray sender.
// No nested event loops are spawned.
// Events are polled from the eframe loop (muda/tray-icon own their OS
// channels). Installing a global event handler here would move events off the
// channel and out of the host's reach.

/// Managed system tray icon and context menu.
pub struct Tray {
    _tray: TrayIcon,
    _sender: Sender<TrayAction>,
    header_item: MenuItem,
    action_items: Vec<(TrayAction, MenuItem)>,
    language: Language,
    volume_pct: u8,
}

impl Tray {
    /// Create and display the system tray icon with context menu.
    ///
    /// - `language`: Initial UI language (En or Vi)
    /// - `volume_pct`: Initial volume level for the disabled header row
    /// - `sender`: Channel sender through which tray clicks are delivered to the main loop
    pub fn new(
        language: Language,
        volume_pct: u8,
        sender: Sender<TrayAction>,
    ) -> Result<Self, String> {
        let menu = Menu::new();
        let s = volumectl::locale::strings(language);

        // Header (disabled)
        let header_label = format!("Volume Control — {volume_pct}%");
        let header_item =
            MenuItem::with_id(TrayAction::Header.menu_id(), header_label, false, None);
        let sep1 = PredefinedMenuItem::separator();

        // Volume group
        let mute_item =
            MenuItem::with_id(TrayAction::ToggleMute.menu_id(), s.tray_mute, true, None);
        let reset_item = MenuItem::with_id(TrayAction::Reset50.menu_id(), s.tray_reset, true, None);
        let mixer_item =
            MenuItem::with_id(TrayAction::ShowMixer.menu_id(), s.tray_mixer, true, None);
        let sep2 = PredefinedMenuItem::separator();

        // Surface & config group
        let settings_item = MenuItem::with_id(
            TrayAction::ShowSettings.menu_id(),
            s.tray_settings,
            true,
            None,
        );
        let help_item = MenuItem::with_id(TrayAction::ShowHelp.menu_id(), s.tray_help, true, None);
        let check_updates_item = MenuItem::with_id(
            TrayAction::CheckUpdates.menu_id(),
            s.tray_check_updates,
            true,
            None,
        );
        let reload_item = MenuItem::with_id(
            TrayAction::ReloadConfig.menu_id(),
            s.tray_reload,
            true,
            None,
        );
        let edit_item =
            MenuItem::with_id(TrayAction::OpenConfig.menu_id(), s.tray_edit, true, None);
        let sep3 = PredefinedMenuItem::separator();

        // Exit group
        let exit_item = MenuItem::with_id(TrayAction::Exit.menu_id(), s.tray_exit, true, None);

        menu.append_items(&[
            &header_item,
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
        ])
        .map_err(|e| e.to_string())?;

        let icon = build_icon().ok_or_else(|| "Failed to build tray icon".to_string())?;

        let tray = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("Volume Control")
            .build()
            .map_err(|e| e.to_string())?;
        #[cfg(target_os = "windows")]
        {
            let themed = menu.clone();
            // SAFETY: the HWND belongs to the live tray icon and the menu is
            // initialized for it before being attached.
            unsafe {
                themed
                    .init_for_hwnd_with_theme(tray.window_handle() as isize, muda::MenuTheme::Dark)
                    .map_err(|e| e.to_string())?;
            }
        }
        tray.set_menu(Some(Box::new(menu)));

        let action_items = vec![
            (TrayAction::ToggleMute, mute_item),
            (TrayAction::Reset50, reset_item),
            (TrayAction::ShowMixer, mixer_item),
            (TrayAction::ShowSettings, settings_item),
            (TrayAction::ShowHelp, help_item),
            (TrayAction::CheckUpdates, check_updates_item),
            (TrayAction::ReloadConfig, reload_item),
            (TrayAction::OpenConfig, edit_item),
            (TrayAction::Exit, exit_item),
        ];

        Ok(Self {
            _tray: tray,
            _sender: sender,
            header_item,
            action_items,
            language,
            volume_pct,
        })
    }

    /// Update the disabled header label with a new volume percentage.
    /// Drain one pending menu/tray event, if any.
    pub fn poll(&self) -> Option<TrayAction> {
        if let Ok(event) = muda::MenuEvent::receiver().try_recv() {
            if let Some(action) = TrayAction::from_menu_id(event.id().as_ref()) {
                if action.is_actionable() {
                    let _ = self._sender.send(action);
                    return Some(action);
                }
            }
        }
        if let Ok(tray_icon::TrayIconEvent::Click {
            button: tray_icon::MouseButton::Left,
            button_state: tray_icon::MouseButtonState::Up,
            ..
        }) = tray_icon::TrayIconEvent::receiver().try_recv()
        {
            let _ = self._sender.send(TrayAction::ShowMixer);
            return Some(TrayAction::ShowMixer);
        }
        None
    }

    pub fn update_header(&mut self, volume_pct: u8) {
        self.volume_pct = volume_pct;
        let label = format!("Volume Control — {volume_pct}%");
        self.header_item.set_text(label);
    }

    /// Re-label all menu items for the new language.
    pub fn set_language(&mut self, language: Language) {
        self.language = language;
        let s = volumectl::locale::strings(language);
        for (action, item) in &self.action_items {
            let label = match action {
                TrayAction::Header => continue,
                TrayAction::ToggleMute => s.tray_mute,
                TrayAction::Reset50 => s.tray_reset,
                TrayAction::ShowMixer => s.tray_mixer,
                TrayAction::ShowSettings => s.tray_settings,
                TrayAction::ShowHelp => s.tray_help,
                TrayAction::CheckUpdates => s.tray_check_updates,
                TrayAction::ReloadConfig => s.tray_reload,
                TrayAction::OpenConfig => s.tray_edit,
                TrayAction::Exit => s.tray_exit,
            };
            item.set_text(label);
        }
    }

    /// Currently configured UI language.
    #[inline]
    pub fn language(&self) -> Language {
        self.language
    }

    /// Currently displayed volume percentage.
    #[inline]
    pub fn volume_pct(&self) -> u8 {
        self.volume_pct
    }
}
