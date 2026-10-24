//! Settings draft: pure model behind the six-tab Settings surface.
//!
//! The draft owns a full copy of the editable configuration. Saving maps it to
//! the same [`SettingsPatch`] the legacy webview sent, so `AppCore` stays the
//! only writer of `config.ini`.

use volumectl::config::{
    BeepConfig, ColorThresholds, Config, HotkeyBindings, HotkeyModifier, Language,
};
use volumectl::host_core::{BeepPatch, ColorThresholdsPatch, SettingsPatch};
use volumectl::ui::{AccentMode, MaterialMode, MotionMode, ThemeMode};

/// The six Settings tabs, in legacy order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Tab {
    #[default]
    General,
    Hotkeys,
    Appearance,
    Blacklist,
    Feedback,
    Storage,
}

impl Tab {
    pub const ALL: [Tab; 6] = [
        Tab::General,
        Tab::Hotkeys,
        Tab::Appearance,
        Tab::Blacklist,
        Tab::Feedback,
        Tab::Storage,
    ];
}

/// A validation failure routed to its owning tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldError {
    pub tab: Tab,
    pub field: String,
    pub message: String,
}

/// Editable snapshot of [`Config`] plus the baseline it was opened from.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsDraft {
    pub volume_step: u32,
    pub volume_step_large: u32,
    pub overlay_duration_ms: u64,
    pub ui_font_size: u8,
    pub language: Language,
    pub app_label_style: volumectl::config::AppLabelStyle,
    pub autostart: bool,
    pub theme: ThemeMode,
    pub material: MaterialMode,
    pub motion: MotionMode,
    pub accent: AccentMode,
    pub hotkeys: HotkeyBindings,
    pub blacklist: Vec<String>,
    pub color_thresholds: ColorThresholds,
    pub beep: BeepConfig,
    baseline: Config,
}

impl SettingsDraft {
    /// Open a draft from the running config.
    #[must_use]
    pub fn from_config(config: &Config) -> Self {
        Self {
            volume_step: config.volume_step,
            volume_step_large: config.volume_step_large,
            overlay_duration_ms: config.overlay_duration_ms,
            ui_font_size: config.ui_font_size,
            language: config.language,
            app_label_style: config.app_label_style,
            autostart: config.autostart,
            theme: config.appearance.theme,
            material: config.appearance.material,
            motion: config.appearance.motion,
            accent: config.appearance.accent,
            hotkeys: config.hotkeys.clone().unwrap_or_default(),
            blacklist: config.blacklist.clone(),
            color_thresholds: config.color_thresholds.clone(),
            beep: config.beep.clone(),
            baseline: config.clone(),
        }
    }

    /// Build a [`Config`] from the draft (used for validation).
    #[must_use]
    pub fn to_config(&self) -> Config {
        let mut config = self.baseline.clone();
        config.volume_step = self.volume_step;
        config.volume_step_large = self.volume_step_large;
        config.overlay_duration_ms = self.overlay_duration_ms;
        config.ui_font_size = self.ui_font_size;
        config.language = self.language;
        config.app_label_style = self.app_label_style;
        config.autostart = self.autostart;
        config.appearance.theme = self.theme;
        config.appearance.material = self.material;
        config.appearance.motion = self.motion;
        config.appearance.accent = self.accent;
        config.hotkeys = Some(self.hotkeys.clone());
        config.blacklist = self.blacklist.clone();
        config.color_thresholds = self.color_thresholds.clone();
        config.beep = self.beep.clone();
        config
    }

