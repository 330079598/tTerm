//! P4-03 acceptance: the real core services the GPUI import dialog calls.
//!
//! The dialog itself remains owned by another change; this test exercises the
//! preview, import and reread services behind it with the same shared fixture
//! as the Tauri command acceptance test. It uses a fresh temporary config
//! directory and database and never reads the user's SSH files or keys.

use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tterm_core::paths::AppPaths;
use tterm_core::profiles::import::import_ssh_config_profiles_blocking;
use tterm_core::profiles::launch::{SavedSshProfileSummary, load_saved_ssh_profile_summaries};
use tterm_core::profiles::parser::build_ssh_config_import_preview;
use tterm_core::profiles::storage::load_profiles;
use tterm_core::profiles::types::{SshConfigImportOptions, SshConfigImportResult};
use tterm_core::tunnel::storage::load_tunnels;

const CONFIG_TEMPLATE: &str =
    include_str!("../../tterm-core/tests/fixtures/ssh-config/p4-03/config");
const INCLUDED_TEMPLATE: &str =
    include_str!("../../tterm-core/tests/fixtures/ssh-config/p4-03/included.conf");
const GROUP: &str = "P4-03 / Acceptance";
const WILDCARD_SKIP_REASON: &str = "Wildcard Host patterns are not imported as saved profiles; their options apply to the hosts they match.";
const TARGET_KEY_WARNING: &str = "ProxyJump entries without an IdentityFile of their own were imported with the target host private key; verify each jump host authentication setting.";

struct Fixture {
    root: PathBuf,
    source: PathBuf,
    include: PathBuf,
    key: PathBuf,
    certificate: PathBuf,
}

impl Fixture {
    fn materialize() -> Self {
        let root = unique_temp_path("gpui-fixture");
        let include = root.join("config.d/included.conf");
        let key = root.join("keys/id_p4_03");
        let certificate = root.join("keys/id_p4_03-cert.pub");
        let source = root.join("config");
        fs::create_dir_all(include.parent().expect("include parent"))
            .expect("create fixture include directory");
        fs::create_dir_all(key.parent().expect("key parent"))
            .expect("create fixture key directory");
        fs::write(&include, INCLUDED_TEMPLATE).expect("write included config");
        let config = CONFIG_TEMPLATE
            .replace("__P4_03_INCLUDE__", &config_text(&include))
            .replace("__P4_03_KEY__", &config_text(&key))
            .replace("__P4_03_CERT__", &config_text(&certificate));
        fs::write(&source, config).expect("write main config");
        Self {
            root,
            source,
            include,
            key,
            certificate,
        }
    }

    fn options(&self, overwrite_existing: bool, selected_hosts: &[&str]) -> SshConfigImportOptions {
        SshConfigImportOptions {
            source_path: Some(self.source.to_string_lossy().to_string()),
            group: Some(GROUP.to_string()),
            overwrite_existing,
            selected_hosts: selected_hosts.iter().map(|host| host.to_string()).collect(),
            import_forwards: true,
        }
    }
}

impl Fixture {
    fn include_text(&self) -> String {
        config_text(&self.include)
    }

    fn key_text(&self) -> String {
        config_text(&self.key)
    }

    fn certificate_text(&self) -> String {
        config_text(&self.certificate)
    }
}

/// Writes a path into the SSH config with `/` separators: the parser follows
/// `ssh` and treats `\` as an escape, so a Windows path must use forward
/// slashes there. `Path` accepts them on every supported platform.
fn config_text(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn unique_temp_path(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after the Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "tterm-p4-03-{label}-{}-{nanos}",
        std::process::id()
    ))
}

/// Opens the one process-global database this test binary may use, under a
/// fresh temporary config directory rather than the user's app data.
fn isolated_storage() {
    static STORAGE: OnceLock<()> = OnceLock::new();
    STORAGE.get_or_init(|| {
        let root = unique_temp_path("gpui-db");
        let paths = AppPaths {
            config_dir: root.join("config"),
            data_dir: root.join("data"),
            local_data_dir: root.join("local-data"),
            cache_dir: root.join("cache"),
            log_dir: root.join("logs"),
            download_dir: None,
            temp_dir: root.join("temp"),
        };
        tterm_core::config::paths::init_config_dir(&paths)
            .expect("initialize the isolated config directory");
        tterm_core::db::init().expect("open the isolated database");
    });
}

