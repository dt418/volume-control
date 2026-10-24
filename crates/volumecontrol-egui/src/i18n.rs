//! Bilingual copy ported verbatim from `frontend/src/lib/i18n.tsx`.
//!
//! Typed keys plus positional `{0}`, `{1}` interpolation, matching the legacy
//! `translatorFor` contract. A missing key is a compile error: the tables are
//! fixed-size arrays indexed by `Key as usize`.

use std::borrow::Cow;

use volumectl::config::Language;

/// Legacy i18n key set.
#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// `app.language`
    AppLanguage,
    /// `app.languageEn`
    AppLanguageEn,
    /// `app.languageVi`
    AppLanguageVi,
    /// `app.name`
    AppName,
    /// `app.window.settings`
    AppWindowSettings,
    /// `app.window.help`
    AppWindowHelp,
    /// `status.saved`
    StatusSaved,
    /// `status.saving`
    StatusSaving,
    /// `common.close`
    CommonClose,
    /// `common.cancel`
    CommonCancel,
    /// `common.save`
    CommonSave,
    /// `common.saveChanges`
    CommonSaveChanges,
    /// `common.mute`
    CommonMute,
    /// `common.unmute`
    CommonUnmute,
    /// `common.reset`
    CommonReset,
    /// `common.volume`
    CommonVolume,
    /// `common.muted`
    CommonMuted,
    /// `common.systemOutput`
    CommonSystemOutput,
    /// `overlay.volume`
    OverlayVolume,
    /// `overlay.systemOutput`
    OverlaySystemOutput,
    /// `mixer.title`
    MixerTitle,
    /// `mixer.systemOutput`
    MixerSystemOutput,
    /// `mixer.mute`
    MixerMute,
    /// `mixer.unmute`
    MixerUnmute,
    /// `mixer.muteSystemOutput`
    MixerMuteSystemOutput,
    /// `mixer.unmuteSystemOutput`
    MixerUnmuteSystemOutput,
    /// `mixer.resetVolume`
    MixerResetVolume,
    /// `mixer.reset`
    MixerReset,
    /// `mixer.close`
    MixerClose,
    /// `mixer.sessions`
    MixerSessions,
    /// `mixer.noSessions`
    MixerNoSessions,
    /// `mixer.noMatches`
    MixerNoMatches,
    /// `mixer.appSessions`
    MixerAppSessions,
    /// `mixer.searchPlaceholder`
    MixerSearchPlaceholder,
    /// `mixer.clearSearch`
    MixerClearSearch,
    /// `mixer.unsupported`
    MixerUnsupported,
    /// `mixer.connectionAttention`
    MixerConnectionAttention,
    /// `mixer.retry`
    MixerRetry,
    /// `mixer.retryConnection`
    MixerRetryConnection,
    /// `mixer.escToClose`
    MixerEscToClose,
    /// `mixer.backendUnavailable`
    MixerBackendUnavailable,
    /// `mixer.backendTemporarilyUnavailable`
    MixerBackendTemporarilyUnavailable,
    /// `mixer.systemSounds`
    MixerSystemSounds,
    /// `mixer.systemSoundsHint`
    MixerSystemSoundsHint,
    /// `mixer.systemSoundsDescription`
    MixerSystemSoundsDescription,
    /// `mixer.muteFailed`
    MixerMuteFailed,
    /// `settings.title`
    SettingsTitle,
    /// `settings.subtitle`
    SettingsSubtitle,
    /// `settings.general`
    SettingsGeneral,
    /// `settings.hotkeys`
    SettingsHotkeys,
    /// `settings.appearance`
    SettingsAppearance,
    /// `settings.blacklist`
    SettingsBlacklist,
    /// `settings.feedback`
    SettingsFeedback,
    /// `settings.storage`
    SettingsStorage,
    /// `settings.appLabelStyle`
    SettingsAppLabelStyle,
    /// `settings.appLabelStyleHelp`
    SettingsAppLabelStyleHelp,
    /// `settings.appLabelFriendly`
    SettingsAppLabelFriendly,
    /// `settings.appLabelFriendlyAndProcess`
    SettingsAppLabelFriendlyAndProcess,
    /// `settings.appLabelProcess`
    SettingsAppLabelProcess,
    /// `settings.volumeStep`
    SettingsVolumeStep,
    /// `settings.volumeStepHelp`
    SettingsVolumeStepHelp,
    /// `settings.volumeStepLarge`
    SettingsVolumeStepLarge,
    /// `settings.volumeStepLargeHelp`
    SettingsVolumeStepLargeHelp,
    /// `settings.overlayDuration`
    SettingsOverlayDuration,
    /// `settings.overlayDurationHelp`
    SettingsOverlayDurationHelp,
    /// `settings.autostart`
    SettingsAutostart,
    /// `settings.autostartHelp`
    SettingsAutostartHelp,
    /// `settings.autostartUnavailable`
    SettingsAutostartUnavailable,
    /// `settings.theme`
    SettingsTheme,
    /// `settings.material`
    SettingsMaterial,
    /// `settings.motion`
    SettingsMotion,
    /// `settings.accent`
    SettingsAccent,
    /// `settings.modifier`
    SettingsModifier,
    /// `settings.openConfig`
    SettingsOpenConfig,
    /// `settings.saveFailed`
    SettingsSaveFailed,
    /// `settings.loadFailed`
    SettingsLoadFailed,
    /// `settings.updates.title`
    SettingsUpdatesTitle,
    /// `settings.updates.subtitle`
    SettingsUpdatesSubtitle,
    /// `settings.updates.current`
    SettingsUpdatesCurrent,
    /// `settings.updates.checkBtn`
    SettingsUpdatesCheckBtn,
    /// `settings.updates.checking`
    SettingsUpdatesChecking,
    /// `settings.updates.upToDate`
    SettingsUpdatesUpToDate,
    /// `settings.updates.available`
    SettingsUpdatesAvailable,
    /// `settings.updates.downloading`
    SettingsUpdatesDownloading,
    /// `settings.updates.ready`
    SettingsUpdatesReady,
    /// `settings.updates.installRestart`
    SettingsUpdatesInstallRestart,
    /// `settings.updates.install`
    SettingsUpdatesInstall,
    /// `settings.updates.confirmInstall`
    SettingsUpdatesConfirmInstall,
    /// `settings.updates.confirmBody`
    SettingsUpdatesConfirmBody,
    /// `settings.updates.later`
    SettingsUpdatesLater,
    /// `settings.updates.retry`
    SettingsUpdatesRetry,
    /// `settings.updates.installFailed`
    SettingsUpdatesInstallFailed,
    /// `settings.updates.managedExternally`
    SettingsUpdatesManagedExternally,
    /// `settings.updates.viewUpdate`
    SettingsUpdatesViewUpdate,
    /// `settings.updates.viewRelease`
    SettingsUpdatesViewRelease,
    /// `update.dismiss`
    UpdateDismiss,
    /// `storage.loadingPath`
    StorageLoadingPath,
    /// `storage.uiOnly`
    StorageUiOnly,
    /// `storage.editingWarning`
    StorageEditingWarning,
    /// `storage.migratedFromJson`
    StorageMigratedFromJson,
    /// `storage.recoveredFromJson`
    StorageRecoveredFromJson,
    /// `storage.defaultsAfterError`
    StorageDefaultsAfterError,
    /// `storage.invalidExternalEdit`
    StorageInvalidExternalEdit,
    /// `settings.keyboard`
    SettingsKeyboard,
    /// `theme.system`
    ThemeSystem,
    /// `theme.light`
    ThemeLight,
    /// `theme.dark`
    ThemeDark,
    /// `material.auto`
    MaterialAuto,
    /// `material.translucent`
    MaterialTranslucent,
    /// `material.opaque`
    MaterialOpaque,
    /// `motion.full`
    MotionFull,
    /// `motion.reduced`
    MotionReduced,
    /// `motion.disabled`
    MotionDisabled,
    /// `accent.system`
    AccentSystem,
    /// `accent.blue`
    AccentBlue,
    /// `accent.green`
    AccentGreen,
    /// `accent.purple`
    AccentPurple,
    /// `accent.orange`
    AccentOrange,
    /// `blacklist.blockedApplication`
    BlacklistBlockedApplication,
    /// `blacklist.empty`
    BlacklistEmpty,
    /// `blacklist.emptyHint`
    BlacklistEmptyHint,
    /// `blacklist.subtitle`
    BlacklistSubtitle,
    /// `blacklist.add`
    BlacklistAdd,
    /// `blacklist.remove`
    BlacklistRemove,
    /// `blacklist.clear`
    BlacklistClear,
    /// `blacklist.applyRecommended`
    BlacklistApplyRecommended,
    /// `blacklist.chooseApps`
    BlacklistChooseApps,
    /// `blacklist.chooseAppsHint`
    BlacklistChooseAppsHint,
    /// `blacklist.refreshAppsHint`
    BlacklistRefreshAppsHint,
    /// `blacklist.doneHint`
    BlacklistDoneHint,
    /// `blacklist.applyRecommendedHint`
    BlacklistApplyRecommendedHint,
    /// `blacklist.restoreDefaultsHint`
    BlacklistRestoreDefaultsHint,
    /// `blacklist.reloadSavedHint`
    BlacklistReloadSavedHint,
    /// `blacklist.clearHint`
    BlacklistClearHint,
    /// `blacklist.addHint`
    BlacklistAddHint,
    /// `blacklist.manualPlaceholder`
    BlacklistManualPlaceholder,
    /// `blacklist.actionsHint`
    BlacklistActionsHint,
    /// `blacklist.appPicker`
    BlacklistAppPicker,
    /// `blacklist.searchApps`
    BlacklistSearchApps,
    /// `blacklist.refresh`
    BlacklistRefresh,
    /// `blacklist.done`
    BlacklistDone,
    /// `blacklist.loadingApps`
    BlacklistLoadingApps,
    /// `blacklist.appsLoadFailed`
    BlacklistAppsLoadFailed,
    /// `blacklist.noRunningApps`
    BlacklistNoRunningApps,
    /// `blacklist.noAppMatches`
    BlacklistNoAppMatches,
    /// `blacklist.selected`
    BlacklistSelected,
    /// `blacklist.blockApp`
    BlacklistBlockApp,
    /// `blacklist.unblockApp`
    BlacklistUnblockApp,
    /// `blacklist.restoreDefaults`
    BlacklistRestoreDefaults,
    /// `blacklist.reloadSaved`
    BlacklistReloadSaved,
    /// `blacklist.unsavedHint`
    BlacklistUnsavedHint,
    /// `hotkeys.subtitle`
    HotkeysSubtitle,
    /// `hotkeys.presets`
    HotkeysPresets,
    /// `hotkeys.presetsHint`
    HotkeysPresetsHint,
    /// `hotkeys.globalShortcutsAria`
    HotkeysGlobalShortcutsAria,
    /// `recorder.record`
    RecorderRecord,
    /// `recorder.clear`
    RecorderClear,
    /// `threshold.heading`
    ThresholdHeading,
    /// `modifier.presetsAria`
    ModifierPresetsAria,
    /// `feedback.beepEnabled`
    FeedbackBeepEnabled,
    /// `feedback.blockedFreq`
    FeedbackBlockedFreq,
    /// `feedback.blockedFreqHelp`
    FeedbackBlockedFreqHelp,
    /// `feedback.blockedDuration`
    FeedbackBlockedDuration,
    /// `feedback.blockedDurationHelp`
    FeedbackBlockedDurationHelp,
    /// `feedback.limitFreq`
    FeedbackLimitFreq,
    /// `feedback.limitFreqHelp`
    FeedbackLimitFreqHelp,
    /// `feedback.limitDuration`
    FeedbackLimitDuration,
    /// `feedback.limitDurationHelp`
    FeedbackLimitDurationHelp,
    /// `threshold.green`
    ThresholdGreen,
    /// `threshold.blue`
    ThresholdBlue,
    /// `threshold.orange`
    ThresholdOrange,
    /// `recorder.cleared`
    RecorderCleared,
    /// `recorder.recorded`
    RecorderRecorded,
    /// `recorder.needModifier`
    RecorderNeedModifier,
    /// `recorder.notSet`
    RecorderNotSet,
    /// `recorder.globalShortcut`
    RecorderGlobalShortcut,
    /// `recorder.pressShortcut`
    RecorderPressShortcut,
    /// `help.title`
    HelpTitle,
    /// `help.shortcuts`
    HelpShortcuts,
    /// `help.close`
    HelpClose,
    /// `hotkey.volumeUp`
    HotkeyVolumeUp,
    /// `hotkey.volumeUpHelp`
    HotkeyVolumeUpHelp,
    /// `hotkey.volumeDown`
    HotkeyVolumeDown,
    /// `hotkey.volumeDownHelp`
    HotkeyVolumeDownHelp,
    /// `hotkey.volumeUpLarge`
    HotkeyVolumeUpLarge,
    /// `hotkey.volumeUpLargeHelp`
    HotkeyVolumeUpLargeHelp,
    /// `hotkey.volumeDownLarge`
    HotkeyVolumeDownLarge,
    /// `hotkey.volumeDownLargeHelp`
    HotkeyVolumeDownLargeHelp,
    /// `hotkey.toggleMute`
    HotkeyToggleMute,
    /// `hotkey.toggleMuteHelp`
    HotkeyToggleMuteHelp,
    /// `hotkey.reset50`
    HotkeyReset50,
    /// `hotkey.reset50Help`
    HotkeyReset50Help,
    /// `hotkey.openMixer`
    HotkeyOpenMixer,
    /// `hotkey.openMixerHelp`
    HotkeyOpenMixerHelp,
    /// `hotkey.openMenu`
    HotkeyOpenMenu,
    /// `hotkey.openMenuHelp`
    HotkeyOpenMenuHelp,
    /// `help.groupVolume`
    HelpGroupVolume,
    /// `help.groupCommands`
    HelpGroupCommands,
    /// `help.groupExtended`
    HelpGroupExtended,
    /// `help.searchPlaceholder`
    HelpSearchPlaceholder,
    /// `help.searchLabel`
    HelpSearchLabel,
    /// `help.editConfig`
    HelpEditConfig,
    /// `help.unavailable`
    HelpUnavailable,
    /// `help.noMatches`
    HelpNoMatches,
    /// `help.capsFallback`
    HelpCapsFallback,
    /// `help.statusInUse`
    HelpStatusInUse,
    /// `help.statusActive`
    HelpStatusActive,
    /// `help.calloutTitle`
    HelpCalloutTitle,
    /// `help.calloutBody`
    HelpCalloutBody,
    /// `help.calloutAction`
    HelpCalloutAction,
    /// `help.calloutTailOne`
    HelpCalloutTailOne,
    /// `help.calloutTailMany`
    HelpCalloutTailMany,
    /// `help.statusReady`
    HelpStatusReady,
    /// `help.statusFallback`
    HelpStatusFallback,
    /// `help.statusDisabled`
    HelpStatusDisabled,
}