    /// Map the draft to the legacy settings patch (every field explicit).
    #[must_use]
    pub fn patch(&self) -> SettingsPatch {
        SettingsPatch {
            volume_step: Some(self.volume_step),
            volume_step_large: Some(self.volume_step_large),
            theme: Some(theme_str(self.theme).to_string()),
            material: Some(material_str(self.material).to_string()),
            motion: Some(motion_str(self.motion).to_string()),
            accent: Some(accent_str(self.accent).to_string()),
            overlay_duration_ms: Some(self.overlay_duration_ms),
            ui_font_size: Some(self.ui_font_size),
            hotkeys: Some(self.hotkeys.clone()),
            beep: Some(BeepPatch {
                enabled: Some(self.beep.enabled),
                blocked_freq: Some(self.beep.blocked_freq),
                blocked_duration_ms: Some(self.beep.blocked_duration_ms),
                limit_freq: Some(self.beep.limit_freq),
                limit_duration_ms: Some(self.beep.limit_duration_ms),
            }),
            color_thresholds: Some(ColorThresholdsPatch {
                green_up_to: Some(self.color_thresholds.green_up_to),
                blue_up_to: Some(self.color_thresholds.blue_up_to),
                orange_up_to: Some(self.color_thresholds.orange_up_to),
            }),
            language: Some(self.language.as_str().to_string()),
            app_label_style: Some(self.app_label_style.as_str().to_string()),
            blacklist: Some(self.blacklist.clone()),
            autostart: Some(self.autostart),
        }
    }

    /// True when nothing changed since the draft was opened.
    #[must_use]
    pub fn is_pristine(&self) -> bool {
        self.to_config() == self.baseline
    }

    /// Discard edits.
    pub fn reset(&mut self) {
        *self = Self::from_config(&self.baseline);
    }

    /// Rebase after a successful save.
    pub fn rebase(&mut self, config: &Config) {
        *self = Self::from_config(config);
    }

    /// Validate the draft and route any failure to its tab.
    pub fn validate(&self) -> Result<(), FieldError> {
        if self.overlay_duration_ms < 500 {
            return Err(FieldError {
                tab: Tab::General,
                field: "overlay_duration_ms".into(),
                message: "overlay_duration_ms: must be at least 500 ms".into(),
            });
        }
        let config = self.to_config();
        volumectl::config::validate(&config).map_err(field_error)?;
        volumectl::config::validate_thresholds(
            self.color_thresholds.green_up_to,
            self.color_thresholds.blue_up_to,
            self.color_thresholds.orange_up_to,
        )
        .map_err(field_error)
    }
}

fn field_error(error: volumectl::config::ConfigValidationError) -> FieldError {
    FieldError {
        tab: tab_for_field(error.field),
        field: error.field.to_string(),
        message: format!("{}: {}", error.field, error.message),
    }
}

fn tab_for_field(field: &str) -> Tab {
    if field.starts_with("hotkeys.") {
        Tab::Hotkeys
    } else if field.starts_with("color_thresholds.") {
        Tab::Appearance
    } else if field.starts_with("beep.") {
        Tab::Feedback
    } else {
        Tab::General
    }
}

pub fn theme_str(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::System => "System",
        ThemeMode::Light => "Light",
        ThemeMode::Dark => "Dark",
    }
}

pub fn material_str(mode: MaterialMode) -> &'static str {
    match mode {
        MaterialMode::Auto => "Auto",
        MaterialMode::Translucent => "Translucent",
        MaterialMode::Opaque => "Opaque",
    }
}

pub fn motion_str(mode: MotionMode) -> &'static str {
    match mode {
        MotionMode::Full => "Full",
        MotionMode::Reduced => "Reduced",
        MotionMode::Disabled => "Disabled",
    }
}

pub fn accent_str(mode: AccentMode) -> &'static str {
    match mode {
        AccentMode::System => "System",
        AccentMode::Blue => "Blue",
        AccentMode::Green => "Green",
        AccentMode::Purple => "Purple",
        AccentMode::Orange => "Orange",
    }
}

/// Modifier presets available in the Hotkeys tab. Caps Lock is a recorded
/// degradation: legacy disables it and falls back to Ctrl + Alt.
pub const MODIFIER_PRESETS: [HotkeyModifier; 4] = [
    HotkeyModifier::CtrlAlt,
    HotkeyModifier::Alt,
    HotkeyModifier::Ctrl,
    HotkeyModifier::CapsLock,
];
