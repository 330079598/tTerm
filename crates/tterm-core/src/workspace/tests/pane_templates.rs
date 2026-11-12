use serde_json::{Value, json};

use crate::workspace::pane_templates::{
    BUILTIN_PANE_TEMPLATES, LayoutShape, MAX_TEMPLATE_PANES, apply_template_to_groups,
    count_template_panes, fill_shape, is_builtin_template_id, normalize_layout_shape,
    normalize_pane_templates, shape_of_tree,
};
use crate::workspace::tab_groups::{PaneNode, SplitOrientation, TabGroup, get_pane_ids, pane_leaf};

use super::ids;

fn pane() -> LayoutShape {
    LayoutShape::Pane
}

fn row(children: Vec<LayoutShape>, sizes: Option<Vec<f64>>) -> LayoutShape {
    let sizes = sizes.unwrap_or_else(|| vec![1.0 / children.len() as f64; children.len()]);
    LayoutShape::Split {
        orientation: SplitOrientation::Horizontal,
        children,
        sizes,
    }
}

fn group(id: &str, root: PaneNode, active_tab_id: &str) -> TabGroup {
    TabGroup {
        id: id.to_owned(),
        root,
        active_tab_id: active_tab_id.to_owned(),
        zoomed_tab_id: None,
    }
}

fn row_json(children: Value, sizes: Value) -> Value {
    json!({
        "type": "split",
        "orientation": "horizontal",
        "children": children,
        "sizes": sizes,
    })
}

#[test]
fn drops_a_trees_panes_and_puts_others_back_in_reading_order() {
    let tree = PaneNode::Split {
        orientation: SplitOrientation::Horizontal,
        children: vec![
            pane_leaf("a"),
            PaneNode::Split {
                orientation: SplitOrientation::Vertical,
                children: vec![pane_leaf("b"), pane_leaf("c")],
                sizes: vec![0.5, 0.5],
            },
        ],
        sizes: vec![0.6, 0.4],
    };
    let shape = shape_of_tree(&tree);
    assert_eq!(count_template_panes(&shape), 3);
    assert_eq!(
        get_pane_ids(&fill_shape(&shape, &ids(&["x", "y", "z"]))),
        ids(&["x", "y", "z"])
    );
    assert_eq!(
        fill_shape(&shape, &ids(&["x", "y", "z"])),
        PaneNode::Split {
            orientation: SplitOrientation::Horizontal,
            children: vec![
                pane_leaf("x"),
                PaneNode::Split {
                    orientation: SplitOrientation::Vertical,
                    children: vec![pane_leaf("y"), pane_leaf("z")],
                    sizes: vec![0.5, 0.5],
                },
            ],
            sizes: vec![0.6, 0.4],
        }
    );
}

#[test]
fn gives_every_built_in_template_a_distinct_id_and_at_least_two_panes() {
    let ids: std::collections::HashSet<&str> = BUILTIN_PANE_TEMPLATES
        .iter()
        .map(|template| template.id)
        .collect();
    assert_eq!(ids.len(), BUILTIN_PANE_TEMPLATES.len());
    for template in BUILTIN_PANE_TEMPLATES.iter() {
        assert!(count_template_panes(&template.root) >= 2, "{}", template.id);
        assert_eq!(
            normalize_layout_shape(&serde_json::to_value(&template.root).unwrap()),
            Some(template.root.clone())
        );
    }
}

#[test]
fn scales_sizes_to_one_and_fills_in_missing_ones() {
    assert_eq!(
        normalize_layout_shape(&row_json(
            json!([{"type":"pane"},{"type":"pane"}]),
            json!([1, 3])
        )),
        Some(row(vec![pane(), pane()], Some(vec![0.25, 0.75])))
    );
    assert_eq!(
        normalize_layout_shape(&json!({
            "type": "split",
            "orientation": "horizontal",
            "children": [{"type":"pane"},{"type":"pane"}],
            "sizes": "x",
        })),
        Some(row(vec![pane(), pane()], None))
    );
}

#[test]
fn rejects_a_lone_pane_bad_nodes_and_too_many_panes() {
    assert_eq!(normalize_layout_shape(&json!({"type":"pane"})), None);
    assert_eq!(
        normalize_layout_shape(&json!({
            "type": "split",
            "orientation": "diagonal",
            "children": [{"type":"pane"},{"type":"pane"}],
        })),
        None
    );
    assert_eq!(
        normalize_layout_shape(&row_json(json!([{"type":"pane"}]), json!([]))),
        None
    );
    assert_eq!(
        normalize_layout_shape(&row_json(
            json!([{"type":"pane"},{"type":"box"}]),
            json!([])
        )),
        None
    );
    let too_many = row_json(
        Value::Array(
            (0..MAX_TEMPLATE_PANES + 1)
                .map(|_| json!({"type":"pane"}))
                .collect(),
        ),
        json!([]),
    );
    assert_eq!(normalize_layout_shape(&too_many), None);
}

