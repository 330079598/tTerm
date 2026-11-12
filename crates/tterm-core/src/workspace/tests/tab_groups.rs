use crate::workspace::tab_groups::{
    PaneNode, SplitDirection, SplitOrientation, TabGroup, break_out_pane, focus_pane_in_groups,
    get_fallback_tab_id, get_pane_ids, get_visible_pane_ids, insert_next_to, merge_group,
    move_group, normalize_pane_tree, pane_leaf, place_pane, reconcile_groups, remove_panes,
    remove_tabs_from_groups, resize_split, split_orientation, toggle_zoom, ungroup_panes,
};

use super::{ids, oracle_11d2};

fn pane(tab_id: &str) -> PaneNode {
    pane_leaf(tab_id)
}

fn split(orientation: SplitOrientation, children: Vec<PaneNode>, sizes: Vec<f64>) -> PaneNode {
    PaneNode::Split {
        orientation,
        children,
        sizes,
    }
}

fn row(children: Vec<PaneNode>) -> PaneNode {
    let count = children.len();
    split(
        SplitOrientation::Horizontal,
        children,
        vec![1.0 / count as f64; count],
    )
}

fn row_with_sizes(children: Vec<PaneNode>, sizes: Vec<f64>) -> PaneNode {
    split(SplitOrientation::Horizontal, children, sizes)
}

fn column(children: Vec<PaneNode>) -> PaneNode {
    let count = children.len();
    split(
        SplitOrientation::Vertical,
        children,
        vec![1.0 / count as f64; count],
    )
}

fn group(id: &str, root: PaneNode) -> TabGroup {
    let active_tab_id = get_pane_ids(&root).first().cloned().unwrap_or_default();
    group_with_active(id, root, &active_tab_id)
}

fn group_with_active(id: &str, root: PaneNode, active_tab_id: &str) -> TabGroup {
    TabGroup {
        id: id.to_string(),
        root,
        active_tab_id: active_tab_id.to_string(),
        zoomed_tab_id: None,
    }
}

fn counter() -> impl FnMut() -> String {
    let mut count = 0;
    move || {
        count += 1;
        format!("new-{count}")
    }
}

fn oracle_group_ids(groups: &[TabGroup]) -> Vec<serde_json::Value> {
    groups
        .iter()
        .map(|candidate| serde_json::json!([candidate.id, get_pane_ids(&candidate.root)]))
        .collect()
}

#[test]
fn splits_a_lone_pane_in_two() {
    assert_eq!(
        insert_next_to(&pane("a"), "a", &pane("b"), SplitDirection::Right),
        row(vec![pane("a"), pane("b")])
    );
    assert_eq!(
        insert_next_to(&pane("a"), "a", &pane("b"), SplitDirection::Above),
        column(vec![pane("b"), pane("a")])
    );
}

#[test]
fn oracle_inserts_into_splits_with_short_size_arrays() {
    let oracle = oracle_11d2();
    for case in oracle["shortSizeInsertions"].as_array().unwrap() {
        let result = insert_next_to(
            &super::tabs_reducer::pane_node(&case["root"]),
            case["target"].as_str().unwrap(),
            &super::tabs_reducer::pane_node(&case["inserted"]),
            super::tabs_reducer::direction(&case["direction"]),
        );
        assert_eq!(
            serde_json::to_value(result).unwrap(),
            case["result"],
            "{case:?}"
        );
    }
}

#[test]
fn joins_a_split_of_the_same_orientation_taking_half_of_the_targets_room() {
    let tree = insert_next_to(
        &row(vec![pane("a"), pane("b")]),
        "b",
        &pane("c"),
        SplitDirection::Right,
    );
    assert_eq!(
        tree,
        row_with_sizes(vec![pane("a"), pane("b"), pane("c")], vec![0.5, 0.25, 0.25])
    );
}

