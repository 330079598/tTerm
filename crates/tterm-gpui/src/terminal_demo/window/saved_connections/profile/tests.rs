//! Persistence rules for the saved SSH connection form.

use super::{
    AuthKind, NormalizedDraft, ProfileDraft, build_profile, draft_from_profile,
    draft_from_quick_profile, load_profile,
};
use rusqlite::Connection;
use serde_json::{Value, json};
use tterm_core::connections::quick_connect::{SshConfigHost, draft_profile_from_input};
use tterm_core::db::Database;
use tterm_core::profiles::groups::{
    create_group, group_paths, list_groups, move_group, resolve_path,
};
use tterm_core::profiles::storage::{encode_profile, get_profile, upsert_profile};
use tterm_core::profiles::types::SavedProfile;
use tterm_core::proxy::ProxyChoice;

type DraftChange = fn(&mut ProfileDraft);

fn profile_json(id: &str) -> Value {
    json!({
        "id": id,
        "name": "Prod database",
        "group_id": "ops",
        "connection_type": "ssh",
        "host": "db.internal",
        "port": 2222,
        "username": "deploy",
        "remember_password": true,
        "ignore_saved_password": false,
        "auth_method": "password",
        "agent_forward": true,
        "keepalive_interval_secs": 30,
        "keepalive_count_max": 5,
        "server_monitor_visible": true,
        "use_jump_host": true,
        "jump_hosts": [{
            "host": "bastion.internal",
            "port": 22,
            "username": "ops",
            "auth_method": "password"
        }],
        "sudo_autofill": false,
        "encoding": "gbk",
        "terminal_theme": "Solarized Dark",
        "login_script": "uptime",
        "ssh_shell_integration": true,
        "session_log": "always",
        "sftp_start_directory": "home",
        "highlight_set": "warnings",
        "proxy": { "mode": "direct" },
        "tab_icon": { "icon": "distro:arch", "color": "#7aa2f7" },
        "distro": "debian"
    })
}

fn profile() -> SavedProfile {
    serde_json::from_value(profile_json("profile-1")).expect("password profile fixture")
}

fn key_profile() -> SavedProfile {
    let mut value = profile_json("key-profile");
    value["remember_password"] = json!(false);
    value["auth_method"] = json!("key");
    value["private_key_path"] = json!("/keys/id_ed25519");
    value["certificate_path"] = json!("/keys/id_ed25519-cert.pub");
    serde_json::from_value(value).expect("key profile fixture")
}

fn draft(id: Option<&str>) -> ProfileDraft {
    ProfileDraft {
        id: id.map(str::to_string),
        name: "Prod database".to_string(),
        host: "db.internal".to_string(),
        port: "2222".to_string(),
        username: "deploy".to_string(),
        auth: AuthKind::Password,
        key_path: String::new(),
        group_path: String::new(),
        group_changed: false,
    }
}

fn grouped_draft(group_path: &str) -> ProfileDraft {
    ProfileDraft {
        group_path: group_path.to_string(),
        group_changed: true,
        ..draft(None)
    }
}

fn draft_for(profile: &SavedProfile) -> ProfileDraft {
    draft_from_profile(profile).expect("editable profile")
}

fn save(existing: Option<&SavedProfile>, draft: ProfileDraft) -> SavedProfile {
    let draft = NormalizedDraft::parse(draft).expect("valid draft");
    build_profile(existing, draft).expect("saved profile")
}

fn invalid(draft: ProfileDraft) -> String {
    NormalizedDraft::parse(draft).expect_err("invalid draft")
}

fn with_database(test: impl FnOnce(&Connection) -> Result<(), String>) {
    let database = Database::open_in_memory().expect("open in-memory database");
    database
        .write(|connection| test(connection))
        .expect("database test");
}

/// Mirrors the group filing the core profile service performs on save: a path
/// is resolved in the same transaction that writes the profile.
fn write_profile(
    connection: &Connection,
    mut profile: SavedProfile,
) -> Result<SavedProfile, String> {
    if let Some(path) = profile.group_path.take() {
        profile.group_id = resolve_path(connection, &path)?;
    }
    upsert_profile(connection, &profile)?;
    Ok(profile)
}

