use super::apply::{
    apply_database_payload, keep_device_settings, merge_arrays_by_keys, to_backup_value,
};
use super::archive::{
    build_archive_for_version, decode_archive, decrypt_payload, derive_key, encrypt_payload,
    validate_relative_path,
};
use super::automatic::{adopt_automatic_backup_settings, read_automatic_backup_settings};
use super::export::{filter_frontend_state, secret_travels, strip_sensitive_fields};
use super::snapshot::{restore_database, snapshot_database, DatabaseSnapshot};
use super::*;
use crate::config::AppConfig;
use crate::db::meta;
use crate::ssh::secret_store::{
    crypto::SecretKey as DataKey, store::get_secret, store::put_secret, store::snapshot_secrets,
};
use chrono::Utc;
use std::fs;

fn db_profile(id: &str, name: &str, group: &str) -> Value {
    serde_json::json!({
        "id": id, "name": name, "group": group, "connection_type": "ssh",
        "host": format!("{id}.example"), "port": 22, "username": "root",
        "auth_method": "password", "private_key_path": null
    })
}

fn db_known_host(host: &str, fingerprint: &str) -> Value {
    serde_json::json!({
        "profile_id": null, "profile_name": host, "host": host, "port": 22,
        "algorithm": "ssh-ed25519", "fingerprint": fingerprint, "trusted_at": 1
    })
}

fn database_selection() -> BackupSelection {
    BackupSelection {
        profiles: true,
        known_hosts: true,
        sftp_directories: true,
        ..BackupSelection::default()
    }
}

fn import_options(strategy: &str) -> BackupImportOptions {
    BackupImportOptions {
        selection: database_selection(),
        backup_password: None,
        use_webdav_password: false,
        conflict_strategy: strategy.to_string(),
    }
}

/// Seeds the database with profiles a and b, group "ops", tunnel t1, proxy
/// x, a known host and an SFTP directory.
fn seed_database(connection: &rusqlite::Connection) {
    let payload = {
        let mut payload = BackupPayload::default();
        payload.profiles = Some(serde_json::json!([
            db_profile("a", "Alpha", "ops"),
            db_profile("b", "Beta", "")
        ]));
        payload.profile_groups = Some(serde_json::json!(["ops"]));
        payload.tunnels = Some(serde_json::json!([{
            "id": "t1", "name": "db", "profileId": "a", "kind": "local",
            "bindHost": "127.0.0.1", "bindPort": 5432,
            "destHost": "db", "destPort": 5432
        }]));
        payload.proxies = Some(serde_json::json!([{
            "id": "x", "name": "Office", "kind": "http", "host": "proxy.corp", "port": 8080
        }]));
        payload.known_hosts =
            Some(serde_json::json!({ "entries": [db_known_host("a.example", "old")] }));
        payload.sftp_directories = Some(serde_json::json!({ "entries": [
            { "host": "a.example", "port": 22, "username": "root", "last_path": "/a" }
        ]}));
        payload
    };
    apply_database_payload(connection, &payload, &import_options("replace")).expect("seed");
}

fn incoming_payload() -> BackupPayload {
    let mut payload = BackupPayload::default();
    payload.profiles = Some(serde_json::json!([
        db_profile("b", "Beta renamed", "dev"),
        db_profile("c", "Gamma", "dev")
    ]));
    payload.profile_groups = Some(serde_json::json!(["dev"]));
    payload.known_hosts = Some(serde_json::json!({ "entries": [
        db_known_host("a.example", "old"),
        db_known_host("c.example", "new")
    ]}));
    payload.sftp_directories = Some(serde_json::json!({ "entries": [
        { "host": "a.example", "port": 22, "username": "root", "last_path": "/b" }
    ]}));
    payload
}

fn profile_summary(connection: &rusqlite::Connection) -> Vec<(String, String, String)> {
    crate::profiles::storage::list_saved_profiles(connection)
        .unwrap()
        .into_iter()
        .map(|p| (p.id, p.name, p.group))
        .collect()
}

fn owned(rows: &[(&str, &str, &str)]) -> Vec<(String, String, String)> {
    rows.iter()
        .map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string()))
        .collect()
}

fn webdav_settings(url: &str) -> remote::WebDavBackupSettings {
    remote::WebDavBackupSettings {
        url: url.to_string(),
        username: "me".to_string(),
        ..remote::WebDavBackupSettings::default()
    }
}

fn settings_and_session_selection() -> BackupSelection {
    BackupSelection {
        settings: true,
        session: true,
        themes: true,
        ..BackupSelection::default()
    }
}

