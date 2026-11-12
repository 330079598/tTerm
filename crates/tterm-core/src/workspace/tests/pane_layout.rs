use crate::workspace::pane_layout::{
    PaneBox, PaneMinimum, SashBox, drag_sash, get_split_at, layout_pane_tree, minimum_extent,
};
use crate::workspace::tab_groups::{PaneNode, SplitOrientation, pane_leaf};

fn tree() -> PaneNode {
    PaneNode::Split {
        orientation: SplitOrientation::Horizontal,
        sizes: vec![0.5, 0.5],
        children: vec![
            pane_leaf("a"),
            PaneNode::Split {
                orientation: SplitOrientation::Vertical,
                sizes: vec![0.25, 0.75],
                children: vec![pane_leaf("b"), pane_leaf("c")],
            },
        ],
    }
}

fn area() -> PaneBox {
    PaneBox {
        left: 0.0,
        top: 0.0,
        width: 1005.0,
        height: 405.0,
    }
}

#[test]
fn fills_the_area_with_the_panes_the_gap_between_them() {
    let layout = layout_pane_tree(&tree(), area(), 5.0);
    assert_eq!(
        layout.panes.get("a"),
        Some(&PaneBox {
            left: 0.0,
            top: 0.0,
            width: 500.0,
            height: 405.0,
        })
    );
    assert_eq!(
        layout.panes.get("b"),
        Some(&PaneBox {
            left: 505.0,
            top: 0.0,
            width: 500.0,
            height: 100.0,
        })
    );
    assert_eq!(
        layout.panes.get("c"),
        Some(&PaneBox {
            left: 505.0,
            top: 105.0,
            width: 500.0,
            height: 300.0,
        })
    );
}

#[test]
fn puts_a_sash_in_each_gap_with_the_split_it_resizes() {
    let layout = layout_pane_tree(&tree(), area(), 5.0);
    assert_eq!(
        layout.sashes,
        vec![
            SashBox {
                left: 500.0,
                top: 0.0,
                width: 5.0,
                height: 405.0,
                path: vec![],
                index: 0,
                orientation: SplitOrientation::Horizontal,
                room: 1000.0,
            },
            SashBox {
                left: 505.0,
                top: 100.0,
                width: 500.0,
                height: 5.0,
                path: vec![1],
                index: 0,
                orientation: SplitOrientation::Vertical,
                room: 400.0,
            },
        ]
    );
}

#[test]
fn rounds_edges_to_whole_pixels_so_panes_meet_without_a_seam() {
    let thirds = PaneNode::Split {
        orientation: SplitOrientation::Horizontal,
        sizes: vec![1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0],
        children: vec![pane_leaf("a"), pane_leaf("b"), pane_leaf("c")],
    };
    let layout = layout_pane_tree(
        &thirds,
        PaneBox {
            left: 0.0,
            top: 0.0,
            width: 100.0,
            height: 10.0,
        },
        0.0,
    );
    let boxes: Vec<&PaneBox> = ["a", "b", "c"]
        .iter()
        .map(|id| layout.panes.get(*id).unwrap())
        .collect();
    assert_eq!(
        boxes.iter().map(|pane| pane.left).collect::<Vec<_>>(),
        vec![0.0, 33.0, 67.0]
    );
    assert_eq!(boxes.iter().map(|pane| pane.width).sum::<f64>(), 100.0);
}

#[test]
fn minimum_extent_adds_up_panes_along_a_split_and_takes_the_widest_across_one() {
    let minimum = PaneMinimum {
        width: 200.0,
        height: 100.0,
    };
    assert_eq!(
        minimum_extent(&tree(), SplitOrientation::Horizontal, minimum, 5.0),
        405.0
    );
    assert_eq!(
        minimum_extent(&tree(), SplitOrientation::Vertical, minimum, 5.0),
        205.0
    );
}

#[test]
fn drag_sash_moves_the_room_between_the_two_panes_beside_the_sash() {
    assert_eq!(
        drag_sash(&[0.5, 0.5], 0, 100.0, 1000.0, &[200.0, 200.0]),
        vec![0.6, 0.4]
    );
}

#[test]
fn drag_sash_stops_where_a_pane_would_drop_under_its_minimum() {
    assert_eq!(
        drag_sash(&[0.5, 0.5], 0, 400.0, 1000.0, &[200.0, 200.0]),
        vec![0.8, 0.2]
    );
    assert_eq!(
        drag_sash(&[0.5, 0.5], 0, -400.0, 1000.0, &[200.0, 200.0]),
        vec![0.2, 0.8]
    );
}