fn uuid_like(id: &str) -> bool {
    let lengths = [8, 4, 4, 4, 12];
    let parts: Vec<&str> = id.split('-').collect();
    parts.len() == lengths.len()
        && parts
            .iter()
            .zip(lengths)
            .all(|(part, length)| part.len() == length)
        && id
            .chars()
            .all(|character| character.is_ascii_hexdigit() || character == '-')
}

#[test]
fn the_default_form_is_password_on_port_22() {
    let draft = ProfileDraft::default();
    assert_eq!(draft.id, None);
    assert_eq!(draft.port, "22");
    assert_eq!(draft.auth, AuthKind::Password);
    assert!(draft.name.is_empty());
    assert!(draft.host.is_empty());
    assert!(draft.username.is_empty());
    assert!(draft.key_path.is_empty());
    assert!(draft.group_path.is_empty());
    assert!(!draft.group_changed);
}

#[test]
fn a_new_profile_gets_a_real_id_and_never_a_saved_password() {
    let first = save(None, draft(None));
    let second = save(None, draft(None));
    assert!(uuid_like(&first.id));
    assert!(uuid_like(&second.id));
    assert_ne!(first.id, second.id);
    assert_eq!(first.connection_type, "ssh");
    assert_eq!(first.auth_method.as_deref(), Some("password"));
    assert_eq!(first.keepalive_interval_secs, 15);
    assert_eq!(first.keepalive_count_max, 3);
    assert_eq!(first.use_jump_host, Some(false));
    assert!(!first.remember_password);
    assert!(!first.ignore_saved_password);
    assert!(first.sudo_autofill);
    assert!(first.password.is_none());
    assert!(first.sudo_password.is_none());
    assert!(first.group_id.is_none());
    assert!(first.group_path.is_none());
}

#[test]
fn a_draft_without_an_id_creates_a_new_profile_instead_of_reusing_one() {
    let existing = profile();
    let created = save(Some(&existing), draft(None));
    assert_ne!(created.id, existing.id);
    assert_eq!(created.name, existing.name);
    assert!(created.group_id.is_none());
    assert!(created.jump_hosts.is_empty());
}

#[test]
fn loading_reads_the_editable_fields_without_a_secret() {
    let mut value = profile_json("profile-1");
    value.as_object_mut().expect("object").remove("auth_method");
    value.as_object_mut().expect("object").remove("port");
    let profile: SavedProfile = serde_json::from_value(value).expect("profile");

    let draft = draft_for(&profile);

    assert_eq!(draft.id.as_deref(), Some("profile-1"));
    assert_eq!(draft.name, "Prod database");
    assert_eq!(draft.host, "db.internal");
    assert_eq!(draft.port, "22");
    assert_eq!(draft.username, "deploy");
    assert_eq!(draft.auth, AuthKind::Password);
    assert!(draft.key_path.is_empty());
}

#[test]
fn loading_key_authentication_returns_its_key_path() {
    let draft = draft_for(&key_profile());
    assert_eq!(draft.auth, AuthKind::Key);
    assert_eq!(draft.key_path, "/keys/id_ed25519");
}

#[test]
fn the_launcher_draft_prefills_only_the_safe_address_fields() {
    let profile =
        draft_profile_from_input("tester@db.internal:2222", &[]).expect("a typed endpoint");

    let draft = draft_from_quick_profile(profile);

    assert_eq!(draft.id, None, "the form must only ever create a profile");
    assert_eq!(draft.name, "tester@db.internal:2222");
    assert_eq!(draft.host, "db.internal");
    assert_eq!(draft.port, "2222");
    assert_eq!(draft.username, "tester");
    assert_eq!(draft.auth, AuthKind::Password);
    assert!(draft.key_path.is_empty());
    assert!(draft.group_path.is_empty());
    assert!(!draft.group_changed);
}

