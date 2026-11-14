use super::{
    DEFAULT_SSH_PORT, JumpHostConnection, QUICK_CONNECT_RECENTS_LIMIT, QuickConnectEndpoint,
    QuickConnectTarget, SshAuthUsed, SshConfigHost, add_recent_quick_connection,
    apply_quick_connect_auth, build_quick_connect_tab, config_host_to_look_up,
    draft_profile_from_input, draft_profile_from_tab, format_endpoint, parse_endpoint,
    parse_quick_connect_input, parse_recent_quick_connections, resolve_quick_connect_target,
};
use crate::workspace::tab::Tab;
use serde_json::{Value, json};

fn config_host() -> SshConfigHost {
    SshConfigHost {
        host_pattern: "bastion".to_owned(),
        name: "bastion".to_owned(),
        host: Some("203.0.113.10".to_owned()),
        port: 2222,
        username: Some("ops".to_owned()),
        private_key_path: Some("~/.ssh/bastion".to_owned()),
        agent_forward: false,
        keepalive_interval_secs: 15,
        keepalive_count_max: 3,
        jump_hosts: Vec::new(),
        skipped: false,
    }
}

fn tab_for(connection: &super::QuickConnection) -> Tab {
    build_quick_connect_tab(connection)
}

fn endpoint(username: Option<&str>, host: &str, port: Option<u16>) -> QuickConnectEndpoint {
    QuickConnectEndpoint {
        username: username.map(str::to_owned),
        host: host.to_owned(),
        port,
    }
}

fn target(host: &str) -> QuickConnectTarget {
    QuickConnectTarget {
        username: None,
        host: host.to_owned(),
        port: None,
        jump_host: None,
        private_key_path: None,
    }
}

fn plain_connection() -> super::QuickConnection {
    super::QuickConnection {
        title: "root@db".to_owned(),
        username: "root".to_owned(),
        host: "db".to_owned(),
        port: 22,
        private_key_path: None,
        agent_forward: None,
        keepalive_interval_secs: None,
        keepalive_count_max: None,
        jump_hosts: None,
    }
}

#[test]
fn reads_a_user_and_host() {
    assert_eq!(
        parse_quick_connect_input("root@10.0.0.5"),
        Some(QuickConnectTarget {
            username: Some("root".to_owned()),
            host: "10.0.0.5".to_owned(),
            port: None,
            jump_host: None,
            private_key_path: None,
        })
    );
}

#[test]
fn reads_a_user_host_and_port() {
    let parsed = parse_quick_connect_input("root@10.0.0.5:2222").expect("quick connect");
    assert_eq!(parsed.username.as_deref(), Some("root"));
    assert_eq!(parsed.host, "10.0.0.5");
    assert_eq!(parsed.port, Some(2222));
}

#[test]
fn reads_a_bare_host() {
    let parsed = parse_quick_connect_input("web-01.example.com").expect("quick connect");
    assert_eq!(parsed.username, None);
    assert_eq!(parsed.host, "web-01.example.com");
    assert_eq!(parsed.port, None);
}

#[test]
fn reads_a_bracketed_ipv6_address_and_port() {
    let parsed = parse_quick_connect_input("admin@[2001:db8::1]:2200").expect("quick connect");
    assert_eq!(parsed.username.as_deref(), Some("admin"));
    assert_eq!(parsed.host, "2001:db8::1");
    assert_eq!(parsed.port, Some(2200));
}

#[test]
fn reads_an_unbracketed_ipv6_address() {
    let parsed = parse_endpoint("fe80::1").expect("endpoint");
    assert_eq!(parsed.host, "fe80::1");
    assert_eq!(parsed.port, None);
}

#[test]
fn reads_ssh_port_option() {
    let parsed = parse_quick_connect_input("ssh -p 2222 deploy@web").expect("quick connect");
    assert_eq!(parsed.username.as_deref(), Some("deploy"));
    assert_eq!(parsed.host, "web");
    assert_eq!(parsed.port, Some(2222));
}

#[test]
fn reads_attached_ssh_options() {
    let parsed = parse_quick_connect_input("ssh -l deploy -p2222 web").expect("quick connect");
    assert_eq!(parsed.username.as_deref(), Some("deploy"));
    assert_eq!(parsed.host, "web");
    assert_eq!(parsed.port, Some(2222));
}

