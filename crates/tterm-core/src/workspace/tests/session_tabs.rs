use super::oracle_11d2;
use crate::workspace::local_tab_title::normalize_saved_local_tab;
use crate::workspace::tab::Tab;
use crate::workspace::tab_icons::normalize_saved_ssh_tab;

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
