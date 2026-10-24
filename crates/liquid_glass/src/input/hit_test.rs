//! Semantic hit testing evaluating static layout bounds independently of visual deformation.

use crate::scene::geometry::Point;
use crate::scene::graph::Scene;
use crate::scene::node::SemanticId;

/// Hit test the scene at given cursor point, returning the top-most matching node.
///
/// **Critical Invariant**: Hit testing always evaluates against static un-deformed
/// `layout_bounds` and clipping rects, completely ignoring pointer velocity stretch,
/// trailing, or fluid ripple displacements.
pub fn hit_test(scene: &Scene, point: Point) -> Option<SemanticId> {
    // Collect visible nodes
    let mut candidates: Vec<_> = scene
        .iter()
        .filter(|(_, node)| node.visible)
        .map(|(_, node)| (node.id, node.z_order, node.layout_bounds, node.clip))
        .collect();

    // Sort descending by z_order so top-most controls are tested first
    candidates.sort_by_key(|a| std::cmp::Reverse(a.1));

    for (id, _, bounds, clip) in candidates {
        // If clipped, point must be within clipping rectangle
        if let Some(clip_rect) = clip {
            if !clip_rect.contains_point(point) {
                continue;
            }
        }

        // Test static semantic layout bounds
        if bounds.contains_point(point) {
            return Some(id);
        }
    }

    None
}
