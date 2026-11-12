use super::oracle_11d2;
use crate::workspace::local_tab_title::normalize_saved_local_tab;
use crate::workspace::tab::Tab;
use crate::workspace::tab_icons::normalize_saved_ssh_tab;

#[test]
fn oracle_setters_replace_or_remove_the_only_canonical_field() {
    use crate::workspace::tabs_state::action::TabsAction;
    use crate::workspace::tabs_state::{TabsState, tabs_reducer};
    for case in oracle_11d2()["fieldMutations"].as_array().unwrap() {
        let mut tab: Tab = serde_json::from_value(case["input"].clone()).unwrap();
        match case["field"].as_str().unwrap() {
            "sessionNonce" => {
                tab.set_session_nonce(case["replacement"].as_u64().map(|value| value as u32))
            }
            "iconStyle" => tab.set_icon_style(
                case.get("replacement")
                    .map(|value| serde_json::from_value(value.clone()).unwrap()),
            ),
            "connection" => {
                let mut connection = tab.connection.clone().unwrap();
                connection.set_port(None);
                tab.set_connection(Some(connection));
            }
            _ => unreachable!(),
        }
        assert_eq!(
            serde_json::to_string(&tab).unwrap(),
            case["bytes"].as_str().unwrap()
        );
        assert_eq!(
            serde_json::to_string(&normalize_saved_ssh_tab(&tab)).unwrap(),
            case["normalizedBytes"].as_str().unwrap()
        );
        let state = TabsState {
            tabs: vec![tab],
            active_tab_id: Some("a".into()),
            groups: vec![],
            group_count: 0,
        };
        let result = tabs_reducer(
            state,
            TabsAction::Duplicate {
                source_id: "a".into(),
                new_id: "b".into(),
                split: None,
            },
            Some("/home/me"),
        );
        assert_eq!(
            serde_json::to_string(&result.tabs).unwrap(),
            case["duplicateBytes"].as_str().unwrap()
        );
    }
}

#[test]
fn oracle_preserves_old_session_fields_and_normalizes_only_explicit_fields() {
    let oracle = oracle_11d2();
    let cases = oracle["sessionTabs"].as_array().expect("session tabs");
    let tabs: Vec<Tab> = serde_json::from_value(serde_json::Value::Array(
        cases.iter().map(|case| case["input"].clone()).collect(),
    ))
    .expect("all session tabs survive");
    assert_eq!(tabs.len(), cases.len());
    for (tab, case) in tabs.iter().zip(cases) {
        assert_eq!(
            serde_json::to_value(crate::workspace::tabs_state::ensure_tab_defaults(tab)).unwrap(),
            case["defaults"]
        );
        assert_eq!(
            serde_json::to_string(tab).unwrap(),
            case["readBytes"].as_str().unwrap()
        );
        let ssh = normalize_saved_ssh_tab(tab);
        assert_eq!(serde_json::to_value(&ssh).unwrap(), case["ssh"]);
        assert_eq!(
            serde_json::to_string(&ssh).unwrap(),
            case["sshBytes"].as_str().unwrap()
        );
        let refreshed = normalize_saved_local_tab(tab, "Terminal");
        assert_eq!(serde_json::to_value(&refreshed).unwrap(), case["refreshed"]);
        assert_eq!(
            serde_json::to_string(&refreshed).unwrap(),
            case["refreshedBytes"].as_str().unwrap()
        );
    }
}