impl Key {
    /// Every key, in table order.
    pub const ALL: [Key; 215] = [
        Key::AppLanguage,
        Key::AppLanguageEn,
        Key::AppLanguageVi,
        Key::AppName,
        Key::AppWindowSettings,
        Key::AppWindowHelp,
        Key::StatusSaved,
        Key::StatusSaving,
        Key::CommonClose,
        Key::CommonCancel,
        Key::CommonSave,
        Key::CommonSaveChanges,
        Key::CommonMute,
        Key::CommonUnmute,
        Key::CommonReset,
        Key::CommonVolume,
        Key::CommonMuted,
        Key::CommonSystemOutput,
        Key::OverlayVolume,
        Key::OverlaySystemOutput,
        Key::MixerTitle,
        Key::MixerSystemOutput,
        Key::MixerMute,
        Key::MixerUnmute,
        Key::MixerMuteSystemOutput,
        Key::MixerUnmuteSystemOutput,
        Key::MixerResetVolume,
        Key::MixerReset,
        Key::MixerClose,
        Key::MixerSessions,
        Key::MixerNoSessions,
        Key::MixerNoMatches,
        Key::MixerAppSessions,
        Key::MixerSearchPlaceholder,
        Key::MixerClearSearch,
        Key::MixerUnsupported,
        Key::MixerConnectionAttention,
        Key::MixerRetry,
        Key::MixerRetryConnection,
        Key::MixerEscToClose,
        Key::MixerBackendUnavailable,
        Key::MixerBackendTemporarilyUnavailable,
        Key::MixerSystemSounds,
        Key::MixerSystemSoundsHint,
        Key::MixerSystemSoundsDescription,
        Key::MixerMuteFailed,
        Key::SettingsTitle,
        Key::SettingsSubtitle,
        Key::SettingsGeneral,
        Key::SettingsHotkeys,
        Key::SettingsAppearance,
        Key::SettingsBlacklist,
        Key::SettingsFeedback,
        Key::SettingsStorage,
        Key::SettingsAppLabelStyle,
        Key::SettingsAppLabelStyleHelp,
        Key::SettingsAppLabelFriendly,
        Key::SettingsAppLabelFriendlyAndProcess,
        Key::SettingsAppLabelProcess,
        Key::SettingsVolumeStep,
        Key::SettingsVolumeStepHelp,
        Key::SettingsVolumeStepLarge,
        Key::SettingsVolumeStepLargeHelp,
        Key::SettingsOverlayDuration,
        Key::SettingsOverlayDurationHelp,
        Key::SettingsAutostart,
        Key::SettingsAutostartHelp,
        Key::SettingsAutostartUnavailable,
        Key::SettingsTheme,
        Key::SettingsMaterial,
        Key::SettingsMotion,
        Key::SettingsAccent,
        Key::SettingsModifier,
        Key::SettingsOpenConfig,
        Key::SettingsSaveFailed,
        Key::SettingsLoadFailed,
        Key::SettingsUpdatesTitle,
        Key::SettingsUpdatesSubtitle,
        Key::SettingsUpdatesCurrent,
        Key::SettingsUpdatesCheckBtn,
        Key::SettingsUpdatesChecking,
        Key::SettingsUpdatesUpToDate,
        Key::SettingsUpdatesAvailable,
        Key::SettingsUpdatesDownloading,
        Key::SettingsUpdatesReady,
        Key::SettingsUpdatesInstallRestart,
        Key::SettingsUpdatesInstall,
        Key::SettingsUpdatesConfirmInstall,
        Key::SettingsUpdatesConfirmBody,
        Key::SettingsUpdatesLater,
        Key::SettingsUpdatesRetry,
        Key::SettingsUpdatesInstallFailed,
        Key::SettingsUpdatesManagedExternally,
        Key::SettingsUpdatesViewUpdate,
        Key::SettingsUpdatesViewRelease,
        Key::UpdateDismiss,
        Key::StorageLoadingPath,
        Key::StorageUiOnly,
        Key::StorageEditingWarning,
        Key::StorageMigratedFromJson,
        Key::StorageRecoveredFromJson,
        Key::StorageDefaultsAfterError,
        Key::StorageInvalidExternalEdit,
        Key::SettingsKeyboard,
        Key::ThemeSystem,
        Key::ThemeLight,
        Key::ThemeDark,
        Key::MaterialAuto,
        Key::MaterialTranslucent,
        Key::MaterialOpaque,
        Key::MotionFull,
        Key::MotionReduced,
        Key::MotionDisabled,
        Key::AccentSystem,
        Key::AccentBlue,
        Key::AccentGreen,
        Key::AccentPurple,
        Key::AccentOrange,
        Key::BlacklistBlockedApplication,
        Key::BlacklistEmpty,
        Key::BlacklistEmptyHint,
        Key::BlacklistSubtitle,
        Key::BlacklistAdd,
        Key::BlacklistRemove,
        Key::BlacklistClear,
        Key::BlacklistApplyRecommended,
        Key::BlacklistChooseApps,
        Key::BlacklistChooseAppsHint,
        Key::BlacklistRefreshAppsHint,
        Key::BlacklistDoneHint,
        Key::BlacklistApplyRecommendedHint,
        Key::BlacklistRestoreDefaultsHint,
        Key::BlacklistReloadSavedHint,
        Key::BlacklistClearHint,
        Key::BlacklistAddHint,
        Key::BlacklistManualPlaceholder,
        Key::BlacklistActionsHint,
        Key::BlacklistAppPicker,
        Key::BlacklistSearchApps,
        Key::BlacklistRefresh,
        Key::BlacklistDone,
        Key::BlacklistLoadingApps,
        Key::BlacklistAppsLoadFailed,
        Key::BlacklistNoRunningApps,
        Key::BlacklistNoAppMatches,
        Key::BlacklistSelected,
        Key::BlacklistBlockApp,
        Key::BlacklistUnblockApp,
        Key::BlacklistRestoreDefaults,
        Key::BlacklistReloadSaved,
        Key::BlacklistUnsavedHint,
        Key::HotkeysSubtitle,
        Key::HotkeysPresets,
        Key::HotkeysPresetsHint,
        Key::HotkeysGlobalShortcutsAria,
        Key::RecorderRecord,
        Key::RecorderClear,
        Key::ThresholdHeading,
        Key::ModifierPresetsAria,
        Key::FeedbackBeepEnabled,
        Key::FeedbackBlockedFreq,
        Key::FeedbackBlockedFreqHelp,
        Key::FeedbackBlockedDuration,
        Key::FeedbackBlockedDurationHelp,
        Key::FeedbackLimitFreq,
        Key::FeedbackLimitFreqHelp,
        Key::FeedbackLimitDuration,
        Key::FeedbackLimitDurationHelp,
        Key::ThresholdGreen,
        Key::ThresholdBlue,
        Key::ThresholdOrange,
        Key::RecorderCleared,
        Key::RecorderRecorded,
        Key::RecorderNeedModifier,
        Key::RecorderNotSet,
        Key::RecorderGlobalShortcut,
        Key::RecorderPressShortcut,
        Key::HelpTitle,
        Key::HelpShortcuts,
        Key::HelpClose,
        Key::HotkeyVolumeUp,
        Key::HotkeyVolumeUpHelp,
        Key::HotkeyVolumeDown,
        Key::HotkeyVolumeDownHelp,
        Key::HotkeyVolumeUpLarge,
        Key::HotkeyVolumeUpLargeHelp,
        Key::HotkeyVolumeDownLarge,
        Key::HotkeyVolumeDownLargeHelp,
        Key::HotkeyToggleMute,
        Key::HotkeyToggleMuteHelp,
        Key::HotkeyReset50,
        Key::HotkeyReset50Help,
        Key::HotkeyOpenMixer,
        Key::HotkeyOpenMixerHelp,
        Key::HotkeyOpenMenu,
        Key::HotkeyOpenMenuHelp,
        Key::HelpGroupVolume,
        Key::HelpGroupCommands,
        Key::HelpGroupExtended,
        Key::HelpSearchPlaceholder,
        Key::HelpSearchLabel,
        Key::HelpEditConfig,
        Key::HelpUnavailable,
        Key::HelpNoMatches,
        Key::HelpCapsFallback,
        Key::HelpStatusInUse,
        Key::HelpStatusActive,
        Key::HelpCalloutTitle,
        Key::HelpCalloutBody,
        Key::HelpCalloutAction,
        Key::HelpCalloutTailOne,
        Key::HelpCalloutTailMany,
        Key::HelpStatusReady,
        Key::HelpStatusFallback,
        Key::HelpStatusDisabled,
    ];
}

