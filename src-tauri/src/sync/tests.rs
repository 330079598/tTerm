use super::snapshot::{
    COMMANDS, KNOWN_HOSTS, PROFILES, PROFILE_GROUPS, PROXIES, SECRETS, SERIAL_SENDS, THEMES,
};
use super::*;
use crate::command_library::{models::SavedCommand, repository::CommandRepository};
use crate::db::Database;
use crate::profiles::types::ProfileGroup;
use crate::ssh::secret_store::{
    crypto::SecretKey as DataKey, store::get_secret, store::put_secret,
};
use crate::ssh::store::KnownHostRecord;
use serde_json::json;

const CATEGORIES: [&str; 9] = [
    PROFILES,
    PROFILE_GROUPS,
    TUNNELS,
    PROXIES,
    SECRETS,
    COMMANDS,
    KNOWN_HOSTS,
    THEMES,
    SERIAL_SENDS,
];

/// The server's file: collections and revision history.
type Remote = Option<(Collections, Vec<String>)>;

/// One device: its database, data key and last synced base.
struct Device {
    database: Database,
    key: DataKey,
    base: Option<(Collections, Vec<String>)>,
    /// The base when syncing through a real server.
    live_base: Option<state::SyncBase>,
}

impl Device {
    fn new() -> Self {
        Self {
            database: Database::open_in_memory().unwrap(),
            key: DataKey::generate(),
            base: None,
            live_base: None,
        }
    }

    fn collect(&self) -> Collections {
        self.database
            .read(|connection| snapshot::collect(connection, &self.key, &CATEGORIES))
            .unwrap()
    }

    /// The sync without the network: merge with `remote`, apply, upload.
    fn sync(&mut self, remote: &mut Remote) -> Vec<&'static str> {
        let (merged, history) = self.merge(remote.as_ref());
        let changed = self.apply(&merged);
        *remote = Some((merged.clone(), history.clone()));
        self.base = Some((merged, history));
        changed
    }

    /// Merges with `remote` and returns the result and its new history.
    fn merge(&self, remote: Option<&(Collections, Vec<String>)>) -> (Collections, Vec<String>) {
        let local = self.collect();
        let base_revision = self.base.as_ref().and_then(|(_, history)| history.last());
        let base = self
            .base
            .as_ref()
            .filter(|_| {
                descends_from(
                    remote.map(|(_, history)| history.as_slice()),
                    base_revision.map(String::as_str),
                )
            })
            .map(|(base, _)| normalize_lenient(base, &CATEGORIES));
        let merged = merge_all(&CATEGORIES, base.as_ref(), &local, remote.map(|(c, _)| c));
        let mut history = remote
            .map(|(_, history)| history.clone())
            .unwrap_or_default();
        history.push(uuid::Uuid::new_v4().to_string());
        (merged, history)
    }

    /// A sync through a real server, as `sync` does it minus the app.
    async fn sync_live(&mut self, remote: &RemoteFile<'_>) -> Exchanged {
        let local = self.collect();
        let base_normalized = self
            .live_base
            .as_ref()
            .map(|base| normalize_lenient(&base.collections, &CATEGORIES));
        let exchanged = exchange(
            remote,
            &CATEGORIES,
            &local,
            self.live_base.as_ref(),
            base_normalized.as_ref(),
        )
        .await
        .unwrap();
        self.apply(&exchanged.agreed.collections);
        self.live_base = Some(state::SyncBase {
            collections: exchanged.agreed.collections.clone(),
            kdf: exchanged.agreed.kdf.clone(),
            history: exchanged.agreed.history.clone(),
            etag: exchanged.agreed.etag.clone(),
        });
        exchanged
    }

    fn apply(&self, merged: &Collections) -> Vec<&'static str> {
        let local = self.collect();
        let mut changed = Vec::new();
        self.database
            .write(|transaction| {
                for category in CATEGORIES {
                    let normalized = snapshot::normalize(category, &merged[category])?;
                    if !same_data(&local[category], &normalized) {
                        changed.push(category);
                        snapshot::apply_database(
                            transaction,
                            &self.key,
                            category,
                            &local[category],
                            &normalized,
                        )?;
                    }
                }
                if changed
                    .iter()
                    .any(|category| [PROFILES, PROFILE_GROUPS].contains(category))
                {
                    crate::profiles::groups::repair(transaction)?;
                }
                Ok(())
            })
            .unwrap();
        changed
    }

    fn save_profile(&self, id: &str, host: &str) {
        let profile = serde_json::from_value(json!({
            "id": id, "name": id, "connection_type": "ssh",
            "host": host, "port": 22, "username": "root",
            "auth_method": "password", "private_key_path": null
        }))
        .unwrap();
        self.database
            .write(|transaction| crate::profiles::storage::upsert_profile(transaction, &profile))
            .unwrap();
    }

    fn save_themes(&self, themes: &[Value]) {
        self.database
            .write(|transaction| crate::themes::replace_themes(transaction, themes))
            .unwrap();
    }

    fn themes(&self) -> Vec<Value> {
        self.database.read(crate::themes::list_themes).unwrap()
    }

    fn profile(&self, id: &str) -> Option<crate::profiles::types::SavedProfile> {
        self.database
            .read(|connection| crate::profiles::storage::get_profile(connection, id))
            .unwrap()
    }

    fn delete_profile(&self, id: &str) {
        self.database
            .write(|transaction| {
                crate::profiles::storage::delete_profiles(transaction, &[id.to_string()])
            })
            .unwrap();
    }

    fn save_secret(&self, key: &str, value: &str) {
        self.database
            .write(|transaction| put_secret(transaction, &self.key, key, value))
            .unwrap();
    }

    fn secret(&self, key: &str) -> Option<String> {
        self.database
            .read(|connection| get_secret(connection, &self.key, key))
            .unwrap()
            .map(|value| value.to_string())
    }
}