#[test]
fn reads_a_jump_host_and_key_file() {
    assert_eq!(
        parse_quick_connect_input("ssh -J jump@bastion:2200 -i ~/.ssh/k root@db"),
        Some(QuickConnectTarget {
            username: Some("root".to_owned()),
            host: "db".to_owned(),
            port: None,
            jump_host: Some(endpoint(Some("jump"), "bastion", Some(2200))),
            private_key_path: Some("~/.ssh/k".to_owned()),
        })
    );
}

#[test]
fn rejects_empty_input() {
    assert_eq!(parse_quick_connect_input(""), None);
}

#[test]
fn rejects_search_words() {
    assert_eq!(parse_quick_connect_input("prod db"), None);
}

#[test]
fn rejects_ssh_without_a_host() {
    assert_eq!(parse_quick_connect_input("ssh"), None);
}

#[test]
fn rejects_unsupported_options() {
    assert_eq!(parse_quick_connect_input("ssh -v root@host"), None);
}

#[test]
fn rejects_a_remote_command() {
    assert_eq!(parse_quick_connect_input("ssh root@host uptime"), None);
}

#[test]
fn rejects_a_port_above_the_ssh_range() {
    assert_eq!(parse_quick_connect_input("root@host:70000"), None);
}

#[test]
fn rejects_a_non_numeric_port() {
    assert_eq!(parse_quick_connect_input("root@host:abc"), None);
}

#[test]
fn rejects_an_empty_user_name() {
    assert_eq!(parse_quick_connect_input("@host"), None);
}

#[test]
fn rejects_a_jump_host_chain() {
    assert_eq!(parse_quick_connect_input("ssh -J a,b root@host"), None);
}

#[test]
fn rejects_a_path_as_a_host() {
    assert_eq!(parse_quick_connect_input("a/b"), None);
}

#[test]
fn needs_a_user_name_to_resolve_a_host() {
    assert_eq!(resolve_quick_connect_target(&target("10.0.0.5"), &[]), None);
}

#[test]
fn fills_an_alias_from_ssh_config_letting_typed_values_win() {
    let hosts = [config_host()];
    let resolved =
        resolve_quick_connect_target(&target("bastion"), &hosts).expect("resolved alias");
    assert_eq!(resolved.title, "ops@bastion");
    assert_eq!(resolved.username, "ops");
    assert_eq!(resolved.host, "203.0.113.10");
    assert_eq!(resolved.port, 2222);
    assert_eq!(resolved.private_key_path.as_deref(), Some("~/.ssh/bastion"));

    let typed = resolve_quick_connect_target(
        &QuickConnectTarget {
            username: Some("root".to_owned()),
            host: "bastion".to_owned(),
            port: Some(22),
            jump_host: None,
            private_key_path: None,
        },
        &hosts,
    )
    .expect("typed alias");
    assert_eq!(typed.title, "root@bastion");
    assert_eq!(typed.username, "root");
    assert_eq!(typed.port, 22);
}

#[test]
fn looks_up_a_typed_host_only_when_config_does_not_list_it() {
    let hosts = [config_host()];
    assert_eq!(
        config_host_to_look_up("deploy@db9.corp:2200", &hosts).as_deref(),
        Some("db9.corp")
    );
    assert_eq!(
        config_host_to_look_up("ssh -l root 10.0.0.5", &hosts).as_deref(),
        Some("10.0.0.5")
    );
    assert_eq!(config_host_to_look_up("bastion", &hosts), None);
    assert_eq!(config_host_to_look_up("prod db", &hosts), None);
}

#[test]
fn uses_the_same_user_for_a_typed_jump_host_unless_one_is_given() {
    let resolved = resolve_quick_connect_target(
        &QuickConnectTarget {
            username: Some("root".to_owned()),
            host: "db".to_owned(),
            port: None,
            jump_host: Some(endpoint(None, "bastion", None)),
            private_key_path: None,
        },
        &[],
    )
    .expect("jump host");
    assert_eq!(
        resolved.jump_hosts,
        Some(vec![JumpHostConnection {
            host: "bastion".to_owned(),
            port: DEFAULT_SSH_PORT,
            username: "root".to_owned(),
            auth_method: "auto".to_owned(),
            private_key_path: None,
        }])
    );
}

#[test]
fn formats_endpoints_without_the_default_port_and_with_brackets_before_one() {
    assert_eq!(format_endpoint(&endpoint(Some("u"), "h", Some(22))), "u@h");
    assert_eq!(
        format_endpoint(&endpoint(None, "2001:db8::1", Some(2200))),
        "[2001:db8::1]:2200"
    );
}

