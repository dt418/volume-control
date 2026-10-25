//! Retained semantic scene graph with dirty tracking and content revisions.

use std::collections::{HashMap, HashSet};

use crate::scene::node::{SceneNode, SemanticId};

/// Retained semantic scene graph storing UI hierarchy with stable identities.
pub struct Scene {
    nodes: HashMap<SemanticId, SceneNode>,
    root: SemanticId,
    dirty_nodes: HashSet<SemanticId>,
    content_revision: u64,
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

impl Scene {
    /// Construct a new scene with an empty node graph and initial content revision.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            root: SemanticId::ROOT,
            dirty_nodes: HashSet::new(),
            content_revision: 1,
        }
    }

    /// Insert or update a node in the scene, marking it dirty.
    pub fn insert(&mut self, node: SceneNode) {
        let id = node.id;
        self.dirty_nodes.insert(id);
        self.nodes.insert(id, node);
        self.content_revision += 1;
    }

    /// Remove a node and its reference from its parent.
    pub fn remove(&mut self, id: SemanticId) -> Option<SceneNode> {
        if let Some(node) = self.nodes.remove(&id) {
            if let Some(parent_id) = node.parent {
                if let Some(parent) = self.nodes.get_mut(&parent_id) {
                    parent.children.retain(|&c| c != id);
                }
            }
            self.dirty_nodes.remove(&id);
            self.content_revision += 1;
            Some(node)
        } else {
            None
        }
    }

    /// Add a child relationship between two existing nodes.
    pub fn add_child(&mut self, parent_id: SemanticId, child_id: SemanticId) {
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            if !parent.children.contains(&child_id) {
                parent.children.push(child_id);
            }
        }
        if let Some(child) = self.nodes.get_mut(&child_id) {
            child.parent = Some(parent_id);
        }
        self.dirty_nodes.insert(parent_id);
        self.content_revision += 1;
    }

    /// Get shared reference to a node by its semantic identity.
    pub fn get(&self, id: SemanticId) -> Option<&SceneNode> {
        self.nodes.get(&id)
    }

    /// Get mutable reference to a node by its semantic identity, marking it dirty.
    pub fn get_mut(&mut self, id: SemanticId) -> Option<&mut SceneNode> {
        self.dirty_nodes.insert(id);
        self.content_revision += 1;
        self.nodes.get_mut(&id)
    }

    /// Root node identifier.
    pub fn root_id(&self) -> SemanticId {
        self.root
    }

    /// Set root node identifier.
    pub fn set_root_id(&mut self, root: SemanticId) {
        self.root = root;
    }

    /// Current content revision (used to invalidate or reuse the blur pyramid cache).
    pub fn content_revision(&self) -> u64 {
        self.content_revision
    }

    /// Drain the set of dirty nodes.
    pub fn take_dirty(&mut self) -> HashSet<SemanticId> {
        std::mem::take(&mut self.dirty_nodes)
    }

    /// Total number of nodes in the scene.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// True if the scene contains no nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Access all nodes in the scene.
    pub fn iter(&self) -> impl Iterator<Item = (&SemanticId, &SceneNode)> {
        self.nodes.iter()
    }

    /// Clear all nodes from the scene and reset the revision.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.dirty_nodes.clear();
        self.content_revision += 1;
    }
}
