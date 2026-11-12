use crate::workspace::pane_navigation::{
    PaneDirection, PaneRect, find_pane_in_direction, get_sibling_pane_id,
    sort_panes_by_reading_order,
};

fn stress_pane(id: &str) -> PaneRect {
    match id {
        "L1" => pane("L1", 0.0, 0.0, 496.0, 200.0),
        "L2" => pane("L2", 0.0, 209.0, 496.0, 600.0),
        "M1" => pane("M1", 504.0, 0.0, 1000.0, 200.0),
        "M2" => pane("M2", 504.0, 207.0, 1000.0, 600.0),
        "R1" => pane("R1", 1008.0, 0.0, 1504.0, 200.0),
        "R2" => pane("R2", 1008.0, 205.0, 1504.0, 600.0),
        other => panic!("unexpected stress pane {other}"),
    }
}

fn pane(id: &str, left: f64, top: f64, right: f64, bottom: f64) -> PaneRect {
    PaneRect {
        id: id.to_owned(),
        left,
        top,
        right,
        bottom,
    }
}

fn tall_left() -> Vec<PaneRect> {
    vec![
        pane("c", 508.0, 304.0, 1000.0, 600.0),
        pane("a", 0.0, 0.0, 500.0, 600.0),
        pane("b", 508.0, 0.0, 1000.0, 296.0),
    ]
}