const EN: [&str; 215] = [
    "Language", "English", "Tiếng Việt",
    "Volume Control", "Volume Control Settings", "Volume Control Help",
    "Saved", "Saving…", "Close",
    "Cancel", "Save", "Save changes",
    "Mute", "Unmute", "Reset",
    "Volume", "Muted", "System output",
    "Volume", "System output", "Volume Mixer",
    "System output", "Mute", "Unmute",
    "Mute system output", "Unmute system output", "Reset volume to 50%",
    "Reset volume to 50 percent", "Close mixer", "Applications",
    "No audio sessions", "No matching apps", "app sessions",
    "Search apps…", "Clear search", "Per-app audio isn't available on this platform.",
    "Mixer connection needs attention", "Retry", "Retry connection",
    "Esc to close", "Audio backend unavailable", "Audio backend temporarily unavailable",
    "System sounds", "Notifications & alerts", "Windows notification and alert sounds.",
    "Could not change mute", "Settings", "Configure volume, hotkeys, appearance, and feedback",
    "General", "Hotkeys", "Appearance",
    "Blacklist", "Feedback", "Storage",
    "App name display", "Choose how applications are named in the mixer and blocked-app list.", "Friendly name",
    "Friendly name + process", "Process name only", "Volume step",
    "Small change applied by ↑ / ↓.", "Large volume step", "Shift + ↑ / ↓ applies the large change.",
    "Overlay duration", "How long the volume overlay stays visible.", "Start with Windows",
    "Launch Volume Control when you sign in.", "Auto-start is unavailable on this platform.", "Theme",
    "Material", "Motion", "Accent",
    "Modifier", "Open config.ini (advanced)", "Save failed",
    "Could not load settings", "Updates", "Check for new Volume Control releases",
    "Current version", "Check for updates", "Checking for updates…",
    "Volume Control is up to date", "New version available: {0}", "Downloading update in background…",
    "Version {0} is ready to install", "Install and restart", "Install",
    "Confirm installation", "VolumeControl will restart to apply version {0}. Unsaved session changes will be preserved.", "Later",
    "Retry check", "Update failed: {0}", "This installation is managed externally. Please update via your package manager.",
    "View update", "View release notes", "Dismiss update notification",
    "Loading config path…", "Every setting is available in this window. The config file is only for backup or advanced recovery.", "Editing config.ini directly is an advanced option; changes reload automatically. Legacy config.json is kept as a migration backup.",
    "Configuration migrated from legacy JSON to config.ini; the JSON backup was kept.", "Configuration recovered from the legacy JSON backup after an invalid INI edit.", "Configuration files could not be loaded, so validated defaults are active. Your backup was not deleted.",
    "An external INI edit was invalid; the last valid configuration remains active. Correct the file and retry.", "Keyboard", "System",
    "Light", "Dark", "Auto",
    "Translucent", "Opaque", "Full",
    "Reduced", "Disabled", "System",
    "Blue", "Green", "Purple",
    "Orange", "Blocked application", "No blocked applications",
    "Volume Control will respond to shortcuts everywhere.", "Block shortcuts while these apps have focus.", "Add",
    "Remove", "Clear", "Apply Recommended",
    "Choose running apps", "Pick from apps that currently have a visible window.", "Reload the list of running apps.",
    "Close the app picker; changes stay in the draft.", "Add the recommended apps for the current modifier without removing your own entries.", "Replace this draft list with the default recommendations for the current modifier.",
    "Discard draft edits and restore the list from the last save.", "Remove every entry from this draft list. Save commits the change.", "Add a process name such as chrome.exe.",
    "e.g. chrome.exe", "Apply recommended adds suggestions; Restore defaults replaces the list; Reload saved discards draft edits.", "Running applications",
    "Search running apps", "Refresh", "Done",
    "Loading running apps…", "Could not list running apps.", "No running apps with a visible window.",
    "No apps match your search.", "selected", "Block",
    "Unblock", "Restore defaults", "Reload saved",
    "Changes stay in this draft until Save.", "Press Record, then press a modifier and key. Changes stay in the draft until you save.", "Presets",
    "Restore a familiar shortcut layout.", "Global shortcuts", "Record",
    "Clear", "Volume thresholds", "Shortcut presets",
    "Enable beep feedback", "Blocked beep frequency", "Beep when a shortcut is blocked.",
    "Blocked beep duration", "How long the blocked beep sounds.", "Limit beep frequency",
    "Beep when volume is already at the limit.", "Limit beep duration", "How long the limit beep sounds.",
    "Green up to", "Blue up to", "Orange up to",
    "Shortcut cleared", "Shortcut recorded", "Press a modifier such as Ctrl, Alt, Shift, or Cmd with a key",
    "Not set", "Global shortcut", "Press shortcut…",
    "Help", "Keyboard shortcuts", "Close",
    "Volume Up", "Increase volume by the small step.", "Volume Down",
    "Decrease volume by the small step.", "Volume Up (large)", "Increase volume by the large step.",
    "Volume Down (large)", "Decrease volume by the large step.", "Toggle Mute",
    "Mute or unmute the current output.", "Reset to 50%", "Set the current output to 50%.",
    "Open Mixer", "Show or hide the mixer panel.", "Open Menu",
    "Open the Volume Control tray menu.", "Volume", "Commands",
    "Extended", "Search shortcuts…", "Search shortcuts",
    "Edit config", "Help unavailable", "No shortcuts match",
    "Caps Lock falls back to Ctrl + Alt", "In use", "Active",
    "Shortcut conflict", "Another application already owns one of these combinations.", "Change the modifier in Settings.",
    "is used by another app.", "are used by another app.", "Ready",
    "Fallback", "Disabled",
];