#[test]
fn switches_a_quick_tab_to_a_reusable_key() {
    let tab = apply_quick_connect_auth(
        &tab_for(&plain_connection()),
        &SshAuthUsed {
            method: "key".to_owned(),
            private_key_path: Some("/home/me/.ssh/id_ed25519".to_owned()),
            reusable: true,
        },
    );
    let connection = tab.connection.as_ref().expect("connection");
    assert_eq!(connection.auth_method.as_deref(), Some("key"));
    assert_eq!(
        connection.private_key_path.as_deref(),
        Some("/home/me/.ssh/id_ed25519")
    );
}

#[test]
fn keeps_asking_after_an_auth_method_that_was_not_reusable() {
    let tab = apply_quick_connect_auth(
        &tab_for(&plain_connection()),
        &SshAuthUsed {
            method: "password".to_owned(),
            private_key_path: None,
            reusable: false,
        },
    );
    assert_eq!(
        tab.connection
            .as_ref()
            .and_then(|connection| connection.auth_method.as_deref()),
        Some("auto")
    );
    let auth: SshAuthUsed =
        serde_json::from_value(tab.saved_field("quickConnectAuth").cloned().expect("auth"))
            .expect("auth fields");
    assert_eq!(auth.method, "password");
}

#[test]
fn leaves_a_configured_auth_method_alone() {
    let source = tab_for(&plain_connection());
    let mut connection = source
        .saved_field("connection")
        .and_then(Value::as_object)
        .cloned()
        .expect("connection");
    connection.insert("authMethod".to_owned(), json!("agent"));
    let mut tab = source.clone();
    tab.set_saved_field("connection", Some(Value::Object(connection)));
    assert_eq!(
        apply_quick_connect_auth(
            &tab,
            &SshAuthUsed {
                method: "password".to_owned(),
                private_key_path: None,
                reusable: false,
            }
        ),
        tab
    );
}

#[test]
fn drafts_a_profile_with_the_method_that_worked() {
    let mut connection = plain_connection();
    connection.port = 2222;
    let tab = apply_quick_connect_auth(
        &tab_for(&connection),
        &SshAuthUsed {
            method: "interactive".to_owned(),
            private_key_path: None,
            reusable: false,
        },
    );
    let profile = draft_profile_from_tab(&tab);
    assert_eq!(profile.name, "root@db");
    assert_eq!(profile.connection_type, "ssh");
    assert_eq!(profile.host.as_deref(), Some("db"));
    assert_eq!(profile.port, Some(2222));
    assert_eq!(profile.username.as_deref(), Some("root"));
    assert_eq!(profile.auth_method.as_deref(), Some("interactive"));
}

#[test]
fn prefills_the_connection_dialog_from_typed_host() {
    let profile = draft_profile_from_input("10.0.0.5:2200", &[]).expect("profile");
    assert_eq!(profile.host.as_deref(), Some("10.0.0.5"));
    assert_eq!(profile.port, Some(2200));
    assert_eq!(profile.username.as_deref(), Some(""));
    assert_eq!(profile.auth_method.as_deref(), Some("password"));
    assert!(draft_profile_from_input("prod db", &[]).is_none());
}

#[test]
fn moves_a_repeated_quick_connection_to_the_front() {
    let connection = plain_connection();
    let mut recents = add_recent_quick_connection(&[], &connection, 1);
    for index in 0..QUICK_CONNECT_RECENTS_LIMIT {
        let mut next = connection.clone();
        next.host = format!("h{index}");
        next.title = next.host.clone();
        recents = add_recent_quick_connection(&recents, &next, index as i64 + 2);
    }
    recents = add_recent_quick_connection(&recents, &connection, 100);
    assert_eq!(recents.len(), QUICK_CONNECT_RECENTS_LIMIT);
    assert_eq!(recents[0].connection.host, "db");
    assert_eq!(recents[0].last_used_at, 100);
    assert_eq!(
        recents
            .iter()
            .filter(|recent| recent.connection.host == "db")
            .count(),
        1
    );
}

#[test]
fn ignores_stored_quick_connections_it_cannot_read() {
    let stored = json!([
        {
            "title": "root@db",
            "username": "root",
            "host": "db",
            "port": 22,
            "lastUsedAt": 1
        },
        { "host": "broken" }
    ])
    .to_string();
    assert_eq!(parse_recent_quick_connections(Some(&stored)).len(), 1);
    assert!(parse_recent_quick_connections(Some("{not json")).is_empty());
}