fn local_user() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_default()
}

fn jump_user() -> String {
    match local_user() {
        user if user.is_empty() => "root".to_string(),
        user => user,
    }
}

fn expected_focus(fixture: &Fixture) -> Value {
    json!({
        "hostPattern": "focus",
        "name": "focus",
        "host": "focus.example.com",
        "port": 2222,
        "username": "deploy",
        "authMethod": "key",
        "privateKeyPath": fixture.key_text(),
        "certificatePath": fixture.certificate_text(),
        "agentForward": true,
        "keepaliveIntervalSecs": 30,
        "keepaliveCountMax": 5,
        "jumpHosts": [{
            "host": "bastion.example.com",
            "port": 22,
            "username": jump_user(),
            "auth_method": "key",
            "private_key_path": fixture.key_text(),
            "certificate_path": fixture.certificate_text()
        }],
        "proxy": null,
        "warnings": [
            TARGET_KEY_WARNING,
            format!(
                "ProxyJump entries without a User were imported with the user name '{}', the local user ssh signs in as; verify each jump host username.",
                jump_user()
            )
        ],
        "unsupportedOptions": [],
        "forwards": [
            {
                "kind": "local",
                "bindHost": "localhost",
                "bindPort": 15432,
                "destHost": "db.internal",
                "destPort": 5432
            },
            {
                "kind": "remote",
                "bindHost": "localhost",
                "bindPort": 19000,
                "destHost": "localhost",
                "destPort": 3000
            },
            {
                "kind": "dynamic",
                "bindHost": "localhost",
                "bindPort": 11080,
                "destHost": "",
                "destPort": 0
            }
        ],
        "skipped": false,
        "skipReason": null,
        "existingProfileId": null
    })
}

fn expected_optional() -> Value {
    json!({
        "hostPattern": "optional",
        "name": "optional",
        "host": "optional.example.com",
        "port": 2201,
        "username": "optional-user",
        "authMethod": "password",
        "privateKeyPath": null,
        "certificatePath": null,
        "agentForward": false,
        "keepaliveIntervalSecs": 15,
        "keepaliveCountMax": 3,
        "jumpHosts": [],
        "proxy": null,
        "warnings": [],
        "unsupportedOptions": [],
        "forwards": [],
        "skipped": false,
        "skipReason": null,
        "existingProfileId": null
    })
}

fn expected_wildcard() -> Value {
    json!({
        "hostPattern": "*.dropped",
        "name": "*.dropped",
        "host": null,
        "port": 2022,
        "username": "wildcard-user",
        "authMethod": "password",
        "privateKeyPath": null,
        "certificatePath": null,
        "agentForward": false,
        "keepaliveIntervalSecs": 15,
        "keepaliveCountMax": 3,
        "jumpHosts": [],
        "proxy": null,
        "warnings": [],
        "unsupportedOptions": [],
        "forwards": [],
        "skipped": true,
        "skipReason": WILDCARD_SKIP_REASON,
        "existingProfileId": null
    })
}

fn expected_preview(fixture: &Fixture) -> Value {
    json!({
        "sourcePath": fixture.source.to_string_lossy(),
        "hosts": [expected_focus(fixture), expected_optional(), expected_wildcard()],
        "warnings": [format!(
            "{}:3: Match exec cannot be checked during import, so the block under 'Match exec true' is not applied.",
            fixture.include_text()
        )]
    })
}

fn profile<'a>(profiles: &'a [Value], name: &str) -> &'a Value {
    profiles
        .iter()
        .find(|profile| profile["name"] == name)
        .unwrap_or_else(|| panic!("{name} should be saved in {profiles:?}"))
}

fn profiles(result: &SshConfigImportResult) -> Vec<Value> {
    result
        .profiles
        .iter()
        .map(|profile| serde_json::to_value(profile).expect("profile is JSON"))
        .collect()
}

