//! AccessKit tree update construction from the retained semantic scene.

use accesskit::{Node, NodeId, Rect, Role, TreeId, TreeInfo, TreeUpdate};

use crate::scene::graph::Scene;
use crate::scene::node::SemanticId;

/// Builds AccessKit accessibility tree snapshots and updates from the semantic scene.
pub struct A11yTreeBuilder;

impl A11yTreeBuilder {
    /// Build a complete tree update from current scene hierarchy and active focus.
    pub fn build(scene: &Scene, focused_id: Option<SemanticId>, app_name: &str) -> TreeUpdate {
        let root_id = NodeId(scene.root_id().0);
        let mut nodes = Vec::new();

        let mut root_node = Node::new(Role::Window);
        root_node.set_label(app_name);

        let mut root_children = Vec::new();

        for (&id, scene_node) in scene.iter() {
            if !scene_node.visible || id == scene.root_id() {
                continue;
            }

            let node_id = NodeId(id.0);
            if scene_node.parent.is_none() {
                root_children.push(node_id);
            }

            let a11y_role =
                scene_node
                    .a11y
                    .as_ref()
                    .map(|data| data.role)
                    .unwrap_or(match scene_node.role {
                        crate::material::MaterialRole::Panel
                        | crate::material::MaterialRole::Sidebar => Role::GenericContainer,
                        crate::material::MaterialRole::Control => {
                            if scene_node.focusable {
                                Role::Button
                            } else {
                                Role::GenericContainer
                            }
                        }
                        crate::material::MaterialRole::Accent => Role::ProgressIndicator,
                        crate::material::MaterialRole::Tooltip => Role::Tooltip,
                        crate::material::MaterialRole::Menu => Role::Menu,
                        crate::material::MaterialRole::Hud => Role::GenericContainer,
                    });

            let mut node = Node::new(a11y_role);
            if let Some(data) = scene_node.a11y.as_ref() {
                if let Some(name) = &data.name {
                    node.set_label(name.as_str());
                }
                if let Some(value) = data.value {
                    node.set_numeric_value(value);
                }
                if let Some(min) = data.min_value {
                    node.set_min_numeric_value(min);
                }
                if let Some(max) = data.max_value {
                    node.set_max_numeric_value(max);
                }
                if let Some(step) = data.step {
                    node.set_numeric_value_step(step);
                }
                if let Some(toggled) = data.toggled {
                    node.set_toggled(if toggled {
                        accesskit::Toggled::True
                    } else {
                        accesskit::Toggled::False
                    });
                }
                if data.disabled {
                    node.set_disabled();
                }
            }
            node.set_bounds(Rect {
                x0: scene_node.layout_bounds.min_x() as f64,
                y0: scene_node.layout_bounds.min_y() as f64,
                x1: scene_node.layout_bounds.max_x() as f64,
                y1: scene_node.layout_bounds.max_y() as f64,
            });

            if let Some(text) = &scene_node.text {
                if scene_node
                    .a11y
                    .as_ref()
                    .and_then(|data| data.name.as_ref())
                    .is_none()
                {
                    node.set_label(text.as_str());
                }
            }

            let child_ids: Vec<NodeId> = scene_node.children.iter().map(|c| NodeId(c.0)).collect();
            if !child_ids.is_empty() {
                node.set_children(child_ids);
            }

            nodes.push((node_id, node));
        }

        root_node.set_children(root_children);
        nodes.push((root_id, root_node));

        let focus = focused_id.map(|f| NodeId(f.0)).unwrap_or(root_id);

        let mut tree_info = TreeInfo::new(root_id);
        tree_info.toolkit_name = Some("liquid_glass".into());
        tree_info.toolkit_version = Some(crate::VERSION.into());

        TreeUpdate {
            nodes,
            tree: Some(tree_info),
            tree_id: TreeId::ROOT,
            focus,
        }
    }
}
