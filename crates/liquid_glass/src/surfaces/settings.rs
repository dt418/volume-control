//! Native Settings window surface (760x620 logical px with 620x520 minimum bounds).

use crate::material::{Appearance, ColorScheme, MaterialRole, MaterialTokens};
use crate::scene::geometry::Rect;
use crate::scene::graph::Scene;
use crate::scene::node::{SceneNode, SemanticId};
use volumectl_lib::config::Language;
use volumectl_lib::ui::{AccentMode, AppAction, AppState, MaterialMode, MotionMode, ThemeMode};

pub const SETTINGS_DEFAULT_WIDTH: f32 = 760.0;
pub const SETTINGS_DEFAULT_HEIGHT: f32 = 620.0;
pub const SETTINGS_MIN_WIDTH: f32 = 620.0;
pub const SETTINGS_MIN_HEIGHT: f32 = 520.0;
pub const SIDEBAR_WIDTH: f32 = 180.0;

// Root container layout IDs
pub const SETTINGS_ROOT_ID: SemanticId = SemanticId(300);
pub const SETTINGS_SIDEBAR_ID: SemanticId = SemanticId(301);
pub const SETTINGS_CONTENT_ID: SemanticId = SemanticId(302);
pub const SETTINGS_ACTION_BAR_ID: SemanticId = SemanticId(303);

// Action buttons
pub const BTN_SAVE_ID: SemanticId = SemanticId(310);
pub const BTN_CANCEL_ID: SemanticId = SemanticId(311);
pub const BTN_RESET_ID: SemanticId = SemanticId(312);
pub const BTN_CHECK_UPDATES_ID: SemanticId = SemanticId(330);
pub const BTN_APPLY_REC_BLACKLIST_ID: SemanticId = SemanticId(331);
pub const BTN_CLEAR_BLACKLIST_ID: SemanticId = SemanticId(332);

// Sidebar category IDs
pub const TAB_GENERAL_ID: SemanticId = SemanticId(320);
pub const TAB_APPEARANCE_ID: SemanticId = SemanticId(321);
pub const TAB_HOTKEYS_ID: SemanticId = SemanticId(322);
pub const TAB_BLACKLIST_ID: SemanticId = SemanticId(323);
pub const TAB_UPDATES_ID: SemanticId = SemanticId(324);

// Appearance control option IDs
pub const OPT_THEME_SYSTEM: SemanticId = SemanticId(501);
pub const OPT_THEME_LIGHT: SemanticId = SemanticId(502);
pub const OPT_THEME_DARK: SemanticId = SemanticId(503);

pub const OPT_MAT_AUTO: SemanticId = SemanticId(511);
pub const OPT_MAT_TRANSLUCENT: SemanticId = SemanticId(512);
pub const OPT_MAT_OPAQUE: SemanticId = SemanticId(513);

pub const OPT_MOT_FULL: SemanticId = SemanticId(521);
pub const OPT_MOT_REDUCED: SemanticId = SemanticId(522);
pub const OPT_MOT_DISABLED: SemanticId = SemanticId(523);

pub const OPT_ACC_SYSTEM: SemanticId = SemanticId(531);
pub const OPT_ACC_BLUE: SemanticId = SemanticId(532);
pub const OPT_ACC_GREEN: SemanticId = SemanticId(533);
pub const OPT_ACC_PURPLE: SemanticId = SemanticId(534);
pub const OPT_ACC_ORANGE: SemanticId = SemanticId(535);

pub const OPT_LANG_EN: SemanticId = SemanticId(541);
pub const OPT_LANG_VI: SemanticId = SemanticId(542);

pub const PREVIEW_CARD_ID: SemanticId = SemanticId(550);

// General control option IDs
pub const OPT_GEN_AUTOSTART: SemanticId = SemanticId(601);
pub const OPT_GEN_BEEP_LIMIT: SemanticId = SemanticId(602);
pub const OPT_GEN_STEP_1: SemanticId = SemanticId(611);
pub const OPT_GEN_STEP_2: SemanticId = SemanticId(612);
pub const OPT_GEN_STEP_5: SemanticId = SemanticId(613);

/// Navigation category sections in the settings sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    General,
    Appearance,
    Hotkeys,
    Blacklist,
    Updates,
}

