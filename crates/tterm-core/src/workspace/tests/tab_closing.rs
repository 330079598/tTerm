use crate::workspace::tab::TabContextMenuAction;
use crate::workspace::tab_closing::{
    TabCloseAction, TabCloseMenuLabels, get_tab_close_context, get_tab_close_menu_actions,
    get_tab_ids_for_close_action,
};

use super::ids;

fn labels() -> TabCloseMenuLabels {
    TabCloseMenuLabels {
        close_tab: "Close".to_owned(),
        close_other_tabs: "Close others".to_owned(),
        close_tabs_to_left: "Close left".to_owned(),
        close_tabs_to_right: "Close right".to_owned(),
    }
}

fn find_action<'a>(
    actions: &'a [TabContextMenuAction],
    name: &str,
) -> Option<&'a TabContextMenuAction> {
    actions.iter().find(|action| action.action == name)
}

#[test]
fn calculates_close_targets_from_the_supplied_group_order() {
    let group_tab_ids = ids(&["group-b", "group-d", "group-a"]);
    assert_eq!(
        get_tab_ids_for_close_action(&group_tab_ids, "group-d", TabCloseAction::CloseLeft),
        ids(&["group-b"])
    );
    assert_eq!(
        get_tab_ids_for_close_action(&group_tab_ids, "group-d", TabCloseAction::CloseRight),
        ids(&["group-a"])
    );
    assert_eq!(
        get_tab_ids_for_close_action(&group_tab_ids, "group-d", TabCloseAction::CloseOthers),
        ids(&["group-b", "group-a"])
    );
}

#[test]
fn does_not_include_tabs_from_another_group() {
    let group_tab_ids = ids(&["a", "c"]);
    assert_eq!(
        get_tab_ids_for_close_action(&group_tab_ids, "a", TabCloseAction::CloseRight),
        ids(&["c"])
    );
    assert!(
        !get_tab_ids_for_close_action(&group_tab_ids, "a", TabCloseAction::CloseOthers)
            .contains(&"b".to_owned())
    );
}

#[test]
fn reports_empty_sides_so_invalid_menu_actions_can_be_disabled() {
    let first = get_tab_close_context(&ids(&["a", "b"]), "a");
    let last = get_tab_close_context(&ids(&["a", "b"]), "b");
    let only = get_tab_close_context(&ids(&["a"]), "a");
    assert_eq!(first.unwrap().left_ids, Vec::<String>::new());
    assert_eq!(last.unwrap().right_ids, Vec::<String>::new());
    assert_eq!(only.unwrap().other_ids, Vec::<String>::new());
}

#[test]
fn builds_consistent_menu_actions_from_the_group_context() {
    let actions = get_tab_close_menu_actions(&ids(&["a", "b"]), "a", &labels(), false);
    assert_eq!(
        actions
            .iter()
            .map(|action| action.action.as_str())
            .collect::<Vec<_>>(),
        vec!["close-tab", "close-others", "close-left", "close-right"]
    );
    assert_eq!(
        find_action(&actions, "close-left").unwrap().disabled,
        Some(true)
    );
    assert_eq!(
        find_action(&actions, "close-right").unwrap().disabled,
        Some(false)
    );
}

#[test]
fn shows_the_close_shortcut_only_for_a_tab_with_one_pane_which_it_closes() {
    let shortcut = |is_split| {
        get_tab_close_menu_actions(&ids(&["a"]), "a", &labels(), is_split)
            .iter()
            .find(|action| action.action == "close-tab")
            .unwrap()
            .shortcut
    };
    assert_eq!(
        shortcut(false).map(|id| id.as_str()),
        Some("tabs.closeActive")
    );
    assert_eq!(shortcut(true), None);
}

#[test]
fn oracle_returns_empty_and_disabled_when_the_tab_is_unknown() {
    assert_eq!(
        get_tab_ids_for_close_action(&ids(&["a"]), "missing", TabCloseAction::CloseOthers),
        Vec::<String>::new()
    );
    assert_eq!(get_tab_close_context(&ids(&["a"]), "missing"), None);
    let actions = get_tab_close_menu_actions(&ids(&["a"]), "missing", &labels(), false);
    for action in &actions[1..] {
        assert_eq!(action.disabled, Some(true));
    }
    assert_eq!(
        actions[0].shortcut.map(|id| id.as_str()),
        Some("tabs.closeActive")
    );
}
