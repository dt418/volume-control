//! Unit tests for AccessKit accessibility tree building and node mapping.

use accesskit::{NodeId, Role};

use crate::a11y::node::A11yNodeData;
use crate::a11y::tree::A11yTreeBuilder;
use crate::material::MaterialRole;
use crate::scene::geometry::Rect;
use crate::scene::graph::Scene;
use crate::scene::node::{SceneNode, SemanticId};

#[test]
fn a11y_node_data_constructors() {
    let slider = A11yNodeData::slider("Master Volume", 75.0, 0.0, 100.0, 1.0);
    assert_eq!(slider.role, Role::Slider);
    assert_eq!(slider.name.as_deref(), Some("Master Volume"));
    assert_eq!(slider.value, Some(75.0));
    assert_eq!(slider.min_value, Some(0.0));
    assert_eq!(slider.max_value, Some(100.0));
    assert_eq!(slider.step, Some(1.0));

    let toggle = A11yNodeData::toggle_button("Mute Audio", true);
    assert_eq!(toggle.role, Role::CheckBox);
    assert_eq!(toggle.name.as_deref(), Some("Mute Audio"));
    assert_eq!(toggle.toggled, Some(true));

    let button = A11yNodeData::button("Save");
    assert_eq!(button.role, Role::Button);
    assert_eq!(button.name.as_deref(), Some("Save"));
}

#[test]
fn tree_builder_mirrors_scene_and_focus() {
    let mut scene = Scene::new();

    let root = SceneNode::new(
        SemanticId(1),
        MaterialRole::Panel,
        Rect::new(0.0, 0.0, 400.0, 300.0),
        0,
    );

    let slider = SceneNode::new(
        SemanticId(2),
        MaterialRole::Control,
        Rect::new(20.0, 50.0, 200.0, 30.0),
        10,
    )
    .with_focusable(true)
    .with_text("Master Volume Slider");

    scene.insert(root);
    scene.insert(slider);
    scene.add_child(SemanticId(1), SemanticId(2));

    let tree_update = A11yTreeBuilder::build(&scene, Some(SemanticId(2)), "VolumeControl");

    // Root node plus child nodes
    assert!(tree_update.nodes.len() >= 2);
    assert_eq!(tree_update.focus, NodeId(2), "Focus must match focused_id");

    // Check tree metadata
    let tree_info = tree_update
        .tree
        .expect("Initial update must include tree info");
    assert_eq!(tree_info.toolkit_name.as_deref(), Some("liquid_glass"));

    // Find slider node in update
    let slider_node = tree_update
        .nodes
        .iter()
        .find(|(id, _)| *id == NodeId(2))
        .map(|(_, node)| node)
        .expect("Slider node must be present in a11y tree");

    assert_eq!(slider_node.label(), Some("Master Volume Slider"));
    let bounds = slider_node.bounds().expect("Bounds must be set");
    assert_eq!(bounds.x0, 20.0);
    assert_eq!(bounds.y0, 50.0);
    assert_eq!(bounds.x1, 220.0);
    assert_eq!(bounds.y1, 80.0);
}