fn command(id: &str, text: &str) -> SavedCommand {
    serde_json::from_value(json!({
        "id": id, "name": id, "commandText": text, "description": "",
        "scopeType": "global", "scopeId": null, "shellType": "any", "platform": "any",
        "isFavorite": false, "confirmBeforeRun": false, "sortOrder": 0,
        "useCount": 0, "lastUsedAt": null, "createdAt": 1, "updatedAt": 1,
        "tags": ["ops"], "variables": []
    }))
    .unwrap()
}

#[test]
fn two_devices_converge_and_deletions_travel() {
    let mut remote = None;
    let (mut a, mut b) = (Device::new(), Device::new());
    a.save_profile("p1", "one.example");
    a.save_profile("p2", "two.example");
    a.save_secret("p1", "secret-1");
    a.save_secret("webdav:password", "never-synced");
    b.save_profile("p3", "three.example");

    assert!(a.sync(&mut remote).is_empty());
    assert_eq!(b.sync(&mut remote), [PROFILES, SECRETS]);
    assert_eq!(b.secret("p1").as_deref(), Some("secret-1"));
    assert_eq!(a.sync(&mut remote), [PROFILES]);
    assert!(a.profile("p3").is_some());
    assert_eq!(
        b.profile("p1").unwrap().host.as_deref(),
        Some("one.example")
    );
    assert!(b.secret("webdav:password").is_none());

    // A deletes a profile; B deletes nothing but receives the deletion.
    a.delete_profile("p2");
    a.sync(&mut remote);
    b.sync(&mut remote);
    assert!(b.profile("p2").is_none());
    assert!(b.profile("p1").is_some());

    // Both edit different fields of the same profile.
    let mut edited = b.profile("p1").unwrap();
    edited.port = Some(2222);
    b.database
        .write(|transaction| crate::profiles::storage::upsert_profile(transaction, &edited))
        .unwrap();
    a.save_profile("p1", "renamed.example");
    a.sync(&mut remote);
    b.sync(&mut remote);
    a.sync(&mut remote);
    for device in [&a, &b] {
        let profile = device.profile("p1").unwrap();
        assert_eq!(profile.host.as_deref(), Some("renamed.example"));
        assert_eq!(profile.port, Some(2222));
    }

    // Nothing left to do on either side.
    assert!(a.sync(&mut remote).is_empty());
    assert!(b.sync(&mut remote).is_empty());
    assert!(same_collections(&a.collect(), &b.collect()));
}