#[test]
fn the_launcher_draft_reads_an_alias_like_the_quick_row_does() {
    let host = SshConfigHost {
        host_pattern: "prod".to_string(),
        name: "Production".to_string(),
        host: Some("prod.internal".to_string()),
        port: 2200,
        username: Some("alice".to_string()),
        private_key_path: None,
        agent_forward: false,
        keepalive_interval_secs: 15,
        keepalive_count_max: 3,
        jump_hosts: Vec::new(),
        skipped: false,
    };
    let profile = draft_profile_from_input("prod", &[host]).expect("a config alias");

    let draft = draft_from_quick_profile(profile);

    assert_eq!(draft.name, "alice@Production");
    assert_eq!(draft.host, "prod.internal");
    assert_eq!(draft.port, "2200");
    assert_eq!(draft.username, "alice");
}

#[test]
fn text_that_is_not_a_connection_has_no_launcher_draft() {
    assert!(draft_profile_from_input("prod db", &[]).is_none());
    assert!(draft_profile_from_input("", &[]).is_none());
}

#[test]
fn unsupported_authentication_is_rejected_instead_of_downgraded() {
    for method in ["agent", "interactive", "auto", ""] {
        let mut value = profile_json("profile-1");
        value["auth_method"] = json!(method);
        let profile: SavedProfile = serde_json::from_value(value).expect("profile fixture");

        let load_error = draft_from_profile(&profile).expect_err("load must reject the method");
        assert!(load_error.contains("cannot edit"), "{method}: {load_error}");
        let save_error = save_error(Some(&profile), draft(Some("profile-1")));
        assert!(save_error.contains("cannot edit"), "{method}: {save_error}");
    }
}

fn save_error(existing: Option<&SavedProfile>, draft: ProfileDraft) -> String {
    let draft = NormalizedDraft::parse(draft).expect("valid draft");
    build_profile(existing, draft).expect_err("save must be rejected")
}

#[test]
fn a_non_ssh_profile_is_not_overwritten_and_a_missing_id_is_not_resurrected() {
    let database = Database::open_in_memory().expect("open in-memory database");
    database
        .write(|connection| {
            let mut value = profile_json("local-1");
            value["connection_type"] = json!("terminal");
            let local: SavedProfile = serde_json::from_value(value).expect("local profile");
            upsert_profile(connection, &local)?;

            let error = save_error(Some(&local), draft(Some("local-1")));
            assert!(error.contains("Only SSH"), "{error}");
            let stored = get_profile(connection, "local-1")?.expect("local profile");
            assert_eq!(stored.connection_type, "terminal");

            let error = save_error(None, draft(Some("missing")));
            assert!(error.contains("no longer exists"), "{error}");
            assert!(get_profile(connection, "missing")?.is_none());
            Ok(())
        })
        .expect("database checks");
}

#[test]
fn editing_keeps_the_settings_the_form_does_not_show() {
    let existing = profile();
    let mut edited = draft_for(&existing);
    edited.name = "Renamed database".to_string();

    let saved = save(Some(&existing), edited);

    assert_eq!(saved.id, existing.id);
    assert_eq!(saved.name, "Renamed database");
    assert_eq!(saved.group_id.as_deref(), Some("ops"));
    assert!(saved.agent_forward);
    assert_eq!(saved.keepalive_interval_secs, 30);
    assert_eq!(saved.keepalive_count_max, 5);
    assert!(saved.server_monitor_visible);
    assert_eq!(saved.use_jump_host, Some(true));
    assert_eq!(saved.jump_hosts.len(), 1);
    assert!(!saved.sudo_autofill);
    assert_eq!(saved.encoding.as_deref(), Some("gbk"));
    assert_eq!(saved.terminal_theme.as_deref(), Some("Solarized Dark"));
    assert_eq!(saved.login_script.as_deref(), Some("uptime"));
    assert_eq!(saved.ssh_shell_integration, Some(true));
    assert_eq!(saved.session_log.as_deref(), Some("always"));
    assert_eq!(saved.sftp_start_directory.as_deref(), Some("home"));
    assert_eq!(saved.highlight_set.as_deref(), Some("warnings"));
    assert_eq!(saved.proxy, ProxyChoice::Direct);
    assert_eq!(
        saved
            .tab_icon
            .as_ref()
            .and_then(|icon| icon.icon.as_deref()),
        Some("distro:arch")
    );
    assert_eq!(saved.distro.as_deref(), Some("debian"));
    assert!(saved.remember_password);
    assert!(!saved.ignore_saved_password);
    assert!(!saved.clear_sudo_password);
    assert!(saved.password.is_none());
    assert!(saved.sudo_password.is_none());
    assert!(saved.private_key_passphrase.is_none());
}

