use super::{
    apply_profile_to_tab, build_connection_from_profile, profile_tab_icon, same_connection_tabs,
};
use crate::profiles::types::SavedProfile;
use crate::workspace::tab::Tab;
use serde_json::{Map, Value, json};

fn merge(base: Value, overrides: Value) -> Value {
    let mut base = base.as_object().cloned().unwrap_or_default();
    if let Value::Object(overrides) = overrides {
        base.extend(overrides);
    }
    Value::Object(base)
}

fn profile(overrides: Value) -> SavedProfile {
    serde_json::from_value(merge(
        json!({
            "id": "p1",
            "name": "web",
            "connection_type": "ssh",
            "host": "10.0.0.1",
            "port": 22,
            "username": "root",
            "remember_password": true,
            "auth_method": "password",
            "keepalive_interval_secs": 15,
            "keepalive_count_max": 3
        }),
        overrides,
    ))
    .expect("profile")
}

fn tab(overrides: Value) -> Tab {
    serde_json::from_value(merge(
        json!({
            "id": "t1",
            "title": "web",
            "type": "ssh",
            "isActive": true,
            "sessionNonce": 2,
            "connection": {
                "type": "ssh",
                "profileId": "p1",
                "profileName": "web",
                "host": "10.0.0.1",
                "port": 22,
                "username": "root",
                "authMethod": "password",
                "password": "typed"
            }
        }),
        overrides,
    ))
    .expect("tab")
}

fn connection(tab: &Tab) -> Map<String, Value> {
    tab.saved_field("connection")
        .and_then(Value::as_object)
        .cloned()
        .expect("connection")
}

#[test]
fn connects_with_a_keys_certificate_and_drops_it_with_the_key() {
    let keyed = profile(json!({
        "auth_method": "key",
        "private_key_path": "/keys/id",
        "certificate_path": "/keys/id-ca-cert.pub"
    }));
    let keyed_tab = apply_profile_to_tab(&tab(json!({})), &keyed);
    assert_eq!(
        keyed_tab
            .connection
            .as_ref()
            .and_then(|connection| connection.certificate_path.as_deref()),
        Some("/keys/id-ca-cert.pub")
    );
    let agent = profile(json!({
        "auth_method": "agent",
        "certificate_path": "/keys/id-ca-cert.pub"
    }));
    assert_eq!(
        apply_profile_to_tab(&keyed_tab, &agent)
            .connection
            .as_ref()
            .and_then(|connection| connection.certificate_path.as_deref()),
        None
    );
}

#[test]
fn connects_with_saved_settings_and_keeps_the_session() {
    let next = apply_profile_to_tab(
        &tab(json!({})),
        &profile(json!({ "port": 2222, "encoding": "gbk" })),
    );
    assert_eq!(next.session_nonce, Some(2));
    assert_eq!(
        next.connection
            .as_ref()
            .and_then(|connection| connection.port),
        Some(2222)
    );
    assert_eq!(
        next.connection
            .as_ref()
            .and_then(|connection| connection.encoding.as_deref()),
        Some("gbk")
    );
}

#[test]
fn keeps_a_typed_password_only_for_the_same_account() {
    let same = apply_profile_to_tab(&tab(json!({})), &profile(json!({ "login_script": "ls" })));
    assert_eq!(
        same.connection
            .as_ref()
            .and_then(|connection| connection.password.as_deref()),
        Some("typed")
    );
    let changed = apply_profile_to_tab(&tab(json!({})), &profile(json!({ "username": "deploy" })));
    assert_eq!(
        changed
            .connection
            .as_ref()
            .and_then(|connection| connection.password.as_deref()),
        None
    );
}

#[test]
fn follows_a_rename_unless_the_tab_was_renamed() {
    assert_eq!(
        apply_profile_to_tab(&tab(json!({})), &profile(json!({ "name": "api" }))).title,
        "api"
    );
    assert_eq!(
        apply_profile_to_tab(
            &tab(json!({ "title": "mine" })),
            &profile(json!({ "name": "api" }))
        )
        .title,
        "mine"
    );
    let copy = apply_profile_to_tab(
        &tab(json!({ "title": "web-2", "duplicateBaseTitle": "web" })),
        &profile(json!({ "name": "api" })),
    );
    assert_eq!(copy.title, "api-2");
    assert_eq!(copy.duplicate_base_title.as_deref(), Some("api"));
}

#[test]
fn gives_a_saved_ssh_tabs_its_icon_and_host_distribution() {
    let saved = profile(json!({
        "tab_icon": { "icon": "database" },
        "distro": "debian"
    }));
    let opened = build_connection_from_profile(&saved);
    assert_eq!(
        opened
            .icon_style
            .as_ref()
            .and_then(|style| style.icon.as_deref()),
        Some("database")
    );
    assert_eq!(opened.distro_id.as_deref(), Some("debian"));
    let applied = apply_profile_to_tab(&tab(json!({ "distroId": "arch" })), &saved);
    assert_eq!(applied.distro_id.as_deref(), Some("debian"));
}

#[test]
fn drops_a_distribution_that_is_not_known() {
    assert_eq!(
        profile_tab_icon(&profile(json!({ "distro": "beos" }))).distro_id,
        None
    );
}