fn assert_focus_profile(profile: &Value, fixture: &Fixture, id: &str, group_path: Option<&str>) {
    assert_eq!(profile["id"], id);
    assert_eq!(profile["name"], "focus");
    assert!(
        profile["group_id"].is_string(),
        "the profile is filed in a group"
    );
    match group_path {
        Some(path) => assert_eq!(profile["group_path"], path),
        None => assert!(profile.get("group_path").is_none()),
    }
    assert_eq!(profile["connection_type"], "ssh");
    assert_eq!(profile["host"], "focus.example.com");
    assert_eq!(profile["port"], 2222);
    assert_eq!(profile["username"], "deploy");
    assert_eq!(profile["auth_method"], "key");
    assert_eq!(profile["private_key_path"], fixture.key_text());
    assert_eq!(profile["certificate_path"], fixture.certificate_text());
    assert_eq!(profile["agent_forward"], true);
    assert_eq!(profile["keepalive_interval_secs"], 30);
    assert_eq!(profile["keepalive_count_max"], 5);
    assert_eq!(profile["use_jump_host"], true);
    assert_eq!(profile["remember_password"], false);
    assert_eq!(profile["server_monitor_visible"], false);
    assert_eq!(profile["sudo_autofill"], true);
    assert!(profile.get("password").is_none(), "no password is imported");
    assert!(profile.get("proxy").is_none(), "the host sets no proxy");
    let jump = &profile["jump_hosts"][0];
    assert_eq!(profile["jump_hosts"].as_array().map(Vec::len), Some(1));
    assert_eq!(jump["host"], "bastion.example.com");
    assert_eq!(jump["port"], 22);
    assert_eq!(jump["username"], jump_user());
    assert_eq!(jump["auth_method"], "key");
    assert_eq!(jump["private_key_path"], fixture.key_text());
    assert_eq!(jump["certificate_path"], fixture.certificate_text());
    assert!(jump.get("password").is_none());
}

fn assert_optional_profile(profile: &Value, id: &str, group_path: Option<&str>) {
    assert_eq!(profile["id"], id);
    assert_eq!(profile["name"], "optional");
    match group_path {
        Some(path) => assert_eq!(profile["group_path"], path),
        None => assert!(profile.get("group_path").is_none()),
    }
    assert_eq!(profile["connection_type"], "ssh");
    assert_eq!(profile["host"], "optional.example.com");
    assert_eq!(profile["port"], 2201);
    assert_eq!(profile["username"], "optional-user");
    assert_eq!(profile["auth_method"], "password");
    assert!(profile["private_key_path"].is_null());
    assert!(profile.get("certificatePath").is_none());
    assert_eq!(profile["agent_forward"], false);
    assert_eq!(profile["keepalive_interval_secs"], 15);
    assert_eq!(profile["keepalive_count_max"], 3);
    assert_eq!(profile["use_jump_host"], false);
    assert_eq!(profile["jump_hosts"], json!([]));
    assert_eq!(profile["sudo_autofill"], true);
    assert!(profile.get("password").is_none());
}

fn assert_forward_rules(rules: &Value, profile_id: &str) {
    let expected = [
        (
            "local",
            15432,
            "db.internal",
            5432,
            "focus · db.internal:5432",
        ),
        ("remote", 19000, "localhost", 3000, "focus · localhost:3000"),
        ("dynamic", 11080, "", 0, "focus · SOCKS 11080"),
    ];
    let rules = rules.as_array().expect("tunnel rules is an array");
    assert_eq!(rules.len(), expected.len(), "{rules:?}");
    for (rule, (kind, bind_port, dest_host, dest_port, name)) in rules.iter().zip(expected) {
        assert_eq!(rule["profileId"], profile_id);
        assert_eq!(rule["kind"], kind);
        assert_eq!(rule["bindHost"], "localhost");
        assert_eq!(rule["bindPort"], bind_port);
        assert_eq!(rule["destHost"], dest_host);
        assert_eq!(rule["destPort"], dest_port);
        assert_eq!(rule["name"], name);
        assert_eq!(rule["autoStart"], false);
        assert!(
            rule["id"]
                .as_str()
                .is_some_and(|id| id.starts_with("tunnel-"))
        );
    }
}