/// Settings, upload history, a session and a theme as this device has them.
fn seed_settings(connection: &rusqlite::Connection) -> Result<(), String> {
    write_automatic_backup_settings(
        connection,
        &AutomaticBackupSettings {
            frequency: "weekly".to_string(),
            ..AutomaticBackupSettings::default()
        },
    )?;
    meta::set(connection, AUTOMATIC_LAST_BACKUP_AT_KEY, &7_i64)?;
    remote::restore_settings(connection, webdav_settings("https://a.example/dav"))?;
    meta::set(connection, "backup.webdav.last_backup_at", &8_i64)?;
    meta::set(connection, remote::LAST_ERROR_KEY, &"timeout")?;
    crate::sync::restore_settings(
        connection,
        &crate::sync::SyncSettings {
            enabled: true,
            ..crate::sync::SyncSettings::default()
        },
    )?;
    crate::session::write_session(
        connection,
        crate::session::SessionData {
            active_tab_id: Some("mine".to_string()),
            ..crate::session::SessionData::default()
        },
    )?;
    crate::app_state::set_collapsed_profile_group_keys(connection, &["mine".to_string()])?;
    crate::themes::replace_themes(connection, &[serde_json::json!({"id": "mine"})])
}

fn settings_summary(connection: &rusqlite::Connection) -> Value {
    let automatic = read_automatic_backup_settings(connection).unwrap();
    let webdav = remote::read_settings(connection).unwrap();
    serde_json::json!({
        "automatic": [automatic.frequency, automatic.last_backup_at],
        "webdav": [webdav.url, webdav.last_backup_at, webdav.last_error],
        "sync": crate::sync::read_settings(connection).unwrap().enabled,
        "session": crate::session::read_session(connection).unwrap().and_then(|s| s.active_tab_id),
        "themes": crate::themes::list_themes(connection).unwrap(),
        "collapsed": crate::app_state::collapsed_profile_group_keys(connection).unwrap(),
    })
}

#[test]
fn imported_settings_and_session_land_in_the_database_and_roll_back() {
    let database = crate::db::Database::open_in_memory().unwrap();
    database
        .write(|connection| {
            seed_settings(connection)?;
            let before = settings_summary(connection);
            let snapshot = snapshot_database(connection, &settings_and_session_selection())?;

            let mut payload = BackupPayload::default();
            payload.automatic_backup = Some(AutomaticBackupSettings {
                frequency: "daily".to_string(),
                // Another device's history never replaces this one's.
                last_backup_at: Some(1),
                ..AutomaticBackupSettings::default()
            });
            payload.webdav_backup = Some(remote::WebDavBackupSettings {
                last_backup_at: Some(1),
                last_error: Some("theirs".to_string()),
                ..webdav_settings("https://b.example/dav")
            });
            payload.sync = Some(crate::sync::SyncSettings::default());
            payload.session = Some(serde_json::json!({
                "tabs": [], "active_tab_id": "theirs", "last_saved": 2
            }));
            payload.frontend_state = Some(serde_json::json!({"customThemes": [{"id": "theirs"}]}));
            payload.collapsed_profile_group_keys = Some(vec!["theirs".to_string()]);
            let options = BackupImportOptions {
                selection: settings_and_session_selection(),
                backup_password: None,
                use_webdav_password: false,
                conflict_strategy: "replace".to_string(),
            };
            apply_database_payload(connection, &payload, &options)?;

            assert_eq!(
                settings_summary(connection),
                serde_json::json!({
                    "automatic": ["daily", 7],
                    // A new server: the old server's error no longer applies.
                    "webdav": ["https://b.example/dav", 8, null],
                    "sync": false,
                    "session": "theirs",
                    "themes": [{"id": "theirs"}],
                    "collapsed": ["theirs"],
                })
            );
            // Saved settings never carry the upload history.
            let saved: Value = meta::get(connection, AUTOMATIC_SETTINGS_KEY)?.unwrap();
            assert!(saved.get("lastBackupAt").is_none());
            let saved: Value = meta::get(connection, remote::SETTINGS_KEY)?.unwrap();
            assert!(saved.get("lastBackupAt").is_none() && saved.get("lastError").is_none());

            restore_database(connection, &snapshot)?;
            assert_eq!(settings_summary(connection), before);
            Ok(())
        })
        .unwrap();
}

