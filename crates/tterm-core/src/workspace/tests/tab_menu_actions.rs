use crate::workspace::tab::{Tab, TabConnection, TabContextMenuAction, TabType};
use crate::workspace::tab_menu_actions::{
    get_layout_templates_menu_action, get_pane_menu_actions, get_reopen_closed_tab_menu_action,
    get_tab_connection_menu_action, get_tab_log_menu_actions, get_tab_pin_menu_action,
    get_tab_rename_menu_action, get_tab_split_menu_actions, get_ungroup_menu_action,
    tab_connection_action_label,
};

use super::tab;

fn translate(key: &str, _default: Option<&str>) -> String {
    key.to_owned()
}

fn labels(tab: &Tab) -> Vec<String> {
    get_tab_connection_menu_action(tab, &translate)
        .into_iter()
        .map(|item| item.label)
        .collect()
}

fn find_action<'a>(
    actions: &'a [TabContextMenuAction],
    name: &str,
) -> Option<&'a TabContextMenuAction> {
    actions.iter().find(|action| action.action == name)
}

fn connection(profile_id: Option<&str>) -> TabConnection {
    let mut connection = TabConnection::default();
    connection.set_profile_id(profile_id.map(str::to_owned));
    connection
}

fn assert_actions_match_oracle(actions: &[TabContextMenuAction], expected: &serde_json::Value) {
    let expected = expected.as_array().expect("an action list");
    assert_eq!(actions.len(), expected.len());
    for (action, item) in actions.iter().zip(expected) {
        assert_eq!(action.label, item["label"].as_str().expect("a label"));
        assert_eq!(action.action, item["action"].as_str().expect("an action"));
        assert_eq!(action.icon.as_deref(), item["icon"].as_str());
        assert_eq!(
            action.shortcut.map(|shortcut| shortcut.as_str()),
            item["shortcut"].as_str()
        );
        assert_eq!(
            action.separator,
            item["separator"].as_bool().unwrap_or(false)
        );
        assert_eq!(action.disabled, item["disabled"].as_bool());
    }
}

#[test]
fn offers_the_four_splits_for_terminal_tabs_only() {
    let ssh = get_tab_split_menu_actions(&tab("a", "a", TabType::Ssh), &translate);
    assert_eq!(
        ssh.iter()
            .map(|item| item.action.as_str())
            .collect::<Vec<_>>(),
        vec!["split-right", "split-left", "split-down", "split-above"]
    );
    assert_eq!(
        get_tab_split_menu_actions(&tab("a", "a", TabType::Terminal), &translate).len(),
        4
    );
    assert_eq!(
        get_tab_split_menu_actions(&tab("a", "a", TabType::Settings), &translate),
        Vec::new()
    );
}

#[test]
fn pins_or_unpins_an_ssh_tabs_connection_header() {
    let mut pinned = tab("a", "a", TabType::Ssh);
    pinned.set_connection_header_pinned(Some(false));
    assert_eq!(
        get_tab_pin_menu_action(&pinned, &translate)
            .iter()
            .map(|item| item.action.as_str())
            .collect::<Vec<_>>(),
        vec!["pin-header"]
    );
    assert_eq!(
        get_tab_pin_menu_action(&tab("a", "a", TabType::Ssh), &translate)
            .iter()
            .map(|item| item.action.as_str())
            .collect::<Vec<_>>(),
        vec!["unpin-header"]
    );
    assert_eq!(
        get_tab_pin_menu_action(&tab("a", "a", TabType::Terminal), &translate),
        Vec::new()
    );
}

#[test]
fn offers_a_page_only_the_panes_layout_and_closing() {
    let page = get_pane_menu_actions(
        &tab("a", "Settings", TabType::Settings),
        false,
        &translate,
        false,
    );
    assert_eq!(
        page.iter()
            .map(|item| item.action.as_str())
            .collect::<Vec<_>>(),
        vec![
            "toggle-zoom",
            "move-to-new-tab",
            "ungroup",
            "layout-templates",
            "",
            "close",
        ]
    );
}

#[test]
fn offers_a_terminal_its_copies_splits_renaming_and_the_panes_layout() {
    let actions = get_pane_menu_actions(
        &tab("a", "tTerm", TabType::Terminal),
        false,
        &translate,
        false,
    )
    .into_iter()
    .map(|item| item.action)
    .collect::<Vec<_>>();
    for expected in [
        "duplicate",
        "split-right",
        "rename",
        "toggle-zoom",
        "move-to-new-tab",
        "ungroup",
        "close",
    ] {
        assert!(actions.contains(&expected.to_owned()), "{expected}");
    }
}

#[test]
fn restores_a_zoomed_pane_instead_of_zooming_it() {
    let tab = tab("a", "tTerm", TabType::Terminal);
    let zoom = |is_zoomed| {
        find_action(
            &get_pane_menu_actions(&tab, is_zoomed, &translate, false),
            "toggle-zoom",
        )
        .unwrap()
        .label
        .clone()
    };
    assert_eq!(zoom(false), "workspace.zoomPane");
    assert_eq!(zoom(true), "workspace.unzoomPane");
}