#[test]
fn an_edit_round_trips_through_a_disposable_database_without_a_secret() {
    let database = Database::open_in_memory().expect("open in-memory database");
    database
        .write(|connection| {
            let existing = profile();
            upsert_profile(connection, &existing)?;

            let mut edited = draft_for(&existing);
            edited.username = "release".to_string();
            let saved = save(Some(&existing), edited);
            upsert_profile(connection, &saved)?;

            let stored = get_profile(connection, &saved.id)?.expect("saved profile");
            assert_eq!(stored.id, existing.id);
            assert_eq!(stored.username.as_deref(), Some("release"));
            assert_eq!(stored.group_id.as_deref(), Some("ops"));
            assert_eq!(stored.jump_hosts.len(), 1);
            assert!(stored.password.is_none());
            assert!(stored.sudo_password.is_none());
            assert!(!encode_profile(&stored)?.contains("\"password\":"));
            Ok(())
        })
        .expect("database round trip");
}

#[test]
fn changing_the_endpoint_turns_off_the_stored_credentials() {
    let existing = profile();
    let changes: [(&str, DraftChange); 3] = [
        ("host", |draft| draft.host = "other.internal".to_string()),
        ("port", |draft| draft.port = "2200".to_string()),
        ("username", |draft| draft.username = "release".to_string()),
    ];
    for (field, change) in changes {
        let mut edited = draft_for(&existing);
        change(&mut edited);
        let saved = save(Some(&existing), edited);

        assert!(!saved.remember_password, "{field}");
        assert!(saved.ignore_saved_password, "{field}");
        assert!(saved.clear_sudo_password, "{field}");
        assert!(saved.password.is_none(), "{field}");
        assert!(saved.sudo_password.is_none(), "{field}");
        assert_eq!(saved.group_id.as_deref(), Some("ops"), "{field}");
        assert_eq!(saved.jump_hosts.len(), 1, "{field}");
    }
}

#[test]
fn renaming_alone_keeps_the_stored_credential_settings() {
    let mut existing = profile();
    existing.ignore_saved_password = true;
    let mut edited = draft_for(&existing);
    edited.name = "Renamed database".to_string();
    let saved = save(Some(&existing), edited);
    assert!(saved.remember_password);
    assert!(saved.ignore_saved_password);
    assert!(!saved.clear_sudo_password);
}

#[test]
fn a_key_certificate_is_kept_only_for_the_same_key() {
    let existing = key_profile();
    let draft = draft_for(&existing);
    let saved = save(Some(&existing), draft.clone());
    assert_eq!(saved.auth_method.as_deref(), Some("key"));
    assert_eq!(saved.private_key_path.as_deref(), Some("/keys/id_ed25519"));
    assert_eq!(
        saved.certificate_path.as_deref(),
        Some("/keys/id_ed25519-cert.pub")
    );

    let mut changed_key = draft.clone();
    changed_key.key_path = "/keys/id_ed25519_new".to_string();
    let saved = save(Some(&existing), changed_key);
    assert_eq!(
        saved.private_key_path.as_deref(),
        Some("/keys/id_ed25519_new")
    );
    assert!(saved.certificate_path.is_none());

    let mut password = draft;
    password.auth = AuthKind::Password;
    password.key_path = "/keys/stale".to_string();
    let saved = save(Some(&existing), password);
    assert_eq!(saved.auth_method.as_deref(), Some("password"));
    assert!(saved.private_key_path.is_none());
    assert!(saved.certificate_path.is_none());
    assert!(!saved.remember_password);
}

#[test]
fn switching_a_password_profile_to_a_key_does_not_keep_a_certificate() {
    let existing = profile();
    let mut value = serde_json::to_value(&existing).expect("serialize profile");
    value["certificate_path"] = json!("/keys/old-cert.pub");
    let existing: SavedProfile = serde_json::from_value(value).expect("profile");
    let mut draft = draft_for(&existing);
    draft.auth = AuthKind::Key;
    draft.key_path = "/keys/new".to_string();
    let saved = save(Some(&existing), draft);
    assert_eq!(saved.auth_method.as_deref(), Some("key"));
    assert_eq!(saved.private_key_path.as_deref(), Some("/keys/new"));
    assert!(saved.certificate_path.is_none());
    assert!(!saved.remember_password);
}

