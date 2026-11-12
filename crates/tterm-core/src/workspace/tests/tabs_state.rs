use crate::workspace::tab::{Tab, TabType};
use crate::workspace::tabs_state::{
    RemoveTabsOptions, ensure_tab_defaults, get_duplicate_tab_title, remove_tabs_from_state,
};

use super::oracle_11d2;

fn tab_from(value: &serde_json::Value) -> Tab {
    serde_json::from_value(value.clone()).expect("a tab")
}

fn ids(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

#[test]
fn oracle_matches_remove_tabs_from_state() {
    let oracle = oracle_11d2();
    for case in oracle["tabsState"]["remove"]
        .as_array()
        .expect("remove cases")
    {
        let tabs: Vec<Tab> = case["tabs"]
            .as_array()
            .expect("tabs")
            .iter()
            .map(tab_from)
            .collect();
        let active_tab_id = case["activeTabId"].as_str();
        let ids: Vec<String> = case["ids"]
            .as_array()
            .expect("ids")
            .iter()
            .map(|id| id.as_str().expect("an id").to_string())
            .collect();
        let options = RemoveTabsOptions {
            preferred_active_tab_id: case["options"]["preferredActiveTabId"]
                .as_str()
                .map(str::to_string),
            activate_preferred: case["options"]["activatePreferred"]
                .as_bool()
                .unwrap_or(false),
        };
        let result = remove_tabs_from_state(tabs.clone(), active_tab_id, &ids, &options);
        assert_eq!(
            serde_json::to_value(&result.tabs).unwrap(),
            case["result"]["tabs"],
            "{case:?}"
        );
        assert_eq!(
            result.active_tab_id,
            case["result"]["activeTabId"].as_str().map(str::to_string),
            "{case:?}"
        );
        assert_eq!(
            result.tabs == tabs,
            case["sameReference"].as_bool().unwrap_or(false),
            "{case:?}"
        );
    }
}

#[test]
fn oracle_matches_ensure_tab_defaults() {
    let oracle = oracle_11d2();
    for case in oracle["tabsState"]["defaults"]
        .as_array()
        .expect("default cases")
    {
        let input = tab_from(&case["tab"]);
        let next = ensure_tab_defaults(&input);
        assert_eq!(
            serde_json::to_value(&next).unwrap(),
            case["next"],
            "{case:?}"
        );
        assert_eq!(
            next.has_connected,
            case["fields"]["hasConnected"].as_bool(),
            "{case:?}"
        );
        assert_eq!(
            next.session_nonce,
            case["fields"]["sessionNonce"]
                .as_u64()
                .map(|value| value as u32),
            "{case:?}"
        );
        assert_eq!(
            next.connection_header_pinned,
            case["fields"]["connectionHeaderPinned"].as_bool(),
            "{case:?}"
        );
        let expected_connection = if case["fields"]["connection"].as_str() == Some("__undefined__")
        {
            None
        } else {
            Some(
                serde_json::from_value(case["fields"]["connection"].clone()).expect("a connection"),
            )
        };
        assert_eq!(next.connection, expected_connection, "{case:?}");
    }
}

#[test]
fn oracle_matches_duplicate_tab_titles() {
    let oracle = oracle_11d2();
    for case in oracle["tabsState"]["duplicateTitles"]
        .as_array()
        .expect("title cases")
    {
        let existing: Vec<String> = case["existing"]
            .as_array()
            .expect("titles")
            .iter()
            .map(|title| title.as_str().expect("a title").to_string())
            .collect();
        assert_eq!(
            get_duplicate_tab_title(case["base"].as_str().expect("base"), &existing),
            case["title"].as_str().expect("title"),
            "{case:?}"
        );
    }
}

#[test]
fn removes_tabs_and_picks_the_next_active_tab() {
    let tabs = vec![
        Tab::new("a", "a", TabType::Terminal),
        Tab::new("b", "b", TabType::Terminal),
        Tab::new("c", "c", TabType::Terminal),
    ];
    let result =
        remove_tabs_from_state(tabs, Some("b"), &ids(&["b"]), &RemoveTabsOptions::default());
    assert_eq!(result.active_tab_id.as_deref(), Some("c"));
    assert_eq!(result.tabs.len(), 2);
    assert!(result.tabs[1].is_active);
    assert_eq!(result.tabs[1].has_connected, Some(true));
}

#[test]
fn removed_tabs_return_the_input_when_none_match() {
    let tabs = vec![Tab::new("a", "a", TabType::Terminal)];
    let result = remove_tabs_from_state(
        tabs.clone(),
        Some("a"),
        &ids(&["missing"]),
        &RemoveTabsOptions::default(),
    );
    assert_eq!(result.tabs, tabs);
    assert_eq!(result.active_tab_id.as_deref(), Some("a"));
}

#[test]
fn removing_every_tab_leaves_no_active_tab() {
    let result = remove_tabs_from_state(
        vec![Tab::new("a", "a", TabType::Terminal)],
        Some("a"),
        &ids(&["a"]),
        &RemoveTabsOptions::default(),
    );
    assert!(result.tabs.is_empty());
    assert_eq!(result.active_tab_id, None);
}

#[test]
fn ensure_defaults_distinguishes_missing_from_false() {
    let mut tab = Tab::new("a", "a", TabType::Terminal);
    tab.has_connected = Some(false);
    tab.connection_header_pinned = Some(false);
    tab.session_nonce = Some(7);
    let next = ensure_tab_defaults(&tab);
    assert_eq!(next.has_connected, Some(false));
    assert_eq!(next.connection_header_pinned, Some(false));
    assert_eq!(next.session_nonce, Some(7));
}

#[test]
fn ensure_defaults_blank_page_connections_but_keep_file_editor_connections() {
    let mut page = Tab::new("a", "Settings", TabType::Settings);
    page.connection = Some(Default::default());
    let page = ensure_tab_defaults(&page);
    assert_eq!(page.has_connected, Some(true));
    assert_eq!(page.session_nonce, Some(0));
    assert_eq!(page.connection, None);

    let mut editor = Tab::new("a", "file", TabType::RemoteFileEditor);
    editor.connection = Some(Default::default());
    let editor = ensure_tab_defaults(&editor);
    assert_eq!(editor.has_connected, Some(true));
    assert_eq!(editor.session_nonce, Some(0));
    assert!(editor.connection.is_some());
}

#[test]
fn duplicate_titles_skip_taken_numbers_and_unicode_bases() {
    assert_eq!(get_duplicate_tab_title("web", &ids(&["web-2"])), "web-3");
    assert_eq!(
        get_duplicate_tab_title("web", &ids(&["web-2", "web-4"])),
        "web-3"
    );
    assert_eq!(get_duplicate_tab_title("标签", &ids(&["标签-2"])), "标签-3");
}
