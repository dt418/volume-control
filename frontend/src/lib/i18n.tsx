/**
 * Minimal typed i18n for the webview surfaces (EN/VI).
 *
 * The language comes from the host bootstrap payload (`config.language`);
 * `useT()` falls back to English when no provider is mounted so component
 * tests keep rendering without i18n setup. Dictionaries are flat and typed:
 * a missing key in either language is a TypeScript error, not a silent
 * fallback.
 */
import { createContext, useContext, useMemo, type ReactNode } from "react";

export type Language = "en" | "vi";

const EN = {
  "app.language": "Language",
  "app.languageEn": "English",
  "app.languageVi": "Tiếng Việt",

  "status.saved": "Saved",
  "status.saving": "Saving…",

  "common.close": "Close",
  "common.cancel": "Cancel",
  "common.save": "Save",
  "common.saveChanges": "Save changes",
  "common.mute": "Mute",
  "common.unmute": "Unmute",
  "common.reset": "Reset",
  "common.volume": "Volume",
  "common.muted": "Muted",
  "common.systemOutput": "System output",

  "overlay.volume": "Volume",
  "overlay.systemOutput": "System output",

  "mixer.title": "Volume Mixer",
  "mixer.systemOutput": "System output",
  "mixer.mute": "Mute",
  "mixer.unmute": "Unmute",
  "mixer.muteSystemOutput": "Mute system output",
  "mixer.unmuteSystemOutput": "Unmute system output",
  "mixer.resetVolume": "Reset volume to 50%",
  "mixer.reset": "Reset volume to 50 percent",
  "mixer.close": "Close mixer",
  "mixer.sessions": "Applications",
  "mixer.noSessions": "No audio sessions",
  "mixer.noMatches": "No matching apps",
  "mixer.appSessions": "app sessions",
  "mixer.searchPlaceholder": "Search apps…",
  "mixer.unsupported": "Per-app audio isn't available on this platform.",
  "mixer.connectionAttention": "Mixer connection needs attention",
  "mixer.retry": "Retry",
  "mixer.retryConnection": "Retry connection",
  "mixer.escToClose": "Esc to close",
  "mixer.backendUnavailable": "Audio backend unavailable",
  "mixer.backendTemporarilyUnavailable": "Audio backend temporarily unavailable",

  "settings.title": "Settings",
  "settings.subtitle": "Configure volume, hotkeys, appearance, and feedback",
  "settings.general": "General",
  "settings.hotkeys": "Hotkeys",
  "settings.appearance": "Appearance",
  "settings.blacklist": "Blacklist",
  "settings.feedback": "Feedback",
  "settings.storage": "Storage",

  "settings.volumeStep": "Volume step",
  "settings.volumeStepHelp": "Small change applied by ↑ / ↓.",
  "settings.volumeStepLarge": "Large volume step",
  "settings.volumeStepLargeHelp": "Shift + ↑ / ↓ applies the large change.",
  "settings.overlayDuration": "Overlay duration",
  "settings.overlayDurationHelp": "How long the volume overlay stays visible.",
  "settings.autostart": "Start with Windows",
  "settings.autostartHelp": "Launch VolumeControl when you sign in.",
  "settings.autostartUnavailable": "Auto-start is unavailable on this platform.",
  "settings.theme": "Theme",
  "settings.material": "Material",
  "settings.motion": "Motion",
  "settings.accent": "Accent",
  "settings.modifier": "Modifier",
  "settings.openConfig": "Open config.ini (advanced)",
  "settings.saveFailed": "Save failed",
  "settings.loadFailed": "Could not load settings",

  "storage.loadingPath": "Loading config path…",
  "storage.editingWarning": "Editing config.ini directly is an advanced option; changes reload automatically. Legacy config.json is kept as a migration backup.",
  "storage.migratedFromJson": "Configuration migrated from legacy JSON to config.ini; the JSON backup was kept.",
  "storage.recoveredFromJson": "Configuration recovered from the legacy JSON backup after an invalid INI edit.",
  "storage.defaultsAfterError": "Configuration files could not be loaded, so validated defaults are active. Your backup was not deleted.",
  "storage.invalidExternalEdit": "An external INI edit was invalid; the last valid configuration remains active. Correct the file and retry.",

  "settings.keyboard": "Keyboard",
  "theme.system": "System",
  "theme.light": "Light",
  "theme.dark": "Dark",
  "material.auto": "Auto",
  "material.translucent": "Translucent",
  "material.opaque": "Opaque",
  "motion.full": "Full",
  "motion.reduced": "Reduced",
  "motion.disabled": "Disabled",
  "accent.system": "System",
  "accent.blue": "Blue",
  "accent.green": "Green",
  "accent.purple": "Purple",
  "accent.orange": "Orange",
  "blacklist.blockedApplication": "Blocked application",
  "blacklist.empty": "No blocked applications",
  "blacklist.emptyHint": "VolumeControl will respond to shortcuts everywhere.",
  "blacklist.subtitle": "Block shortcuts while these apps have focus.",
  "blacklist.add": "Add",
  "blacklist.remove": "Remove",
  "blacklist.clear": "Clear",
  "blacklist.applyRecommended": "Apply Recommended",
  "hotkeys.subtitle": "Press Record, then press a modifier and key. Changes stay in the draft until you save.",
  "hotkeys.presets": "Presets",
  "hotkeys.presetsHint": "Restore a familiar shortcut layout.",
  "hotkeys.globalShortcutsAria": "Global shortcuts",
  "recorder.record": "Record",
  "recorder.clear": "Clear",
  "threshold.heading": "Volume thresholds",
  "modifier.presetsAria": "Shortcut presets",
  "feedback.beepEnabled": "Enable beep feedback",
  "feedback.blockedFreq": "Blocked beep frequency",
  "feedback.blockedFreqHelp": "Beep when a shortcut is blocked.",
  "feedback.blockedDuration": "Blocked beep duration",
  "feedback.blockedDurationHelp": "How long the blocked beep sounds.",
  "feedback.limitFreq": "Limit beep frequency",
  "feedback.limitFreqHelp": "Beep when volume is already at the limit.",
  "feedback.limitDuration": "Limit beep duration",
  "feedback.limitDurationHelp": "How long the limit beep sounds.",
  "threshold.green": "Green up to",
  "threshold.blue": "Blue up to",
  "threshold.orange": "Orange up to",
  "recorder.cleared": "Shortcut cleared",
  "recorder.recorded": "Shortcut recorded",
  "recorder.needModifier": "Press a modifier such as Ctrl, Alt, Shift, or Cmd with a key",
  "recorder.notSet": "Not set",
  "recorder.globalShortcut": "Global shortcut",
  "recorder.pressShortcut": "Press shortcut…",

  "help.title": "Help",
  "help.shortcuts": "Keyboard shortcuts",
  "help.close": "Close",

  "hotkey.volumeUp": "Volume Up",
  "hotkey.volumeUpHelp": "Increase volume by the small step.",
  "hotkey.volumeDown": "Volume Down",
  "hotkey.volumeDownHelp": "Decrease volume by the small step.",
  "hotkey.volumeUpLarge": "Volume Up (large)",
  "hotkey.volumeUpLargeHelp": "Increase volume by the large step.",
  "hotkey.volumeDownLarge": "Volume Down (large)",
  "hotkey.volumeDownLargeHelp": "Decrease volume by the large step.",
  "hotkey.toggleMute": "Toggle Mute",
  "hotkey.toggleMuteHelp": "Mute or unmute the current output.",
  "hotkey.reset50": "Reset to 50%",
  "hotkey.reset50Help": "Set the current output to 50%.",
  "hotkey.openMixer": "Open Mixer",
  "hotkey.openMixerHelp": "Show or hide the mixer panel.",
  "hotkey.openMenu": "Open Menu",
  "hotkey.openMenuHelp": "Open the VolumeControl tray menu.",

  "help.groupVolume": "Volume",
  "help.groupCommands": "Commands",
  "help.groupExtended": "Extended",
  "help.searchPlaceholder": "Search shortcuts…",
  "help.searchLabel": "Search shortcuts",
  "help.editConfig": "Edit config",
  "help.unavailable": "Help unavailable",
  "help.noMatches": "No shortcuts match",
  "help.capsFallback": "Caps Lock falls back to Ctrl + Alt",
  "help.statusInUse": "In use",
  "help.statusActive": "Active",
  "help.calloutTitle": "Shortcut conflict",
  "help.calloutBody": "Another application already owns one of these combinations.",
  "help.calloutAction": "Change the modifier in Settings.",
  "help.calloutTailOne": "is used by another app.",
  "help.calloutTailMany": "are used by another app.",
  "help.statusReady": "Ready",
  "help.statusFallback": "Fallback",
  "help.statusDisabled": "Disabled",
} as const;