/// Uncommitted appearance and configuration draft state modified by user interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsDraft {
    pub theme: ThemeMode,
    pub material: MaterialMode,
    pub motion: MotionMode,
    pub accent: AccentMode,
    pub language: Language,
    pub volume_step: u8,
    pub autostart: bool,
    pub sound_feedback: bool,
}

impl SettingsDraft {
    pub fn from_state(state: &AppState) -> Self {
        Self {
            theme: state.theme,
            material: state.material,
            motion: state.motion,
            accent: state.accent,
            language: state.language,
            volume_step: 2,
            autostart: true,
            sound_feedback: true,
        }
    }
}

/// Manages layout, sidebar navigation, form controls, and draft/commit state for Settings.
pub struct SettingsSurface {
    current_tab: SettingsTab,
    confirmed_state: AppState,
    draft: SettingsDraft,
    has_unsaved_changes: bool,
}

impl SettingsSurface {
    /// Construct a new Settings surface bound to the confirmed host application state.
    pub fn new(confirmed: &AppState) -> Self {
        Self {
            current_tab: SettingsTab::General,
            confirmed_state: confirmed.clone(),
            draft: SettingsDraft::from_state(confirmed),
            has_unsaved_changes: false,
        }
    }

    /// Synchronize host-confirmed state without discarding pending draft edits.
    pub fn sync_confirmed(&mut self, confirmed: &AppState) {
        self.confirmed_state = confirmed.clone();
        if !self.has_unsaved_changes {
            self.draft = SettingsDraft::from_state(confirmed);
        }
    }

    /// Select active navigation section.
    pub fn select_tab(&mut self, tab: SettingsTab) {
        self.current_tab = tab;
    }

    pub fn current_tab(&self) -> SettingsTab {
        self.current_tab
    }

    pub fn draft(&self) -> &SettingsDraft {
        &self.draft
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.has_unsaved_changes
    }

    pub fn set_theme(&mut self, theme: ThemeMode) {
        self.draft.theme = theme;
        self.has_unsaved_changes = true;
    }

    pub fn set_material(&mut self, material: MaterialMode) {
        self.draft.material = material;
        self.has_unsaved_changes = true;
    }

    pub fn set_motion(&mut self, motion: MotionMode) {
        self.draft.motion = motion;
        self.has_unsaved_changes = true;
    }

    pub fn set_accent(&mut self, accent: AccentMode) {
        self.draft.accent = accent;
        self.has_unsaved_changes = true;
    }

    pub fn set_language(&mut self, language: Language) {
        self.draft.language = language;
        self.has_unsaved_changes = true;
    }

    pub fn set_volume_step(&mut self, step: u8) {
        self.draft.volume_step = step;
        self.has_unsaved_changes = true;
    }

    pub fn toggle_autostart(&mut self) {
        self.draft.autostart = !self.draft.autostart;
        self.has_unsaved_changes = true;
    }

    pub fn toggle_sound_feedback(&mut self) {
        self.draft.sound_feedback = !self.draft.sound_feedback;
        self.has_unsaved_changes = true;
    }

    /// Live preview Appearance resolved directly from current uncommitted draft.
    pub fn preview_appearance(&self) -> Appearance {
        Appearance {
            color_scheme: match self.draft.theme {
                ThemeMode::Dark => ColorScheme::Dark,
                ThemeMode::Light => ColorScheme::Light,
                ThemeMode::System => ColorScheme::Dark,
            },
            reduce_transparency: self.draft.material == MaterialMode::Opaque,
            increase_contrast: false,
            reduce_motion: self.draft.motion != MotionMode::Full,
        }
    }

    /// Live preview optical tokens resolved for the given material role.
    pub fn preview_tokens(&self, role: MaterialRole) -> MaterialTokens {
        MaterialTokens::resolve(role, &self.preview_appearance())
    }

    /// Discard draft edits: reverts draft to confirmed state and returns `AppAction::CancelConfig`.
    pub fn cancel(&mut self) -> AppAction {
        self.draft = SettingsDraft::from_state(&self.confirmed_state);
        self.has_unsaved_changes = false;
        AppAction::CancelConfig
    }