#[test]
fn nests_a_split_across_the_orientation() {
    let tree = insert_next_to(
        &row(vec![pane("a"), pane("b")]),
        "b",
        &pane("c"),
        SplitDirection::Below,
    );
    assert_eq!(
        tree,
        row(vec![pane("a"), column(vec![pane("b"), pane("c")])])
    );
}

#[test]
fn flattens_an_inserted_tree_that_runs_the_same_way() {
    let tree = insert_next_to(
        &pane("a"),
        "a",
        &row(vec![pane("b"), pane("c")]),
        SplitDirection::Right,
    );
    assert_eq!(
        tree,
        row_with_sizes(vec![pane("a"), pane("b"), pane("c")], vec![0.5, 0.25, 0.25])
    );
}

#[test]
fn gives_the_room_of_a_removed_pane_to_the_others_in_proportion() {
    let tree = remove_panes(
        &row_with_sizes(vec![pane("a"), pane("b"), pane("c")], vec![0.5, 0.25, 0.25]),
        &["c".to_string()],
    );
    assert_eq!(
        tree,
        Some(row_with_sizes(
            vec![pane("a"), pane("b")],
            vec![2.0 / 3.0, 1.0 / 3.0]
        ))
    );
}

#[test]
fn collapses_a_split_left_with_one_child_and_joins_it_to_its_parent() {
    let tree = row(vec![
        pane("a"),
        column(vec![pane("b"), row(vec![pane("c"), pane("d")])]),
    ]);
    assert_eq!(
        remove_panes(&tree, &["b".to_string()]),
        Some(row_with_sizes(
            vec![pane("a"), pane("c"), pane("d")],
            vec![0.5, 0.25, 0.25]
        ))
    );
}

#[test]
fn returns_null_once_no_pane_is_left() {
    assert_eq!(
        remove_panes(
            &row(vec![pane("a"), pane("b")]),
            &["a".to_string(), "b".to_string()]
        ),
        None
    );
}

#[test]
fn scales_sizes_that_do_not_sum_to_one() {
    assert_eq!(
        normalize_pane_tree(&row_with_sizes(vec![pane("a"), pane("b")], vec![3.0, 1.0])),
        row_with_sizes(vec![pane("a"), pane("b")], vec![0.75, 0.25])
    );
}

#[test]
fn shares_the_room_out_evenly_when_no_size_is_usable() {
    assert_eq!(
        normalize_pane_tree(&row_with_sizes(vec![pane("a"), pane("b")], vec![0.0, -1.0])),
        row_with_sizes(vec![pane("a"), pane("b")], vec![0.5, 0.5])
    );
}

#[test]
fn sets_the_sizes_of_the_split_at_the_path() {
    let tree = row(vec![pane("a"), column(vec![pane("b"), pane("c")])]);
    assert_eq!(
        resize_split(&tree, &[1], &[0.25, 0.75]),
        row(vec![
            pane("a"),
            split(
                SplitOrientation::Vertical,
                vec![pane("b"), pane("c")],
                vec![0.25, 0.75]
            )
        ])
    );
}

#[test]
fn ignores_sizes_that_do_not_match_the_split() {
    let tree = row(vec![pane("a"), pane("b")]);
    assert_eq!(resize_split(&tree, &[], &[1.0]), tree);
}

#[test]
fn drops_emptied_groups_and_moves_a_groups_focus_to_the_nearest_pane() {
    let groups = vec![
        group_with_active(
            "g1",
            row_with_sizes(
                vec![pane("a"), pane("b"), pane("c")],
                vec![1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0],
            ),
            "b",
        ),
        group("g2", pane("d")),
    ];
    let result = remove_tabs_from_groups(&groups, &["b".to_string(), "d".to_string()]);
    assert_eq!(
        result,
        vec![TabGroup {
            id: "g1".to_string(),
            root: row_with_sizes(vec![pane("a"), pane("c")], vec![0.5, 0.5]),
            active_tab_id: "c".to_string(),
            zoomed_tab_id: None,
        }]
    );
}

