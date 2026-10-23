//! Localized user-visible strings (EN/VI) keyed by stable IDs.
//!
//! Menu item IDs, config keys, and accessibility names stay untranslated;
//! only the human-readable labels come from here. Every field must be
//! non-empty in both languages — `table_covers_all_keys` enforces it.
//! Call sites pass [`Language`](crate::config::Language) from the loaded
//! config, so a language switch is a config change, not a code change.

use crate::config::Language;

/// Every localizable label in the native surfaces.
#[derive(Debug, Clone, Copy)]
pub struct Strings {
    pub tray_mute: &'static str,
    pub tray_reset: &'static str,
    pub tray_mixer: &'static str,
    pub tray_settings: &'static str,
    pub tray_help: &'static str,
    pub tray_reload: &'static str,
    pub tray_check_updates: &'static str,
    pub tray_edit: &'static str,
    pub tray_exit: &'static str,
    pub update_check_up_to_date: &'static str,
    pub update_check_available_prefix: &'static str,
    pub update_check_failed: &'static str,
    pub update_check_in_progress: &'static str,
    pub update_notify_title: &'static str,
    pub update_notify_body: &'static str,
    pub update_notify_install: &'static str,
    pub update_notify_later: &'static str,
    pub toast_editing_config: &'static str,
    pub toast_config_reloaded: &'static str,
    pub mixer_mute: &'static str,
    pub mixer_unmute: &'static str,
    pub mixer_reset: &'static str,
    pub mixer_output: &'static str,
    pub mixer_close: &'static str,
    pub mixer_output_name: &'static str,
    pub mixer_title: &'static str,
    pub settings_title: &'static str,
    pub settings_subtitle: &'static str,
    pub section_general: &'static str,
    pub section_hotkeys: &'static str,
    pub section_appearance: &'static str,
    pub section_blacklist: &'static str,
    pub section_feedback: &'static str,
    pub section_storage: &'static str,
    pub gtk_reset: &'static str,
    pub gtk_close: &'static str,
    pub overlay_volume: &'static str,
}

/// The full label table for a language.
pub fn strings(lang: Language) -> Strings {
    match lang {
        Language::En => Strings {
            tray_mute: "Mute",
            tray_reset: "Reset to 50%",
            tray_mixer: "Open mixer",
            tray_settings: "Settings",
            tray_help: "Help",
            tray_reload: "Reload configuration",
            tray_check_updates: "Check for updates",
            tray_edit: "Open config file",
            tray_exit: "Exit Volume Control",
            update_check_up_to_date: "Volume Control is up to date",
            update_check_available_prefix: "Volume Control update available:",
            update_check_failed: "Update check failed",
            update_check_in_progress: "Checking for updates…",
            update_notify_title: "Update available",
            update_notify_body: "Volume Control {0} is ready. Open Settings to install it.",
            update_notify_install: "Install",
            update_notify_later: "Later",
            toast_editing_config: "Editing config — changes reload automatically",
            toast_config_reloaded: "Config reloaded",
            mixer_mute: "Mute",
            mixer_unmute: "Unmute",
            mixer_reset: "Reset volume to 50 percent",
            mixer_output: "System output",
            mixer_close: "Close mixer",
            mixer_output_name: "System output volume",
            mixer_title: "Volume Mixer",
            settings_title: "Settings",
            settings_subtitle: "Configure how Volume Control behaves",
            section_general: "General",
            section_hotkeys: "Hotkeys",
            section_appearance: "Appearance",
            section_blacklist: "Blacklist",
            section_feedback: "Feedback",
            section_storage: "Storage",
            gtk_reset: "Reset",
            gtk_close: "Close",
            overlay_volume: "Volume",
        },
        Language::Vi => Strings {
            tray_mute: "Tắt tiếng",
            tray_reset: "Đặt lại 50%",
            tray_mixer: "Mở bộ trộn âm thanh",
            tray_settings: "Cài đặt",
            tray_help: "Trợ giúp",
            tray_reload: "Tải lại cấu hình",
            tray_check_updates: "Kiểm tra cập nhật",
            tray_edit: "Mở file cấu hình",
            tray_exit: "Thoát Điều khiển âm lượng",
            update_check_up_to_date: "Điều khiển âm lượng đã là bản mới nhất",
            update_check_available_prefix: "Đã có bản cập nhật Điều khiển âm lượng:",
            update_check_failed: "Kiểm tra cập nhật thất bại",
            update_check_in_progress: "Đang kiểm tra cập nhật…",
            update_notify_title: "Có bản cập nhật",
            update_notify_body: "Điều khiển âm lượng {0} đã sẵn sàng. Mở Cài đặt để cài đặt.",
            update_notify_install: "Cài đặt",
            update_notify_later: "Để sau",
            toast_editing_config: "Đang sửa cấu hình — thay đổi tự áp dụng",
            toast_config_reloaded: "Đã tải lại cấu hình",
            mixer_mute: "Tắt tiếng",
            mixer_unmute: "Bật tiếng",
            mixer_reset: "Đặt âm lượng về 50 phần trăm",
            mixer_output: "Đầu ra hệ thống",
            mixer_close: "Đóng bộ trộn",
            mixer_output_name: "Âm lượng đầu ra hệ thống",
            mixer_title: "Bộ trộn âm lượng",
            settings_title: "Cài đặt",
            settings_subtitle: "Cấu hình cách Điều khiển âm lượng hoạt động",
            section_general: "Chung",
            section_hotkeys: "Phím tắt",
            section_appearance: "Giao diện",
            section_blacklist: "Danh sách chặn",
            section_feedback: "Phản hồi",
            section_storage: "Lưu trữ",
            gtk_reset: "Đặt lại",
            gtk_close: "Đóng",
            overlay_volume: "Âm lượng",
        },
    }
}

