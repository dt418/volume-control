//! Accessibility integration via AccessKit.

pub mod adapter;
pub mod node;
pub mod tree;

#[cfg(test)]
pub mod tests;

pub use adapter::A11yAdapter;
pub use node::A11yNodeData;
pub use tree::A11yTreeBuilder;

// Re-export common accesskit types
pub use accesskit::{Action, ActionHandler, ActionRequest, Node, NodeId, Role, TreeUpdate};