#[test]
fn ends_the_zoom_of_a_removed_pane() {
    let groups = vec![TabGroup {
        id: "g1".to_string(),
        root: row(vec![pane("a"), pane("b")]),
        active_tab_id: "a".to_string(),
        zoomed_tab_id: Some("b".to_string()),
    }];
    assert_eq!(
        remove_tabs_from_groups(&groups, &["b".to_string()])[0].zoomed_tab_id,
        None
    );
}

#[test]
fn returns_the_same_groups_when_none_holds_the_tabs() {
    let groups = vec![group("g1", pane("a"))];
    assert_eq!(remove_tabs_from_groups(&groups, &["x".to_string()]), groups);
}

fn fallback_groups() -> Vec<TabGroup> {
    vec![
        group("g1", pane("a")),
        group_with_active(
            "g2",
            row_with_sizes(vec![pane("b"), pane("c")], vec![0.5, 0.5]),
            "c",
        ),
        group("g3", pane("d")),
    ]
}

#[test]
fn stays_in_the_group_when_another_of_its_panes_is_left() {
    let before = fallback_groups();
    let after = remove_tabs_from_groups(&before, &["c".to_string()]);
    assert_eq!(
        get_fallback_tab_id(&before, &after, Some("c")),
        Some("b".to_string())
    );
}

#[test]
fn moves_to_the_group_that_takes_the_closed_groups_place() {
    let before = fallback_groups();
    let after = remove_tabs_from_groups(&before, &["b".to_string(), "c".to_string()]);
    assert_eq!(
        get_fallback_tab_id(&before, &after, Some("c")),
        Some("d".to_string())
    );
}

#[test]
fn moves_to_the_last_group_once_the_closed_one_was_last() {
    let before = fallback_groups();
    let after = remove_tabs_from_groups(&before, &["d".to_string()]);
    assert_eq!(
        get_fallback_tab_id(&before, &after, Some("d")),
        Some("c".to_string())
    );
}

#[test]
fn gives_tabs_no_group_holds_a_group_of_their_own_and_drops_panes_of_closed_tabs() {
    let mut create = counter();
    let result = reconcile_groups(
        &[group("g1", row(vec![pane("a"), pane("gone")]))],
        &["a".to_string(), "b".to_string()],
        &mut create,
    );
    assert_eq!(
        serde_json::to_value(oracle_group_ids(&result)).unwrap(),
        serde_json::json!([["g1", ["a"]], ["new-1", ["b"]]])
    );
}

#[test]
fn keeps_a_tab_listed_in_two_groups_in_only_one() {
    let mut create = counter();
    let result = reconcile_groups(
        &[group("g1", pane("a")), group("g2", pane("a"))],
        &["a".to_string()],
        &mut create,
    );
    assert_eq!(
        result
            .iter()
            .flat_map(|candidate| get_pane_ids(&candidate.root))
            .collect::<Vec<_>>(),
        vec!["a".to_string()]
    );
}

#[test]
fn returns_the_same_groups_when_they_already_match_the_tabs() {
    let groups = vec![group("g1", pane("a"))];
    let mut create = counter();
    assert_eq!(
        reconcile_groups(&groups, &["a".to_string()], &mut create),
        groups
    );
}

fn moving_groups() -> Vec<TabGroup> {
    vec![
        group("g1", row(vec![pane("a"), pane("b")])),
        group("g2", pane("c")),
    ]
}

#[test]
fn places_a_pane_beside_another_leaving_its_own_group_if_need_be() {
    let groups = moving_groups();
    let result = place_pane(&groups, "c", "a", SplitDirection::Below);
    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0].root,
        row(vec![column(vec![pane("a"), pane("c")]), pane("b")])
    );
    assert_eq!(result[0].active_tab_id, "c");
}