#[test]
fn shows_the_duplicate_shortcut_beside_the_copy_item() {
    let actions = get_pane_menu_actions(
        &tab("a", "tTerm", TabType::Terminal),
        false,
        &translate,
        false,
    );
    assert_eq!(
        find_action(&actions, "duplicate")
            .unwrap()
            .shortcut
            .map(|id| id.as_str()),
        Some("tabs.duplicateActive")
    );
}

#[test]
fn edits_the_profile_a_session_tab_was_opened_from() {
    for tab_type in [TabType::Ssh, TabType::Serial, TabType::Terminal] {
        let mut candidate = tab("a", "tab", tab_type);
        candidate.set_connection(Some(connection(Some("p"))));
        let actions = get_pane_menu_actions(&candidate, false, &translate, false)
            .into_iter()
            .map(|item| item.action)
            .collect::<Vec<_>>();
        assert!(actions.contains(&"edit-connection".to_owned()));
    }
    let actions =
        get_pane_menu_actions(&tab("a", "tab", TabType::Serial), false, &translate, false)
            .into_iter()
            .map(|item| item.action)
            .collect::<Vec<_>>();
    assert!(!actions.contains(&"edit-connection".to_owned()));
}

#[test]
fn saves_an_ssh_or_local_tab_that_has_no_profile_and_edits_one_that_has() {
    assert_eq!(
        labels(&tab("a", "a", TabType::Ssh)),
        vec!["contextMenu.saveConnection"]
    );
    assert_eq!(
        labels(&tab("a", "a", TabType::Terminal)),
        vec!["contextMenu.saveLocalConnection"]
    );
    let mut local = tab("a", "a", TabType::Terminal);
    local.set_connection(Some(connection(Some("p"))));
    assert_eq!(labels(&local), vec!["contextMenu.editConnection"]);
}

#[test]
fn shows_the_shortcut_which_acts_only_where_the_item_is_offered() {
    assert_eq!(
        get_tab_connection_menu_action(&tab("a", "a", TabType::Terminal), &translate)[0]
            .shortcut
            .map(|id| id.as_str()),
        Some("tabs.saveConnection")
    );
    assert_eq!(
        tab_connection_action_label(&tab("a", "a", TabType::Terminal)),
        Some("contextMenu.saveLocalConnection")
    );
    assert_eq!(
        tab_connection_action_label(&tab("a", "a", TabType::Serial)),
        None
    );
    assert_eq!(
        tab_connection_action_label(&tab("a", "a", TabType::Settings)),
        None
    );
}

#[test]
fn offers_nothing_for_a_serial_tab_without_a_profile_or_a_remote_file() {
    assert_eq!(
        labels(&tab("a", "a", TabType::Serial)),
        Vec::<String>::new()
    );
    let mut remote = tab("a", "a", TabType::RemoteFileEditor);
    remote.set_connection(Some(connection(Some("p"))));
    assert_eq!(labels(&remote), Vec::<String>::new());
}

#[test]
fn edits_a_terminal_tab_whose_icon_can_be_chosen_and_renames_others() {
    assert_eq!(
        get_tab_rename_menu_action(&tab("a", "a", TabType::Terminal), &translate).label,
        "contextMenu.editTab"
    );
    assert_eq!(
        get_tab_rename_menu_action(&tab("a", "a", TabType::Ssh), &translate).label,
        "contextMenu.editTab"
    );
    assert_eq!(
        get_tab_rename_menu_action(&tab("a", "a", TabType::RemoteFileEditor), &translate).label,
        "contextMenu.renameTab"
    );
}

#[test]
fn oracle_offers_splits_to_a_serial_tab() {
    assert_eq!(
        get_tab_split_menu_actions(&tab("a", "a", TabType::Serial), &translate)
            .iter()
            .map(|item| item.action.as_str())
            .collect::<Vec<_>>(),
        vec!["split-right", "split-left", "split-down", "split-above"]
    );
}

#[test]
fn oracle_pins_an_ssh_tab_with_a_missing_or_true_flag() {
    assert_eq!(
        get_tab_pin_menu_action(&tab("a", "a", TabType::Ssh), &translate)[0].action,
        "unpin-header"
    );
    let mut pinned = tab("a", "a", TabType::Ssh);
    pinned.set_connection_header_pinned(Some(true));
    assert_eq!(
        get_tab_pin_menu_action(&pinned, &translate)[0].action,
        "unpin-header"
    );
}

#[test]
fn oracle_edits_a_serial_profile_but_saves_a_local_tab_with_an_empty_profile() {
    let mut serial = tab("a", "a", TabType::Serial);
    serial.set_connection(Some(connection(Some("p"))));
    assert_eq!(labels(&serial), vec!["contextMenu.editConnection"]);
    let mut local = tab("a", "a", TabType::Terminal);
    local.set_connection(Some(connection(Some(""))));
    assert_eq!(labels(&local), vec!["contextMenu.saveLocalConnection"]);
}

