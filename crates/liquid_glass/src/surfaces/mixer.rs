//! Native Volume Mixer floating surface (400x224 logical px).

use crate::material::MaterialRole;
use crate::scene::geometry::Rect;
use crate::scene::graph::Scene;
use crate::scene::node::{SceneNode, SemanticId};
use volumectl_lib::ui::{AppAction, AppState, AudioSessionState};

pub const MIXER_WIDTH: f32 = 400.0;
pub const MIXER_HEIGHT: f32 = 224.0;

pub const MIXER_ROOT_ID: SemanticId = SemanticId(200);
pub const MASTER_SLIDER_ID: SemanticId = SemanticId(201);
pub const MASTER_FILL_ID: SemanticId = SemanticId(204);
pub const MASTER_MUTE_ID: SemanticId = SemanticId(202);
pub const SEARCH_INPUT_ID: SemanticId = SemanticId(210);
const SESSION_BASE_ID: u64 = 1000;
const SESSION_STRIDE: u64 = 10;

/// Manages layout, session rails, filtering, and user interaction dispatch for the Volume Mixer HUD.
pub struct MixerSurface {
    master_volume: u8,
    master_muted: bool,
    device_name: String,
    search_query: String,
    sessions: Vec<AudioSessionState>,
}

impl Default for MixerSurface {
    fn default() -> Self {
        Self::new()
    }
}

/// Truncate a label with an ellipsis so it fits the fixed label column
/// (104 px pill minus 16 px padding). Rough per-char estimate is enough at
/// this font size; measured truncation needs a font system the scene
/// builder does not own.
fn fit_label(text: &str) -> String {
    const MAX_CONTENT: usize = 11; // chars fitting 88 px at ~7.6 px/char
    if text.chars().count() <= MAX_CONTENT {
        return text.to_string();
    }
    let mut fitted: String = text.chars().take(MAX_CONTENT - 1).collect();
    fitted.push('…');
    fitted
}

impl MixerSurface {
    /// Construct a new Volume Mixer surface with default master state.
    pub fn new() -> Self {
        Self {
            master_volume: 50,
            master_muted: false,
            device_name: "Speakers".into(),
            search_query: String::new(),
            sessions: Vec::new(),
        }
    }

    /// Synchronize confirmed host state into mixer state.
    pub fn sync_state(&mut self, state: &AppState) {
        self.master_volume = state.volume_percent;
        self.master_muted = state.muted;
        if let Some(ref dev) = state.device {
            self.device_name = dev.clone();
        }
        self.sessions = state.sessions.clone();
    }

    /// Current active search filter query.
    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    /// Update search filter query (case-insensitive substring filter).
    pub fn set_search_query(&mut self, query: &str) {
        self.search_query = query.to_lowercase();
    }

    /// Append a single character to the search query.
    pub fn append_search_char(&mut self, c: char) {
        self.search_query.extend(c.to_lowercase());
    }

    /// Remove the last character from the search query.
    pub fn pop_search_char(&mut self) -> Option<char> {
        self.search_query.pop()
    }

    /// Append a committed IME or pasted text segment to the search query.
    pub fn commit_search_text(&mut self, text: &str) {
        self.search_query.push_str(&text.to_lowercase());
    }

    /// Clear the active search query.
    pub fn clear_search(&mut self) {
        self.search_query.clear();
    }

    /// List active sessions matching the current search filter.
    pub fn filtered_sessions(&self) -> Vec<&AudioSessionState> {
        if self.search_query.is_empty() {
            return self.sessions.iter().collect();
        }

        self.sessions
            .iter()
            .filter(|s| {
                s.name.to_lowercase().contains(&self.search_query)
                    || s.process.to_lowercase().contains(&self.search_query)
                    || s.display_name
                        .as_deref()
                        .map(|d| d.to_lowercase().contains(&self.search_query))
                        .unwrap_or(false)
            })
            .collect()
    }

    /// Translate a click on a semantic node ID into a host `AppAction`.
    pub fn handle_click(&self, id: SemanticId) -> Option<AppAction> {
        if id == MASTER_MUTE_ID {
            return Some(AppAction::ToggleMute);
        }

        let raw = id.0;
        if raw >= SESSION_BASE_ID {
            let filtered = self.filtered_sessions();
            let index = ((raw - SESSION_BASE_ID) / SESSION_STRIDE) as usize;
            let offset = (raw - SESSION_BASE_ID) % SESSION_STRIDE;

            if index < filtered.len() && offset == 2 {
                // Offset 2 corresponds to the session mute button
                return Some(AppAction::ToggleSessionMute {
                    id: filtered[index].id.clone(),
                });
            }
        }

        None
    }