#[test]
fn merges_a_whole_group_beside_a_pane_of_another() {
    let groups = moving_groups();
    let result = merge_group(&groups, "c", "b", SplitDirection::Right);
    assert_eq!(
        result
            .iter()
            .map(|candidate| get_pane_ids(&candidate.root))
            .collect::<Vec<_>>(),
        vec![vec!["a".to_string(), "b".to_string(), "c".to_string()]]
    );
}

#[test]
fn does_not_merge_a_group_into_itself() {
    let groups = moving_groups();
    assert_eq!(
        merge_group(&groups, "a", "b", SplitDirection::Right),
        groups
    );
}

#[test]
fn breaks_a_pane_out_into_a_group_right_after_its_own_or_at_an_index() {
    let groups = moving_groups();
    assert_eq!(
        break_out_pane(&groups, "b", "g3", None)
            .iter()
            .map(|candidate| candidate.id.as_str())
            .collect::<Vec<_>>(),
        vec!["g1", "g3", "g2"]
    );
    assert_eq!(
        break_out_pane(&groups, "b", "g3", Some(0))
            .iter()
            .map(|candidate| candidate.id.as_str())
            .collect::<Vec<_>>(),
        vec!["g3", "g1", "g2"]
    );
    assert_eq!(break_out_pane(&groups, "c", "g3", None), groups);
}

#[test]
fn ungroups_a_split_into_one_group_per_pane_in_place() {
    let mut create = counter();
    let result = ungroup_panes(
        &[
            group("g0", pane("x")),
            group_with_active("g1", row(vec![pane("a"), pane("b")]), "b"),
        ],
        "a",
        &mut create,
    );
    assert_eq!(
        result
            .iter()
            .map(|candidate| (candidate.id.as_str(), get_pane_ids(&candidate.root)))
            .collect::<Vec<_>>(),
        vec![
            ("g0", vec!["x".to_string()]),
            ("new-1", vec!["a".to_string()]),
            ("g1", vec!["b".to_string()]),
        ]
    );
}

#[test]
fn reorders_groups() {
    let groups = moving_groups();
    assert_eq!(
        move_group(&groups, 1, 0)
            .iter()
            .map(|candidate| candidate.id.as_str())
            .collect::<Vec<_>>(),
        vec!["g2", "g1"]
    );
    assert_eq!(move_group(&groups, 0, 5), groups);
}

fn zoom_groups() -> Vec<TabGroup> {
    vec![group("g1", row(vec![pane("a"), pane("b")]))]
}

#[test]
fn zooms_a_pane_and_restores_it_on_the_second_toggle() {
    let groups = zoom_groups();
    let zoomed = toggle_zoom(&groups, "b");
    assert_eq!(zoomed[0].zoomed_tab_id.as_deref(), Some("b"));
    assert_eq!(zoomed[0].active_tab_id, "b");
    assert_eq!(get_visible_pane_ids(&zoomed, Some("b")), vec!["b"]);
    assert_eq!(toggle_zoom(&zoomed, "b")[0].zoomed_tab_id, None);
}

#[test]
fn has_nothing_to_zoom_in_a_lone_pane() {
    let lone = vec![group("g1", pane("a"))];
    assert_eq!(toggle_zoom(&lone, "a"), lone);
}

#[test]
fn ends_the_zoom_when_another_pane_of_the_group_is_focused() {
    let groups = zoom_groups();
    let zoomed = toggle_zoom(&groups, "b");
    let focused = focus_pane_in_groups(&zoomed, "a");
    assert_eq!(focused[0].active_tab_id, "a");
    assert_eq!(focused[0].zoomed_tab_id, None);
    assert_eq!(focus_pane_in_groups(&zoomed, "b"), zoomed);
}

#[test]
fn shows_every_pane_of_the_active_group_when_none_is_zoomed() {
    let groups = zoom_groups();
    assert_eq!(
        get_visible_pane_ids(&groups, Some("a")),
        vec!["a".to_string(), "b".to_string()]
    );
    assert_eq!(get_visible_pane_ids(&groups, None), Vec::<String>::new());
}