const VI: [&str; 215] = [
    "Ngôn ngữ", "English", "Tiếng Việt",
    "Điều khiển âm lượng", "Cài đặt Điều khiển âm lượng", "Trợ giúp Điều khiển âm lượng",
    "Đã lưu", "Đang lưu…", "Đóng",
    "Huỷ", "Lưu", "Lưu thay đổi",
    "Tắt tiếng", "Bật tiếng", "Đặt lại",
    "Âm lượng", "Đã tắt tiếng", "Đầu ra hệ thống",
    "Âm lượng", "Đầu ra hệ thống", "Bộ trộn âm lượng",
    "Đầu ra hệ thống", "Tắt tiếng", "Bật tiếng",
    "Tắt tiếng đầu ra hệ thống", "Bật tiếng đầu ra hệ thống", "Đặt âm lượng về 50%",
    "Đặt âm lượng về 50 phần trăm", "Đóng bộ trộn", "Ứng dụng",
    "Không có phiên âm thanh", "Không có ứng dụng khớp", "phiên ứng dụng",
    "Tìm ứng dụng…", "Xoá tìm kiếm", "Âm thanh theo ứng dụng không khả dụng trên nền tảng này.",
    "Kết nối bộ trộn cần được chú ý", "Thử lại", "Thử lại kết nối",
    "Esc để đóng", "Âm thanh không khả dụng", "Âm thanh tạm thời không khả dụng",
    "Âm thanh hệ thống", "Thông báo và cảnh báo", "Âm thanh thông báo và cảnh báo của Windows.",
    "Không thể đổi trạng thái tắt tiếng", "Cài đặt", "Cấu hình âm lượng, phím tắt, giao diện và phản hồi",
    "Chung", "Phím tắt", "Giao diện",
    "Danh sách chặn", "Phản hồi", "Lưu trữ",
    "Tên ứng dụng hiển thị", "Chọn cách hiển thị tên ứng dụng trong mixer và danh sách chặn.", "Tên thân thiện",
    "Tên thân thiện + tên gốc", "Chỉ tên gốc", "Bước âm lượng",
    "Mức thay đổi nhỏ khi nhấn ↑ / ↓.", "Bước âm lượng lớn", "Shift + ↑ / ↓ áp dụng mức thay đổi lớn.",
    "Thời gian hiện overlay", "Overlay âm lượng hiển thị trong bao lâu.", "Khởi động cùng Windows",
    "Tự chạy Điều khiển âm lượng khi bạn đăng nhập.", "Tự khởi động không khả dụng trên nền tảng này.", "Chủ đề",
    "Chất liệu", "Chuyển động", "Màu nhấn",
    "Phím bổ trợ", "Mở config.ini (nâng cao)", "Lưu thất bại",
    "Không tải được cài đặt", "Cập nhật", "Kiểm tra các bản phát hành Điều khiển âm lượng mới",
    "Phiên bản hiện tại", "Kiểm tra cập nhật", "Đang kiểm tra cập nhật…",
    "Điều khiển âm lượng đã là bản mới nhất", "Có phiên bản mới: {0}", "Đang tải bản cập nhật ngầm…",
    "Phiên bản {0} đã sẵn sàng cài đặt", "Cài đặt và khởi động lại", "Cài đặt",
    "Xác nhận cài đặt", "VolumeControl sẽ khởi động lại để áp dụng phiên bản {0}. Cấu hình sẽ được giữ nguyên.", "Để sau",
    "Thử lại", "Cập nhật thất bại: {0}", "Bản cài đặt này được quản lý bên ngoài. Vui lòng cập nhật qua trình quản lý gói.",
    "Xem cập nhật", "Xem ghi chú phát hành", "Đóng thông báo cập nhật",
    "Đang tải đường dẫn cấu hình…", "Mọi cài đặt đều có trong cửa sổ này. Tệp cấu hình chỉ dùng để sao lưu hoặc khôi phục nâng cao.", "Sửa trực tiếp config.ini là tuỳ chọn nâng cao; thay đổi sẽ tự tải lại. config.json cũ được giữ làm bản sao dự phòng.",
    "Cấu hình đã được chuyển từ JSON cũ sang config.ini; bản JSON dự phòng vẫn được giữ.", "Cấu hình đã được phục hồi từ bản JSON dự phòng sau khi INI bị sửa sai.", "Không đọc được file cấu hình nên đang dùng giá trị mặc định hợp lệ. Bản dự phòng của bạn chưa bị xoá.",
    "File INI bị sửa sai từ bên ngoài; cấu hình hợp lệ gần nhất vẫn đang hoạt động. Sửa lại file rồi thử lại.", "Bàn phím", "Hệ thống",
    "Sáng", "Tối", "Tự động",
    "Trong mờ", "Đục", "Đầy đủ",
    "Giảm", "Tắt", "Hệ thống",
    "Xanh dương", "Xanh lá", "Tím",
    "Cam", "Ứng dụng bị chặn", "Chưa chặn ứng dụng nào",
    "Điều khiển âm lượng sẽ phản hồi phím tắt ở mọi nơi.", "Chặn phím tắt khi các ứng dụng này đang focus.", "Thêm",
    "Xoá", "Xoá hết", "Áp dụng gợi ý",
    "Chọn ứng dụng đang chạy", "Chọn từ các ứng dụng đang có cửa sổ hiển thị.", "Tải lại danh sách ứng dụng đang chạy.",
    "Đóng bảng chọn ứng dụng; thay đổi vẫn chỉ nằm trong bản nháp.", "Thêm các ứng dụng gợi ý cho phím bổ trợ hiện tại mà không xoá mục của bạn.", "Thay danh sách nháp bằng gợi ý mặc định cho phím bổ trợ hiện tại.",
    "Bỏ thay đổi nháp và tải lại danh sách đã lưu.", "Xoá toàn bộ mục khỏi bản nháp. Chỉ khi lưu mới áp dụng.", "Thêm tên tiến trình, ví dụ chrome.exe.",
    "ví dụ chrome.exe", "Áp dụng gợi ý sẽ thêm đề xuất; Khôi phục mặc định thay cả danh sách; Tải lại đã lưu bỏ thay đổi nháp.", "Ứng dụng đang chạy",
    "Tìm ứng dụng đang chạy", "Làm mới", "Xong",
    "Đang tải ứng dụng đang chạy…", "Không thể liệt kê ứng dụng đang chạy.", "Không có ứng dụng nào có cửa sổ hiển thị.",
    "Không có ứng dụng khớp tìm kiếm.", "đã chọn", "Chặn",
    "Bỏ chặn", "Khôi phục mặc định", "Tải lại đã lưu",
    "Thay đổi chỉ áp dụng khi bạn lưu.", "Nhấn Record rồi nhấn phím bổ trợ cùng một phím. Thay đổi chỉ áp dụng khi bạn lưu.", "Bộ đặt sẵn",
    "Khôi phục bố cục phím tắt quen thuộc.", "Phím tắt toàn cục", "Ghi",
    "Xoá", "Ngưỡng âm lượng", "Bộ phím bổ trợ đặt sẵn",
    "Bật phản hồi âm bíp", "Tần số bíp khi bị chặn", "Phát bíp khi phím tắt bị chặn.",
    "Thời lượng bíp khi bị chặn", "Bíp bị chặn kéo dài bao lâu.", "Tần số bíp giới hạn",
    "Phát bíp khi âm lượng đã chạm giới hạn.", "Thời lượng bíp giới hạn", "Bíp giới hạn kéo dài bao lâu.",
    "Xanh lá tới", "Xanh dương tới", "Cam tới",
    "Đã xoá phím tắt", "Đã ghi phím tắt", "Nhấn phím bổ trợ như Ctrl, Alt, Shift hoặc Cmd cùng một phím",
    "Chưa đặt", "Phím tắt toàn cục", "Nhấn phím tắt…",
    "Trợ giúp", "Phím tắt bàn phím", "Đóng",
    "Tăng âm lượng", "Tăng âm lượng theo bước nhỏ.", "Giảm âm lượng",
    "Giảm âm lượng theo bước nhỏ.", "Tăng âm lượng (lớn)", "Tăng âm lượng theo bước lớn.",
    "Giảm âm lượng (lớn)", "Giảm âm lượng theo bước lớn.", "Bật/tắt tiếng",
    "Tắt hoặc bật tiếng đầu ra hiện tại.", "Đặt lại 50%", "Đặt đầu ra hiện tại về 50%.",
    "Mở bộ trộn", "Hiện hoặc ẩn bảng bộ trộn.", "Mở menu",
    "Mở menu khay của Điều khiển âm lượng.", "Âm lượng", "Lệnh",
    "Mở rộng", "Tìm phím tắt…", "Tìm phím tắt",
    "Sửa cấu hình", "Trợ giúp không khả dụng", "Không có phím tắt khớp",
    "Caps Lock dùng dự phòng Ctrl + Alt", "Đang bị dùng", "Hoạt động",
    "Xung đột phím tắt", "Một ứng dụng khác đã chiếm một trong các tổ hợp này.", "Đổi phím bổ trợ trong Cài đặt.",
    "đang bị ứng dụng khác dùng.", "đang bị ứng dụng khác dùng.", "Sẵn sàng",
    "Dự phòng", "Đã tắt",
];

/// Copy table for `lang`.
#[must_use]
pub fn table(lang: Language) -> &'static [&'static str] {
    match lang {
        Language::En => &EN,
        Language::Vi => &VI,
    }
}

/// Resolve a key for `lang`, substituting positional `{0}`, `{1}` arguments.
#[must_use]
pub fn tr(key: Key, lang: Language, args: &[&str]) -> Cow<'static, str> {
    let text = table(lang)[key as usize];
    if args.is_empty() {
        return Cow::Borrowed(text);
    }
    let mut rendered = text.to_string();
    for (index, value) in args.iter().enumerate() {
        rendered = rendered.replace(&format!("{{{index}}}"), value);
    }
    Cow::Owned(rendered)
}