export type TranslationKey = keyof typeof EN;

const VI: Record<TranslationKey, string> = {
  "app.language": "Ngôn ngữ",
  "app.languageEn": "English",
  "app.languageVi": "Tiếng Việt",

  "status.saved": "Đã lưu",
  "status.saving": "Đang lưu…",

  "common.close": "Đóng",
  "common.cancel": "Huỷ",
  "common.save": "Lưu",
  "common.saveChanges": "Lưu thay đổi",
  "common.mute": "Tắt tiếng",
  "common.unmute": "Bật tiếng",
  "common.reset": "Đặt lại",
  "common.volume": "Âm lượng",
  "common.muted": "Đã tắt tiếng",
  "common.systemOutput": "Đầu ra hệ thống",

  "overlay.volume": "Âm lượng",
  "overlay.systemOutput": "Đầu ra hệ thống",

  "mixer.title": "Bộ trộn âm lượng",
  "mixer.systemOutput": "Đầu ra hệ thống",
  "mixer.mute": "Tắt tiếng",
  "mixer.unmute": "Bật tiếng",
  "mixer.muteSystemOutput": "Tắt tiếng đầu ra hệ thống",
  "mixer.unmuteSystemOutput": "Bật tiếng đầu ra hệ thống",
  "mixer.resetVolume": "Đặt âm lượng về 50%",
  "mixer.reset": "Đặt âm lượng về 50 phần trăm",
  "mixer.close": "Đóng bộ trộn",
  "mixer.sessions": "Ứng dụng",
  "mixer.noSessions": "Không có phiên âm thanh",
  "mixer.noMatches": "Không có ứng dụng khớp",
  "mixer.appSessions": "phiên ứng dụng",
  "mixer.searchPlaceholder": "Tìm ứng dụng…",
  "mixer.unsupported": "Âm thanh theo ứng dụng không khả dụng trên nền tảng này.",
  "mixer.connectionAttention": "Kết nối bộ trộn cần được chú ý",
  "mixer.retry": "Thử lại",
  "mixer.retryConnection": "Thử lại kết nối",
  "mixer.escToClose": "Esc để đóng",
  "mixer.backendUnavailable": "Âm thanh không khả dụng",
  "mixer.backendTemporarilyUnavailable": "Âm thanh tạm thời không khả dụng",

  "settings.title": "Cài đặt",
  "settings.subtitle": "Cấu hình âm lượng, phím tắt, giao diện và phản hồi",
  "settings.general": "Chung",
  "settings.hotkeys": "Phím tắt",
  "settings.appearance": "Giao diện",
  "settings.blacklist": "Danh sách chặn",
  "settings.feedback": "Phản hồi",
  "settings.storage": "Lưu trữ",

  "settings.volumeStep": "Bước âm lượng",
  "settings.volumeStepHelp": "Mức thay đổi nhỏ khi nhấn ↑ / ↓.",
  "settings.volumeStepLarge": "Bước âm lượng lớn",
  "settings.volumeStepLargeHelp": "Shift + ↑ / ↓ áp dụng mức thay đổi lớn.",
  "settings.overlayDuration": "Thời gian hiện overlay",
  "settings.overlayDurationHelp": "Overlay âm lượng hiển thị trong bao lâu.",
  "settings.autostart": "Khởi động cùng Windows",
  "settings.autostartHelp": "Tự chạy VolumeControl khi bạn đăng nhập.",
  "settings.autostartUnavailable": "Tự khởi động không khả dụng trên nền tảng này.",
  "settings.theme": "Chủ đề",
  "settings.material": "Chất liệu",
  "settings.motion": "Chuyển động",
  "settings.accent": "Màu nhấn",
  "settings.modifier": "Phím bổ trợ",
  "settings.openConfig": "Mở config.ini (nâng cao)",
  "settings.saveFailed": "Lưu thất bại",
  "settings.loadFailed": "Không tải được cài đặt",

  "storage.loadingPath": "Đang tải đường dẫn cấu hình…",
  "storage.editingWarning": "Sửa trực tiếp config.ini là tuỳ chọn nâng cao; thay đổi sẽ tự tải lại. config.json cũ được giữ làm bản sao dự phòng.",
  "storage.migratedFromJson": "Cấu hình đã được chuyển từ JSON cũ sang config.ini; bản JSON dự phòng vẫn được giữ.",
  "storage.recoveredFromJson": "Cấu hình đã được phục hồi từ bản JSON dự phòng sau khi INI bị sửa sai.",
  "storage.defaultsAfterError": "Không đọc được file cấu hình nên đang dùng giá trị mặc định hợp lệ. Bản dự phòng của bạn chưa bị xoá.",
  "storage.invalidExternalEdit": "File INI bị sửa sai từ bên ngoài; cấu hình hợp lệ gần nhất vẫn đang hoạt động. Sửa lại file rồi thử lại.",

  "settings.keyboard": "Bàn phím",
  "theme.system": "Hệ thống",
  "theme.light": "Sáng",
  "theme.dark": "Tối",
  "material.auto": "Tự động",
  "material.translucent": "Trong mờ",
  "material.opaque": "Đục",
  "motion.full": "Đầy đủ",
  "motion.reduced": "Giảm",
  "motion.disabled": "Tắt",
  "accent.system": "Hệ thống",
  "accent.blue": "Xanh dương",
  "accent.green": "Xanh lá",
  "accent.purple": "Tím",
  "accent.orange": "Cam",
  "blacklist.blockedApplication": "Ứng dụng bị chặn",
  "blacklist.empty": "Chưa chặn ứng dụng nào",
  "blacklist.emptyHint": "VolumeControl sẽ phản hồi phím tắt ở mọi nơi.",
  "blacklist.subtitle": "Chặn phím tắt khi các ứng dụng này đang focus.",
  "blacklist.add": "Thêm",
  "blacklist.remove": "Xoá",
  "blacklist.clear": "Xoá hết",
  "blacklist.applyRecommended": "Áp dụng gợi ý",
  "hotkeys.subtitle": "Nhấn Record rồi nhấn phím bổ trợ cùng một phím. Thay đổi chỉ áp dụng khi bạn lưu.",
  "hotkeys.presets": "Bộ đặt sẵn",
  "hotkeys.presetsHint": "Khôi phục bố cục phím tắt quen thuộc.",
  "hotkeys.globalShortcutsAria": "Phím tắt toàn cục",
  "recorder.record": "Ghi",
  "recorder.clear": "Xoá",
  "threshold.heading": "Ngưỡng âm lượng",
  "modifier.presetsAria": "Bộ phím bổ trợ đặt sẵn",
  "feedback.beepEnabled": "Bật phản hồi âm bíp",
  "feedback.blockedFreq": "Tần số bíp khi bị chặn",
  "feedback.blockedFreqHelp": "Phát bíp khi phím tắt bị chặn.",
  "feedback.blockedDuration": "Thời lượng bíp khi bị chặn",
  "feedback.blockedDurationHelp": "Bíp bị chặn kéo dài bao lâu.",
  "feedback.limitFreq": "Tần số bíp giới hạn",
  "feedback.limitFreqHelp": "Phát bíp khi âm lượng đã chạm giới hạn.",
  "feedback.limitDuration": "Thời lượng bíp giới hạn",
  "feedback.limitDurationHelp": "Bíp giới hạn kéo dài bao lâu.",
  "threshold.green": "Xanh lá tới",
  "threshold.blue": "Xanh dương tới",
  "threshold.orange": "Cam tới",
  "recorder.cleared": "Đã xoá phím tắt",
  "recorder.recorded": "Đã ghi phím tắt",
  "recorder.needModifier": "Nhấn phím bổ trợ như Ctrl, Alt, Shift hoặc Cmd cùng một phím",
  "recorder.notSet": "Chưa đặt",
  "recorder.globalShortcut": "Phím tắt toàn cục",
  "recorder.pressShortcut": "Nhấn phím tắt…",

  "help.title": "Trợ giúp",
  "help.shortcuts": "Phím tắt bàn phím",
  "help.close": "Đóng",

  "hotkey.volumeUp": "Tăng âm lượng",
  "hotkey.volumeUpHelp": "Tăng âm lượng theo bước nhỏ.",
  "hotkey.volumeDown": "Giảm âm lượng",
  "hotkey.volumeDownHelp": "Giảm âm lượng theo bước nhỏ.",
  "hotkey.volumeUpLarge": "Tăng âm lượng (lớn)",
  "hotkey.volumeUpLargeHelp": "Tăng âm lượng theo bước lớn.",
  "hotkey.volumeDownLarge": "Giảm âm lượng (lớn)",
  "hotkey.volumeDownLargeHelp": "Giảm âm lượng theo bước lớn.",
  "hotkey.toggleMute": "Bật/tắt tiếng",
  "hotkey.toggleMuteHelp": "Tắt hoặc bật tiếng đầu ra hiện tại.",
  "hotkey.reset50": "Đặt lại 50%",
  "hotkey.reset50Help": "Đặt đầu ra hiện tại về 50%.",
  "hotkey.openMixer": "Mở bộ trộn",
  "hotkey.openMixerHelp": "Hiện hoặc ẩn bảng bộ trộn.",
  "hotkey.openMenu": "Mở menu",
  "hotkey.openMenuHelp": "Mở menu khay của VolumeControl.",

  "help.groupVolume": "Âm lượng",
  "help.groupCommands": "Lệnh",
  "help.groupExtended": "Mở rộng",
  "help.searchPlaceholder": "Tìm phím tắt…",
  "help.searchLabel": "Tìm phím tắt",
  "help.editConfig": "Sửa cấu hình",
  "help.unavailable": "Trợ giúp không khả dụng",
  "help.noMatches": "Không có phím tắt khớp",
  "help.capsFallback": "Caps Lock dùng dự phòng Ctrl + Alt",
  "help.statusInUse": "Đang bị dùng",
  "help.statusActive": "Hoạt động",
  "help.calloutTitle": "Xung đột phím tắt",
  "help.calloutBody": "Một ứng dụng khác đã chiếm một trong các tổ hợp này.",
  "help.calloutAction": "Đổi phím bổ trợ trong Cài đặt.",
  "help.calloutTailOne": "đang bị ứng dụng khác dùng.",
  "help.calloutTailMany": "đang bị ứng dụng khác dùng.",
  "help.statusReady": "Sẵn sàng",
  "help.statusFallback": "Dự phòng",
  "help.statusDisabled": "Đã tắt",
};

const DICTIONARIES: Record<Language, Record<TranslationKey, string>> = {
  en: EN,
  vi: VI,
};

export type Translator = (key: TranslationKey) => string;

export function translatorFor(language: Language): Translator {
  const table = DICTIONARIES[language] ?? EN;
  return (key) => table[key] ?? EN[key];
}

const LanguageContext = createContext<Language>("en");

export function LanguageProvider({
  language,
  children,
}: {
  language: Language;
  children: ReactNode;
}) {
  return (
    <LanguageContext.Provider value={language}>{children}</LanguageContext.Provider>
  );
}

export function useLanguage(): Language {
  return useContext(LanguageContext);
}

export function useT(): Translator {
  const language = useContext(LanguageContext);
  return useMemo(() => translatorFor(language), [language]);
}

/** Narrow an arbitrary bootstrap payload value to a supported language. */
export function asLanguage(value: unknown): Language {
  return value === "vi" ? "vi" : "en";
}