#[test]
fn oracle_treats_empty_ids_and_zooms_as_falsy() {
    let oracle = oracle_11d2();
    let groups = oracle["tabGroups"].clone();
    assert_eq!(
        serde_json::to_value(get_visible_pane_ids(
            &[TabGroup {
                id: "g1".to_string(),
                root: row(vec![pane(""), pane("b")]),
                active_tab_id: String::new(),
                zoomed_tab_id: None,
            }],
            Some("")
        ))
        .unwrap(),
        groups["visibleEmptyActive"]
    );
    assert_eq!(
        serde_json::to_value(get_visible_pane_ids(
            &[TabGroup {
                id: "g1".to_string(),
                root: row(vec![pane(""), pane("b")]),
                active_tab_id: String::new(),
                zoomed_tab_id: Some(String::new()),
            }],
            Some("")
        ))
        .unwrap(),
        groups["visibleEmptyZoom"]
    );
    assert_eq!(
        serde_json::to_value(get_fallback_tab_id(
            &fallback_groups(),
            &fallback_groups(),
            Some("")
        ))
        .unwrap(),
        groups["fallbackEmptyActive"]
    );
    assert_eq!(
        serde_json::to_value(get_fallback_tab_id(
            &fallback_groups(),
            &fallback_groups(),
            None
        ))
        .unwrap(),
        groups["fallbackNullActive"]
    );
    assert_eq!(
        serde_json::to_value(get_visible_pane_ids(
            &[TabGroup {
                id: "g1".to_string(),
                root: row(vec![pane("a"), pane("b")]),
                active_tab_id: "a".to_string(),
                zoomed_tab_id: Some(String::new()),
            }],
            Some("a")
        ))
        .unwrap(),
        groups["visibleEmptyZoomWithActive"]
    );
}

#[test]
fn oracle_matches_tree_and_group_removal_edges() {
    let oracle = oracle_11d2();
    let expected = &oracle["tabGroups"];
    let tree = row(vec![pane("a"), column(vec![pane("b"), pane("c")])]);
    assert_eq!(
        serde_json::to_value(get_pane_ids(&row(vec![
            pane("a"),
            column(vec![pane("b"), pane("a")]),
        ])))
        .unwrap(),
        expected["paneIds"]
    );
    assert_eq!(
        serde_json::to_value(get_pane_ids(&split(
            SplitOrientation::Horizontal,
            Vec::new(),
            Vec::new()
        )))
        .unwrap(),
        expected["paneIdsEmptySplit"]
    );
    assert_eq!(
        serde_json::to_value(insert_next_to(
            &row(vec![pane("a"), pane("b")]),
            "x",
            &pane("c"),
            SplitDirection::Left,
        ))
        .unwrap(),
        expected["insertMissingTarget"]
    );
    assert_eq!(
        serde_json::to_value(insert_next_to(
            &pane(""),
            "",
            &pane("x"),
            SplitDirection::Below,
        ))
        .unwrap(),
        expected["insertEmptyId"]
    );
    assert_eq!(
        serde_json::to_value(remove_panes(&tree, &[])).unwrap(),
        expected["removeNone"]
    );
    assert_eq!(
        serde_json::to_value(remove_panes(&tree, &ids(&["a", "b", "c"]))).unwrap(),
        expected["removeAll"]
    );
    assert_eq!(
        serde_json::to_value(remove_panes(
            &row(vec![pane("a"), pane("a"), pane("b")]),
            &ids(&["a"])
        ))
        .unwrap(),
        expected["removeDuplicate"]
    );
    assert_eq!(
        serde_json::to_value(resize_split(&tree, &[], &[1.0])).unwrap(),
        expected["resizeWrongCount"]
    );

    let fixture = vec![
        group_with_active("g1", row(vec![pane("a"), pane("b")]), "b"),
        group("g2", pane("c")),
    ];
    assert_eq!(
        serde_json::to_value(remove_tabs_from_groups(&fixture, &ids(&["x"]))).unwrap(),
        expected["removeGroupsMissing"]
    );
    assert_eq!(
        serde_json::to_value(remove_tabs_from_groups(
            &[TabGroup {
                id: "g1".to_string(),
                root: row(vec![pane("a"), pane("b")]),
                active_tab_id: "a".to_string(),
                zoomed_tab_id: Some(String::new()),
            }],
            &ids(&["b"]),
        ))
        .unwrap(),
        expected["removeGroupsEmptyZoom"]
    );
}