    /// Reset settings to defaults and return `AppAction::ResetConfig`.
    pub fn reset_defaults(&mut self) -> AppAction {
        self.draft = SettingsDraft {
            theme: ThemeMode::System,
            material: MaterialMode::Auto,
            motion: MotionMode::Full,
            accent: AccentMode::System,
            language: Language::En,
            volume_step: 2,
            autostart: true,
            sound_feedback: true,
        };
        self.has_unsaved_changes = true;
        AppAction::ResetConfig
    }

    /// Handle user click on any semantic control node in Settings.
    pub fn handle_click(&mut self, id: SemanticId) -> Option<AppAction> {
        // Tab switching
        match id {
            TAB_GENERAL_ID => {
                self.select_tab(SettingsTab::General);
                return None;
            }
            TAB_APPEARANCE_ID => {
                self.select_tab(SettingsTab::Appearance);
                return None;
            }
            TAB_HOTKEYS_ID => {
                self.select_tab(SettingsTab::Hotkeys);
                return None;
            }
            TAB_BLACKLIST_ID => {
                self.select_tab(SettingsTab::Blacklist);
                return None;
            }
            TAB_UPDATES_ID => {
                self.select_tab(SettingsTab::Updates);
                return None;
            }
            _ => {}
        }

        // Appearance options
        match id {
            OPT_THEME_SYSTEM => self.set_theme(ThemeMode::System),
            OPT_THEME_LIGHT => self.set_theme(ThemeMode::Light),
            OPT_THEME_DARK => self.set_theme(ThemeMode::Dark),

            OPT_MAT_AUTO => self.set_material(MaterialMode::Auto),
            OPT_MAT_TRANSLUCENT => self.set_material(MaterialMode::Translucent),
            OPT_MAT_OPAQUE => self.set_material(MaterialMode::Opaque),

            OPT_MOT_FULL => self.set_motion(MotionMode::Full),
            OPT_MOT_REDUCED => self.set_motion(MotionMode::Reduced),
            OPT_MOT_DISABLED => self.set_motion(MotionMode::Disabled),

            OPT_ACC_SYSTEM => self.set_accent(AccentMode::System),
            OPT_ACC_BLUE => self.set_accent(AccentMode::Blue),
            OPT_ACC_GREEN => self.set_accent(AccentMode::Green),
            OPT_ACC_PURPLE => self.set_accent(AccentMode::Purple),
            OPT_ACC_ORANGE => self.set_accent(AccentMode::Orange),

            OPT_LANG_EN => self.set_language(Language::En),
            OPT_LANG_VI => self.set_language(Language::Vi),

            // General options
            OPT_GEN_AUTOSTART => self.toggle_autostart(),
            OPT_GEN_BEEP_LIMIT => self.toggle_sound_feedback(),
            OPT_GEN_STEP_1 => self.set_volume_step(1),
            OPT_GEN_STEP_2 => self.set_volume_step(2),
            OPT_GEN_STEP_5 => self.set_volume_step(5),

            // Actions
            BTN_SAVE_ID => {
                let draft = self.draft.clone();
                self.has_unsaved_changes = false;
                return Some(AppAction::ApplyConfigDraft(Box::new(
                    volumectl_lib::host_core::SettingsPatch {
                        theme: Some(format!("{:?}", draft.theme)),
                        material: Some(format!("{:?}", draft.material)),
                        motion: Some(format!("{:?}", draft.motion)),
                        accent: Some(format!("{:?}", draft.accent)),
                        language: Some(match draft.language {
                            Language::En => "en".into(),
                            Language::Vi => "vi".into(),
                        }),
                        volume_step: Some(u32::from(draft.volume_step)),
                        autostart: Some(draft.autostart),
                        ..Default::default()
                    },
                )));
            }
            BTN_CANCEL_ID => return Some(self.cancel()),
            BTN_RESET_ID => return Some(self.reset_defaults()),
            BTN_CHECK_UPDATES_ID => return Some(AppAction::CheckUpdates),
            BTN_APPLY_REC_BLACKLIST_ID => return Some(AppAction::ApplyRecommendedBlacklist),
            BTN_CLEAR_BLACKLIST_ID => return Some(AppAction::ClearBlacklist),

            _ => {}
        }

        None
    }