/// Dynamic toasts that embed runtime values.
pub fn toast_blacklist_none(lang: Language, modifier: &str) -> String {
    match lang {
        Language::En => format!("No blacklist needed for {modifier}"),
        Language::Vi => format!("Không cần danh sách chặn cho {modifier}"),
    }
}

pub fn toast_blacklist_applied(lang: Language, added: usize) -> String {
    match lang {
        Language::En => format!("Blacklist: +{added} app(s) applied"),
        Language::Vi => format!("Danh sách chặn: đã thêm {added} ứng dụng"),
    }
}

pub fn toast_blacklist_current(lang: Language) -> &'static str {
    match lang {
        Language::En => "Blacklist already up to date",
        Language::Vi => "Danh sách chặn đã được cập nhật",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! all_fields {
        ($s:ident, $check:expr) => {{
            let s = $s;
            $check(s.tray_mute);
            $check(s.tray_reset);
            $check(s.tray_mixer);
            $check(s.tray_settings);
            $check(s.tray_help);
            $check(s.tray_reload);
            $check(s.tray_check_updates);
            $check(s.tray_edit);
            $check(s.tray_exit);
            $check(s.update_check_up_to_date);
            $check(s.update_check_available_prefix);
            $check(s.update_check_failed);
            $check(s.update_check_in_progress);
            $check(s.update_notify_title);
            $check(s.update_notify_body);
            $check(s.update_notify_install);
            $check(s.update_notify_later);
            $check(s.toast_editing_config);
            $check(s.toast_config_reloaded);
            $check(s.mixer_mute);
            $check(s.mixer_unmute);
            $check(s.mixer_reset);
            $check(s.mixer_output);
            $check(s.mixer_close);
            $check(s.mixer_output_name);
            $check(s.mixer_title);
            $check(s.settings_title);
            $check(s.settings_subtitle);
            $check(s.section_general);
            $check(s.section_hotkeys);
            $check(s.section_appearance);
            $check(s.section_blacklist);
            $check(s.section_feedback);
            $check(s.section_storage);
            $check(s.gtk_reset);
            $check(s.gtk_close);
            $check(s.overlay_volume);
        }};
    }

    #[test]
    fn table_covers_all_keys_in_both_languages() {
        for lang in [Language::En, Language::Vi] {
            let table = strings(lang);
            all_fields!(table, |text: &'static str| {
                assert!(!text.is_empty(), "empty label for {lang:?}");
            });
        }
    }

    #[test]
    fn vietnamese_differs_from_english_where_translated() {
        let en = strings(Language::En);
        let vi = strings(Language::Vi);
        assert_ne!(en.tray_mute, vi.tray_mute);
        assert_ne!(en.settings_title, vi.settings_title);
        assert_ne!(en.toast_config_reloaded, vi.toast_config_reloaded);
        // Product name stays identical across languages.
        assert_eq!(en.tray_exit, "Exit Volume Control");
        assert!(vi.tray_exit.contains("Điều khiển âm lượng"));
    }

    #[test]
    fn dynamic_toasts_render_in_both_languages() {
        for lang in [Language::En, Language::Vi] {
            assert!(!toast_blacklist_none(lang, "CtrlAlt").is_empty());
            assert!(!toast_blacklist_applied(lang, 2).is_empty());
            assert!(!toast_blacklist_current(lang).is_empty());
        }
        assert!(toast_blacklist_applied(Language::En, 2).contains('2'));
        assert_ne!(
            toast_blacklist_current(Language::En),
            toast_blacklist_current(Language::Vi)
        );
    }
}