#[test]
fn keeps_valid_templates_and_drops_broken_ones_repeats_and_built_in_ids() {
    let templates = normalize_pane_templates(&json!([
        {"id":"a","name":" Ops ","root":row_json(json!([{"type":"pane"},{"type":"pane"}]), json!([]))},
        {"id":"a","name":"again","root":row_json(json!([{"type":"pane"},{"type":"pane"}]), json!([]))},
        {"id":"builtin:columns-2","name":"fake","root":row_json(json!([{"type":"pane"},{"type":"pane"}]), json!([]))},
        {"id":"b","name":"broken","root":{"type":"pane"}},
        "nonsense"
    ]));
    assert_eq!(
        templates,
        vec![crate::workspace::pane_templates::PaneLayoutTemplate {
            id: "a".to_owned(),
            name: "Ops".to_owned(),
            root: row(vec![pane(), pane()], None),
        }]
    );
    assert_eq!(normalize_pane_templates(&Value::Null), Vec::new());
}

#[test]
fn fills_the_shape_with_the_panes_there_are_then_the_new_ones() {
    let groups = vec![
        group("g1", pane_leaf("a"), "a"),
        group("g2", pane_leaf("b"), "b"),
    ];
    let mut next = 0;
    let mut create_id = || {
        next += 1;
        format!("new-{next}")
    };
    let result = apply_template_to_groups(
        &groups,
        "a",
        &row(vec![pane(), pane(), pane()], None),
        &ids(&["c", "d"]),
        &mut create_id,
    );
    assert_eq!(
        result
            .iter()
            .map(|candidate| get_pane_ids(&candidate.root))
            .collect::<Vec<_>>(),
        vec![ids(&["a", "c", "d"]), ids(&["b"])]
    );
    assert_eq!(result[0].id, "g1");
    assert_eq!(result[0].active_tab_id, "a");
}

#[test]
fn keeps_the_active_pane_when_the_shape_is_smaller_and_moves_the_rest_out_after_it() {
    let groups = vec![
        group(
            "g1",
            PaneNode::Split {
                orientation: SplitOrientation::Vertical,
                children: vec![pane_leaf("a"), pane_leaf("b"), pane_leaf("c")],
                sizes: vec![1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0],
            },
            "c",
        ),
        group("g2", pane_leaf("d"), "d"),
    ];
    let mut next = 0;
    let mut create_id = || {
        next += 1;
        format!("new-{next}")
    };
    let result = apply_template_to_groups(
        &groups,
        "c",
        &row(vec![pane(), pane()], None),
        &[],
        &mut create_id,
    );
    assert_eq!(
        result
            .iter()
            .map(|candidate| get_pane_ids(&candidate.root))
            .collect::<Vec<_>>(),
        vec![ids(&["a", "c"]), ids(&["b"]), ids(&["d"])]
    );
    assert_eq!(
        match &result[0].root {
            PaneNode::Split { orientation, .. } => *orientation,
            PaneNode::Pane { .. } => panic!("expected a split"),
        },
        SplitOrientation::Horizontal
    );
}

#[test]
fn changes_nothing_without_enough_panes_to_fill_the_shape() {
    let groups = vec![group("g1", pane_leaf("a"), "a")];
    let mut create_id = || "new".to_owned();
    assert_eq!(
        apply_template_to_groups(
            &groups,
            "a",
            &row(vec![pane(), pane()], None),
            &[],
            &mut create_id,
        ),
        groups
    );
}

