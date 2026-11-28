//! Tests for `TabLaunch::quick_connection`.
//!
//! A quick connection is typed in the launcher and never saved as a profile.
//! These tests pin the unsaved SSH tab the window persists and the options
//! the session starts with.

use serde_json::{Map, Value};
use tterm_core::connections::quick_connect::{JumpHostConnection, QuickConnection};
use tterm_core::workspace::tab::{ConnectionType, TabType};

use super::TabLaunch;

fn connection() -> QuickConnection {
    QuickConnection {
        title: "deploy@web.example".to_string(),
        username: "deploy".to_string(),
        host: "web.example".to_string(),
        port: 2222,
        private_key_path: Some("/keys/work".to_string()),
        agent_forward: Some(true),
        keepalive_interval_secs: Some(30),
        keepalive_count_max: Some(90_000),
        jump_hosts: Some(vec![JumpHostConnection {
            host: "bastion.example".to_string(),
            port: 22,
            username: "ops".to_string(),
            auth_method: "auto".to_string(),
            private_key_path: Some("/keys/bastion".to_string()),
        }]),
    }
}

fn plain_connection() -> QuickConnection {
    QuickConnection {
        title: "root@example.com".to_string(),
        username: "root".to_string(),
        host: "example.com".to_string(),
        port: 22,
        private_key_path: None,
        agent_forward: None,
        keepalive_interval_secs: None,
        keepalive_count_max: None,
        jump_hosts: None,
    }
}

/// The tab object as it is written to the workspace.
fn serialized_tab(launch: &TabLaunch) -> Map<String, Value> {
    match serde_json::to_value(launch.saved()).expect("the tab serializes") {
        Value::Object(tab) => tab,
        _ => panic!("the tab serializes as an object"),
    }
}

/// The connection object inside the serialized tab.
fn serialized_connection(launch: &TabLaunch) -> Map<String, Value> {
    let mut tab = serialized_tab(launch);
    match tab.remove("connection") {
        Some(Value::Object(connection)) => connection,
        _ => panic!("the tab serializes its connection as an object"),
    }
}

#[test]
fn the_launch_keeps_the_window_tab_id_and_the_typed_title() {
    let launch = TabLaunch::quick_connection("tab-7", connection());

    assert_eq!(launch.saved_id(), "tab-7");
    let saved = launch.saved();
    assert_eq!(saved.id, "tab-7");
    assert_eq!(saved.title, "deploy@web.example");
    assert_eq!(saved.tab_type, TabType::Ssh);

    let tab = serialized_tab(&launch);
    assert_eq!(tab.get("id").and_then(Value::as_str), Some("tab-7"));
    assert_eq!(
        tab.get("title").and_then(Value::as_str),
        Some("deploy@web.example")
    );
    assert_eq!(tab.get("type").and_then(Value::as_str), Some("ssh"));
    assert_eq!(tab.get("isModified").and_then(Value::as_bool), Some(false));
    assert_eq!(tab.get("quickConnect").and_then(Value::as_bool), Some(true));
    assert!(!tab.contains_key("quickConnectAuth"));

    let saved_connection = serialized_connection(&launch);
    assert_eq!(
        saved_connection.get("type").and_then(Value::as_str),
        Some("ssh")
    );
    assert_eq!(
        saved_connection.get("profileName").and_then(Value::as_str),
        Some("deploy@web.example")
    );
    assert_eq!(
        saved_connection.get("host").and_then(Value::as_str),
        Some("web.example")
    );
    assert_eq!(
        saved_connection.get("port").and_then(Value::as_u64),
        Some(2222)
    );
    assert_eq!(
        saved_connection.get("username").and_then(Value::as_str),
        Some("deploy")
    );
    assert_eq!(
        saved_connection.get("authMethod").and_then(Value::as_str),
        Some("auto")
    );
    assert!(!saved_connection.contains_key("profileId"));
}

#[test]
fn a_quick_connection_is_an_unsaved_ssh_tab_with_no_profile_to_credit() {
    let launch = TabLaunch::quick_connection("tab-8", connection());

    assert!(launch.is_ssh());
    assert_eq!(launch.search_kind(), "ssh");
    assert!(!launch.is_restored());
    assert!(!launch.is_default_shell());
    assert!(launch.may_resume_agent());
    assert!(launch.local_shell().is_none());
    assert!(launch.local_custom_path().is_none());

    let options = launch
        .connection_options()
        .expect("the quick connection opens");
    assert_eq!(options.connection_type.as_deref(), Some("ssh"));
    assert_eq!(options.profile_name.as_deref(), Some("deploy@web.example"));
    assert!(options.profile_id.is_none());
    assert!(launch.saved_profile_id(&options).is_none());
}

