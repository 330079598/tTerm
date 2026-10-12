use super::files::{parse_backup_file_name, sanitize_device_name};
use super::*;
use crate::backup::webdav::DavEntry;
use chrono::TimeZone;

#[test]
fn saving_settings_keeps_history_and_clearing_forgets_everything() {
    let database = crate::db::Database::open_in_memory().unwrap();
    database
        .write(|connection| {
            let settings = |url: &str| WebDavBackupSettings {
                url: url.to_string(),
                username: "me".to_string(),
                ..WebDavBackupSettings::default()
            };
            save_keeping_history(connection, settings("https://a.example/dav"))?;
            meta::set(connection, LAST_BACKUP_AT_KEY, &5_i64)?;
            meta::set(connection, LAST_ERROR_KEY, &"timeout")?;

            // The UI sends back what it was shown; that is ignored.
            save_keeping_history(
                connection,
                WebDavBackupSettings {
                    frequency: "weekly".to_string(),
                    last_backup_at: Some(1),
                    last_error: None,
                    ..settings("https://a.example/dav")
                },
            )?;
            let saved = read_settings(connection)?;
            assert_eq!(saved.frequency, "weekly");
            assert_eq!(saved.last_backup_at, Some(5));
            assert_eq!(saved.last_error.as_deref(), Some("timeout"));

            save_keeping_history(connection, settings("https://b.example/dav"))?;
            let saved = read_settings(connection)?;
            assert_eq!(saved.last_backup_at, Some(5));
            assert_eq!(saved.last_error, None);

            clear_settings(connection)?;
            let cleared = read_settings(connection)?;
            assert!(cleared.url.is_empty());
            assert_eq!((cleared.last_backup_at, cleared.last_error), (None, None));
            Ok(())
        })
        .unwrap();
}

fn file(name: &str) -> DavEntry {
    DavEntry {
        name: name.to_string(),
        is_collection: false,
        size: 1,
        modified_at: Some(0),
    }
}

/// An encrypted archive survives the upload, pruning and download path.
/// `TTERM_WEBDAV_URL=... TTERM_WEBDAV_USER=... TTERM_WEBDAV_PASSWORD=... cargo test live_archive -- --ignored`
#[tokio::test]
#[ignore]
async fn live_archive_upload_prune_and_restore() {
    use super::super::archive::{build_archive_for_version, decode_archive};
    use super::super::{BackupPayload, MigrationSecretRecord};

    let env = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"));
    let client = WebDavClient::new(
        &env("TTERM_WEBDAV_URL"),
        &env("TTERM_WEBDAV_USER"),
        Zeroizing::new(env("TTERM_WEBDAV_PASSWORD")),
        "tTerm-test",
    )
    .unwrap();
    let directory =
        split_remote_directory(&format!("tterm-e2e-{}/tTerm", std::process::id())).unwrap();
    client.ensure_directory(&directory).await.unwrap();

    let selection = default_selection();
    let mut payload = BackupPayload::default();
    payload.profiles = Some(serde_json::json!([]));
    payload.secrets.push(MigrationSecretRecord {
        key: "profile-id".to_string(),
        password: "top-secret".to_string(),
    });
    let archive =
        build_archive_for_version("test", &selection, &payload, Some("backup-pass")).unwrap();

    // Three uploads from this device and one from another; keep two of ours.
    for (device, day) in [("mac", 1), ("mac", 2), ("pc", 2), ("mac", 3)] {
        let at = Utc.with_ymd_and_hms(2026, 9, day, 0, 0, 0).unwrap();
        client
            .put(&directory, &backup_file_name(device, at), archive.clone())
            .await
            .unwrap();
    }
    let entries = client.list(&directory).await.unwrap();
    for name in stale_backups(&entries, "mac", 2) {
        client.delete(&directory, &name).await.unwrap();
    }
    let listed = remote_backups(&client.list(&directory).await.unwrap(), "mac");
    let names: Vec<_> = listed
        .iter()
        .map(|entry| entry.file_name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "tterm-mac-20260903-000000.tterm-backup",
            "tterm-pc-20260902-000000.tterm-backup",
            "tterm-mac-20260902-000000.tterm-backup",
        ]
    );
    assert!(listed
        .iter()
        .all(|entry| entry.size_bytes == archive.len() as u64));

    let data = client
        .get(&directory, &listed[0].file_name, MAX_BACKUP_SIZE)
        .await
        .unwrap();
    let path = std::env::temp_dir().join(format!("tterm-e2e-{}.tterm-backup", std::process::id()));
    fs::write(&path, data).unwrap();
    let decoded = decode_archive(&path, Some("backup-pass")).unwrap();
    assert_eq!(
        decoded.payload.as_ref().unwrap().secrets[0].password,
        "top-secret"
    );
    assert!(decode_archive(&path, Some("wrong-pass")).is_err());
    fs::remove_file(path).unwrap();

    client.delete_directory(&directory[..1]).await.unwrap();
}