#[test]
fn oracle_normalizes_nested_sizes_missing_sizes_and_zero_sizes() {
    let nested = normalize_layout_shape(&json!({
        "type": "split",
        "orientation": "horizontal",
        "sizes": [2, 2],
        "children": [
            {"type": "pane"},
            {
                "type": "split",
                "orientation": "vertical",
                "children": [{"type": "pane"}, {"type": "pane"}],
            },
        ],
    }));
    assert_eq!(
        nested,
        Some(row(
            vec![
                pane(),
                LayoutShape::Split {
                    orientation: SplitOrientation::Vertical,
                    children: vec![pane(), pane()],
                    sizes: vec![0.5, 0.5],
                },
            ],
            Some(vec![0.5, 0.5])
        ))
    );
    assert_eq!(
        normalize_layout_shape(&json!({
            "type": "split",
            "orientation": "horizontal",
            "children": [{"type": "pane"}, {"type": "pane"}],
        })),
        Some(row(vec![pane(), pane()], Some(vec![0.5, 0.5])))
    );
    assert_eq!(
        normalize_layout_shape(&json!({
            "type": "split",
            "orientation": "horizontal",
            "sizes": [0, 0],
            "children": [{"type": "pane"}, {"type": "pane"}],
        })),
        Some(row(vec![pane(), pane()], Some(vec![0.5, 0.5])))
    );
}

#[test]
fn oracle_rejects_a_split_deeper_than_four_levels_but_accepts_a_pane_there() {
    let too_deep = json!({
        "type": "split",
        "orientation": "horizontal",
        "children": [
            {
                "type": "split",
                "orientation": "vertical",
                "children": [
                    {
                        "type": "split",
                        "orientation": "horizontal",
                        "children": [
                            {
                                "type": "split",
                                "orientation": "vertical",
                                "children": [
                                    {
                                        "type": "split",
                                        "orientation": "horizontal",
                                        "children": [{"type": "pane"}, {"type": "pane"}],
                                    },
                                    {"type": "pane"},
                                ],
                            },
                            {"type": "pane"},
                        ],
                    },
                    {"type": "pane"},
                ],
            },
            {"type": "pane"},
        ],
    });
    assert_eq!(normalize_layout_shape(&too_deep), None);

    let deepest_pane = json!({
        "type": "split",
        "orientation": "horizontal",
        "children": [
            {
                "type": "split",
                "orientation": "vertical",
                "children": [
                    {
                        "type": "split",
                        "orientation": "horizontal",
                        "children": [
                            {
                                "type": "split",
                                "orientation": "vertical",
                                "children": [{"type": "pane"}, {"type": "pane"}],
                            },
                            {"type": "pane"},
                        ],
                    },
                    {"type": "pane"},
                ],
            },
            {"type": "pane"},
        ],
    });
    assert_eq!(
        count_template_panes(&normalize_layout_shape(&deepest_pane).unwrap()),
        5
    );
}

#[test]
fn oracle_trims_javascript_whitespace_from_template_names() {
    let templates = normalize_pane_templates(&json!([{
        "id": "t1",
        "name": "\u{3000}Ops\u{feff}",
        "root": row_json(json!([{"type":"pane"},{"type":"pane"}]), json!([])),
    }]));
    assert_eq!(templates[0].name, "Ops");
}

#[test]
fn oracle_shape_serializes_exactly_like_typescript() {
    let tree = PaneNode::Split {
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
    };
    assert_eq!(
        serde_json::to_string(&shape_of_tree(&tree)).unwrap(),
        r#"{"type":"split","orientation":"horizontal","children":[{"type":"pane"},{"type":"split","orientation":"vertical","children":[{"type":"pane"},{"type":"pane"}],"sizes":[0.25,0.75]}],"sizes":[0.5,0.5]}"#
    );
    let nested_shape = normalize_layout_shape(&json!({
        "type": "split",
        "orientation": "horizontal",
        "sizes": [2, 2],
        "children": [
            {"type": "pane"},
            {
                "type": "split",
                "orientation": "vertical",
                "children": [{"type": "pane"}, {"type": "pane"}],
            },
        ],
    }));
    assert_eq!(
        serde_json::to_string(&nested_shape.unwrap()).unwrap(),
        r#"{"type":"split","orientation":"horizontal","children":[{"type":"pane"},{"type":"split","orientation":"vertical","children":[{"type":"pane"},{"type":"pane"}],"sizes":[0.5,0.5]}],"sizes":[0.5,0.5]}"#
    );
}

#[test]
fn oracle_template_json_matches_typescript() {
    let templates = normalize_pane_templates(&json!([{
        "id": "t1",
        "name": " Ops ",
        "root": {
            "type": "split",
            "orientation": "horizontal",
            "sizes": [2, 2],
            "children": [
                {"type": "pane"},
                {
                    "type": "split",
                    "orientation": "vertical",
                    "children": [{"type": "pane"}, {"type": "pane"}],
                },
            ],
        },
    }]));
    assert_eq!(
        serde_json::to_string(&templates).unwrap(),
        r#"[{"id":"t1","name":"Ops","root":{"type":"split","orientation":"horizontal","children":[{"type":"pane"},{"type":"split","orientation":"vertical","children":[{"type":"pane"},{"type":"pane"}],"sizes":[0.5,0.5]}],"sizes":[0.5,0.5]}}]"#
    );
}