fn recent(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn sorts_panes_top_to_bottom_then_left_to_right() {
    assert_eq!(
        sort_panes_by_reading_order(&tall_left())
            .iter()
            .map(|pane| pane.id.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "b", "c"]
    );
}

#[test]
fn treats_tops_a_pixel_apart_as_one_row() {
    let panes = vec![
        pane("right", 504.0, 0.0, 1000.0, 600.0),
        pane("left", 0.0, 1.0, 496.0, 600.0),
    ];
    assert_eq!(
        sort_panes_by_reading_order(&panes)
            .iter()
            .map(|pane| pane.id.as_str())
            .collect::<Vec<_>>(),
        vec!["left", "right"]
    );
}

#[test]
fn leaves_out_panes_hidden_behind_a_maximized_pane() {
    let panes = vec![
        pane("shown", 0.0, 0.0, 1000.0, 600.0),
        pane("hidden", 0.0, 0.0, 0.0, 0.0),
    ];
    assert_eq!(
        sort_panes_by_reading_order(&panes)
            .iter()
            .map(|pane| pane.id.as_str())
            .collect::<Vec<_>>(),
        vec!["shown"]
    );
}

#[test]
fn steps_through_panes_in_reading_order_and_wraps_around() {
    let panes = tall_left();
    assert_eq!(
        get_sibling_pane_id(&panes, Some("a"), 1),
        Some("b".to_owned())
    );
    assert_eq!(
        get_sibling_pane_id(&panes, Some("c"), 1),
        Some("a".to_owned())
    );
    assert_eq!(
        get_sibling_pane_id(&panes, Some("a"), -1),
        Some("c".to_owned())
    );
}

#[test]
fn finds_no_sibling_with_a_single_pane_in_sight() {
    assert_eq!(
        get_sibling_pane_id(&[pane("only", 0.0, 0.0, 100.0, 100.0)], Some("only"), 1),
        None
    );
}

#[test]
fn finds_no_sibling_when_the_active_pane_is_unknown() {
    let panes = tall_left();
    assert_eq!(get_sibling_pane_id(&panes, Some("missing"), 1), None);
    assert_eq!(get_sibling_pane_id(&panes, None, 1), None);
}

#[test]
fn moves_to_the_pane_beside_the_active_one() {
    let panes = tall_left();
    assert_eq!(
        find_pane_in_direction(&panes, Some("b"), PaneDirection::Left, &[]),
        Some("a".to_owned())
    );
    assert_eq!(
        find_pane_in_direction(&panes, Some("b"), PaneDirection::Down, &[]),
        Some("c".to_owned())
    );
    assert_eq!(
        find_pane_in_direction(&panes, Some("c"), PaneDirection::Up, &[]),
        Some("b".to_owned())
    );
}

#[test]
fn does_not_wrap_at_the_edge_of_the_workspace() {
    let panes = tall_left();
    assert_eq!(
        find_pane_in_direction(&panes, Some("a"), PaneDirection::Left, &[]),
        None
    );
    assert_eq!(
        find_pane_in_direction(&panes, Some("b"), PaneDirection::Up, &[]),
        None
    );
}

#[test]
fn skips_panes_that_share_no_edge_with_the_active_one() {
    assert_eq!(
        find_pane_in_direction(&tall_left(), Some("a"), PaneDirection::Down, &[]),
        None
    );
}

#[test]
fn prefers_the_most_recently_used_pane_when_neighbours_share_the_edge_equally() {
    let panes = tall_left();
    assert_eq!(
        find_pane_in_direction(
            &panes,
            Some("a"),
            PaneDirection::Right,
            &recent(&["c", "a", "b"])
        ),
        Some("c".to_owned())
    );
    assert_eq!(
        find_pane_in_direction(
            &panes,
            Some("a"),
            PaneDirection::Right,
            &recent(&["b", "c"])
        ),
        Some("b".to_owned())
    );
}

#[test]
fn prefers_the_neighbour_sharing_more_of_the_edge_over_a_more_recent_one() {
    let panes = vec![
        pane("a", 0.0, 0.0, 500.0, 600.0),
        pane("big", 508.0, 0.0, 1000.0, 400.0),
        pane("small", 508.0, 408.0, 1000.0, 600.0),
    ];
    assert_eq!(
        find_pane_in_direction(&panes, Some("a"), PaneDirection::Right, &recent(&["small"])),
        Some("big".to_owned())
    );
}

#[test]
fn prefers_the_nearest_pane_over_one_further_along() {
    let panes = vec![
        pane("a", 0.0, 0.0, 300.0, 600.0),
        pane("near", 308.0, 0.0, 600.0, 600.0),
        pane("far", 608.0, 0.0, 1000.0, 600.0),
    ];
    assert_eq!(
        find_pane_in_direction(&panes, Some("a"), PaneDirection::Right, &recent(&["far"])),
        Some("near".to_owned())
    );
}

#[test]
fn ignores_panes_hidden_behind_a_maximized_pane() {
    let panes = vec![
        pane("a", 0.0, 0.0, 1000.0, 600.0),
        pane("hidden", 0.0, 0.0, 0.0, 0.0),
    ];
    assert_eq!(
        find_pane_in_direction(&panes, Some("a"), PaneDirection::Right, &[]),
        None
    );
}

#[test]
fn oracle_treats_tops_exactly_two_apart_as_one_row_but_not_three() {
    let exactly_two = vec![
        pane("right", 504.0, 0.0, 1000.0, 600.0),
        pane("left", 0.0, 2.0, 496.0, 600.0),
    ];
    assert_eq!(
        sort_panes_by_reading_order(&exactly_two)
            .iter()
            .map(|pane| pane.id.as_str())
            .collect::<Vec<_>>(),
        vec!["left", "right"]
    );
    let more_than_two = vec![
        pane("right", 504.0, 0.0, 1000.0, 600.0),
        pane("left", 0.0, 3.0, 496.0, 600.0),
    ];
    assert_eq!(
        sort_panes_by_reading_order(&more_than_two)
            .iter()
            .map(|pane| pane.id.as_str())
            .collect::<Vec<_>>(),
        vec!["right", "left"]
    );
}

#[test]
fn oracle_accepts_a_distance_of_exactly_negative_two_but_not_beyond() {
    let exactly = vec![
        pane("a", 0.0, 0.0, 50.0, 50.0),
        pane("b", 48.0, 0.0, 100.0, 50.0),
    ];
    assert_eq!(
        find_pane_in_direction(&exactly, Some("a"), PaneDirection::Right, &[]),
        Some("b".to_owned())
    );
    let beyond = vec![
        pane("a", 0.0, 0.0, 50.0, 50.0),
        pane("b", 47.0, 0.0, 100.0, 50.0),
    ];
    assert_eq!(
        find_pane_in_direction(&beyond, Some("a"), PaneDirection::Right, &[]),
        None
    );
}

#[test]
fn oracle_keeps_the_input_order_when_neighbours_are_equally_recent() {
    assert_eq!(
        find_pane_in_direction(&tall_left(), Some("a"), PaneDirection::Right, &[]),
        Some("c".to_owned())
    );
}

#[test]
fn oracle_treats_an_empty_active_id_as_absent() {
    assert_eq!(
        find_pane_in_direction(&tall_left(), Some(""), PaneDirection::Right, &[]),
        None
    );
    assert_eq!(get_sibling_pane_id(&tall_left(), Some(""), 1), None);
}

#[test]
fn oracle_returns_none_for_an_offset_beyond_the_negative_end_instead_of_throwing() {
    // The TypeScript function indexes the array with a negative number here
    // and throws a TypeError; the Rust port returns None because production
    // code must not panic. The deviation is recorded in the card report.
    assert_eq!(get_sibling_pane_id(&tall_left(), Some("a"), -4), None);
}

fn sorted_ids(panes: &[PaneRect]) -> Vec<&str> {
    panes.iter().map(|pane| pane.id.as_str()).collect()
}

#[test]
fn oracle_breaks_the_reading_order_cycle_like_v8() {
    let oracle = super::oracle_json();
    let example = &oracle["paneNavigation"]["readingOrder"]["example"];
    let input: Vec<PaneRect> = example["input"]
        .as_array()
        .expect("the example has an input list")
        .iter()
        .map(|id| stress_pane(id.as_str().expect("an input id")))
        .collect();
    let expected: Vec<&str> = example["sorted"]
        .as_array()
        .expect("the example has a sorted list")
        .iter()
        .map(|id| id.as_str().expect("a sorted id"))
        .collect();

    assert_eq!(sorted_ids(&sort_panes_by_reading_order(&input)), expected);
    assert_eq!(
        get_sibling_pane_id(&input, Some("R1"), 1).as_deref(),
        example["sibling"].as_str()
    );
}

#[test]
fn oracle_matches_v8_for_every_permutation_of_the_reading_order_stress_case() {
    let oracle = super::oracle_json();
    let permutations = oracle["paneNavigation"]["readingOrder"]["permutations"]
        .as_array()
        .expect("the oracle has the permutation results");
    assert_eq!(permutations.len(), 720);

    for permutation in permutations {
        let input: Vec<PaneRect> = permutation["input"]
            .as_array()
            .expect("an input list")
            .iter()
            .map(|id| stress_pane(id.as_str().expect("an input id")))
            .collect();
        let expected_sorted: Vec<&str> = permutation["sorted"]
            .as_array()
            .expect("a sorted list")
            .iter()
            .map(|id| id.as_str().expect("a sorted id"))
            .collect();
        let actual = sort_panes_by_reading_order(&input);
        assert_eq!(
            sorted_ids(&actual),
            expected_sorted,
            "input order: {:?}",
            permutation["input"]
        );
        assert_eq!(
            get_sibling_pane_id(&input, Some("R1"), 1).as_deref(),
            permutation["sibling"].as_str(),
            "sibling after input order: {:?}",
            permutation["input"]
        );
    }
}

#[test]
fn oracle_sorts_seventy_panes_with_a_non_transitive_comparator_without_panicking() {
    let panes: Vec<PaneRect> = (0..70)
        .map(|index| {
            let group = index % 3;
            let left = ((2 - group) * 1000 + index) as f64;
            let top = (group * 2) as f64;
            pane(&format!("p{index}"), left, top, left + 100.0, top + 100.0)
        })
        .collect();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        sort_panes_by_reading_order(&panes)
    }));
    let Ok(sorted) = result else {
        panic!("sorting 70 panes with a non-transitive comparator panicked");
    };
    assert_eq!(sorted.len(), 70);

    let mut actual: Vec<&str> = sorted.iter().map(|pane| pane.id.as_str()).collect();
    actual.sort_unstable();
    let mut expected: Vec<String> = (0..70).map(|index| format!("p{index}")).collect();
    expected.sort_unstable();
    assert_eq!(
        actual,
        expected.iter().map(String::as_str).collect::<Vec<_>>()
    );
}