#[test]
fn sanitizes_device_names() {
    assert_eq!(
        sanitize_device_name("Stone's MacBook Pro.local"),
        "stone-s-macbook-pro"
    );
    assert_eq!(sanitize_device_name("DESKTOP-AB12"), "desktop-ab12");
    assert_eq!(sanitize_device_name("王的电脑"), "device");
    assert_eq!(sanitize_device_name(""), "device");
    assert!(sanitize_device_name(&"a".repeat(80)).len() <= 32);
}

#[test]
fn round_trips_backup_file_names() {
    let at = Utc.with_ymd_and_hms(2026, 9, 29, 8, 30, 5).unwrap();
    let name = backup_file_name("my-mac", at);
    assert_eq!(name, "tterm-my-mac-20260929-083005.tterm-backup");
    assert_eq!(
        parse_backup_file_name(&name),
        Some(("my-mac".to_string(), at.timestamp_millis()))
    );
    assert_eq!(
        parse_backup_file_name("tterm-20260929-083005.tterm-backup"),
        None
    );
    assert_eq!(parse_backup_file_name("manual.tterm-backup"), None);
    assert_eq!(
        parse_backup_file_name("tterm-x-2026-bad.tterm-backup"),
        None
    );
}

#[test]
fn lists_newest_first_and_prunes_only_this_device() {
    let entries = vec![
        file("tterm-mac-20260901-000000.tterm-backup"),
        file("tterm-mac-20260903-000000.tterm-backup"),
        file("tterm-pc-20260902-000000.tterm-backup"),
        file("tterm-mac-20260902-000000.tterm-backup"),
        file("manual.tterm-backup"),
        file("notes.txt"),
        DavEntry {
            is_collection: true,
            ..file("tterm-mac-20260904-000000.tterm-backup")
        },
    ];
    let listed = remote_backups(&entries, "mac");
    let names: Vec<_> = listed.iter().map(|b| b.file_name.as_str()).collect();
    assert_eq!(
        names,
        [
            "tterm-mac-20260903-000000.tterm-backup",
            "tterm-pc-20260902-000000.tterm-backup",
            "tterm-mac-20260902-000000.tterm-backup",
            "tterm-mac-20260901-000000.tterm-backup",
            "manual.tterm-backup",
        ]
    );
    assert!(!listed[1].current_device);
    assert_eq!(listed[4].device, None);
    assert_eq!(
        stale_backups(&entries, "mac", 1),
        [
            "tterm-mac-20260902-000000.tterm-backup",
            "tterm-mac-20260901-000000.tterm-backup",
        ]
    );
    assert!(stale_backups(&entries, "mac", 3).is_empty());
}

#[test]
fn validates_remote_file_names() {
    assert!(validate_file_name("tterm-mac-20260901-000000.tterm-backup").is_ok());
    assert!(validate_file_name("../x.tterm-backup").is_err());
    assert!(validate_file_name("a/b.tterm-backup").is_err());
    assert!(validate_file_name("backup.zip").is_err());
}

#[test]
fn normalizes_and_validates_settings() {
    let settings = normalized_settings(WebDavBackupSettings {
        url: " https://dav.jianguoyun.com/dav ".to_string(),
        username: " me@example.com ".to_string(),
        remote_directory: "/tTerm//backups/".to_string(),
        ..WebDavBackupSettings::default()
    })
    .unwrap();
    assert_eq!(settings.url, "https://dav.jianguoyun.com/dav");
    assert_eq!(settings.username, "me@example.com");
    assert_eq!(settings.remote_directory, "tTerm/backups");

    let valid = || WebDavBackupSettings {
        url: "https://example.com/".to_string(),
        username: "me".to_string(),
        ..WebDavBackupSettings::default()
    };
    assert!(normalized_settings(WebDavBackupSettings {
        username: " ".into(),
        ..valid()
    })
    .is_err());
    assert!(normalized_settings(WebDavBackupSettings {
        retention_count: 0,
        ..valid()
    })
    .is_err());
    assert!(normalized_settings(WebDavBackupSettings {
        frequency: "hourly".into(),
        ..valid()
    })
    .is_err());
    let mut no_profiles = valid();
    no_profiles.selection.profiles = false;
    assert!(normalized_settings(no_profiles).is_err());
}

#[test]
fn schedules_by_frequency() {
    let day = 24 * 60 * 60 * 1_000;
    let mut settings = WebDavBackupSettings::default();
    assert!(backup_due(&settings, day));
    settings.last_backup_at = Some(day);
    assert!(!backup_due(&settings, 2 * day - 1));
    assert!(backup_due(&settings, 2 * day));
    settings.frequency = "weekly".to_string();
    assert!(!backup_due(&settings, 2 * day));
    settings.frequency = "off".to_string();
    settings.last_backup_at = None;
    assert!(!backup_due(&settings, day));
}