#[test]
fn the_form_is_trimmed_and_an_empty_name_falls_back_to_the_endpoint() {
    let mut edited = draft(None);
    edited.name = "  Lab  ".to_string();
    edited.host = "  [::1]  ".to_string();
    edited.port = " 2222 ".to_string();
    edited.username = " deploy ".to_string();
    edited.auth = AuthKind::Key;
    edited.key_path = "  /keys/id_ed25519  ".to_string();
    let saved = save(None, edited);
    assert_eq!(saved.name, "Lab");
    assert_eq!(saved.host.as_deref(), Some("::1"));
    assert_eq!(saved.port, Some(2222));
    assert_eq!(saved.username.as_deref(), Some("deploy"));
    assert_eq!(saved.private_key_path.as_deref(), Some("/keys/id_ed25519"));

    let mut blank_name = draft(None);
    blank_name.name = "   ".to_string();
    blank_name.host = " lab.internal ".to_string();
    blank_name.username = " root ".to_string();
    let saved = save(None, blank_name);
    assert_eq!(saved.name, "root@lab.internal");
}

#[test]
fn hosts_users_ports_and_key_paths_are_validated_with_actionable_errors() {
    let mut empty_host = draft(None);
    empty_host.host = "   ".to_string();
    assert!(invalid(empty_host).contains("host name"));

    let mut spaced_host = draft(None);
    spaced_host.host = "two words".to_string();
    assert!(invalid(spaced_host).contains("spaces"));

    let mut user_in_host = draft(None);
    user_in_host.host = "root@db.internal".to_string();
    assert!(invalid(user_in_host).contains("without a user name"));

    let mut port_in_host = draft(None);
    port_in_host.host = "[::1]:22".to_string();
    assert!(invalid(port_in_host).contains("host name"));

    let mut empty_user = draft(None);
    empty_user.username = "  ".to_string();
    assert!(invalid(empty_user).contains("user name"));

    let mut spaced_user = draft(None);
    spaced_user.username = "two words".to_string();
    assert!(invalid(spaced_user).contains("spaces"));

    for port in ["", "0", "65536", "abc", "-1"] {
        let mut bad_port = draft(None);
        bad_port.port = port.to_string();
        assert!(invalid(bad_port).contains("port"), "{port}");
    }

    let mut missing_key = draft(None);
    missing_key.auth = AuthKind::Key;
    missing_key.key_path = "   ".to_string();
    assert!(invalid(missing_key).contains("private key"));

    let mut multiline_key = draft(None);
    multiline_key.auth = AuthKind::Key;
    multiline_key.key_path = "/keys/id\nother".to_string();
    assert!(invalid(multiline_key).contains("one line"));
}

#[test]
fn an_unchanged_group_keeps_the_stored_group_id_for_the_service_to_leave_alone() {
    let existing = profile();
    let saved = save(Some(&existing), draft_for(&existing));
    assert!(saved.group_path.is_none());
    assert_eq!(saved.group_id.as_deref(), Some("ops"));
}

#[test]
fn a_new_profile_is_filed_in_a_nested_group_created_from_the_path() {
    with_database(|connection| {
        let saved = write_profile(connection, save(None, grouped_draft(" Prod / Web / API ")))?;
        let group_id = saved.group_id.clone().expect("group id");
        let groups = list_groups(connection)?;
        let paths = group_paths(&groups);
        assert_eq!(paths[&group_id], "Prod / Web / API");
        assert_eq!(groups.len(), 3);
        assert_eq!(
            get_profile(connection, &saved.id)?
                .unwrap()
                .group_id
                .as_deref(),
            Some(group_id.as_str())
        );
        Ok(())
    });
}