#[test]
fn themes_sync_with_their_order_edits_and_deletions() {
    let mut remote = None;
    let (mut a, mut b) = (Device::new(), Device::new());
    let dark = json!({"id": "custom-dark", "name": "Dark", "colors": {"bg": "#000"}});
    let light = json!({"id": "custom-light", "name": "Light", "colors": {"bg": "#fff"}});
    a.save_themes(&[dark.clone(), light.clone()]);
    assert_eq!(a.sync(&mut remote), Vec::<&str>::new());
    assert_eq!(b.sync(&mut remote), [THEMES]);
    assert_eq!(b.themes(), [dark.clone(), light.clone()]);

    // B renames one and reorders; A receives both.
    let renamed = json!({"id": "custom-light", "name": "Paper", "colors": {"bg": "#fff"}});
    b.save_themes(&[renamed.clone(), dark.clone()]);
    b.sync(&mut remote);
    assert_eq!(a.sync(&mut remote), [THEMES]);
    assert_eq!(a.themes(), [renamed.clone(), dark.clone()]);

    // A deletes one; B receives the deletion.
    a.save_themes(std::slice::from_ref(&renamed));
    a.sync(&mut remote);
    b.sync(&mut remote);
    assert_eq!(b.themes(), [renamed]);

    assert!(a.sync(&mut remote).is_empty());
    assert!(b.sync(&mut remote).is_empty());
}

#[test]
fn an_overwritten_upload_is_merged_back_instead_of_deleted() {
    let mut remote = None;
    let (mut a, mut b) = (Device::new(), Device::new());
    a.save_profile("p1", "one.example");
    a.sync(&mut remote);
    b.sync(&mut remote);

    // Both read the same file; A uploads first, then B uploads over it
    // (a server that ignores If-Match lets the stale upload through).
    let seen_by_b = remote.clone();
    a.save_profile("p2", "from-a.example");
    a.sync(&mut remote);
    b.save_profile("p3", "from-b.example");
    let (merged, history) = b.merge(seen_by_b.as_ref());
    b.apply(&merged);
    remote = Some((merged.clone(), history.clone()));
    b.base = Some((merged, history));

    // A's profile is missing from the file, but A must not take that as a deletion.
    a.sync(&mut remote);
    b.sync(&mut remote);
    for device in [&a, &b] {
        for id in ["p1", "p2", "p3"] {
            assert!(device.profile(id).is_some(), "{id} lost");
        }
    }

    // Deletions work again once the lineage is back.
    a.delete_profile("p1");
    a.sync(&mut remote);
    b.sync(&mut remote);
    assert!(b.profile("p1").is_none());
}

