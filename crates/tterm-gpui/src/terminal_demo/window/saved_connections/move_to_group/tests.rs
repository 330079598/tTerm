//! Regressions for the checked set a group move sends.

use std::collections::HashSet;

use super::MoveSelection;

#[test]
fn toggling_rows_adds_and_removes_them_in_a_stable_order() {
    let mut selection = MoveSelection::default();

    assert!(selection.toggle("web"));
    assert!(selection.toggle("api"));
    assert_eq!(selection.checked_ids(), ["api", "web"]);
    assert!(selection.is_checked("web"));

    assert!(!selection.toggle("web"));
    assert_eq!(selection.checked_ids(), ["api"]);
    assert!(!selection.is_checked("web"));
}

#[test]
fn a_reload_forgets_ids_that_are_no_longer_saved() {
    let mut selection = MoveSelection::default();
    assert!(selection.toggle("kept"));
    assert!(selection.toggle("deleted"));

    let existing: HashSet<&str> = ["kept"].into_iter().collect();
    selection.retain_existing(&existing);

    assert_eq!(selection.checked_ids(), ["kept"]);
}