#[test]
fn equivalent_paths_reuse_the_groups_they_name() {
    with_database(|connection| {
        let prod = create_group(connection, "Prod", None)?;
        let nested = create_group(connection, "Web", Some(&prod.id))?;
        // A group with the same name elsewhere must not be reused.
        create_group(connection, "Web", None)?;

        let first = write_profile(connection, save(None, grouped_draft("Prod / Web")))?;
        assert_eq!(first.group_id.as_deref(), Some(nested.id.as_str()));
        let second = write_profile(connection, save(None, grouped_draft(" Prod /Web/ ")))?;
        assert_eq!(second.group_id, first.group_id);
        assert_eq!(list_groups(connection)?.len(), 3);
        Ok(())
    });
}

#[test]
fn an_unchanged_group_survives_the_group_moving_while_the_form_is_open() {
    with_database(|connection| {
        let prod = create_group(connection, "Prod", None)?;
        let web = create_group(connection, "Web", Some(&prod.id))?;
        let staging = create_group(connection, "Staging", None)?;
        let mut existing = profile();
        existing.group_id = Some(web.id.clone());
        upsert_profile(connection, &existing)?;
        let mut sibling = profile();
        sibling.id = "profile-2".to_string();
        sibling.name = "Sibling".to_string();
        sibling.group_id = Some(web.id.clone());
        upsert_profile(connection, &sibling)?;

        let loaded = load_profile(connection, &existing.id)?;
        assert_eq!(loaded.group_path.as_deref(), Some("Prod / Web"));
        let mut draft = draft_from_profile(&loaded)?;
        draft.name = "Renamed database".to_string();

        // Another window moves the group between the load and the save. The
        // save reads the profile as it is now, as the service does.
        move_group(connection, &web.id, Some(&staging.id))?;
        let current = get_profile(connection, &existing.id)?.unwrap();
        let saved = write_profile(connection, save(Some(&current), draft))?;

        assert_eq!(saved.id, existing.id);
        assert_eq!(saved.group_id.as_deref(), Some(web.id.as_str()));
        assert_eq!(saved.name, "Renamed database");
        assert!(saved.agent_forward);
        assert_eq!(saved.keepalive_interval_secs, 30);
        assert_eq!(saved.jump_hosts.len(), 1);
        assert!(saved.remember_password);
        assert!(!saved.ignore_saved_password);
        assert_eq!(list_groups(connection)?.len(), 3, "no group is recreated");
        assert_eq!(
            get_profile(connection, "profile-2")?
                .unwrap()
                .group_id
                .as_deref(),
            Some(web.id.as_str())
        );
        assert_eq!(
            load_profile(connection, &existing.id)?
                .group_path
                .as_deref(),
            Some("Staging / Web")
        );
        Ok(())
    });
}

#[test]
fn an_unedited_profile_keeps_a_group_changed_by_another_window() {
    with_database(|connection| {
        let prod = create_group(connection, "Prod", None)?;
        let web = create_group(connection, "Web", Some(&prod.id))?;
        let dev = create_group(connection, "Dev", None)?;
        let api = create_group(connection, "API", Some(&dev.id))?;
        let mut existing = profile();
        existing.group_id = Some(web.id.clone());
        upsert_profile(connection, &existing)?;

        let mut draft = draft_from_profile(&load_profile(connection, &existing.id)?)?;
        draft.name = "Renamed database".to_string();

        // Another window files the profile in a different group while the
        // form is open; the untouched group field must not undo that.
        let mut moved = existing.clone();
        moved.group_id = Some(api.id.clone());
        upsert_profile(connection, &moved)?;
        let current = get_profile(connection, &existing.id)?.unwrap();
        let saved = write_profile(connection, save(Some(&current), draft))?;

        assert_eq!(saved.id, existing.id);
        assert_eq!(saved.group_id.as_deref(), Some(api.id.as_str()));
        assert_eq!(saved.name, "Renamed database");
        assert_eq!(list_groups(connection)?.len(), 4);
        assert_eq!(
            load_profile(connection, &existing.id)?
                .group_path
                .as_deref(),
            Some("Dev / API")
        );
        Ok(())
    });
}