    /// Populate scene nodes with layout geometry matching `settings_general.png` / `settings_appearance.png`.
    pub fn populate_scene(&self, scene: &mut Scene, width: f32, height: f32) {
        let w = width.clamp(SETTINGS_MIN_WIDTH, 4000.0);
        let h = height.clamp(SETTINGS_MIN_HEIGHT, 4000.0);
        let lang = self.draft.language;

        // 1. Root window panel (760 x 620 minimum)
        let root = SceneNode::new(
            SETTINGS_ROOT_ID,
            MaterialRole::Panel,
            Rect::new(0.0, 0.0, w, h),
            0,
        );
        scene.insert(root);

        // 2. Navigation sidebar (180px wide) with 5 category items
        let sidebar = SceneNode::new(
            SETTINGS_SIDEBAR_ID,
            MaterialRole::Sidebar,
            Rect::new(0.0, 0.0, SIDEBAR_WIDTH, h),
            1,
        );
        scene.insert(sidebar);
        scene.add_child(SETTINGS_ROOT_ID, SETTINGS_SIDEBAR_ID);

        let tabs = [
            (
                TAB_GENERAL_ID,
                SettingsTab::General,
                match lang {
                    Language::Vi => "Cài đặt chung",
                    _ => "General",
                },
            ),
            (
                TAB_APPEARANCE_ID,
                SettingsTab::Appearance,
                match lang {
                    Language::Vi => "Giao diện",
                    _ => "Appearance",
                },
            ),
            (
                TAB_HOTKEYS_ID,
                SettingsTab::Hotkeys,
                match lang {
                    Language::Vi => "Phím tắt",
                    _ => "Hotkeys",
                },
            ),
            (
                TAB_BLACKLIST_ID,
                SettingsTab::Blacklist,
                match lang {
                    Language::Vi => "Danh sách đen",
                    _ => "Blacklist",
                },
            ),
            (
                TAB_UPDATES_ID,
                SettingsTab::Updates,
                match lang {
                    Language::Vi => "Cập nhật",
                    _ => "Updates",
                },
            ),
        ];

        let mut tab_y = 24.0;
        for (id, tab, label) in tabs {
            let is_active = self.current_tab == tab;
            let role = if is_active {
                MaterialRole::Accent
            } else {
                MaterialRole::Control
            };
            let tab_node = SceneNode::new(
                id,
                role,
                Rect::new(12.0, tab_y, SIDEBAR_WIDTH - 24.0, 36.0),
                2,
            )
            .with_focusable(true)
            .with_text(label);

            scene.insert(tab_node);
            scene.add_child(SETTINGS_SIDEBAR_ID, id);
            tab_y += 44.0;
        }

        // 3. Main content area
        let content_width = w - SIDEBAR_WIDTH;
        let content_height = h - 60.0; // Reserve 60px for action bar
        let content = SceneNode::new(
            SETTINGS_CONTENT_ID,
            MaterialRole::Panel,
            Rect::new(SIDEBAR_WIDTH, 0.0, content_width, content_height),
            1,
        );
        scene.insert(content);
        scene.add_child(SETTINGS_ROOT_ID, SETTINGS_CONTENT_ID);

        // 4. Form controls for active tab
        match self.current_tab {
            SettingsTab::Appearance => {
                let mut form_y = 24.0;

                // Theme section
                let theme_label = SceneNode::new(
                    SemanticId(500),
                    MaterialRole::Hud,
                    Rect::new(SIDEBAR_WIDTH + 24.0, form_y, 200.0, 24.0),
                    2,
                )
                .with_text(match lang {
                    Language::Vi => "Chủ đề hiển thị",
                    _ => "Theme Mode",
                });
                scene.insert(theme_label);
                scene.add_child(SETTINGS_CONTENT_ID, SemanticId(500));
                form_y += 30.0;

                let theme_opts = [
                    (OPT_THEME_SYSTEM, ThemeMode::System, "System"),
                    (OPT_THEME_LIGHT, ThemeMode::Light, "Light"),
                    (OPT_THEME_DARK, ThemeMode::Dark, "Dark"),
                ];
                let mut opt_x = SIDEBAR_WIDTH + 24.0;
                for (id, mode, name) in theme_opts {
                    let is_sel = self.draft.theme == mode;
                    let role = if is_sel {
                        MaterialRole::Accent
                    } else {
                        MaterialRole::Control
                    };
                    let opt_node =
                        SceneNode::new(id, role, Rect::new(opt_x, form_y, 90.0, 30.0), 3)
                            .with_focusable(true)
                            .with_text(name);
                    scene.insert(opt_node);
                    scene.add_child(SETTINGS_CONTENT_ID, id);
                    opt_x += 100.0;
                }
                form_y += 46.0;

                // Material section
                let mat_label = SceneNode::new(
                    SemanticId(510),
                    MaterialRole::Hud,
                    Rect::new(SIDEBAR_WIDTH + 24.0, form_y, 200.0, 24.0),
                    2,
                )
                .with_text(match lang {
                    Language::Vi => "Chất liệu kính",
                    _ => "Material Effect",
                });
                scene.insert(mat_label);
                scene.add_child(SETTINGS_CONTENT_ID, SemanticId(510));
                form_y += 30.0;

                let mat_opts = [
                    (OPT_MAT_AUTO, MaterialMode::Auto, "Auto Glass"),
                    (
                        OPT_MAT_TRANSLUCENT,
                        MaterialMode::Translucent,
                        "Translucent",
                    ),
                    (OPT_MAT_OPAQUE, MaterialMode::Opaque, "Opaque"),
                ];
                let mut opt_x = SIDEBAR_WIDTH + 24.0;
                for (id, mode, name) in mat_opts {
                    let is_sel = self.draft.material == mode;
                    let role = if is_sel {
                        MaterialRole::Accent
                    } else {
                        MaterialRole::Control
                    };
                    let opt_node =
                        SceneNode::new(id, role, Rect::new(opt_x, form_y, 110.0, 30.0), 3)
                            .with_focusable(true)
                            .with_text(name);
                    scene.insert(opt_node);
                    scene.add_child(SETTINGS_CONTENT_ID, id);
                    opt_x += 120.0;
                }
                form_y += 46.0;

                // Live Preview Card
                let preview_card = SceneNode::new(
                    PREVIEW_CARD_ID,
                    match self.draft.material {
                        MaterialMode::Opaque => MaterialRole::Control,
                        _ => MaterialRole::Hud,
                    },
                    Rect::new(SIDEBAR_WIDTH + 24.0, form_y, content_width - 48.0, 100.0),
                    2,
                )
                .with_text(match lang {
                    Language::Vi => "Xem trước giao diện kính thực tế (Live Preview)",
                    _ => "Live Liquid Glass Preview",
                });
                scene.insert(preview_card);
                scene.add_child(SETTINGS_CONTENT_ID, PREVIEW_CARD_ID);
            }
            SettingsTab::General => {
                let mut form_y = 24.0;

                // Autostart toggle
                let autostart_node = SceneNode::new(
                    OPT_GEN_AUTOSTART,
                    MaterialRole::Control,
                    Rect::new(SIDEBAR_WIDTH + 24.0, form_y, 300.0, 32.0),
                    2,
                )
                .with_focusable(true)
                .with_text(if self.draft.autostart {
                    match lang {
                        Language::Vi => "Khởi động cùng Windows: [BẬT]",
                        _ => "Start with Windows: [ON]",
                    }
                } else {
                    match lang {
                        Language::Vi => "Khởi động cùng Windows: [TẮT]",
                        _ => "Start with Windows: [OFF]",
                    }
                });
                scene.insert(autostart_node);
                scene.add_child(SETTINGS_CONTENT_ID, OPT_GEN_AUTOSTART);
                form_y += 44.0;

                // Volume step buttons (1%, 2%, 5%)
                let step_label = SceneNode::new(
                    SemanticId(610),
                    MaterialRole::Hud,
                    Rect::new(SIDEBAR_WIDTH + 24.0, form_y, 200.0, 24.0),
                    2,
                )
                .with_text(match lang {
                    Language::Vi => "Bước âm lượng phím tắt",
                    _ => "Volume Step",
                });
                scene.insert(step_label);
                scene.add_child(SETTINGS_CONTENT_ID, SemanticId(610));
                form_y += 30.0;

                let steps = [
                    (OPT_GEN_STEP_1, 1, "1%"),
                    (OPT_GEN_STEP_2, 2, "2% (Mặc định)"),
                    (OPT_GEN_STEP_5, 5, "5%"),
                ];
                let mut step_x = SIDEBAR_WIDTH + 24.0;
                for (id, val, text) in steps {
                    let is_sel = self.draft.volume_step == val;
                    let role = if is_sel {
                        MaterialRole::Accent
                    } else {
                        MaterialRole::Control
                    };
                    let btn = SceneNode::new(id, role, Rect::new(step_x, form_y, 110.0, 30.0), 3)
                        .with_focusable(true)
                        .with_text(text);
                    scene.insert(btn);
                    scene.add_child(SETTINGS_CONTENT_ID, id);
                    step_x += 120.0;
                }
            }
            SettingsTab::Updates => {
                let mut form_y = 24.0;

                let title = SceneNode::new(
                    SemanticId(710),
                    MaterialRole::Hud,
                    Rect::new(SIDEBAR_WIDTH + 24.0, form_y, 350.0, 28.0),
                    2,
                )
                .with_text(match lang {
                    Language::Vi => "Cập nhật ứng dụng",
                    _ => "Software Updates",
                });
                scene.insert(title);
                scene.add_child(SETTINGS_CONTENT_ID, SemanticId(710));
                form_y += 36.0;

                let version_info = SceneNode::new(
                    SemanticId(711),
                    MaterialRole::Hud,
                    Rect::new(SIDEBAR_WIDTH + 24.0, form_y, 350.0, 24.0),
                    2,
                )
                .with_text(match lang {
                    Language::Vi => format!("Phiên bản hiện tại: v{}", crate::VERSION),
                    _ => format!("Current Version: v{}", crate::VERSION),
                });
                scene.insert(version_info);
                scene.add_child(SETTINGS_CONTENT_ID, SemanticId(711));
                form_y += 36.0;

                let check_btn = SceneNode::new(
                    BTN_CHECK_UPDATES_ID,
                    MaterialRole::Accent,
                    Rect::new(SIDEBAR_WIDTH + 24.0, form_y, 220.0, 36.0),
                    2,
                )
                .with_focusable(true)
                .with_text(match lang {
                    Language::Vi => "Kiểm tra cập nhật ngay",
                    _ => "Check for Updates Now",
                });
                scene.insert(check_btn);
                scene.add_child(SETTINGS_CONTENT_ID, BTN_CHECK_UPDATES_ID);
            }
            SettingsTab::Hotkeys => {
                let mut form_y = 24.0;

                let title = SceneNode::new(
                    SemanticId(720),
                    MaterialRole::Hud,
                    Rect::new(SIDEBAR_WIDTH + 24.0, form_y, 350.0, 28.0),
                    2,
                )
                .with_text(match lang {
                    Language::Vi => "Phím tắt toàn cục (Global Hotkeys)",
                    _ => "Global Hotkeys",
                });
                scene.insert(title);
                scene.add_child(SETTINGS_CONTENT_ID, SemanticId(720));
                form_y += 36.0;

                let shortcuts = [
                    (
                        "Ctrl + Alt + Up",
                        match lang {
                            Language::Vi => "Tăng âm lượng (+2%)",
                            _ => "Volume Up (+2%)",
                        },
                    ),
                    (
                        "Ctrl + Alt + Down",
                        match lang {
                            Language::Vi => "Giảm âm lượng (-2%)",
                            _ => "Volume Down (-2%)",
                        },
                    ),
                    (
                        "Ctrl + Alt + M",
                        match lang {
                            Language::Vi => "Bật / Tắt tiếng",
                            _ => "Toggle Mute",
                        },
                    ),
                    (
                        "Ctrl + Alt + X",
                        match lang {
                            Language::Vi => "Mở bộ trộn âm lượng",
                            _ => "Open Volume Mixer",
                        },
                    ),
                ];

                for (key, desc) in shortcuts {
                    let row = SceneNode::new(
                        SemanticId(form_y as u64 + 1000),
                        MaterialRole::Control,
                        Rect::new(SIDEBAR_WIDTH + 24.0, form_y, content_width - 48.0, 32.0),
                        2,
                    )
                    .with_text(format!("{}: {}", key, desc));
                    scene.insert(row);
                    scene.add_child(SETTINGS_CONTENT_ID, SemanticId(form_y as u64 + 1000));
                    form_y += 38.0;
                }
            }
            SettingsTab::Blacklist => {
                let mut form_y = 24.0;

                let title = SceneNode::new(
                    SemanticId(730),
                    MaterialRole::Hud,
                    Rect::new(SIDEBAR_WIDTH + 24.0, form_y, 350.0, 28.0),
                    2,
                )
                .with_text(match lang {
                    Language::Vi => "Danh sách ứng dụng loại trừ (Blacklist)",
                    _ => "Excluded Applications (Blacklist)",
                });
                scene.insert(title);
                scene.add_child(SETTINGS_CONTENT_ID, SemanticId(730));
                form_y += 36.0;

                let rec_btn = SceneNode::new(
                    BTN_APPLY_REC_BLACKLIST_ID,
                    MaterialRole::Control,
                    Rect::new(SIDEBAR_WIDTH + 24.0, form_y, 220.0, 32.0),
                    2,
                )
                .with_focusable(true)
                .with_text(match lang {
                    Language::Vi => "Khôi phục danh sách đề xuất",
                    _ => "Apply Recommended",
                });
                scene.insert(rec_btn);
                scene.add_child(SETTINGS_CONTENT_ID, BTN_APPLY_REC_BLACKLIST_ID);

                let clear_btn = SceneNode::new(
                    BTN_CLEAR_BLACKLIST_ID,
                    MaterialRole::Control,
                    Rect::new(SIDEBAR_WIDTH + 260.0, form_y, 160.0, 32.0),
                    2,
                )
                .with_focusable(true)
                .with_text(match lang {
                    Language::Vi => "Xóa danh sách",
                    _ => "Clear Blacklist",
                });
                scene.insert(clear_btn);
                scene.add_child(SETTINGS_CONTENT_ID, BTN_CLEAR_BLACKLIST_ID);
            }
        }

        // 5. Bottom action bar (Save, Cancel, Reset)
        let action_bar = SceneNode::new(
            SETTINGS_ACTION_BAR_ID,
            MaterialRole::Panel,
            Rect::new(SIDEBAR_WIDTH, content_height, content_width, 60.0),
            1,
        );
        scene.insert(action_bar);
        scene.add_child(SETTINGS_ROOT_ID, SETTINGS_ACTION_BAR_ID);

        let btn_reset = SceneNode::new(
            BTN_RESET_ID,
            MaterialRole::Control,
            Rect::new(SIDEBAR_WIDTH + 20.0, content_height + 16.0, 100.0, 32.0),
            2,
        )
        .with_focusable(true)
        .with_text(match lang {
            Language::Vi => "Khôi phục",
            _ => "Reset",
        });

        let btn_cancel = SceneNode::new(
            BTN_CANCEL_ID,
            MaterialRole::Control,
            Rect::new(w - 210.0, content_height + 16.0, 90.0, 32.0),
            2,
        )
        .with_focusable(true)
        .with_text(match lang {
            Language::Vi => "Hủy",
            _ => "Cancel",
        });

        let btn_save = SceneNode::new(
            BTN_SAVE_ID,
            if self.has_unsaved_changes {
                MaterialRole::Accent
            } else {
                MaterialRole::Control
            },
            Rect::new(w - 110.0, content_height + 16.0, 90.0, 32.0),
            2,
        )
        .with_focusable(true)
        .with_text(match lang {
            Language::Vi => "Lưu",
            _ => "Save",
        });

        scene.insert(btn_reset);
        scene.insert(btn_cancel);
        scene.insert(btn_save);

        scene.add_child(SETTINGS_ACTION_BAR_ID, BTN_RESET_ID);
        scene.add_child(SETTINGS_ACTION_BAR_ID, BTN_CANCEL_ID);
        scene.add_child(SETTINGS_ACTION_BAR_ID, BTN_SAVE_ID);
    }
}
