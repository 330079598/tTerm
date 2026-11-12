use crate::workspace::tab_navigation::{
    TabShortcutTarget, compare_tab_ids, get_sibling_tab_id, get_tab_id_at_position,
    resolve_tab_shortcut_target,
};

use super::ids;

#[test]
fn moves_forward_and_wraps_around() {
    let tabs = ids(&["t1", "t2", "t3"]);
    assert_eq!(
        get_sibling_tab_id(&tabs, Some("t1"), 1),
        Some("t2".to_owned())
    );
    assert_eq!(
        get_sibling_tab_id(&tabs, Some("t3"), 1),
        Some("t1".to_owned())
    );
}

#[test]
fn moves_backward_and_wraps_around() {
    let tabs = ids(&["t1", "t2", "t3"]);
    assert_eq!(
        get_sibling_tab_id(&tabs, Some("t2"), -1),
        Some("t1".to_owned())
    );
    assert_eq!(
        get_sibling_tab_id(&tabs, Some("t1"), -1),
        Some("t3".to_owned())
    );
}

#[test]
fn stays_put_when_there_is_a_single_tab() {
    let tabs = ids(&["t1"]);
    assert_eq!(
        get_sibling_tab_id(&tabs, Some("t1"), 1),
        Some("t1".to_owned())
    );
    assert_eq!(
        get_sibling_tab_id(&tabs, Some("t1"), -1),
        Some("t1".to_owned())
    );
}

#[test]
fn returns_none_without_an_active_tab_or_an_unknown_id() {
    let tabs = ids(&["t1", "t2", "t3"]);
    assert_eq!(get_sibling_tab_id(&tabs, None, 1), None);
    assert_eq!(get_sibling_tab_id(&tabs, Some("missing"), 1), None);
    assert_eq!(get_sibling_tab_id(&[], Some("t1"), 1), None);
}

#[test]
fn returns_one_based_positions() {
    let tabs = ids(&["t1", "t2", "t3"]);
    assert_eq!(get_tab_id_at_position(&tabs, 1), Some("t1".to_owned()));
    assert_eq!(get_tab_id_at_position(&tabs, 3), Some("t3".to_owned()));
}

#[test]
fn returns_none_for_out_of_range_positions() {
    let tabs = ids(&["t1", "t2", "t3"]);
    assert_eq!(get_tab_id_at_position(&tabs, 0), None);
    assert_eq!(get_tab_id_at_position(&tabs, 4), None);
}

#[test]
fn returns_the_last_tab_for_position_nine() {
    let tabs = ids(&["t1", "t2", "t3"]);
    assert_eq!(get_tab_id_at_position(&tabs, 9), Some("t3".to_owned()));
    let many: Vec<String> = (1..=12).map(|index| format!("t{index}")).collect();
    assert_eq!(get_tab_id_at_position(&many, 9), Some("t12".to_owned()));
    assert_eq!(get_tab_id_at_position(&many, 8), Some("t8".to_owned()));
    assert_eq!(get_tab_id_at_position(&[], 9), None);
}

#[test]
fn switches_to_a_tab_other_than_the_active_one() {
    assert_eq!(
        resolve_tab_shortcut_target(Some("t2"), Some("t1")),
        TabShortcutTarget::Switch("t2".to_owned())
    );
}

#[test]
fn reports_the_active_tab_as_the_current_target() {
    assert_eq!(
        resolve_tab_shortcut_target(Some("t1"), Some("t1")),
        TabShortcutTarget::Current("t1".to_owned())
    );
}

#[test]
fn reports_no_target_when_the_shortcut_found_no_tab() {
    assert_eq!(
        resolve_tab_shortcut_target(None, Some("t1")),
        TabShortcutTarget::None
    );
}

#[test]
fn orders_tabs_as_they_were_opened_by_number_rather_than_by_text() {
    let mut tabs = ids(&["tab-10", "tab-2", "tab-1"]);
    tabs.sort_by(|first, second| {
        compare_tab_ids(first, second)
            .partial_cmp(&0.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    assert_eq!(tabs, ids(&["tab-1", "tab-2", "tab-10"]));
}

#[test]
fn falls_back_to_the_text_for_ids_that_are_not_numbered() {
    let mut tabs = ids(&["b", "tab-1", "a"]);
    tabs.sort_by(|first, second| {
        compare_tab_ids(first, second)
            .partial_cmp(&0.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    assert_eq!(tabs, ids(&["a", "b", "tab-1"]));
}

#[test]
fn oracle_orders_leading_zero_ids_by_number() {
    let mut tabs = ids(&["tab-10", "tab-001", "tab-2"]);
    tabs.sort_by(|first, second| {
        compare_tab_ids(first, second)
            .partial_cmp(&0.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    assert_eq!(tabs, ids(&["tab-001", "tab-2", "tab-10"]));
    assert_eq!(compare_tab_ids("tab-001", "tab-1"), 0.0);
}

#[test]
fn oracle_parses_long_digit_ids_as_floating_point_numbers() {
    assert_eq!(compare_tab_ids("tab-999999999999999999999", "tab-2"), 1e21);
    assert_eq!(
        compare_tab_ids("tab-999999999999999999999", "tab-999999999999999999998"),
        0.0
    );
}

#[test]
fn oracle_treats_an_empty_active_tab_id_as_absent() {
    assert_eq!(get_sibling_tab_id(&ids(&["t1"]), Some(""), 1), None);
    assert_eq!(
        resolve_tab_shortcut_target(Some(""), Some("t1")),
        TabShortcutTarget::None
    );
}

#[test]
fn oracle_returns_none_for_an_offset_beyond_the_negative_end_instead_of_throwing() {
    // The TypeScript function indexes the array with a negative number here
    // and throws a TypeError; the Rust port returns None because production
    // code must not panic. The deviation is recorded in the card report.
    assert_eq!(
        get_sibling_tab_id(&ids(&["t1", "t2", "t3"]), Some("t1"), -4),
        None
    );
}