fn summary(id: &str, name: &str, username: &str, host: &str, port: u16) -> SavedSshProfileSummary {
    SavedSshProfileSummary {
        id: id.to_string(),
        name: name.to_string(),
        username: Some(username.to_string()),
        host: Some(host.to_string()),
        port: Some(port),
        group_path: Some(GROUP.to_string()),
    }
}

#[test]
fn p4_03_gpui_services_preview_select_import_and_reread_the_shared_fixture() {
    isolated_storage();
    let fixture = Fixture::materialize();

    let preview = build_ssh_config_import_preview(Some(fixture.source.to_string_lossy().into()))
        .expect("preview the shared fixture");
    assert_eq!(
        serde_json::to_value(&preview).expect("preview is JSON"),
        expected_preview(&fixture)
    );

    let first = import_ssh_config_profiles_blocking(fixture.options(false, &["focus"]))
        .expect("import the selected host");
    assert_eq!(
        (
            first.imported,
            first.updated,
            first.skipped,
            first.tunnels_imported,
            first.tunnels_skipped
        ),
        (1, 0, 2, 3, 0)
    );
    let first_profiles = profiles(&first);
    assert_eq!(first_profiles.len(), 1);
    let focus_id = first_profiles[0]["id"]
        .as_str()
        .expect("the imported profile has an id")
        .to_string();
    assert_focus_profile(&first_profiles[0], &fixture, &focus_id, None);

    let summaries = load_saved_ssh_profile_summaries().expect("read the GPUI list summaries");
    assert_eq!(
        summaries,
        [summary(
            &focus_id,
            "focus",
            "deploy",
            "focus.example.com",
            2222
        )],
        "the unselected optional host is not imported"
    );
    let stored: Vec<Value> = load_profiles()
        .expect("read saved profiles")
        .iter()
        .map(|profile| serde_json::to_value(profile).expect("profile is JSON"))
        .collect();
    assert_focus_profile(&stored[0], &fixture, &focus_id, None);
    let rules = serde_json::to_value(load_tunnels().expect("read saved forwards"))
        .expect("forwards are JSON");
    assert_forward_rules(&rules, &focus_id);

    let second_preview =
        build_ssh_config_import_preview(Some(fixture.source.to_string_lossy().into()))
            .expect("preview after the first import");
    assert_eq!(
        second_preview.hosts[0].existing_profile_id.as_deref(),
        Some(focus_id.as_str())
    );
    assert!(second_preview.hosts[1].existing_profile_id.is_none());
    assert!(second_preview.hosts[2].existing_profile_id.is_none());

    let second = import_ssh_config_profiles_blocking(fixture.options(true, &["focus", "optional"]))
        .expect("update the first host and import the second");
    assert_eq!(
        (
            second.imported,
            second.updated,
            second.skipped,
            second.tunnels_imported,
            second.tunnels_skipped
        ),
        (1, 1, 1, 0, 3)
    );
    let second_profiles = profiles(&second);
    let optional_id = profile(&second_profiles, "optional")["id"]
        .as_str()
        .expect("the imported profile has an id")
        .to_string();
    assert_focus_profile(
        profile(&second_profiles, "focus"),
        &fixture,
        &focus_id,
        None,
    );
    assert_optional_profile(profile(&second_profiles, "optional"), &optional_id, None);

    let summaries = load_saved_ssh_profile_summaries().expect("read summaries after the update");
    assert_eq!(summaries.len(), 2);
    assert_eq!(
        summaries[0],
        summary(&focus_id, "focus", "deploy", "focus.example.com", 2222)
    );
    assert_eq!(
        summaries[1],
        summary(
            &optional_id,
            "optional",
            "optional-user",
            "optional.example.com",
            2201
        )
    );
    let stored: Vec<Value> = load_profiles()
        .expect("read saved profiles after the update")
        .iter()
        .map(|profile| serde_json::to_value(profile).expect("profile is JSON"))
        .collect();
    assert_focus_profile(profile(&stored, "focus"), &fixture, &focus_id, None);
    assert_optional_profile(profile(&stored, "optional"), &optional_id, None);
    let rules = serde_json::to_value(load_tunnels().expect("read forwards after the update"))
        .expect("forwards are JSON");
    assert_forward_rules(&rules, &focus_id);
}