#[test]
fn sync_settings_default_and_validate_intervals() {
    let settings: SyncSettings = serde_json::from_str(r#"{"enabled": true}"#).unwrap();
    assert_eq!(
        (settings.push_interval_secs, settings.pull_interval_mins),
        (30, 5)
    );
    assert!(settings.validate().is_ok());
    let odd = SyncSettings {
        push_interval_secs: 1,
        ..SyncSettings::default()
    };
    assert!(odd.validate().is_err());
    let odd = SyncSettings {
        pull_interval_mins: 0,
        ..SyncSettings::default()
    };
    assert!(odd.validate().is_err());
}

#[test]
fn lineage_decides_whether_the_base_applies() {
    let history = ["r1".to_string(), "r2".to_string()];
    assert!(descends_from(Some(&history), Some("r1")));
    assert!(descends_from(Some(&history), Some("r2")));
    assert!(!descends_from(Some(&history), Some("r0")));
    assert!(!descends_from(Some(&history), None));
    assert!(descends_from(None, None));
}

#[test]
fn passwords_sync_and_follow_their_profile() {
    let mut remote = None;
    let (mut a, mut b) = (Device::new(), Device::new());
    a.save_profile("p1", "one.example");
    a.save_secret("p1", "first");
    a.save_secret("p1:sudo", "sudo-pass");
    a.sync(&mut remote);
    b.sync(&mut remote);
    assert_eq!(b.secret("p1").as_deref(), Some("first"));
    assert_eq!(b.secret("p1:sudo").as_deref(), Some("sudo-pass"));

    b.save_secret("p1", "second");
    b.sync(&mut remote);
    a.sync(&mut remote);
    assert_eq!(a.secret("p1").as_deref(), Some("second"));

    // Deleting the profile deletes its passwords everywhere.
    a.delete_profile("p1");
    a.sync(&mut remote);
    b.sync(&mut remote);
    assert!(b.profile("p1").is_none());
    assert!(b.secret("p1").is_none());
    assert!(b.secret("p1:sudo").is_none());
}

#[test]
fn commands_sync_but_usage_counts_stay_per_device() {
    let mut remote = None;
    let (mut a, mut b) = (Device::new(), Device::new());
    CommandRepository::new(&a.database)
        .save(&command("c1", "uptime"))
        .unwrap();
    a.sync(&mut remote);
    b.sync(&mut remote);
    let command_b = |device: &Device| {
        CommandRepository::new(&device.database)
            .get("c1")
            .unwrap()
            .unwrap()
    };
    assert_eq!(command_b(&b).tags, ["ops"]);

    // Using a command changes nothing to sync.
    CommandRepository::new(&b.database)
        .record_use("c1", 99)
        .unwrap();
    assert!(b.sync(&mut remote).is_empty());

    let mut edited = command("c1", "uptime -p");
    edited.updated_at = 2;
    CommandRepository::new(&a.database).save(&edited).unwrap();
    a.sync(&mut remote);
    b.sync(&mut remote);
    let synced = command_b(&b);
    assert_eq!(synced.command_text, "uptime -p");
    assert_eq!(synced.use_count, 1);
    assert_eq!(synced.last_used_at, Some(99));
}

#[test]
fn groups_tunnels_and_known_hosts_sync() {
    let mut remote = None;
    let (mut a, mut b) = (Device::new(), Device::new());
    let rule: crate::tunnel::types::TunnelRule = serde_json::from_value(json!({
        "id": "t1", "name": "db", "profileId": "p1", "kind": "local",
        "bindHost": "127.0.0.1", "bindPort": 8080, "destHost": "db", "destPort": 5432
    }))
    .unwrap();
    a.database
        .write(|transaction| {
            crate::profiles::groups::replace_groups(
                transaction,
                &[ProfileGroup {
                    id: "g1".to_string(),
                    name: "Empty group".to_string(),
                    parent_id: None,
                }],
            )?;
            crate::tunnel::storage::replace_tunnels(transaction, &[rule])?;
            crate::ssh::store::merge_known_host(
                transaction,
                &KnownHostRecord {
                    profile_id: Some("p1".to_string()),
                    profile_name: "p1".to_string(),
                    host: "one.example".to_string(),
                    port: 22,
                    algorithm: "ssh-ed25519".to_string(),
                    fingerprint: "SHA256:abc".to_string(),
                    trusted_at: 5,
                },
            )
        })
        .unwrap();
    a.sync(&mut remote);
    assert_eq!(b.sync(&mut remote), [PROFILE_GROUPS, TUNNELS, KNOWN_HOSTS]);
    b.database
        .read(|connection| {
            assert_eq!(
                crate::profiles::groups::list_groups(connection)?
                    .into_iter()
                    .map(|group| group.name)
                    .collect::<Vec<_>>(),
                ["Empty group"]
            );
            assert_eq!(
                crate::tunnel::storage::list_tunnel_rules(connection)?[0].dest_port,
                5432
            );
            assert_eq!(
                crate::ssh::store::list_known_hosts(connection)?.entries[0].fingerprint,
                "SHA256:abc"
            );
            Ok(())
        })
        .unwrap();
    assert!(b.sync(&mut remote).is_empty());
}

#[test]
fn proxies_and_their_passwords_sync_in_order() {
    let mut remote = None;
    let (mut a, mut b) = (Device::new(), Device::new());
    let server = |id: &str, name: &str| crate::proxy::ProxyServer {
        id: id.to_string(),
        name: name.to_string(),
        kind: crate::proxy::ProxyKind::Http,
        host: "proxy.corp".to_string(),
        port: 8080,
        username: Some("me".to_string()),
        command: None,
    };
    a.database
        .write(|transaction| {
            crate::proxy::storage::replace_proxies(
                transaction,
                &[server("p2", "Second"), server("p1", "First")],
            )
        })
        .unwrap();
    a.save_secret(&crate::proxy::proxy_secret_key("p1"), "pw");
    a.sync(&mut remote);
    assert_eq!(b.sync(&mut remote), [PROXIES, SECRETS]);
    let names = |device: &Device| -> Vec<String> {
        device
            .database
            .read(crate::proxy::storage::list_proxies)
            .unwrap()
            .into_iter()
            .map(|server| server.name)
            .collect()
    };
    assert_eq!(names(&b), ["Second", "First"]);
    assert_eq!(
        b.secret(&crate::proxy::proxy_secret_key("p1")).as_deref(),
        Some("pw")
    );

    a.database
        .write(|transaction| crate::proxy::storage::delete_proxy(transaction, "p2"))
        .unwrap();
    a.sync(&mut remote);
    assert_eq!(b.sync(&mut remote), [PROXIES]);
    assert_eq!(names(&b), ["First"]);
}

impl Device {
    fn send(&self, key: &str, text: &str, sent_at: i64) {
        self.database
            .write(|transaction| {
                crate::serial::history::store::record(
                    transaction,
                    key,
                    crate::serial::history::SerialSendMode::Text,
                    text,
                    sent_at,
                )
            })
            .unwrap();
    }

    fn sends(&self, key: &str) -> Vec<(String, i64)> {
        self.database
            .read(|connection| crate::serial::history::store::list(connection, key))
            .unwrap()
            .into_iter()
            .map(|send| (send.text, send.sent_at))
            .collect()
    }
}

#[test]
fn serial_sends_sync_as_one_history_and_clearing_travels() {
    let mut remote = None;
    let (mut a, mut b) = (Device::new(), Device::new());
    a.send("profile:board", "AT", 1);
    a.send("profile:board", "AT+RST", 2);
    a.sync(&mut remote);
    assert_eq!(b.sync(&mut remote), [SERIAL_SENDS]);
    assert_eq!(
        b.sends("profile:board"),
        [("AT+RST".to_string(), 2), ("AT".to_string(), 1)]
    );

    // Both send AT again; the one record keeps the later time.
    a.send("profile:board", "AT", 5);
    b.send("profile:board", "AT", 4);
    a.sync(&mut remote);
    b.sync(&mut remote);
    a.sync(&mut remote);
    let expected = [("AT".to_string(), 5), ("AT+RST".to_string(), 2)];
    assert_eq!(a.sends("profile:board"), expected);
    assert_eq!(b.sends("profile:board"), expected);

    // B clears the history; A loses it too.
    b.database
        .write(|transaction| crate::serial::history::store::clear(transaction, "profile:board"))
        .unwrap();
    b.sync(&mut remote);
    a.sync(&mut remote);
    assert!(a.sends("profile:board").is_empty());
    assert!(a.sync(&mut remote).is_empty());
}

#[test]
fn categories_this_device_skips_are_carried_over() {
    let mut remote = Some(Collections::from([(
        "themes".to_string(),
        merge::Collection::from([(
            "t".to_string(),
            merge::SyncRecord::new(json!({"id": "t"}), None),
        )]),
    )]));
    let local = Collections::from([(PROFILES.to_string(), merge::Collection::new())]);
    let merged = merge_all(&[PROFILES], None, &local, remote.as_ref());
    assert_eq!(merged["themes"], remote.as_ref().unwrap()["themes"]);
    assert!(merged[PROFILES].is_empty());

    // A remote file without a category never deletes local records.
    let local = Collections::from([(
        PROFILES.to_string(),
        merge::Collection::from([(
            "p".to_string(),
            merge::SyncRecord::new(json!({"a": 1}), None),
        )]),
    )]);
    let base = local.clone();
    remote.as_mut().unwrap().remove(PROFILES);
    let merged = merge_all(&[PROFILES], Some(&base), &local, remote.as_ref());
    assert_eq!(merged[PROFILES], local[PROFILES]);
}

#[test]
fn fields_from_a_newer_version_survive_an_older_device() {
    let mut remote = None;
    let (mut a, mut b) = (Device::new(), Device::new());
    a.save_profile("p1", "one.example");
    a.sync(&mut remote);
    // A newer tTerm adds a field B does not know.
    remote
        .as_mut()
        .unwrap()
        .0
        .get_mut(PROFILES)
        .unwrap()
        .get_mut("p1")
        .unwrap()
        .data["futureField"] = json!("keep me");
    b.sync(&mut remote);
    // B edits the profile; the unknown field must stay in the remote file.
    let mut edited = b.profile("p1").unwrap();
    edited.port = Some(2200);
    b.database
        .write(|transaction| crate::profiles::storage::upsert_profile(transaction, &edited))
        .unwrap();
    b.sync(&mut remote);
    let record = &remote.as_ref().unwrap().0[PROFILES]["p1"].data;
    assert_eq!(record["futureField"], "keep me");
    assert_eq!(record["port"], 2200);
    assert!(b.sync(&mut remote).is_empty());
}

/// Two devices syncing through a real server:
/// `TTERM_WEBDAV_URL=... TTERM_WEBDAV_USER=... TTERM_WEBDAV_PASSWORD=... cargo test live_devices -- --ignored`
#[tokio::test]
#[ignore]
async fn live_devices_sync_through_a_server() {
    let env = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"));
    let client = WebDavClient::new(
        &env("TTERM_WEBDAV_URL"),
        &env("TTERM_WEBDAV_USER"),
        zeroize::Zeroizing::new(env("TTERM_WEBDAV_PASSWORD")),
        "tTerm-test",
    )
    .unwrap();
    let directory = vec![format!("tterm-sync-{}", std::process::id())];
    client.ensure_directory(&directory).await.unwrap();
    let remote = |device| RemoteFile {
        client: &client,
        directory: &directory,
        password: "sync-password",
        device,
        app_version: "test",
    };
    let (mut a, mut b) = (Device::new(), Device::new());

    a.save_profile("p1", "one.example");
    a.save_profile("p2", "two.example");
    a.save_secret("p1", "secret-1");
    b.save_profile("p3", "three.example");
    let first = a.sync_live(&remote("a")).await;
    assert!(first.uploaded && first.union);
    let joined = b.sync_live(&remote("b")).await;
    assert!(
        joined.uploaded && joined.union,
        "B adds p3 on its first sync"
    );
    a.sync_live(&remote("a")).await;
    for device in [&a, &b] {
        for id in ["p1", "p2", "p3"] {
            assert!(device.profile(id).is_some(), "{id} missing");
        }
    }
    assert_eq!(b.secret("p1").as_deref(), Some("secret-1"));

    // Nothing changed: no upload, and the file is not downloaded again
    // when the server honors If-None-Match.
    let idle = b.sync_live(&remote("b")).await;
    assert!(!idle.uploaded && !idle.union);

    // A deletes, B edits another field of a profile both have.
    a.delete_profile("p2");
    let mut edited = b.profile("p1").unwrap();
    edited.port = Some(2222);
    b.database
        .write(|transaction| crate::profiles::storage::upsert_profile(transaction, &edited))
        .unwrap();
    a.sync_live(&remote("a")).await;
    let merged = b.sync_live(&remote("b")).await;
    assert!(merged.uploaded && !merged.union);
    a.sync_live(&remote("a")).await;
    for device in [&a, &b] {
        assert!(device.profile("p2").is_none(), "deletion travels");
        assert_eq!(device.profile("p1").unwrap().port, Some(2222));
    }
    assert!(same_collections(&a.collect(), &b.collect()));

    // A device that forgot its base merges without deleting anything.
    b.save_profile("p9", "local-only.example");
    b.live_base = None;
    let rejoined = b.sync_live(&remote("b")).await;
    assert!(rejoined.union);
    a.sync_live(&remote("a")).await;
    assert!(a.profile("p9").is_some());
    assert!(b.profile("p3").is_some());

    client.delete_directory(&directory).await.unwrap();
}

/// Both devices change and sync at the same moment, repeatedly; whatever
/// the server does with If-Match, nothing may be lost.
#[tokio::test]
#[ignore]
async fn live_devices_syncing_at_once_lose_nothing() {
    let env = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"));
    let client = WebDavClient::new(
        &env("TTERM_WEBDAV_URL"),
        &env("TTERM_WEBDAV_USER"),
        zeroize::Zeroizing::new(env("TTERM_WEBDAV_PASSWORD")),
        "tTerm-test",
    )
    .unwrap();
    let directory = vec![format!("tterm-race-{}", std::process::id())];
    client.ensure_directory(&directory).await.unwrap();
    let remote = |device| RemoteFile {
        client: &client,
        directory: &directory,
        password: "sync-password",
        device,
        app_version: "test",
    };
    let (mut a, mut b) = (Device::new(), Device::new());
    a.sync_live(&remote("a")).await;
    b.sync_live(&remote("b")).await;
    for round in 0..6 {
        a.save_profile(&format!("a{round}"), "a.example");
        b.save_profile(&format!("b{round}"), "b.example");
        let (ra, rb) = (remote("a"), remote("b"));
        tokio::join!(a.sync_live(&ra), b.sync_live(&rb));
    }
    for _ in 0..2 {
        a.sync_live(&remote("a")).await;
        b.sync_live(&remote("b")).await;
    }
    for device in [&a, &b] {
        for round in 0..6 {
            for id in [format!("a{round}"), format!("b{round}")] {
                assert!(device.profile(&id).is_some(), "{id} lost");
            }
        }
    }
    assert!(same_collections(&a.collect(), &b.collect()));
    client.delete_directory(&directory).await.unwrap();
}

#[test]
fn triggers_record_when_rows_change() {
    let device = Device::new();
    device.save_profile("p1", "one.example");
    let updated_at = |device: &Device| -> Option<i64> {
        device
            .database
            .read(|connection| {
                connection
                    .query_row(
                        "SELECT updated_at FROM profiles WHERE id = 'p1'",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(|error| error.to_string())
            })
            .unwrap()
    };
    let first = updated_at(&device).expect("stamped on insert");
    device
        .database
        .write(|transaction| {
            transaction
                .execute("UPDATE profiles SET updated_at = 1 WHERE id = 'p1'", [])
                .map_err(|error| error.to_string())
        })
        .unwrap();
    device.save_profile("p1", "one.example");
    assert_eq!(
        updated_at(&device),
        Some(1),
        "an unchanged save keeps the time"
    );
    device.save_profile("p1", "two.example");
    assert!(updated_at(&device).unwrap() >= first);
}

impl Device {
    /// Runs a change to the groups and profiles on this device.
    fn edit<T>(&self, change: impl FnOnce(&rusqlite::Connection) -> Result<T, String>) -> T {
        self.database
            .write(|transaction| change(transaction))
            .unwrap()
    }

    /// Each group's path, by id.
    fn group_paths(&self) -> std::collections::HashMap<String, String> {
        self.database
            .read(|connection| {
                Ok(crate::profiles::groups::group_paths(
                    &crate::profiles::groups::list_groups(connection)?,
                ))
            })
            .unwrap()
    }
}

#[test]
fn nested_groups_sync_and_merge_a_rename_with_a_move() {
    use crate::profiles::groups::{create_group, move_group, rename_group};
    let mut remote = None;
    let (mut a, mut b) = (Device::new(), Device::new());
    let (prod, web, dev) = a.edit(|connection| {
        let prod = create_group(connection, "prod", None)?;
        let web = create_group(connection, "web", Some(&prod.id))?;
        let dev = create_group(connection, "dev", None)?;
        Ok((prod.id, web.id, dev.id))
    });
    a.save_profile("p1", "one.example");
    a.edit(|connection| crate::profiles::storage::move_profile(connection, "p1", Some(&web), None));
    a.sync(&mut remote);
    assert_eq!(b.sync(&mut remote), [PROFILES, PROFILE_GROUPS]);
    assert_eq!(b.group_paths()[&web], "prod / web");
    assert_eq!(b.profile("p1").unwrap().group_id, Some(web.clone()));

    a.edit(|connection| rename_group(connection, &prod, "production"));
    b.edit(|connection| move_group(connection, &web, Some(&dev)));
    a.sync(&mut remote);
    b.sync(&mut remote);
    a.sync(&mut remote);
    for device in [&a, &b] {
        let paths = device.group_paths();
        assert_eq!(paths[&prod], "production");
        assert_eq!(paths[&web], "dev / web");
    }
    assert!(a.sync(&mut remote).is_empty());
    assert!(b.sync(&mut remote).is_empty());
}

#[test]
fn groups_moved_into_each_other_on_two_devices_settle_on_one_tree() {
    use crate::profiles::groups::{create_group, move_group};
    let mut remote = None;
    let (mut a, mut b) = (Device::new(), Device::new());
    let (x, y) = a.edit(|connection| {
        Ok((
            create_group(connection, "x", None)?.id,
            create_group(connection, "y", None)?.id,
        ))
    });
    a.sync(&mut remote);
    b.sync(&mut remote);

    a.edit(|connection| move_group(connection, &x, Some(&y)));
    b.edit(|connection| move_group(connection, &y, Some(&x)));
    for _ in 0..2 {
        a.sync(&mut remote);
        b.sync(&mut remote);
    }
    // The loop is cut at the group with the smaller id.
    let (top, nested) = if x < y { ("x", &y) } else { ("y", &x) };
    for device in [&a, &b] {
        assert_eq!(device.group_paths()[nested].split(" / ").next(), Some(top));
    }
    assert!(a.sync(&mut remote).is_empty());
    assert!(b.sync(&mut remote).is_empty());
}

#[test]
fn a_sync_file_from_before_groups_had_ids_files_profiles_by_group_name() {
    let collections = Collections::from([
        (
            PROFILE_GROUPS.to_string(),
            merge::Collection::from([(
                "ops".to_string(),
                merge::SyncRecord::new(json!(true), None),
            )]),
        ),
        (
            PROFILES.to_string(),
            merge::Collection::from([(
                "p1".to_string(),
                merge::SyncRecord::new(
                    json!({
                        "id": "p1", "name": "p1", "group": "ops", "connection_type": "ssh",
                        "host": "one.example", "_position": 0
                    }),
                    Some(1),
                ),
            )]),
        ),
    ]);
    let mut remote = Some((collections, vec!["older".to_string()]));
    let mut b = Device::new();
    b.sync(&mut remote);
    assert_eq!(b.group_paths()["ops"], "ops");
    assert_eq!(b.profile("p1").unwrap().group_id.as_deref(), Some("ops"));
    assert!(b.sync(&mut remote).is_empty());
}