#[test]
fn drag_sash_leaves_a_pane_already_under_its_minimum_where_it_is() {
    assert_eq!(
        drag_sash(&[0.1, 0.9], 0, -50.0, 1000.0, &[200.0, 200.0]),
        vec![0.1, 0.9]
    );
}

#[test]
fn get_split_at_follows_the_path_to_a_split_and_finds_no_split_at_a_pane() {
    let tree = tree();
    assert_eq!(
        get_split_at(&tree, &[1]),
        Some(&PaneNode::Split {
            orientation: SplitOrientation::Vertical,
            sizes: vec![0.25, 0.75],
            children: vec![pane_leaf("b"), pane_leaf("c")],
        })
    );
    assert_eq!(get_split_at(&tree, &[0]), None);
}

#[test]
fn oracle_layout_rounds_negative_halves_toward_positive_infinity() {
    let tree = PaneNode::Split {
        orientation: SplitOrientation::Horizontal,
        sizes: vec![0.5, 0.5],
        children: vec![pane_leaf("a"), pane_leaf("b")],
    };
    let layout = layout_pane_tree(
        &tree,
        PaneBox {
            left: -10.0,
            top: -3.0,
            width: 5.0,
            height: 10.0,
        },
        0.0,
    );
    assert_eq!(
        layout.panes.get("a"),
        Some(&PaneBox {
            left: -10.0,
            top: -3.0,
            width: 3.0,
            height: 10.0,
        })
    );
    assert_eq!(
        layout.panes.get("b"),
        Some(&PaneBox {
            left: -7.0,
            top: -3.0,
            width: 2.0,
            height: 10.0,
        })
    );
}

#[test]
fn oracle_drag_sash_returns_unchanged_for_an_invalid_room_or_index() {
    assert_eq!(
        drag_sash(&[0.5, 0.5], 0, 100.0, 0.0, &[200.0, 200.0]),
        vec![0.5, 0.5]
    );
    assert_eq!(
        drag_sash(&[0.5, 0.5], 1, 100.0, 1000.0, &[200.0, 200.0]),
        vec![0.5, 0.5]
    );
}

#[test]
fn oracle_pane_tree_serializes_exactly_like_typescript() {
    assert_eq!(
        serde_json::to_string(&tree()).unwrap(),
        r#"{"type":"split","orientation":"horizontal","children":[{"type":"pane","tabId":"a"},{"type":"split","orientation":"vertical","children":[{"type":"pane","tabId":"b"},{"type":"pane","tabId":"c"}],"sizes":[0.25,0.75]}],"sizes":[0.5,0.5]}"#
    );
}

#[test]
fn oracle_pane_sizes_use_javascript_number_formatting() {
    let tree = PaneNode::Split {
        orientation: SplitOrientation::Horizontal,
        sizes: vec![1.0, 0.5, 0.000001, 0.0000001, 1.0 / 3.0],
        children: ["a", "b", "c", "d", "e"]
            .iter()
            .map(|id| pane_leaf(*id))
            .collect(),
    };
    assert_eq!(
        serde_json::to_string(&tree).unwrap(),
        r#"{"type":"split","orientation":"horizontal","children":[{"type":"pane","tabId":"a"},{"type":"pane","tabId":"b"},{"type":"pane","tabId":"c"},{"type":"pane","tabId":"d"},{"type":"pane","tabId":"e"}],"sizes":[1,0.5,0.000001,1e-7,0.3333333333333333]}"#
    );
}

#[test]
fn oracle_pane_sizes_handle_zero_exponents_and_non_finite_values() {
    let tree = PaneNode::Split {
        orientation: SplitOrientation::Horizontal,
        sizes: vec![
            -0.0,
            1e21,
            0.0000001,
            -0.0000001,
            f64::NAN,
            f64::INFINITY,
            1.0 / 3.0,
        ],
        children: ["a", "b", "c", "d", "e", "f", "g"]
            .iter()
            .map(|id| pane_leaf(*id))
            .collect(),
    };
    assert_eq!(
        serde_json::to_string(&tree).unwrap(),
        r#"{"type":"split","orientation":"horizontal","children":[{"type":"pane","tabId":"a"},{"type":"pane","tabId":"b"},{"type":"pane","tabId":"c"},{"type":"pane","tabId":"d"},{"type":"pane","tabId":"e"},{"type":"pane","tabId":"f"},{"type":"pane","tabId":"g"}],"sizes":[0,1e+21,1e-7,-1e-7,null,null,0.3333333333333333]}"#
    );
}