#[test]
fn unselected_settings_and_session_are_not_touched() {
    let database = crate::db::Database::open_in_memory().unwrap();
    database
        .write(|connection| {
            seed_settings(connection)?;
            let before = settings_summary(connection);
            let mut payload = BackupPayload::default();
            payload.sync = Some(crate::sync::SyncSettings::default());
            payload.session = Some(serde_json::json!({
                "tabs": [], "active_tab_id": "theirs", "last_saved": 2
            }));
            payload.collapsed_profile_group_keys = Some(vec!["theirs".to_string()]);
            apply_database_payload(connection, &payload, &import_options("replace"))?;
            assert_eq!(settings_summary(connection), before);
            Ok(())
        })
        .unwrap();
}

#[test]
fn merged_collapsed_groups_add_to_this_devices() {
    let database = crate::db::Database::open_in_memory().unwrap();
    database
        .write(|connection| {
            crate::app_state::set_collapsed_profile_group_keys(
                connection,
                &["ops".to_string(), "dev".to_string()],
            )?;
            let mut payload = BackupPayload::default();
            payload.collapsed_profile_group_keys =
                Some(vec!["dev".to_string(), "prod".to_string()]);
            let options = BackupImportOptions {
                selection: settings_and_session_selection(),
                backup_password: None,
                use_webdav_password: false,
                conflict_strategy: "merge".to_string(),
            };
            apply_database_payload(connection, &payload, &options)?;
            assert_eq!(
                crate::app_state::collapsed_profile_group_keys(connection)?,
                ["ops", "dev", "prod"]
            );

            // Backups made before the field existed leave the state alone.
            apply_database_payload(connection, &BackupPayload::default(), &options)?;
            assert_eq!(
                crate::app_state::collapsed_profile_group_keys(connection)?,
                ["ops", "dev", "prod"]
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn merge_import_keeps_existing_data_and_updates_by_key() {
    let database = crate::db::Database::open_in_memory().unwrap();
    database
        .write(|connection| {
            seed_database(connection);
            let imported =
                apply_database_payload(connection, &incoming_payload(), &import_options("merge"))?;
            assert_eq!(imported, 2);
            assert_eq!(
                profile_summary(connection),
                owned(&[
                    ("a", "Alpha", "ops"),
                    ("b", "Beta renamed", "dev"),
                    ("c", "Gamma", "dev")
                ])
            );
            assert_eq!(
                crate::profiles::storage::configured_profile_groups(connection)?,
                ["dev", "ops"]
            );
            // Tunnels and proxies missing from an old backup are left alone.
            assert_eq!(
                crate::tunnel::storage::list_tunnel_rules(connection)?.len(),
                1
            );
            assert_eq!(crate::proxy::storage::list_proxies(connection)?.len(), 1);
            let hosts = crate::ssh::store::list_known_hosts(connection)?.entries;
            assert_eq!(
                hosts.len(),
                2,
                "the duplicate known host is not added twice"
            );
            let dirs = crate::sftp::store::list_sftp_directories(connection)?.entries;
            assert_eq!(dirs.len(), 1);
            assert_eq!(dirs[0].last_path, "/b");
            Ok(())
        })
        .unwrap();
}

#[test]
fn merge_import_keeps_known_hosts_of_profiles_sharing_a_host_key() {
    let entry = |profile_id: Option<&str>, name: &str| {
        serde_json::json!({
            "profile_id": profile_id, "profile_name": name,
            "host": "10.0.0.1", "port": 7006,
            "algorithm": "ssh-ed25519", "fingerprint": "SHA256:same", "trusted_at": 1
        })
    };
    let mut payload = BackupPayload::default();
    payload.known_hosts = Some(serde_json::json!({ "entries": [
        entry(Some("p-ubuntu"), "ubuntu"),
        entry(Some("p-test"), "test"),
        entry(None, "jump:10.0.0.1:7006"),
        // Written before profile ids were recorded: no `profile_id` key.
        {
            "profile_name": "legacy", "host": "10.0.0.1", "port": 7006,
            "algorithm": "ssh-ed25519", "fingerprint": "SHA256:same", "trusted_at": 1
        }
    ]}));
    let mut options = import_options("merge");
    options.selection = BackupSelection {
        known_hosts: true,
        ..BackupSelection::default()
    };
    let database = crate::db::Database::open_in_memory().unwrap();
    database
        .write(|connection| {
            apply_database_payload(connection, &payload, &options)?;
            apply_database_payload(connection, &payload, &options)?;
            let names = crate::ssh::store::list_known_hosts(connection)?
                .entries
                .into_iter()
                .map(|entry| entry.profile_name)
                .collect::<Vec<_>>();
            assert_eq!(names, ["ubuntu", "test", "jump:10.0.0.1:7006", "legacy"]);
            Ok(())
        })
        .unwrap();
}

#[test]
fn merge_import_never_lets_a_name_only_entry_replace_a_profile_entry() {
    let mut payload = BackupPayload::default();
    payload.known_hosts = Some(serde_json::json!({ "entries": [
        {
            "profile_id": "p1", "profile_name": "K8s-master", "host": "10.0.0.31",
            "port": 22, "algorithm": "ssh-ed25519", "fingerprint": "SHA256:current",
            "trusted_at": 2
        },
        {
            "profile_id": null, "profile_name": "K8s-master", "host": "10.0.0.31",
            "port": 22, "algorithm": "ssh-ed25519", "fingerprint": "SHA256:stale",
            "trusted_at": 1
        }
    ]}));
    let mut options = import_options("merge");
    options.selection = BackupSelection {
        known_hosts: true,
        ..BackupSelection::default()
    };
    let database = crate::db::Database::open_in_memory().unwrap();
    database
        .write(|connection| {
            apply_database_payload(connection, &payload, &options)?;
            let entries = crate::ssh::store::list_known_hosts(connection)?.entries;
            assert_eq!(entries.len(), 2);
            assert_eq!(entries[0].profile_id.as_deref(), Some("p1"));
            assert_eq!(entries[0].fingerprint, "SHA256:current");
            Ok(())
        })
        .unwrap();
}

#[test]
fn replacing_profiles_keeps_passwords_of_profiles_that_stay() {
    let database = crate::db::Database::open_in_memory().unwrap();
    let data_key = DataKey::generate();
    let profile = |id: &str| -> SavedProfile {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "name": id,
            "connection_type": "ssh",
            "host": "example.com",
            "port": 22,
            "username": "root",
            "auth_method": "password",
        }))
        .unwrap()
    };
    let keys = |connection: &rusqlite::Connection| -> Vec<String> {
        snapshot_secrets(connection)
            .unwrap()
            .into_iter()
            .map(|row| row.key)
            .collect()
    };
    database
        .write(|connection| {
            crate::profiles::storage::replace_profiles(
                connection,
                &[profile("keep"), profile("gone")],
            )?;
            put_secret(connection, &data_key, "keep", "k")?;
            put_secret(connection, &data_key, "gone", "g")?;
            let before = snapshot_database(connection, &database_selection())?;

            crate::profiles::storage::replace_profiles(
                connection,
                &[profile("keep"), profile("new")],
            )?;
            assert_eq!(keys(connection), ["keep"]);

            // Undoing the import brings the removed profile's password back.
            restore_database(connection, &before)?;
            assert_eq!(keys(connection), ["gone", "keep"]);
            let restored = get_secret(connection, &data_key, "gone")?.unwrap();
            assert_eq!(restored.as_str(), "g");
            Ok(())
        })
        .unwrap();
}

#[test]
fn replace_import_drops_data_missing_from_the_backup() {
    let database = crate::db::Database::open_in_memory().unwrap();
    database
        .write(|connection| {
            seed_database(connection);
            apply_database_payload(connection, &incoming_payload(), &import_options("replace"))?;
            assert_eq!(
                profile_summary(connection),
                owned(&[("b", "Beta renamed", "dev"), ("c", "Gamma", "dev")])
            );
            assert_eq!(
                crate::profiles::storage::configured_profile_groups(connection)?,
                ["dev"]
            );
            assert_eq!(
                crate::tunnel::storage::list_tunnel_rules(connection)?.len(),
                1
            );
            assert_eq!(
                crate::ssh::store::list_known_hosts(connection)?
                    .entries
                    .len(),
                2
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn unselected_categories_are_not_touched() {
    let database = crate::db::Database::open_in_memory().unwrap();
    database
        .write(|connection| {
            seed_database(connection);
            let mut options = import_options("replace");
            options.selection = BackupSelection {
                known_hosts: true,
                ..BackupSelection::default()
            };
            apply_database_payload(connection, &incoming_payload(), &options)?;
            assert_eq!(profile_summary(connection).len(), 2);
            assert_eq!(
                crate::sftp::store::list_sftp_directories(connection)?.entries[0].last_path,
                "/a"
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn database_snapshot_undoes_an_import() {
    let database = crate::db::Database::open_in_memory().unwrap();
    database
        .write(|connection| {
            seed_database(connection);
            let before = snapshot_database(connection, &database_selection())?;
            let before_json = |s: &DatabaseSnapshot| {
                serde_json::json!({
                    "profiles": s.profiles.as_ref().map(|(p, g, t, x)| serde_json::json!([p, g, t, x])),
                    "known_hosts": s.known_hosts,
                    "sftp": s.sftp_directories,
                })
            };
            apply_database_payload(connection, &incoming_payload(), &import_options("replace"))?;
            restore_database(connection, &before)?;
            let after = snapshot_database(connection, &database_selection())?;
            assert_eq!(before_json(&after), before_json(&before));
            Ok(())
        })
        .unwrap();
}

#[test]
fn exported_data_imports_back_unchanged() {
    let source = crate::db::Database::open_in_memory().unwrap();
    source
        .write(|connection| {
            seed_database(connection);
            Ok(())
        })
        .unwrap();
    let payload = source
        .read(|connection| {
            Ok({
                let mut payload = BackupPayload::default();
                payload.profiles = Some(to_backup_value(
                    &crate::profiles::storage::list_saved_profiles(connection)?,
                )?);
                payload.profile_groups = Some(to_backup_value(
                    &crate::profiles::storage::configured_profile_groups(connection)?,
                )?);
                payload.tunnels = Some(to_backup_value(
                    &crate::tunnel::storage::list_tunnel_rules(connection)?,
                )?);
                payload.proxies = Some(to_backup_value(&crate::proxy::storage::list_proxies(
                    connection,
                )?)?);
                payload.known_hosts = Some(to_backup_value(&crate::ssh::store::list_known_hosts(
                    connection,
                )?)?);
                payload.sftp_directories = Some(to_backup_value(
                    &crate::sftp::store::list_sftp_directories(connection)?,
                )?);
                payload
            })
        })
        .unwrap();
    validate_payload(&payload).expect("exported payload validates");

    let target = crate::db::Database::open_in_memory().unwrap();
    target
        .write(|connection| {
            apply_database_payload(connection, &payload, &import_options("replace"))?;
            let copied = snapshot_database(connection, &database_selection())?;
            let original = source.read(|c| snapshot_database(c, &database_selection()))?;
            assert_eq!(
                serde_json::to_value(copied.profiles).unwrap(),
                serde_json::to_value(original.profiles).unwrap()
            );
            Ok(())
        })
        .unwrap();
}

/// Imports real backup files made before the database existed. Set
/// `TTERM_BACKUP_FIXTURES` to a `:`-separated list of copies.
#[test]
#[ignore]
fn real_pre_database_backups_import() {
    let paths = std::env::var("TTERM_BACKUP_FIXTURES").expect("TTERM_BACKUP_FIXTURES");
    for path in paths.split(':') {
        let bundle = decode_archive(Path::new(path), None).expect("decode backup");
        eprintln!("{path}: manifest {:?}", bundle.manifest.selection);
        let Some(payload) = bundle.payload.as_ref() else {
            eprintln!("  encrypted, skipped");
            continue;
        };
        validate_payload(payload).expect("old backup validates");
        let expected_profiles = payload
            .profiles
            .as_ref()
            .map(|value| {
                serde_json::from_value::<Vec<SavedProfile>>(value.clone())
                    .unwrap()
                    .len()
            })
            .unwrap_or(0);
        for strategy in ["replace", "merge"] {
            let database = crate::db::Database::open_in_memory().unwrap();
            database
                .write(|connection| {
                    let mut options = import_options(strategy);
                    options.selection = BackupSelection {
                        profiles: payload.profiles.is_some(),
                        known_hosts: payload.known_hosts.is_some(),
                        sftp_directories: payload.sftp_directories.is_some(),
                        ..BackupSelection::default()
                    };
                    // Merging twice must not duplicate anything.
                    apply_database_payload(connection, payload, &options)?;
                    apply_database_payload(connection, payload, &options)?;
                    let profiles = crate::profiles::storage::list_saved_profiles(connection)?.len();
                    let known = crate::ssh::store::list_known_hosts(connection)?.entries.len();
                    let sftp = crate::sftp::store::list_sftp_directories(connection)?
                        .entries
                        .len();
                    eprintln!(
                        "  {strategy}: {profiles} profiles, {known} known hosts, {sftp} SFTP dirs, tunnels in backup: {}",
                        payload.tunnels.is_some()
                    );
                    assert_eq!(profiles, expected_profiles);
                    if let Some(value) = payload.known_hosts.as_ref() {
                        let expected = value["entries"].as_array().map_or(0, Vec::len);
                        assert_eq!(known, expected, "{strategy} keeps every known host");
                    }
                    Ok(())
                })
                .unwrap();
        }
    }
}

#[test]
fn encrypted_payload_round_trip_and_rejects_wrong_password() {
    let plaintext = br#"{"profiles":[],"secrets":[{"key":"p","password":"secret"}]}"#;
    let (ciphertext, crypto) = encrypt_payload(plaintext, "correct horse battery staple").unwrap();
    assert_ne!(ciphertext, plaintext);
    assert_eq!(
        decrypt_payload(&ciphertext, "correct horse battery staple", &crypto).unwrap(),
        plaintext
    );
    assert!(decrypt_payload(&ciphertext, "wrong password", &crypto).is_err());
}

#[test]
fn profile_merge_updates_by_id_and_preserves_other_profiles() {
    let existing = serde_json::json!([
        {"id":"1","name":"old"},
        {"id":"2","name":"keep"}
    ]);
    let incoming = serde_json::json!([
        {"id":"1","name":"new"},
        {"id":"3","name":"add"}
    ]);
    let merged = merge_arrays_by_keys(existing, incoming, &["id"]).unwrap();
    assert_eq!(merged.as_array().unwrap().len(), 3);
    assert_eq!(merged[0]["name"], "new");
}

#[test]
fn backups_without_tunnels_still_load_and_validate() {
    let payload: BackupPayload =
        serde_json::from_str(r#"{"profiles":[]}"#).expect("old payload should deserialize");
    assert!(payload.tunnels.is_none());
    validate_payload(&payload).expect("old payload should validate");
}

#[test]
fn tunnel_data_is_validated_on_import() {
    let mut payload = BackupPayload::default();
    payload.tunnels = Some(serde_json::json!([{
        "id": "t1", "name": "db", "profileId": "p1", "kind": "local",
        "bindHost": "127.0.0.1", "bindPort": 5432,
        "destHost": "db.internal", "destPort": 5432
    }]));
    validate_payload(&payload).expect("well-formed tunnels are accepted");

    payload.tunnels = Some(serde_json::json!([{ "id": "t1", "kind": "sideways" }]));
    assert!(validate_payload(&payload).is_err());
}

#[test]
fn kdf_parameters_are_bounded() {
    assert!(derive_key("password", &[0; 16], 262_145, 3, 1).is_err());
    assert!(derive_key("password", &[0; 16], 65_536, 11, 1).is_err());
}

#[test]
fn terminal_log_paths_reject_traversal_and_platform_separators() {
    assert!(validate_relative_path("session/a.log").is_ok());
    assert!(validate_relative_path("../secret.txt").is_err());
    assert!(validate_relative_path("session\\a.log").is_err());
    assert!(validate_relative_path("/absolute.log").is_err());
}

#[test]
fn automatic_backups_never_accept_credentials() {
    let mut settings = AutomaticBackupSettings::default();
    settings.selection.secrets = true;
    assert!(validate_automatic_backup_settings(&settings).is_err());
}

#[test]
fn imported_schedule_keeps_this_devices_folder_unless_the_backups_exists() {
    let existing = std::env::temp_dir().to_string_lossy().into_owned();
    let current = AutomaticBackupSettings {
        directory: existing.clone(),
        ..AutomaticBackupSettings::default()
    };
    let adopt = |directory: &str| {
        let imported = AutomaticBackupSettings {
            frequency: "weekly".to_string(),
            directory: directory.to_string(),
            ..AutomaticBackupSettings::default()
        };
        adopt_automatic_backup_settings(imported, current.clone())
    };
    let missing = std::env::temp_dir().join("tterm-no-such-backup-folder");
    let adopted = adopt(&missing.to_string_lossy());
    assert_eq!(adopted.directory, existing);
    assert_eq!(adopted.frequency, "weekly");
    assert_eq!(adopt("relative/folder").directory, existing);
    // The default folder is valid on every device.
    assert_eq!(adopt("").directory, "");
    assert_eq!(adopt(&existing).directory, existing);
}

#[test]
fn imported_settings_keep_this_devices_unlock_mode_and_missing_paths() {
    let existing = std::env::temp_dir().to_string_lossy().into_owned();
    let missing = std::env::temp_dir()
        .join("tterm-no-such-path")
        .to_string_lossy()
        .into_owned();
    let current = AppConfig {
        secret_storage_mode: "this-device".to_string(),
        terminal_log_directory: existing.clone(),
        zmodem_download_directory: existing.clone(),
        terminal_shell: "pwsh".to_string(),
        ..AppConfig::default()
    };
    let mut imported = AppConfig {
        secret_storage_mode: "other-device".to_string(),
        terminal_log_directory: missing.clone(),
        zmodem_download_directory: String::new(),
        terminal_shell: "custom".to_string(),
        terminal_shell_custom_path: missing,
        terminal_shell_custom_args: "--login".to_string(),
        font_size: 21,
        ..AppConfig::default()
    };
    keep_device_settings(&mut imported, current.clone());
    assert_eq!(imported.secret_storage_mode, "this-device");
    assert_eq!(imported.terminal_log_directory, existing);
    assert_eq!(imported.zmodem_download_directory, "");
    assert_eq!(imported.terminal_shell, "pwsh");
    assert_eq!(imported.terminal_shell_custom_path, "");
    assert_eq!(imported.terminal_shell_custom_args, "");
    assert_eq!(imported.font_size, 21);

    // A shell found through the PATH is taken as it is.
    let mut imported = AppConfig {
        terminal_shell: "custom".to_string(),
        terminal_shell_custom_path: "nu.exe".to_string(),
        ..AppConfig::default()
    };
    keep_device_settings(&mut imported, current);
    assert_eq!(imported.terminal_shell, "custom");
    assert_eq!(imported.terminal_shell_custom_path, "nu.exe");
}

#[test]
fn imported_settings_bring_their_tab_icons_along() {
    let mut imported = AppConfig::default();
    imported.tab_icons.enabled = false;
    imported.tab_icons.shells.insert(
        "pwsh".to_string(),
        crate::config::tab_icons::TabIconStyle {
            icon: Some("rocket".to_string()),
            color: Some("#e5534b".to_string()),
        },
    );
    let expected = imported.tab_icons.clone();

    keep_device_settings(&mut imported, AppConfig::default());
    assert_eq!(imported.tab_icons, expected);
}

#[test]
fn imported_settings_bring_their_agent_badge_and_spinner_style_along() {
    let mut exported = AppConfig::default();
    exported.tab_agent_indicator.working = "braille".to_string();
    exported.tab_agent_indicator.waiting = "still".to_string();
    exported.tab_agent_indicator.working_color = "#a371f7".to_string();
    let expected = exported.tab_agent_indicator.clone();

    // A backup carries config.json as JSON and reads it back as an AppConfig.
    let mut imported: AppConfig =
        serde_json::from_value(serde_json::to_value(&exported).unwrap()).unwrap();
    keep_device_settings(&mut imported, AppConfig::default());
    assert_eq!(imported.tab_agent_indicator, expected);
}

#[test]
fn missing_key_files_are_listed_once_with_the_profiles_using_them() {
    let present = std::env::current_exe().unwrap();
    let present = present.to_string_lossy();
    let profile = |name: &str, auth: &str, key: &str| {
        serde_json::json!({
            "id": name, "name": name, "connection_type": "ssh", "host": "example.com",
            "port": 22, "username": "root", "auth_method": auth, "private_key_path": key
        })
    };
    let mut through_jump = profile("via-jump", "password", "");
    through_jump["jump_hosts"] = serde_json::json!([{
        "host": "bastion", "port": 22, "username": "root",
        "auth_method": "key", "private_key_path": "/keys/jump"
    }]);
    let mut unused_jump = through_jump.clone();
    unused_jump["name"] = "jump-off".into();
    unused_jump["use_jump_host"] = false.into();
    let profiles: Vec<SavedProfile> = serde_json::from_value(serde_json::json!([
        profile("one", "key", "/keys/shared"),
        profile("two", "key", " /keys/shared "),
        profile("has-key", "key", &present),
        // A path left over from before the profile switched to a password.
        profile("password", "password", "/keys/stale"),
        through_jump,
        unused_jump,
    ]))
    .unwrap();
    assert_eq!(
        missing_key_files(&profiles),
        [
            MissingKeyFile {
                path: "/keys/jump".to_string(),
                profiles: vec!["via-jump".to_string()],
            },
            MissingKeyFile {
                path: "/keys/shared".to_string(),
                profiles: vec!["one".to_string(), "two".to_string()],
            },
        ]
    );
}

#[test]
fn missing_certificate_files_are_listed_with_the_key_files() {
    let profiles: Vec<SavedProfile> = serde_json::from_value(serde_json::json!([{
        "id": "certified", "name": "certified", "connection_type": "ssh",
        "host": "example.com", "port": 22, "username": "root", "auth_method": "key",
        "private_key_path": "/keys/id", "certificate_path": "/keys/id-ca-cert.pub",
        "jump_hosts": [{
            "host": "bastion", "port": 22, "username": "root", "auth_method": "key",
            "private_key_path": "/keys/jump", "certificate_path": "/keys/jump-cert.pub"
        }]
    }]))
    .unwrap();
    let paths = missing_key_files(&profiles)
        .into_iter()
        .map(|missing| missing.path)
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        [
            "/keys/id",
            "/keys/id-ca-cert.pub",
            "/keys/jump",
            "/keys/jump-cert.pub"
        ]
    );
}

#[test]
fn device_setup_secrets_stay_unless_webdav_settings_travel() {
    assert!(secret_travels("profile-id", false));
    assert!(secret_travels("profile-id:jump", false));
    assert!(secret_travels("Old name:jump", false));
    assert!(!secret_travels("sync:base", true));
    for key in remote::SECRET_KEYS {
        assert!(secret_travels(key, true));
        assert!(!secret_travels(key, false));
    }
}

#[test]
fn backup_and_sync_setup_round_trips_with_its_passwords() {
    let selection = BackupSelection {
        settings: true,
        profiles: true,
        secrets: true,
        ..BackupSelection::default()
    };
    let mut payload = BackupPayload::default();
    payload.webdav_backup = Some(remote::WebDavBackupSettings {
        url: "https://dav.example.com/dav/".to_string(),
        username: "stone".to_string(),
        ..remote::WebDavBackupSettings::default()
    });
    payload.automatic_backup = Some(AutomaticBackupSettings::default());
    payload.sync = Some(crate::sync::SyncSettings {
        enabled: true,
        ..crate::sync::SyncSettings::default()
    });
    for key in remote::SECRET_KEYS {
        payload.secrets.push(MigrationSecretRecord {
            key: key.to_string(),
            password: "dav-secret".to_string(),
        });
    }
    let archive = build_archive_for_version("test", &selection, &payload, Some("password123"))
        .expect("build encrypted archive");
    let path = std::env::temp_dir().join(format!(
        "tterm-backup-setup-test-{}-{}.tterm-backup",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    fs::write(&path, archive).expect("write archive");
    let decoded = decode_archive(&path, Some("password123")).expect("decode archive");
    fs::remove_file(path).expect("remove archive");
    let decoded = decoded.payload.unwrap();
    assert_eq!(
        decoded.webdav_backup.as_ref().unwrap().url,
        "https://dav.example.com/dav/"
    );
    assert_eq!(decoded.automatic_backup.as_ref().unwrap().frequency, "off");
    assert!(decoded.sync.as_ref().unwrap().enabled);
    assert_eq!(decoded.secrets.len(), 2);
}

#[test]
fn frontend_state_is_filtered_by_selected_category() {
    let state = serde_json::json!({
        "customThemes": [{"id":"theme"}],
        "recentCommands": [{"id":"recent"}],
        "recentQuickConnections": [{"host":"example.com"}],
        "sftpColumnWidths": [1, 2],
        "sftpView": {"showHidden": false},
        "untrusted": "ignored"
    });
    let selection = BackupSelection {
        settings: true,
        ..Default::default()
    };
    let filtered = filter_frontend_state(&selection, Some(state)).unwrap();
    assert!(filtered.get("customThemes").is_none());
    assert!(filtered.get("recentCommands").is_some());
    assert!(filtered.get("recentQuickConnections").is_some());
    assert!(filtered.get("sftpView").is_some());
    assert!(filtered.get("untrusted").is_none());
}

#[test]
fn session_sanitizer_removes_legacy_inline_credentials() {
    let value = serde_json::json!({
        "tabs": [{"connection": {"password": "do-not-export", "host": "example"}}],
        "secretStorage": "memory"
    });
    let cleaned = strip_sensitive_fields(value);
    assert!(cleaned["tabs"][0]["connection"].get("password").is_none());
    assert!(cleaned.get("secretStorage").is_none());
}

#[test]
fn encrypted_archive_round_trips_manifest_and_credentials() {
    let selection = BackupSelection {
        profiles: true,
        secrets: true,
        ..Default::default()
    };
    let mut payload = BackupPayload::default();
    payload.profiles = Some(serde_json::json!([]));
    payload.secrets.push(MigrationSecretRecord {
        key: "profile-id".to_string(),
        password: "top-secret".to_string(),
    });
    let archive = build_archive_for_version("test", &selection, &payload, Some("password123"))
        .expect("build encrypted archive");
    assert!(!archive
        .windows(b"top-secret".len())
        .any(|value| value == b"top-secret"));

    let path = std::env::temp_dir().join(format!(
        "tterm-backup-test-{}-{}.tterm-backup",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    fs::write(&path, archive).expect("write archive");
    let decoded = decode_archive(&path, Some("password123")).expect("decode archive");
    assert!(decoded.manifest.encrypted);
    assert_eq!(decoded.payload.unwrap().secrets.len(), 1);
    assert!(decode_archive(&path, Some("wrong-password")).is_err());
    fs::remove_file(path).expect("remove archive");
}