#[test]
fn a_quick_connection_carries_no_credentials_and_starts_no_local_shell() {
    let launch = TabLaunch::quick_connection("tab-9", plain_connection());

    let options = launch
        .connection_options()
        .expect("the quick connection opens");
    assert!(options.password.is_none());
    assert!(options.private_key_path.is_none());
    assert!(options.private_key_passphrase.is_none());
    assert!(options.certificate_path.is_none());
    assert_eq!(options.auth_method.as_deref(), Some("auto"));
    assert!(!options.agent_forward);
    assert!(options.jump_hosts.is_empty());
    assert!(options.keepalive_interval_secs.is_none());
    assert!(options.keepalive_count_max.is_none());
    assert!(options.terminal_shell.is_none());
    assert!(options.terminal_shell_custom_path.is_none());
    assert!(options.terminal_shell_custom_args.is_none());

    let tab_connection = launch
        .saved()
        .connection
        .as_ref()
        .expect("the tab's connection");
    assert!(tab_connection.password.is_none());
    assert!(tab_connection.private_key_passphrase.is_none());
    assert!(tab_connection.certificate_path.is_none());
    assert!(tab_connection.profile_id.is_none());
    assert_eq!(tab_connection.agent_forward, Some(false));
    assert!(tab_connection.terminal_shell.is_none());

    let serialized = serialized_connection(&launch);
    for absent in [
        "profileId",
        "password",
        "privateKeyPath",
        "privateKeyPassphrase",
        "certificatePath",
        "jumpHosts",
        "keepaliveIntervalSecs",
        "keepaliveCountMax",
        "terminalShell",
        "terminalShellCustomPath",
        "terminalShellCustomArgs",
        "cwd",
    ] {
        assert!(
            !serialized.contains_key(absent),
            "the serialized connection must not carry {absent}"
        );
    }
    assert_eq!(
        serialized.get("agentForward").and_then(Value::as_bool),
        Some(false)
    );
}

#[test]
fn jump_hosts_and_keepalives_reach_the_tab_and_the_launch_options() {
    let launch = TabLaunch::quick_connection("tab-10", connection());

    let options = launch
        .connection_options()
        .expect("the quick connection opens");
    assert_eq!(options.keepalive_interval_secs, Some(30));
    assert_eq!(options.keepalive_count_max, Some(u16::MAX));
    assert_eq!(options.jump_hosts.len(), 1);
    let jump = &options.jump_hosts[0];
    assert_eq!(jump.host.as_deref(), Some("bastion.example"));
    assert_eq!(jump.port, Some(22));
    assert_eq!(jump.username.as_deref(), Some("ops"));
    assert_eq!(jump.auth_method.as_deref(), Some("auto"));
    assert_eq!(jump.private_key_path.as_deref(), Some("/keys/bastion"));
    assert!(jump.password.is_none());
    assert!(jump.private_key_passphrase.is_none());
    assert!(jump.certificate_path.is_none());
    assert!(jump.profile_id.is_none());

    let tab_connection = launch
        .saved()
        .connection
        .as_ref()
        .expect("the tab's connection");
    assert_eq!(tab_connection.connection_type, Some(ConnectionType::Ssh));
    assert_eq!(tab_connection.keepalive_interval_secs, Some(30));
    assert_eq!(tab_connection.keepalive_count_max, Some(90_000));
    assert_eq!(
        tab_connection.private_key_path.as_deref(),
        Some("/keys/work")
    );
    assert_eq!(tab_connection.agent_forward, Some(true));

    let serialized = serialized_connection(&launch);
    assert_eq!(
        serialized
            .get("keepaliveIntervalSecs")
            .and_then(Value::as_u64),
        Some(30)
    );
    assert_eq!(
        serialized.get("keepaliveCountMax").and_then(Value::as_u64),
        Some(90_000)
    );
    assert!(!serialized.contains_key("password"));
    assert!(!serialized.contains_key("privateKeyPassphrase"));
    assert!(!serialized.contains_key("certificatePath"));
    assert!(!serialized.contains_key("profileId"));

    let jump_hosts = serialized
        .get("jumpHosts")
        .cloned()
        .expect("the serialized jump hosts");
    let jump = jump_hosts
        .as_array()
        .and_then(|hosts| hosts.first())
        .and_then(Value::as_object)
        .expect("a serialized jump host");
    assert_eq!(
        jump.get("privateKeyPath").and_then(Value::as_str),
        Some("/keys/bastion")
    );
    assert!(!jump.contains_key("password"));
    assert!(!jump.contains_key("privateKeyPassphrase"));
    let persisted: Vec<JumpHostConnection> =
        serde_json::from_value(jump_hosts).expect("the serialized jump hosts");
    assert_eq!(
        persisted,
        connection().jump_hosts.expect("the fixture's jump hosts")
    );
}
