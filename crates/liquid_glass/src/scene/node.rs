//! Stable semantic node identifiers and retained scene node descriptors.

use crate::material::MaterialRole;
use crate::scene::geometry::Rect;

/// Stable, allocation-independent identifier for a semantic UI node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SemanticId(pub u64);

impl SemanticId {
    /// Sentinel identifier representing the root of the scene.
    pub const ROOT: Self = Self(0);

    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// A retained node in the semantic scene tree.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneNode {
    pub id: SemanticId,
    pub parent: Option<SemanticId>,
    pub children: Vec<SemanticId>,
    pub role: MaterialRole,
    pub layout_bounds: Rect,
    pub z_order: u16,
    pub focusable: bool,
    pub visible: bool,
    pub clip: Option<Rect>,
    pub content_revision: u64,
    pub text: Option<String>,
    /// Per-node text alignment override. `None` derives alignment from the
    /// material role: `Control`/`Accent` render centered (buttons, pills),
    /// everything else renders left-aligned with padding (labels, titles).
    pub text_centered: Option<bool>,
    /// Optional accessible role, numeric value/range, and toggle state.
    pub a11y: Option<crate::a11y::A11yNodeData>,
}

impl SceneNode {
    /// Construct a new scene node with default properties and unallocated parent/children.
    pub fn new(id: SemanticId, role: MaterialRole, layout_bounds: Rect, z_order: u16) -> Self {
        Self {
            id,
            parent: None,
            children: Vec::new(),
            role,
            layout_bounds,
            z_order,
            focusable: false,
            visible: true,
            clip: None,
            content_revision: 0,
            text: None,
            text_centered: None,
            a11y: None,
        }
    }

    /// Mark this node focusable with Tab/keyboard navigation.
    pub fn with_focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }

    /// Set an explicit clipping boundary.
    pub fn with_clip(mut self, clip: Option<Rect>) -> Self {
        self.clip = clip;
        self
    }

    /// Attach accessible role, value/range, and toggle metadata.
    pub fn with_a11y(mut self, data: crate::a11y::A11yNodeData) -> Self {
        self.a11y = Some(data);
        self
    }

    /// Associate a text string label with this node.
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Override this node's text alignment (see [`SceneNode::text_centered`]).
    pub fn with_text_centered(mut self, centered: bool) -> Self {
        self.text_centered = Some(centered);
        self
    }
}