#[test]
fn oracle_builtin_templates_ids_and_leaf_count_match_typescript() {
    assert!(is_builtin_template_id("builtin:columns-2"));
    assert!(!is_builtin_template_id("custom"));
    assert!(is_builtin_template_id("builtin:"));
    assert_eq!(count_template_panes(&pane()), 1);

    let expected: Value = serde_json::from_str(
        r#"[
            {"id":"builtin:columns-2","labelKey":"columns2","name":"","root":{"type":"split","orientation":"horizontal","children":[{"type":"pane"},{"type":"pane"}],"sizes":[0.5,0.5]}},
            {"id":"builtin:rows-2","labelKey":"rows2","name":"","root":{"type":"split","orientation":"vertical","children":[{"type":"pane"},{"type":"pane"}],"sizes":[0.5,0.5]}},
            {"id":"builtin:columns-3","labelKey":"columns3","name":"","root":{"type":"split","orientation":"horizontal","children":[{"type":"pane"},{"type":"pane"},{"type":"pane"}],"sizes":[0.3333333333333333,0.3333333333333333,0.3333333333333333]}},
            {"id":"builtin:main-right-2","labelKey":"mainRight2","name":"","root":{"type":"split","orientation":"horizontal","children":[{"type":"pane"},{"type":"split","orientation":"vertical","children":[{"type":"pane"},{"type":"pane"}],"sizes":[0.5,0.5]}],"sizes":[0.6,0.4]}},
            {"id":"builtin:main-bottom-2","labelKey":"mainBottom2","name":"","root":{"type":"split","orientation":"vertical","children":[{"type":"pane"},{"type":"split","orientation":"horizontal","children":[{"type":"pane"},{"type":"pane"}],"sizes":[0.5,0.5]}],"sizes":[0.6,0.4]}},
            {"id":"builtin:grid-4","labelKey":"grid4","name":"","root":{"type":"split","orientation":"horizontal","children":[{"type":"split","orientation":"vertical","children":[{"type":"pane"},{"type":"pane"}],"sizes":[0.5,0.5]},{"type":"split","orientation":"vertical","children":[{"type":"pane"},{"type":"pane"}],"sizes":[0.5,0.5]}],"sizes":[0.5,0.5]}}
        ]"#,
    )
    .unwrap();
    let actual: Vec<Value> = BUILTIN_PANE_TEMPLATES
        .iter()
        .map(|template| {
            json!({
                "id": template.id,
                "labelKey": template.label_key,
                "name": template.name,
                "root": serde_json::to_value(&template.root).unwrap(),
            })
        })
        .collect();
    assert_eq!(Value::Array(actual), expected);
}

#[test]
fn oracle_applies_a_template_to_groups_exactly() {
    let oracle = super::oracle_json();
    let expected = &oracle["paneTemplates"]["applied"];
    let groups = vec![TabGroup {
        id: "g1".to_owned(),
        root: PaneNode::Split {
            orientation: SplitOrientation::Vertical,
            children: vec![pane_leaf("a"), pane_leaf("b"), pane_leaf("c")],
            sizes: vec![1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0],
        },
        active_tab_id: "c".to_owned(),
        zoomed_tab_id: None,
    }];
    let shape = row(vec![pane(), pane()], None);
    let mut create_group_id = || "new".to_owned();
    let result = apply_template_to_groups(&groups, "c", &shape, &[], &mut create_group_id);
    assert_eq!(&serde_json::to_value(&result).unwrap(), expected);
}

#[test]
fn oracle_drops_a_template_with_an_empty_id() {
    let oracle = super::oracle_json();
    let template = json!([{
        "id": "",
        "name": "empty",
        "root": {
            "type": "split",
            "orientation": "horizontal",
            "sizes": [2, 2],
            "children": [
                {"type": "pane"},
                {
                    "type": "split",
                    "orientation": "vertical",
                    "children": [{"type": "pane"}, {"type": "pane"}],
                },
            ],
        },
    }]);
    assert_eq!(
        serde_json::to_value(normalize_pane_templates(&template)).unwrap(),
        oracle["paneTemplates"]["emptyIdTemplate"]
    );
}