#[test]
fn finds_every_open_tab_of_the_same_saved_connection() {
    let first = tab(json!({}));
    let second = tab(json!({ "id": "t2" }));
    let other = tab(json!({
        "id": "t3",
        "connection": {
            "type": "ssh",
            "profileId": "p2"
        }
    }));
    assert_eq!(
        same_connection_tabs(&[first.clone(), second.clone(), other], &first),
        vec![first, second]
    );
}

#[test]
fn keeps_an_unsaved_connection_to_its_own_tab() {
    let quick = tab(json!({
        "connection": { "type": "ssh", "host": "10.0.0.1" }
    }));
    assert_eq!(
        same_connection_tabs(&[quick.clone(), tab(json!({ "id": "t2" }))], &quick),
        vec![quick]
    );
}

fn serial_board() -> SavedProfile {
    profile(json!({
        "connection_type": "serial",
        "name": "ESP32",
        "host": null,
        "serial": {
            "port": "COM3",
            "baudRate": 921600,
            "dataBits": 8,
            "parity": "none",
            "stopBits": 1,
            "flowControl": "none",
            "lineEnding": "crlf",
            "receiveNewline": "cr",
            "localEcho": false,
            "timestamps": true,
            "hexView": false
        }
    }))
}

#[test]
fn opens_a_serial_tab_on_the_saved_port() {
    let opened = build_connection_from_profile(&serial_board());
    assert_eq!(opened.tab_type, crate::workspace::tab::TabType::Serial);
    assert_eq!(
        opened
            .connection
            .as_ref()
            .and_then(|connection| connection.profile_id.as_deref()),
        Some("p1")
    );
    assert_eq!(
        opened
            .connection
            .as_ref()
            .and_then(|connection| connection.profile_name.as_deref()),
        Some("ESP32")
    );
    assert_eq!(
        opened
            .connection
            .as_ref()
            .and_then(|connection| connection.host.as_deref()),
        None
    );
    let saved = connection(&opened);
    assert_eq!(saved["serial"]["port"], "COM3");
    assert_eq!(saved["serial"]["baudRate"], 921600);
    assert_eq!(saved["serial"]["lineEnding"], "crlf");
    assert_eq!(saved["serial"]["timestamps"], true);
}

#[test]
fn serial_profiles_carry_no_ssh_tab_icon() {
    assert_eq!(profile_tab_icon(&serial_board()).icon_style, None);
    assert_eq!(profile_tab_icon(&serial_board()).distro_id, None);
}

fn local_profile() -> SavedProfile {
    profile(json!({
        "connection_type": "terminal",
        "host": null,
        "port": null,
        "username": null,
        "login_script": "nvm use",
        "terminal_theme": "nord",
        "local_terminal": {
            "shell": "wsl",
            "wslDistro": "Ubuntu",
            "startDirectory": "~/web"
        }
    }))
}

#[test]
fn opens_the_saved_local_shell_in_its_directory() {
    let opened = build_connection_from_profile(&local_profile());
    assert_eq!(opened.tab_type, crate::workspace::tab::TabType::Terminal);
    assert_eq!(opened.title, "web");
    assert_eq!(
        opened
            .connection
            .as_ref()
            .and_then(|connection| connection.terminal_shell),
        Some(crate::workspace::tab::TerminalShellType::Wsl)
    );
    assert_eq!(
        opened
            .connection
            .as_ref()
            .and_then(|connection| connection.terminal_shell_wsl_distro.as_deref()),
        Some("Ubuntu")
    );
    assert_eq!(
        opened
            .connection
            .as_ref()
            .and_then(|connection| connection.cwd.as_deref()),
        Some("~/web")
    );
    assert_eq!(
        opened
            .connection
            .as_ref()
            .and_then(|connection| connection.login_script.as_deref()),
        Some("nvm use")
    );
    assert_eq!(
        opened
            .connection
            .as_ref()
            .and_then(|connection| connection.terminal_theme.as_deref()),
        Some("nord")
    );
}

#[test]
fn opens_auto_in_home_when_nothing_is_saved() {
    let opened = build_connection_from_profile(&profile(json!({
        "connection_type": "terminal",
        "local_terminal": null
    })));
    assert_eq!(
        opened
            .connection
            .as_ref()
            .and_then(|connection| connection.terminal_shell),
        Some(crate::workspace::tab::TerminalShellType::Auto)
    );
    assert_eq!(
        opened
            .connection
            .as_ref()
            .and_then(|connection| connection.cwd.as_deref()),
        None
    );
}

#[test]
fn leaves_an_open_shell_in_the_directory_it_is_in_when_edited() {
    let open = tab(json!({
        "type": "terminal",
        "title": "web",
        "connection": {
            "type": "terminal",
            "profileId": "p1",
            "profileName": "web",
            "cwd": "/srv/web/src"
        }
    }));
    let updated = apply_profile_to_tab(
        &open,
        &profile(json!({
            "connection_type": "terminal",
            "terminal_theme": "dracula",
            "local_terminal": {
                "shell": "wsl",
                "wslDistro": "Ubuntu",
                "startDirectory": "~/web"
            }
        })),
    );
    assert_eq!(
        updated
            .connection
            .as_ref()
            .and_then(|connection| connection.cwd.as_deref()),
        Some("/srv/web/src")
    );
    assert_eq!(
        updated
            .connection
            .as_ref()
            .and_then(|connection| connection.terminal_theme.as_deref()),
        Some("dracula")
    );
}
