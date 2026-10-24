//! Flattening retained scene hierarchy into z-ordered render primitives.

use crate::material::MaterialRole;
use crate::scene::geometry::Rect;
use crate::scene::graph::Scene;
use crate::scene::node::SemanticId;

/// Flat render primitive ready for WGPU material and SDF passes.
#[derive(Debug, Clone, PartialEq)]
pub struct RenderPrimitive {
    pub id: SemanticId,
    pub bounds: Rect,
    pub role: MaterialRole,
    pub z_order: u16,
    pub clip: Option<Rect>,
}

/// Flattener that maintains a reusable buffer to achieve zero per-frame heap allocations.
#[derive(Default)]
pub struct SceneFlattener {
    buffer: Vec<RenderPrimitive>,
}

impl SceneFlattener {
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(32),
        }
    }

    /// Flatten the visible nodes of a scene into a strictly z-sorted primitive slice.
    pub fn flatten<'a>(&'a mut self, scene: &Scene) -> &'a [RenderPrimitive] {
        self.buffer.clear();

        for (_, node) in scene.iter() {
            if !node.visible {
                continue;
            }

            self.buffer.push(RenderPrimitive {
                id: node.id,
                bounds: node.layout_bounds,
                role: node.role,
                z_order: node.z_order,
                clip: node.clip,
            });
        }

        // Sort strictly by z_order ascending (lower z draws first, higher z on top)
        self.buffer.sort_by_key(|p| p.z_order);
        &self.buffer
    }

    /// Access the current flattened primitives.
    pub fn primitives(&self) -> &[RenderPrimitive] {
        &self.buffer
    }
}
