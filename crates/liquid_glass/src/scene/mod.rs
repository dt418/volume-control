//! Retained semantic scene graph and z-ordered primitive flattening.

pub mod flatten;
pub mod geometry;
pub mod graph;
pub mod node;

#[cfg(test)]
pub mod tests;

pub use flatten::{RenderPrimitive, SceneFlattener};
pub use geometry::{Point, Rect};
pub use graph::Scene;
pub use node::{SceneNode, SemanticId};