#[test]
fn changing_the_group_path_moves_only_that_profile_and_keeps_its_settings() {
    with_database(|connection| {
        let prod = create_group(connection, "Prod", None)?;
        let web = create_group(connection, "Web", Some(&prod.id))?;
        let mut existing = profile();
        existing.group_id = Some(web.id.clone());
        upsert_profile(connection, &existing)?;
        let mut sibling = profile();
        sibling.id = "profile-2".to_string();
        sibling.group_id = Some(web.id.clone());
        upsert_profile(connection, &sibling)?;

        let mut draft = draft_from_profile(&load_profile(connection, &existing.id)?)?;
        draft.group_path = " Dev / API ".to_string();
        draft.group_changed = true;
        let saved = write_profile(connection, save(Some(&existing), draft))?;

        assert_eq!(saved.id, existing.id);
        assert_ne!(saved.group_id, Some(web.id.clone()));
        assert!(saved.agent_forward);
        assert_eq!(saved.keepalive_interval_secs, 30);
        assert_eq!(saved.jump_hosts.len(), 1);
        assert!(saved.remember_password);
        assert!(!saved.ignore_saved_password);
        assert_eq!(
            load_profile(connection, &existing.id)?
                .group_path
                .as_deref(),
            Some("Dev / API")
        );
        assert_eq!(
            get_profile(connection, "profile-2")?
                .unwrap()
                .group_id
                .as_deref(),
            Some(web.id.as_str())
        );
        assert_eq!(list_groups(connection)?.len(), 4);
        Ok(())
    });
}

#[test]
fn a_cleared_group_path_ungroups_only_that_profile() {
    with_database(|connection| {
        let prod = create_group(connection, "Prod", None)?;
        let web = create_group(connection, "Web", Some(&prod.id))?;
        let mut existing = profile();
        existing.group_id = Some(web.id.clone());
        upsert_profile(connection, &existing)?;
        let mut sibling = profile();
        sibling.id = "profile-2".to_string();
        sibling.group_id = Some(web.id.clone());
        upsert_profile(connection, &sibling)?;

        let mut draft = draft_from_profile(&load_profile(connection, &existing.id)?)?;
        draft.group_path.clear();
        draft.group_changed = true;
        let saved = write_profile(connection, save(Some(&existing), draft))?;

        assert_eq!(saved.id, existing.id);
        assert_eq!(saved.group_id, None);
        assert!(saved.remember_password);
        assert!(!saved.ignore_saved_password);
        assert_eq!(list_groups(connection)?.len(), 2);
        assert_eq!(
            get_profile(connection, "profile-2")?
                .unwrap()
                .group_id
                .as_deref(),
            Some(web.id.as_str())
        );
        Ok(())
    });
}

#[test]
fn loading_preloads_the_full_nested_group_path_without_a_secret() {
    with_database(|connection| {
        let prod = create_group(connection, "Prod", None)?;
        let web = create_group(connection, "Web", Some(&prod.id))?;
        let api = create_group(connection, "API", Some(&web.id))?;
        let mut existing = profile();
        existing.group_id = Some(api.id);
        existing.password = Some("hunter2".to_string());
        existing.private_key_passphrase = Some("key-secret".to_string());
        upsert_profile(connection, &existing)?;

        let loaded = load_profile(connection, &existing.id)?;
        assert_eq!(loaded.group_path.as_deref(), Some("Prod / Web / API"));
        assert!(loaded.password.is_none());
        assert!(loaded.private_key_passphrase.is_none());

        let draft = draft_from_profile(&loaded)?;
        assert_eq!(draft.group_path, "Prod / Web / API");
        assert!(!draft.group_changed);
        Ok(())
    });
}

#[test]
fn no_form_value_can_write_a_password_into_the_profile() {
    let mut existing = profile();
    existing.password = Some("hunter2".to_string());
    existing.sudo_password = Some("sudo-secret".to_string());
    existing.private_key_passphrase = Some("key-secret".to_string());
    let saved = save(Some(&existing), draft_for(&existing));

    assert!(saved.password.is_none());
    assert!(saved.sudo_password.is_none());
    assert!(saved.private_key_passphrase.is_none());
    let value = serde_json::to_value(&saved).expect("serialize profile");
    for key in ["password", "sudo_password", "private_key_passphrase"] {
        assert!(value.get(key).is_none(), "{key}");
    }
    let encoded = encode_profile(&saved).expect("encode profile");
    assert!(!encoded.contains("hunter2"));
    assert!(!encoded.contains("sudo-secret"));
    assert!(!encoded.contains("key-secret"));
}