    /// Translate a slider drag interaction into a volume change `AppAction`.
    pub fn handle_slider_drag(
        &self,
        id: SemanticId,
        normalized_fraction: f32,
    ) -> Option<AppAction> {
        let percent = (normalized_fraction.clamp(0.0, 1.0) * 100.0).round() as u8;

        if id == MASTER_SLIDER_ID {
            return Some(AppAction::SetVolumePercent {
                percent: percent as u16,
            });
        }

        let raw = id.0;
        if raw >= SESSION_BASE_ID {
            let filtered = self.filtered_sessions();
            let index = ((raw - SESSION_BASE_ID) / SESSION_STRIDE) as usize;
            let offset = (raw - SESSION_BASE_ID) % SESSION_STRIDE;

            if index < filtered.len() && offset == 1 {
                // Offset 1 corresponds to the session slider
                return Some(AppAction::SetSessionVolume {
                    id: filtered[index].id.clone(),
                    percent,
                });
            }
        }

        None
    }

    /// Adjust a slider by one accessibility step without pointer geometry.
    pub fn handle_slider_adjustment(&self, id: SemanticId, delta: f32) -> Option<AppAction> {
        let percent = if id == MASTER_SLIDER_ID {
            (f32::from(self.master_volume) + delta)
                .clamp(0.0, 100.0)
                .round() as u16
        } else {
            let raw = id.0;
            if raw < SESSION_BASE_ID {
                return None;
            }
            let filtered = self.filtered_sessions();
            let index = ((raw - SESSION_BASE_ID) / SESSION_STRIDE) as usize;
            let offset = (raw - SESSION_BASE_ID) % SESSION_STRIDE;
            if index >= filtered.len() || offset != 1 {
                return None;
            }
            (f32::from(filtered[index].volume_percent) + delta)
                .clamp(0.0, 100.0)
                .round() as u16
        };

        if id == MASTER_SLIDER_ID {
            Some(AppAction::SetVolumePercent { percent })
        } else {
            let filtered = self.filtered_sessions();
            let index = ((id.0 - SESSION_BASE_ID) / SESSION_STRIDE) as usize;
            Some(AppAction::SetSessionVolume {
                id: filtered[index].id.clone(),
                percent: percent as u8,
            })
        }
    }

