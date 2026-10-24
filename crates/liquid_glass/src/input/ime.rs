//! Native IME state machine, Vietnamese preedit tracking, and cursor area calculation.

use crate::scene::geometry::Rect;

/// High-level actions produced by incoming OS IME events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImeAction {
    Enabled,
    PreeditChanged {
        text: String,
        cursor: Option<(usize, usize)>,
    },
    Commit(String),
    Disabled,
}

/// Tracks native IME composition state, preedit strings, and cursor area rects.
#[derive(Debug, Default, Clone)]
pub struct ImeState {
    pub is_enabled: bool,
    pub preedit: String,
    pub cursor_range: Option<(usize, usize)>,
    pub cursor_area: Rect,
}

impl ImeState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update cursor area rectangle where the OS IME candidate menu should anchor.
    pub fn set_cursor_area(&mut self, area: Rect) {
        self.cursor_area = area;
    }

    /// Process an incoming Winit IME event, updating local composition state.
    pub fn handle_event(&mut self, event: &winit::event::Ime) -> ImeAction {
        match event {
            winit::event::Ime::Enabled => {
                self.is_enabled = true;
                ImeAction::Enabled
            }
            winit::event::Ime::Preedit(text, cursor) => {
                self.preedit = text.clone();
                self.cursor_range = *cursor;
                ImeAction::PreeditChanged {
                    text: text.clone(),
                    cursor: *cursor,
                }
            }
            winit::event::Ime::Commit(text) => {
                self.preedit.clear();
                self.cursor_range = None;
                ImeAction::Commit(text.clone())
            }
            winit::event::Ime::Disabled => {
                self.is_enabled = false;
                self.preedit.clear();
                self.cursor_range = None;
                ImeAction::Disabled
            }
        }
    }

    /// Current active preedit text being composed.
    pub fn preedit_text(&self) -> &str {
        &self.preedit
    }

    /// True if an active composition session is currently underway.
    pub fn is_composing(&self) -> bool {
        !self.preedit.is_empty()
    }
}