#[test]
fn oracle_matches_group_reconciliation_and_move_edges() {
    let oracle = oracle_11d2();
    let expected = &oracle["tabGroups"];
    let fixture = vec![
        group_with_active("g1", row(vec![pane("a"), pane("b")]), "b"),
        group("g2", pane("c")),
    ];
    assert_eq!(
        serde_json::to_value(reconcile_groups(
            &[group("g1", pane("a"))],
            &ids(&["a"]),
            &mut || "new".to_string(),
        ))
        .unwrap(),
        expected["atomicNoMissing"]
    );
    assert_eq!(
        serde_json::to_value(reconcile_groups(
            &[group("g1", row(vec![pane("a"), pane("gone")]))],
            &ids(&["a", "b"]),
            &mut || "new-1".to_string(),
        ))
        .unwrap(),
        expected["reconcileMissing"]
    );
    assert_eq!(
        serde_json::to_value(reconcile_groups(
            &[group("g1", pane("a")), group("g2", pane("a"))],
            &ids(&["a"]),
            &mut || "new".to_string(),
        ))
        .unwrap(),
        expected["reconcileDuplicate"]
    );
    assert_eq!(
        serde_json::to_value(focus_pane_in_groups(&fixture, "x")).unwrap(),
        expected["focusUnknown"]
    );
    assert_eq!(
        serde_json::to_value(place_pane(&fixture, "a", "a", SplitDirection::Right)).unwrap(),
        expected["placeSelf"]
    );
    assert_eq!(
        serde_json::to_value(place_pane(&fixture, "a", "x", SplitDirection::Right)).unwrap(),
        expected["placeMissing"]
    );
    assert_eq!(
        serde_json::to_value(merge_group(&fixture, "a", "b", SplitDirection::Right)).unwrap(),
        expected["mergeSame"]
    );
    assert_eq!(
        serde_json::to_value(merge_group(&fixture, "a", "x", SplitDirection::Right)).unwrap(),
        expected["mergeMissing"]
    );
    let moving = vec![
        group("g1", row(vec![pane("a"), pane("b")])),
        group("g2", pane("c")),
    ];
    let group_ids = |groups: &[TabGroup]| -> serde_json::Value {
        serde_json::to_value(
            groups
                .iter()
                .map(|group| group.id.as_str())
                .collect::<Vec<_>>(),
        )
        .unwrap()
    };
    assert_eq!(
        group_ids(&break_out_pane(&moving, "b", "g3", Some(-4))),
        expected["breakNegative"]
    );
    assert_eq!(
        group_ids(&break_out_pane(&moving, "b", "g3", Some(9))),
        expected["breakBeyond"]
    );
    assert_eq!(
        serde_json::to_value(break_out_pane(&[group("g1", pane("a"))], "a", "g2", None)).unwrap(),
        expected["breakLone"]
    );
    assert_eq!(
        serde_json::to_value(ungroup_panes(
            &[group_with_active(
                "g1",
                row(vec![pane("a"), pane("b")]),
                "x",
            )],
            "a",
            &mut || "new".to_string(),
        ))
        .unwrap(),
        expected["ungroupActiveMissing"]
    );
    assert_eq!(
        serde_json::to_value(toggle_zoom(
            &[group("g1", row(vec![pane(""), pane("b")]))],
            "",
        ))
        .unwrap(),
        expected["zoomEmptyId"]
    );
    assert_eq!(
        serde_json::to_value(get_visible_pane_ids(
            &[TabGroup {
                id: "g1".to_string(),
                root: row(vec![pane("a"), pane("b")]),
                active_tab_id: "a".to_string(),
                zoomed_tab_id: Some(String::new()),
            }],
            Some("a")
        ))
        .unwrap(),
        expected["visibleEmptyZoomWithActive"]
    );
}

