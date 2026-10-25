//! Unit tests for retained semantic scene, geometry, and z-order flattening.

use crate::material::MaterialRole;
use crate::scene::flatten::SceneFlattener;
use crate::scene::geometry::{Point, Rect};
use crate::scene::graph::Scene;
use crate::scene::node::{SceneNode, SemanticId};

#[test]
fn rect_point_containment_and_intersection() {
    let r1 = Rect::new(10.0, 10.0, 100.0, 50.0);
    assert!(r1.contains_point(Point::new(15.0, 20.0)));
    assert!(r1.contains_point(Point::new(10.0, 10.0)));
    assert!(!r1.contains_point(Point::new(5.0, 5.0)));
    assert!(!r1.contains_point(Point::new(120.0, 30.0)));

    let r2 = Rect::new(50.0, 20.0, 100.0, 100.0);
    let intersection = r1.intersect(&r2).expect("rectangles should overlap");
    assert_eq!(intersection, Rect::new(50.0, 20.0, 60.0, 40.0));

    let r3 = Rect::new(200.0, 200.0, 10.0, 10.0);
    assert!(r1.intersect(&r3).is_none());
}

#[test]
fn scene_node_hierarchy_and_dirty_tracking() {
    let mut scene = Scene::new();
    let initial_rev = scene.content_revision();

    let root_node = SceneNode::new(
        SemanticId(1),
        MaterialRole::Panel,
        Rect::new(0.0, 0.0, 400.0, 300.0),
        0,
    );
    let child_node = SceneNode::new(
        SemanticId(2),
        MaterialRole::Control,
        Rect::new(20.0, 20.0, 100.0, 32.0),
        10,
    )
    .with_focusable(true)
    .with_text("Click Me");

    scene.insert(root_node);
    scene.insert(child_node);
    scene.add_child(SemanticId(1), SemanticId(2));

    assert!(scene.content_revision() > initial_rev);
    assert_eq!(scene.len(), 2);

    let dirty = scene.take_dirty();
    assert!(dirty.contains(&SemanticId(1)));
    assert!(dirty.contains(&SemanticId(2)));
    assert!(scene.take_dirty().is_empty());

    let child = scene.get(SemanticId(2)).expect("child exists");
    assert_eq!(child.parent, Some(SemanticId(1)));
    assert!(child.focusable);
    assert_eq!(child.text.as_deref(), Some("Click Me"));

    let parent = scene.get(SemanticId(1)).expect("parent exists");
    assert_eq!(parent.children, vec![SemanticId(2)]);

    scene.remove(SemanticId(2));
    assert_eq!(scene.len(), 1);
    let parent_after = scene.get(SemanticId(1)).expect("parent remains");
    assert!(parent_after.children.is_empty());
}

#[test]
fn flattener_sorts_strictly_by_z_order_and_filters_invisible() {
    let mut scene = Scene::new();

    let n1 = SceneNode::new(
        SemanticId(1),
        MaterialRole::Panel,
        Rect::new(0.0, 0.0, 400.0, 300.0),
        10,
    );
    let n2 = SceneNode::new(
        SemanticId(2),
        MaterialRole::Hud,
        Rect::new(10.0, 10.0, 50.0, 50.0),
        100,
    );
    let n3 = SceneNode::new(
        SemanticId(3),
        MaterialRole::Control,
        Rect::new(20.0, 20.0, 30.0, 30.0),
        0,
    );
    let mut n4_hidden = SceneNode::new(
        SemanticId(4),
        MaterialRole::Tooltip,
        Rect::new(0.0, 0.0, 20.0, 20.0),
        50,
    );
    n4_hidden.visible = false;

    scene.insert(n1);
    scene.insert(n2);
    scene.insert(n3);
    scene.insert(n4_hidden);

    let mut flattener = SceneFlattener::new();
    let primitives = flattener.flatten(&scene);

    assert_eq!(primitives.len(), 3, "Invisible node should be filtered out");
    assert_eq!(primitives[0].id, SemanticId(3)); // z_order: 0
    assert_eq!(primitives[1].id, SemanticId(1)); // z_order: 10
    assert_eq!(primitives[2].id, SemanticId(2)); // z_order: 100

    // Second run reuses buffer capacity without reallocation
    let cap = flattener.primitives().len();
    let primitives_again = flattener.flatten(&scene);
    assert_eq!(primitives_again.len(), cap);
}