#[test]
fn oracle_renames_a_serial_tab_as_a_session_tab() {
    assert_eq!(
        get_tab_rename_menu_action(&tab("a", "a", TabType::Serial), &translate).label,
        "contextMenu.editTab"
    );
}

#[test]
fn oracle_page_menu_with_a_zoomed_pane_uses_the_unzoom_labels() {
    let actions = get_pane_menu_actions(&tab("a", "Logs", TabType::Logs), true, &translate, false);
    assert_eq!(
        actions
            .iter()
            .map(|item| item.action.as_str())
            .collect::<Vec<_>>(),
        vec![
            "toggle-zoom",
            "move-to-new-tab",
            "ungroup",
            "layout-templates",
            "",
            "close",
        ]
    );
    assert_eq!(actions[0].label, "workspace.unzoomPane");
    assert_eq!(actions[0].icon.as_deref(), Some("minimize"));
}

#[test]
fn oracle_terminal_pane_menu_keeps_the_log_items_before_the_layout_items() {
    let oracle = super::oracle_json();
    let actions = get_pane_menu_actions(
        &tab("a", "tTerm", TabType::Terminal),
        false,
        &translate,
        false,
    );
    assert_actions_match_oracle(&actions, &oracle["tabMenuActions"]["paneTerminal"]);
    assert_eq!(
        actions
            .iter()
            .map(|item| item.action.as_str())
            .collect::<Vec<_>>(),
        vec![
            "duplicate",
            "split-right",
            "split-left",
            "split-down",
            "split-above",
            "rename",
            "edit-connection",
            "log-start",
            "view-logs",
            "",
            "toggle-zoom",
            "move-to-new-tab",
            "ungroup",
            "layout-templates",
            "",
            "close",
        ]
    );
    assert_eq!(
        find_action(&actions, "split-right")
            .expect("split right")
            .shortcut
            .map(|shortcut| shortcut.as_str()),
        Some("workspace.splitRight")
    );
    assert_eq!(
        find_action(&actions, "split-down")
            .expect("split down")
            .shortcut
            .map(|shortcut| shortcut.as_str()),
        Some("workspace.splitBelow")
    );
    assert_eq!(
        find_action(&actions, "split-left")
            .expect("split left")
            .shortcut,
        None
    );
    assert_eq!(
        find_action(&actions, "split-above")
            .expect("split above")
            .shortcut,
        None
    );
    assert_eq!(
        find_action(&actions, "log-start").unwrap().icon.as_deref(),
        Some("record")
    );
}

#[test]
fn oracle_log_menu_switches_between_start_and_stop() {
    let oracle = super::oracle_json();
    let expected = &oracle["tabMenuActions"];
    let terminal = tab("a", "a", TabType::Terminal);
    assert_actions_match_oracle(
        &get_tab_log_menu_actions(&terminal, &translate, false),
        &expected["logMenuIdle"],
    );
    assert_actions_match_oracle(
        &get_tab_log_menu_actions(&terminal, &translate, true),
        &expected["logMenuRecording"],
    );
    assert_actions_match_oracle(
        &get_tab_log_menu_actions(
            &tab("settings", "Settings", TabType::Settings),
            &translate,
            true,
        ),
        &expected["logMenuRecordingSettings"],
    );
    assert_actions_match_oracle(
        &get_pane_menu_actions(&terminal, false, &translate, true),
        &expected["paneTerminalRecording"],
    );
}

#[test]
fn oracle_standalone_menu_items_keep_their_shortcuts() {
    let reopen = get_reopen_closed_tab_menu_action(&translate);
    assert_eq!(reopen.label, "contextMenu.reopenClosedTab");
    assert_eq!(reopen.action, "reopen-closed");
    assert_eq!(reopen.icon.as_deref(), Some("rotate-ccw"));
    assert_eq!(
        reopen.shortcut.map(|id| id.as_str()),
        Some("tabs.reopenClosed")
    );

    let ungroup = get_ungroup_menu_action(&translate);
    assert_eq!(ungroup.label, "workspace.unsplit");
    assert_eq!(ungroup.action, "ungroup");
    assert_eq!(ungroup.icon.as_deref(), Some("unsplit"));
    assert_eq!(
        ungroup.shortcut.map(|id| id.as_str()),
        Some("workspace.restoreSingleGroup")
    );

    let layout = get_layout_templates_menu_action(&translate);
    assert_eq!(layout.label, "layoutTemplates.menu");
    assert_eq!(layout.action, "layout-templates");
    assert_eq!(layout.icon.as_deref(), Some("layout"));
    assert_eq!(
        layout.shortcut.map(|id| id.as_str()),
        Some("workspace.chooseLayout")
    );
}