#[test]
fn oracle_keeps_missing_paths_and_bounds_group_moves_like_typescript() {
    let oracle = oracle_11d2();
    let expected = &oracle["tabGroups"];
    let moving = vec![
        group_with_active("g1", row(vec![pane("a"), pane("b")]), "b"),
        group("g2", pane("c")),
    ];
    let tree = row(vec![pane("a"), column(vec![pane("b"), pane("c")])]);
    assert_eq!(
        serde_json::to_value(resize_split(&tree, &[9, 0], &[0.5, 0.5])).unwrap(),
        expected["resizeMissingPath"]
    );
    assert_eq!(
        serde_json::to_value(resize_split(&tree, &[-1], &[0.5, 0.5])).unwrap(),
        expected["resizeNegativePath"]
    );
    assert_eq!(
        serde_json::to_value(move_group(&moving, -1, 0)).unwrap(),
        expected["moveNegative"]
    );
    assert_eq!(
        serde_json::to_value(move_group(&moving, 0, 2)).unwrap(),
        expected["moveBeyond"]
    );
}

#[test]
fn oracle_matches_tree_helpers_and_all_split_directions() {
    let oracle = oracle_11d2();
    let expected = &oracle["tabGroups"];
    assert_eq!(
        serde_json::to_value(pane_leaf("a")).unwrap(),
        expected["paneLeaf"]
    );
    assert_eq!(
        serde_json::to_value(pane_leaf("")).unwrap(),
        expected["paneLeafEmpty"]
    );
    let orientations: serde_json::Value = [
        (SplitDirection::Left, "left"),
        (SplitDirection::Right, "right"),
        (SplitDirection::Above, "above"),
        (SplitDirection::Below, "below"),
    ]
    .into_iter()
    .zip(
        expected["splitOrientations"]
            .as_array()
            .expect("orientations"),
    )
    .map(|((direction, _), case)| {
        serde_json::json!({
            "direction": case["direction"],
            "orientation": serde_json::to_value(split_orientation(direction)).unwrap(),
        })
    })
    .collect();
    assert_eq!(orientations, expected["splitOrientations"]);
    let fixture = vec![
        group_with_active("g1", row(vec![pane("a"), pane("b")]), "b"),
        group("g2", pane("c")),
    ];
    assert_eq!(
        serde_json::to_value(
            crate::workspace::tab_groups::find_group_of_tab(&fixture, "b")
                .map(|group| group.id.as_str())
        )
        .unwrap(),
        expected["foundGroup"]
    );
    assert_eq!(
        serde_json::to_value(crate::workspace::tab_groups::find_group_of_tab(
            &fixture, "x"
        ))
        .unwrap(),
        expected["missingGroup"]
    );
    assert_eq!(
        serde_json::to_value(
            crate::workspace::tab_groups::find_group_of_tab(&[group("g1", pane(""))], "")
                .map(|group| group.id.as_str())
        )
        .unwrap(),
        expected["emptyIdGroup"]
    );
    assert_eq!(
        serde_json::to_value(normalize_pane_tree(&split(
            SplitOrientation::Horizontal,
            Vec::new(),
            Vec::new()
        )))
        .unwrap(),
        expected["normalizeEmptySplit"]
    );
}