    /// Populate scene nodes with layout geometry matching `mixer.png`.
    pub fn populate_scene(&self, scene: &mut Scene) {
        // 1. Root window card (400 x 224)
        let root = SceneNode::new(
            MIXER_ROOT_ID,
            MaterialRole::Panel,
            Rect::new(0.0, 0.0, MIXER_WIDTH, MIXER_HEIGHT),
            0,
        );
        scene.insert(root);

        // 2. Master System Output Row (y: 12.0)
        let device_label = SceneNode::new(
            SemanticId(203),
            MaterialRole::Hud,
            Rect::new(16.0, 12.0, 104.0, 24.0),
            1,
        )
        .with_text(fit_label(&self.device_name));

        let master_slider = SceneNode::new(
            MASTER_SLIDER_ID,
            MaterialRole::Control,
            Rect::new(120.0, 12.0, 190.0, 24.0),
            1,
        )
        .with_focusable(true)
        .with_a11y(crate::a11y::A11yNodeData::slider(
            "Master volume",
            f64::from(self.master_volume),
            0.0,
            100.0,
            1.0,
        ));

        let master_fill_width = if self.master_muted {
            0.0
        } else {
            190.0 * (f32::from(self.master_volume) / 100.0)
        };
        let master_fill = SceneNode::new(
            MASTER_FILL_ID,
            MaterialRole::Accent,
            Rect::new(120.0, 12.0, master_fill_width, 24.0),
            2,
        );
        let master_mute = SceneNode::new(
            MASTER_MUTE_ID,
            MaterialRole::Control,
            Rect::new(320.0, 12.0, 64.0, 24.0),
            1,
        )
        .with_focusable(true)
        .with_a11y(crate::a11y::A11yNodeData::toggle_button(
            if self.master_muted { "Unmute" } else { "Mute" },
            self.master_muted,
        ))
        .with_text(if self.master_muted { "Unmute" } else { "Mute" });

        scene.insert(device_label);
        scene.insert(master_slider);
        scene.insert(master_fill);
        scene.insert(master_mute);
        scene.add_child(MIXER_ROOT_ID, SemanticId(203));
        scene.add_child(MIXER_ROOT_ID, MASTER_SLIDER_ID);
        scene.add_child(MASTER_SLIDER_ID, MASTER_FILL_ID);
        scene.add_child(MIXER_ROOT_ID, MASTER_MUTE_ID);

        // 3. Search Filter Row (y: 44.0)
        let search_text = if self.search_query.is_empty() {
            "Tìm ứng dụng…"
        } else {
            &self.search_query
        };
        let search_node = SceneNode::new(
            SEARCH_INPUT_ID,
            MaterialRole::Control,
            Rect::new(16.0, 44.0, 368.0, 24.0),
            1,
        )
        .with_focusable(true)
        .with_text_centered(false)
        .with_text(search_text);
        scene.insert(search_node);
        scene.add_child(MIXER_ROOT_ID, SEARCH_INPUT_ID);

        // 4. Session rails (starting at y: 76.0, row height: 32.0)
        let filtered = self.filtered_sessions();
        let mut y = 76.0;

        for (idx, session) in filtered.iter().enumerate() {
            if y + 30.0 > MIXER_HEIGHT {
                break; // Stop at card bottom
            }

            let base = SESSION_BASE_ID + (idx as u64) * SESSION_STRIDE;
            let name_id = SemanticId(base);
            let slider_id = SemanticId(base + 1);
            let mute_id = SemanticId(base + 2);

            let full_title = if session.system
                || session.name.starts_with("@%SystemRoot%")
                || session.name.contains("AudioSrv")
            {
                "System Sounds"
            } else {
                session.display_name.as_deref().unwrap_or(&session.name)
            };
            // Screen readers announce the full name; the pill shows the fitted one.
            let display_title = fit_label(full_title);

            let session_label = SceneNode::new(
                name_id,
                MaterialRole::Hud,
                Rect::new(16.0, y, 104.0, 24.0),
                1,
            )
            .with_text(&display_title);
            let session_slider = SceneNode::new(
                slider_id,
                MaterialRole::Control,
                Rect::new(120.0, y, 190.0, 24.0),
                1,
            )
            .with_focusable(true)
            .with_a11y(crate::a11y::A11yNodeData::slider(
                format!("{full_title} volume"),
                f64::from(session.volume_percent),
                0.0,
                100.0,
                1.0,
            ));

            let fill_id = SemanticId(base + 3);
            let session_fill_width = if session.muted {
                0.0
            } else {
                190.0 * (f32::from(session.volume_percent) / 100.0)
            };
            let session_fill = SceneNode::new(
                fill_id,
                MaterialRole::Accent,
                Rect::new(120.0, y, session_fill_width, 24.0),
                2,
            );

            let session_mute = SceneNode::new(
                mute_id,
                MaterialRole::Control,
                Rect::new(320.0, y, 64.0, 24.0),
                1,
            )
            .with_focusable(true)
            .with_a11y(crate::a11y::A11yNodeData::toggle_button(
                if session.muted { "Unmute" } else { "Mute" },
                session.muted,
            ))
            .with_text(if session.muted { "Unmute" } else { "Mute" });

            scene.insert(session_label);
            scene.insert(session_slider);
            scene.insert(session_fill);
            scene.insert(session_mute);

            scene.add_child(MIXER_ROOT_ID, name_id);
            scene.add_child(MIXER_ROOT_ID, slider_id);
            scene.add_child(slider_id, fill_id);
            scene.add_child(MIXER_ROOT_ID, mute_id);
            y += 32.0;
        }

        // 5. Footer: session count + dismiss hint (legacy parity).
        let footer = SceneNode::new(
            SemanticId(205),
            MaterialRole::Hud,
            Rect::new(16.0, 198.0, 368.0, 18.0),
            1,
        )
        .with_text(format!("{} phiên ứng dụng  ·  Esc để đóng", filtered.len()));
        scene.insert(footer);
        scene.add_child(MIXER_ROOT_ID, SemanticId(205));
    }
}
