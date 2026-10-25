//! Native Help dialog surface (520x500 logical px) displaying bilingual shortcut references.

use crate::material::MaterialRole;
use crate::scene::geometry::Rect;
use crate::scene::graph::Scene;
use crate::scene::node::{SceneNode, SemanticId};
use volumectl_lib::config::Language;

pub const HELP_WIDTH: f32 = 520.0;
pub const HELP_HEIGHT: f32 = 500.0;

pub const HELP_ROOT_ID: SemanticId = SemanticId(400);
pub const HELP_CLOSE_BTN_ID: SemanticId = SemanticId(401);

/// A single keyboard shortcut entry with English and Vietnamese descriptions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutItem {
    pub key_combination: &'static str,
    pub description_en: &'static str,
    pub description_vi: &'static str,
}

impl ShortcutItem {
    pub const fn new(
        key_combination: &'static str,
        description_en: &'static str,
        description_vi: &'static str,
    ) -> Self {
        Self {
            key_combination,
            description_en,
            description_vi,
        }
    }

    /// Retrieve localized description for current language.
    pub fn description(&self, language: Language) -> &'static str {
        match language {
            Language::Vi => self.description_vi,
            _ => self.description_en,
        }
    }
}

/// Default standard shortcuts configured in VolumeControl.
pub const DEFAULT_SHORTCUTS: &[ShortcutItem] = &[
    ShortcutItem::new("Ctrl + Alt + Up", "Volume Up (+2%)", "Tăng âm lượng (+2%)"),
    ShortcutItem::new(
        "Ctrl + Alt + Down",
        "Volume Down (-2%)",
        "Giảm âm lượng (-2%)",
    ),
    ShortcutItem::new("Ctrl + Alt + M", "Toggle Mute", "Bật/Tắt tiếng"),
    ShortcutItem::new("Ctrl + Alt + X", "Open Volume Mixer", "Mở bộ trộn âm lượng"),
    ShortcutItem::new("Escape", "Dismiss active window", "Đóng cửa sổ hiện tại"),
];

/// Manages layout, bilingual descriptions, and scene generation for the Help dialog.
pub struct HelpSurface {
    language: Language,
    shortcuts: Vec<ShortcutItem>,
    is_open: bool,
}

impl Default for HelpSurface {
    fn default() -> Self {
        Self::new(Language::En)
    }
}

impl HelpSurface {
    pub fn new(language: Language) -> Self {
        Self {
            language,
            shortcuts: DEFAULT_SHORTCUTS.to_vec(),
            is_open: false,
        }
    }

    pub fn set_language(&mut self, language: Language) {
        self.language = language;
    }

    pub fn language(&self) -> Language {
        self.language
    }

    pub fn open(&mut self) {
        self.is_open = true;
    }

    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Populate scene nodes with layout geometry matching the Help reference dialog.
    pub fn populate_scene(&self, scene: &mut Scene) {
        if !self.is_open {
            if let Some(root) = scene.get_mut(HELP_ROOT_ID) {
                root.visible = false;
            }
            return;
        }

        // 1. Root dialog card (520 x 500)
        let mut root = SceneNode::new(
            HELP_ROOT_ID,
            MaterialRole::Panel,
            Rect::new(0.0, 0.0, HELP_WIDTH, HELP_HEIGHT),
            0,
        );
        root.visible = true;
        scene.insert(root);

        // 2. Title header
        let title_text = match self.language {
            Language::Vi => "Phím tắt bàn phím",
            _ => "Keyboard Shortcuts",
        };
        let title_node = SceneNode::new(
            SemanticId(402),
            MaterialRole::Hud,
            Rect::new(24.0, 20.0, 300.0, 28.0),
            1,
        )
        .with_text(title_text);
        scene.insert(title_node);
        scene.add_child(HELP_ROOT_ID, SemanticId(402));

        // 3. Close button (top right)
        let close_btn = SceneNode::new(
            HELP_CLOSE_BTN_ID,
            MaterialRole::Control,
            Rect::new(HELP_WIDTH - 64.0, 16.0, 40.0, 28.0),
            1,
        )
        .with_focusable(true)
        .with_text("✕");
        scene.insert(close_btn);
        scene.add_child(HELP_ROOT_ID, HELP_CLOSE_BTN_ID);

        // 4. Shortcut list rows (starting at y: 64.0, row height: 36.0)
        let mut y = 64.0;
        for (i, shortcut) in self.shortcuts.iter().enumerate() {
            let base = 410 + (i as u64) * 2;
            let key_id = SemanticId(base);
            let desc_id = SemanticId(base + 1);

            let key_node = SceneNode::new(
                key_id,
                MaterialRole::Tooltip,
                Rect::new(24.0, y, 160.0, 28.0),
                1,
            )
            .with_text(shortcut.key_combination);

            let desc_node = SceneNode::new(
                desc_id,
                MaterialRole::Hud,
                Rect::new(200.0, y, 290.0, 28.0),
                1,
            )
            .with_text(shortcut.description(self.language));

            scene.insert(key_node);
            scene.insert(desc_node);

            scene.add_child(HELP_ROOT_ID, key_id);
            scene.add_child(HELP_ROOT_ID, desc_id);

            y += 36.0;
        }
    }
}
