//! Keyboard focus traversal, Tab cycling, and active focus management.

use crate::scene::graph::Scene;
use crate::scene::node::SemanticId;

/// Manages keyboard navigation and active control focus state across the scene.
#[derive(Debug, Default, Clone)]
pub struct FocusManager {
    focused: Option<SemanticId>,
}

impl FocusManager {
    pub fn new() -> Self {
        Self { focused: None }
    }

    /// Current focused node identifier.
    pub fn focused(&self) -> Option<SemanticId> {
        self.focused
    }

    /// Check if a specific node holds active focus.
    pub fn is_focused(&self, id: SemanticId) -> bool {
        self.focused == Some(id)
    }

    /// Set or clear focus directly.
    pub fn set_focus(&mut self, id: Option<SemanticId>) {
        self.focused = id;
    }

    /// Clear current focus.
    pub fn clear_focus(&mut self) {
        self.focused = None;
    }

    /// Collect focusable, visible nodes in logical visual reading order (y top-to-bottom, x left-to-right).
    fn focusable_order(&self, scene: &Scene) -> Vec<SemanticId> {
        let mut list: Vec<_> = scene
            .iter()
            .filter(|(_, node)| node.visible && node.focusable)
            .map(|(&id, node)| (id, node.layout_bounds.y, node.layout_bounds.x))
            .collect();

        // Sort primarily by Y, then by X
        list.sort_by(|a, b| {
            let row_diff = (a.1 - b.1).abs();
            if row_diff > 4.0 {
                a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal)
            }
        });

        list.into_iter().map(|(id, _, _)| id).collect()
    }

    /// Advance focus to the next focusable node (Tab navigation). Wraps around.
    pub fn focus_next(&mut self, scene: &Scene) -> Option<SemanticId> {
        let order = self.focusable_order(scene);
        if order.is_empty() {
            self.focused = None;
            return None;
        }

        let next = match self.focused {
            Some(curr) => match order.iter().position(|&id| id == curr) {
                Some(idx) => order[(idx + 1) % order.len()],
                None => order[0],
            },
            None => order[0],
        };

        self.focused = Some(next);
        Some(next)
    }

    /// Move focus to the previous focusable node (Shift+Tab navigation). Wraps around.
    pub fn focus_prev(&mut self, scene: &Scene) -> Option<SemanticId> {
        let order = self.focusable_order(scene);
        if order.is_empty() {
            self.focused = None;
            return None;
        }

        let prev = match self.focused {
            Some(curr) => match order.iter().position(|&id| id == curr) {
                Some(idx) => {
                    if idx == 0 {
                        order[order.len() - 1]
                    } else {
                        order[idx - 1]
                    }
                }
                None => order[order.len() - 1],
            },
            None => order[order.len() - 1],
        };

        self.focused = Some(prev);
        Some(prev)
    }
}
