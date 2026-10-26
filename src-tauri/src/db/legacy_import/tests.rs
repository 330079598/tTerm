use super::*;
use crate::db::Database;

fn temp_dir(label: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "tterm-legacy-import-{label}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write(dir: &Path, name: &str, value: serde_json::Value) {
    fs::write(dir.join(name), serde_json::to_vec(&value).unwrap()).expect("write fixture");
}

#[test]
fn imports_every_file_once_and_renames_them() {
    let dir = temp_dir("all");
    write(
        &dir,
        "profiles.json",
        serde_json::json!([
            {
                "id": "b", "name": "Beta", "group": "ops", "connection_type": "ssh",
                "host": "b.example", "port": 22, "username": "root",
                "password": "leaked", "auth_method": "password", "private_key_path": null,
                "jump_host": { "host": "bastion", "port": 22, "username": "jump" }
            },
            {
                "id": "a", "name": "Alpha", "connection_type": "local",
                "host": null, "port": null, "username": null,
                "auth_method": null, "private_key_path": null
            }
        ]),
    );
    write(
        &dir,
        "profile_groups.json",
        serde_json::json!(["empty", " ops "]),
    );
    write(
        &dir,
        "tunnels.json",
        serde_json::json!([{
            "id": "t1", "name": "db", "profileId": "b", "kind": "local",
            "bindHost": "127.0.0.1", "bindPort": 5432,
            "destHost": "db.internal", "destPort": 5432
        }]),
    );
    write(
        &dir,
        "ssh_known_hosts.json",
        serde_json::json!({ "entries": [{
            "profile_id": "b", "profile_name": "Beta", "host": "b.example", "port": 22,
            "algorithm": "ssh-ed25519", "fingerprint": "SHA256:x", "trusted_at": 7
        }]}),
    );
    write(
        &dir,
        "sftp_directories.json",
        serde_json::json!({ "entries": [{
            "host": "b.example", "port": 22, "username": "root", "last_path": "/srv"
        }]}),
    );

    let database = Database::open_in_memory().expect("open database");
    assert!(import_legacy_json_files(&database, &dir).expect("import"));
    assert!(legacy_json_imported(&database).expect("marker"));

    database
        .read(|connection| {
            let profiles = crate::profiles::storage::list_saved_profiles(connection)?;
            assert_eq!(
                profiles.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
                ["b", "a"],
                "file order is kept"
            );
            assert_eq!(
                profiles[0].jump_hosts.len(),
                1,
                "legacy jump host is normalized"
            );
            assert!(profiles[0].password.is_none());
            let stored: String = connection
                .query_row("SELECT data FROM profiles WHERE id = 'b'", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert!(
                !stored.contains("leaked"),
                "secrets never reach the database"
            );

            let groups = crate::profiles::groups::list_groups(connection)?
                .into_iter()
                .map(|group| (group.id, group.name))
                .collect::<Vec<_>>();
            let named = |name: &str| (name.to_string(), name.to_string());
            assert_eq!(groups, [named("empty"), named("ops")]);
            assert_eq!(
                crate::profiles::storage::get_profile(connection, "b")?
                    .and_then(|profile| profile.group_id)
                    .as_deref(),
                Some("ops")
            );
            assert_eq!(
                crate::tunnel::storage::list_tunnel_rules(connection)?[0].profile_id,
                "b"
            );
            assert_eq!(
                crate::ssh::store::list_known_hosts(connection)?.entries[0].fingerprint,
                "SHA256:x"
            );
            assert_eq!(
                crate::sftp::store::list_sftp_directories(connection)?.entries[0].last_path,
                "/srv"
            );
            Ok(())
        })
        .expect("verify import");

    for name in IMPORTED_FILES {
        assert!(!dir.join(name).exists(), "{name} should be renamed");
        assert!(dir.join(format!("{name}.migrated")).exists());
    }

    // A file that reappears later is ignored.
    write(&dir, "profiles.json", serde_json::json!([]));
    assert!(!import_legacy_json_files(&database, &dir).expect("second run"));
    assert_eq!(
        database
            .read(crate::profiles::storage::list_saved_profiles)
            .unwrap()
            .len(),
        2
    );

    fs::remove_dir_all(dir).ok();
}

fn file_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn selection_json() -> serde_json::Value {
    serde_json::json!({
        "settings": true, "profiles": true, "session": true, "knownHosts": true,
        "sftpDirectories": true, "commandLibrary": true, "themes": true,
        "secrets": false, "logs": false
    })
}

fn write_settings_files(dir: &Path) {
    write(
        dir,
        "config.json",
        serde_json::json!({
            "theme": "dark", "language": "zh",
            "last_update_check_at": 123,
            "collapsed_profile_group_keys": ["ops"]
        }),
    );
    write(
        dir,
        "session.json",
        serde_json::json!({
            "tabs": [{"id": "1", "connection": {"host": "h", "password": "pw"}}],
            "active_tab_id": "1", "last_saved": 9
        }),
    );
    fs::write(dir.join("window-background"), "#112233\n").unwrap();
    write(
        dir,
        "backup_settings.json",
        serde_json::json!({
            "frequency": "weekly", "directory": "", "retentionCount": 5,
            "selection": selection_json(), "lastBackupAt": 77
        }),
    );
    write(
        dir,
        "webdav_backup_settings.json",
        serde_json::json!({
            "url": "https://dav.example/remote.php", "username": "me",
            "remoteDirectory": "tTerm", "frequency": "daily", "retentionCount": 10,
            "selection": selection_json(), "lastBackupAt": 88, "lastError": "boom"
        }),
    );
    write(
        dir,
        "sync_settings.json",
        serde_json::json!({
            "enabled": true, "selection": {"profiles": true, "secrets": false,
            "commands": true, "knownHosts": true, "settings": true, "themes": true},
            "pushIntervalSecs": 60, "pullIntervalMins": 15
        }),
    );
}

#[test]
fn settings_files_move_into_the_database_leaving_only_config_json() {
    let dir = temp_dir("settings-files");
    let database = Database::open_in_memory().unwrap();
    write_settings_files(&dir);

    import_settings_files(&database, &dir).unwrap();

    assert_eq!(file_names(&dir), ["config.json"]);
    let config: serde_json::Value =
        serde_json::from_slice(&fs::read(dir.join("config.json")).unwrap()).unwrap();
    assert_eq!(
        config,
        serde_json::json!({"theme": "dark", "language": "zh"})
    );
    database
        .read(|connection| {
            let state = crate::app_state::load(connection)?;
            assert_eq!(state.last_update_check_at, Some(123));
            assert_eq!(state.collapsed_profile_group_keys, ["ops"]);

            let session = crate::session::read_session(connection)?.unwrap();
            assert_eq!(session.active_tab_id.as_deref(), Some("1"));
            assert_eq!(session.last_saved, 9);
            assert!(session.tabs[0]["connection"].get("password").is_none());

            assert_eq!(
                crate::config::window_background::read(connection)?,
                Some((0x11, 0x22, 0x33))
            );

            let automatic = crate::backup::automatic::read_automatic_backup_settings(connection)?;
            assert_eq!(automatic.frequency, "weekly");
            assert_eq!(automatic.retention_count, 5);
            assert_eq!(automatic.last_backup_at, Some(77));

            let webdav = crate::backup::remote::read_settings(connection)?;
            assert_eq!(webdav.url, "https://dav.example/remote.php");
            assert_eq!(webdav.username, "me");
            assert_eq!(webdav.last_backup_at, Some(88));
            assert_eq!(webdav.last_error.as_deref(), Some("boom"));

            let sync = crate::sync::read_settings(connection)?;
            assert!(sync.enabled);
            assert!(!sync.selection.secrets);
            assert_eq!((sync.push_interval_secs, sync.pull_interval_mins), (60, 15));
            Ok(())
        })
        .unwrap();

    // Nothing left to do on the next launch.
    import_settings_files(&database, &dir).unwrap();
    assert_eq!(file_names(&dir), ["config.json"]);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn files_written_again_by_an_older_version_do_not_overwrite_the_database() {
    let dir = temp_dir("settings-files-again");
    let database = Database::open_in_memory().unwrap();
    write_settings_files(&dir);
    import_settings_files(&database, &dir).unwrap();
    database
        .write(|connection| {
            crate::session::write_session(
                connection,
                crate::session::SessionData {
                    active_tab_id: Some("newer".to_string()),
                    ..crate::session::SessionData::default()
                },
            )
        })
        .unwrap();

    write_settings_files(&dir);
    fs::write(dir.join("window-background"), "#ffffff").unwrap();
    import_settings_files(&database, &dir).unwrap();

    assert_eq!(file_names(&dir), ["config.json"]);
    database
        .read(|connection| {
            let session = crate::session::read_session(connection)?.unwrap();
            assert_eq!(session.active_tab_id.as_deref(), Some("newer"));
            assert_eq!(
                crate::config::window_background::read(connection)?,
                Some((0x11, 0x22, 0x33))
            );
            Ok(())
        })
        .unwrap();
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn an_unreadable_settings_file_stays_while_the_others_move() {
    let dir = temp_dir("settings-files-corrupt");
    let database = Database::open_in_memory().unwrap();
    write_settings_files(&dir);
    fs::write(dir.join("sync_settings.json"), "{ not json").unwrap();
    fs::write(dir.join("session.json"), "{ not json").unwrap();

    let error = import_settings_files(&database, &dir).unwrap_err();
    assert!(error.contains("sync_settings.json"), "{error}");
    // A broken session is dropped rather than kept.
    assert_eq!(file_names(&dir), ["config.json", "sync_settings.json"]);
    database
        .read(|connection| {
            assert!(crate::session::read_session(connection)?.is_none());
            assert!(!crate::sync::read_settings(connection)?.enabled);
            assert_eq!(
                crate::backup::remote::read_settings(connection)?.last_backup_at,
                Some(88)
            );
            Ok(())
        })
        .unwrap();
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn corrupt_file_aborts_the_whole_import() {
    let dir = temp_dir("corrupt");
    write(&dir, "profile_groups.json", serde_json::json!(["ops"]));
    fs::write(dir.join("tunnels.json"), "{ not json").unwrap();

    let database = Database::open_in_memory().expect("open database");
    let error = import_legacy_json_files(&database, &dir).expect_err("corrupt file");
    assert!(error.contains("tunnels.json"));
    assert!(!legacy_json_imported(&database).unwrap());
    assert!(database
        .read(crate::profiles::groups::list_groups)
        .unwrap()
        .is_empty());
    assert!(
        dir.join("profile_groups.json").exists(),
        "nothing is renamed"
    );

    fs::remove_dir_all(dir).ok();
}

#[test]
fn missing_files_import_as_empty() {
    let dir = temp_dir("empty");
    let database = Database::open_in_memory().expect("open database");
    assert!(import_legacy_json_files(&database, &dir).expect("import"));
    assert!(database
        .read(crate::profiles::storage::list_saved_profiles)
        .unwrap()
        .is_empty());
    fs::remove_dir_all(dir).ok();
}

/// Imports a copy of real app data. Point `TTERM_IMPORT_FIXTURE_DIR` at a
/// directory holding a copy of the config directory's JSON files and
/// `tterm.db`; the copy is modified.
#[test]
#[ignore]
fn imports_a_real_config_directory_copy() {
    let dir = std::path::PathBuf::from(
        std::env::var("TTERM_IMPORT_FIXTURE_DIR").expect("TTERM_IMPORT_FIXTURE_DIR"),
    );
    let json = |name: &str| -> serde_json::Value {
        serde_json::from_slice(&fs::read(dir.join(name)).expect(name)).expect(name)
    };
    let mut expected_profiles: Vec<SavedProfile> =
        serde_json::from_value(json("profiles.json")).unwrap();
    for profile in &mut expected_profiles {
        crate::profiles::storage::normalize_profile(profile);
    }
    let expected_groups: Vec<String> = serde_json::from_value(json("profile_groups.json")).unwrap();
    let expected_groups = expected_groups
        .into_iter()
        .chain(
            serde_json::from_value::<Vec<SavedProfile>>(json("profiles.json"))
                .unwrap()
                .into_iter()
                .map(|profile| profile.legacy_group),
        )
        .collect::<Vec<_>>();
    let expected_tunnels = json("tunnels.json");
    let expected_known_hosts = json("ssh_known_hosts.json");
    let expected_sftp = json("sftp_directories.json");

    let database = Database::open(&dir.join("tterm.db")).expect("open database copy");
    let commands_before = crate::command_library::repository::CommandRepository::new(&database)
        .list()
        .unwrap()
        .len();
    assert!(import_legacy_json_files(&database, &dir).expect("import"));

    database
        .read(|connection| {
            let profiles = crate::profiles::storage::list_saved_profiles(connection)?;
            assert_eq!(
                serde_json::to_value(&profiles).unwrap(),
                serde_json::to_value(&expected_profiles).unwrap()
            );
            let mut groups = crate::profiles::groups::list_groups(connection)?
                .into_iter()
                .map(|group| group.id)
                .collect::<Vec<_>>();
            let mut expected = expected_groups
                .iter()
                .map(|g| g.trim().to_string())
                .filter(|g| !g.is_empty())
                .collect::<Vec<_>>();
            groups.sort();
            expected.sort();
            expected.dedup();
            assert_eq!(groups, expected);
            assert_eq!(
                serde_json::to_value(crate::tunnel::storage::list_tunnel_rules(connection)?)
                    .unwrap(),
                expected_tunnels
            );
            assert_eq!(
                serde_json::to_value(crate::ssh::store::list_known_hosts(connection)?).unwrap(),
                expected_known_hosts
            );
            assert_eq!(
                serde_json::to_value(crate::sftp::store::list_sftp_directories(connection)?)
                    .unwrap(),
                expected_sftp
            );
            eprintln!(
                "imported {} profiles, {} groups, {} tunnels, {} known hosts, {} SFTP directories",
                profiles.len(),
                groups.len(),
                expected_tunnels.as_array().map_or(0, Vec::len),
                expected_known_hosts["entries"]
                    .as_array()
                    .map_or(0, Vec::len),
                expected_sftp["entries"].as_array().map_or(0, Vec::len),
            );
            Ok(())
        })
        .expect("verify import");
    assert_eq!(
        crate::command_library::repository::CommandRepository::new(&database)
            .list()
            .unwrap()
            .len(),
        commands_before,
        "saved commands survive the schema upgrade"
    );
}
