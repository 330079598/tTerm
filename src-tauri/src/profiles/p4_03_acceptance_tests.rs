//! P4-03 acceptance: the real Tauri SSH config import commands over mock IPC.
//!
//! The fixture is the one shared by the GPUI acceptance test. The test writes
//! it to a temporary directory, points the process-global config directory and
//! database at a fresh temporary directory, and never reads the user's
//! `~/.ssh/config`, database or private keys.

use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::MockRuntime;
use tauri::webview::InvokeRequest;
use tauri::{WebviewWindow, test};
use tterm_core::paths::AppPaths;

const CONFIG_TEMPLATE: &str =
    include_str!("../../../crates/tterm-core/tests/fixtures/ssh-config/p4-03/config");
const INCLUDED_TEMPLATE: &str =
    include_str!("../../../crates/tterm-core/tests/fixtures/ssh-config/p4-03/included.conf");
const GROUP: &str = "P4-03 / Acceptance";
const WILDCARD_SKIP_REASON: &str = "Wildcard Host patterns are not imported as saved profiles; their options apply to the hosts they match.";
const TARGET_KEY_WARNING: &str = "ProxyJump entries without an IdentityFile of their own were imported with the target host private key; verify each jump host authentication setting.";

/// The materialized shared fixture and the absolute paths written into it.
struct Fixture {
    root: PathBuf,
    source: PathBuf,
    include: PathBuf,
    key: PathBuf,
    certificate: PathBuf,
}

impl Fixture {
    fn materialize() -> Self {
        let root = unique_temp_path("tauri-fixture");
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
        let root = unique_temp_path("tauri-db");
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

fn invoke(webview: &WebviewWindow<MockRuntime>, command: &str, arguments: Value) -> Value {
    let request = InvokeRequest {
        cmd: command.to_string(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: if cfg!(any(windows, target_os = "android")) {
            "http://tauri.localhost"
        } else {
            "tauri://localhost"
        }
        .parse()
        .expect("valid IPC origin"),
        body: InvokeBody::Json(arguments),
        headers: Default::default(),
        invoke_key: test::INVOKE_KEY.to_string(),
    };
    test::get_ipc_response(webview, request)
        .unwrap_or_else(|error| panic!("{command} failed over IPC: {error}"))
        .deserialize::<Value>()
        .expect("command response is JSON")
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

fn required_string<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key]
        .as_str()
        .unwrap_or_else(|| panic!("{key} should be a string in {value}"))
}

fn expected_focus(fixture: &Fixture) -> Value {
    let user = jump_user();
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
            "username": user,
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

fn find_named<'a>(profiles: &'a [Value], name: &str) -> &'a Value {
    profiles
        .iter()
        .find(|profile| profile["name"] == name)
        .unwrap_or_else(|| panic!("{name} should be saved in {profiles:?}"))
}

fn find_profile<'a>(profiles: &'a Value, name: &str) -> &'a Value {
    find_named(profiles.as_array().expect("profiles is an array"), name)
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
            required_string(rule, "id").starts_with("tunnel-"),
            "imported rules get a generated id"
        );
    }
}

#[test]
fn p4_03_tauri_commands_preview_select_import_and_reread_the_shared_fixture() {
    isolated_storage();
    let fixture = Fixture::materialize();
    let app = test::mock_builder()
        .invoke_handler(tauri::generate_handler![
            crate::profiles::commands::preview_ssh_config_import,
            crate::profiles::commands::import_ssh_config_profiles,
            crate::profiles::commands::list_profiles,
            crate::tunnel::commands::list_tunnels,
        ])
        .build(test::mock_context(test::noop_assets()))
        .expect("build the mock Tauri app");
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .expect("build the mock webview");

    let preview = invoke(
        &webview,
        "preview_ssh_config_import",
        json!({ "sourcePath": fixture.source }),
    );
    assert_eq!(preview, expected_preview(&fixture));

    let first = invoke(
        &webview,
        "import_ssh_config_profiles",
        json!({
            "options": {
                "sourcePath": fixture.source,
                "group": GROUP,
                "overwriteExisting": false,
                "selectedHosts": ["focus"],
                "importForwards": true
            }
        }),
    );
    assert_eq!(first["imported"], 1);
    assert_eq!(first["updated"], 0);
    assert_eq!(first["skipped"], 2);
    assert_eq!(first["tunnelsImported"], 3);
    assert_eq!(first["tunnelsSkipped"], 0);
    assert_eq!(first["profiles"].as_array().map(Vec::len), Some(1));
    let focus_id = required_string(&first["profiles"][0], "id").to_string();
    assert_focus_profile(&first["profiles"][0], &fixture, &focus_id, None);

    let listed = invoke(&webview, "list_profiles", json!({}));
    let listed = listed.as_array().expect("profiles is an array");
    assert_eq!(listed.len(), 1, "the unselected host was not imported");
    assert_focus_profile(&listed[0], &fixture, &focus_id, Some(GROUP));

    let rules = invoke(&webview, "list_tunnels", json!({}));
    assert_forward_rules(&rules, &focus_id);

    let second_preview = invoke(
        &webview,
        "preview_ssh_config_import",
        json!({ "sourcePath": fixture.source }),
    );
    assert_eq!(second_preview["hosts"][0]["existingProfileId"], focus_id);
    assert!(second_preview["hosts"][1]["existingProfileId"].is_null());
    assert!(second_preview["hosts"][2]["existingProfileId"].is_null());

    let second = invoke(
        &webview,
        "import_ssh_config_profiles",
        json!({
            "options": {
                "sourcePath": fixture.source,
                "group": GROUP,
                "overwriteExisting": true,
                "selectedHosts": ["focus", "optional"],
                "importForwards": true
            }
        }),
    );
    assert_eq!(second["imported"], 1);
    assert_eq!(second["updated"], 1);
    assert_eq!(second["skipped"], 1, "the wildcard host is still skipped");
    assert_eq!(second["tunnelsImported"], 0, "the forwards already exist");
    assert_eq!(second["tunnelsSkipped"], 3);
    let optional_id =
        required_string(find_profile(&second["profiles"], "optional"), "id").to_string();
    assert_focus_profile(
        find_profile(&second["profiles"], "focus"),
        &fixture,
        &focus_id,
        None,
    );
    assert_optional_profile(
        find_profile(&second["profiles"], "optional"),
        &optional_id,
        None,
    );

    let listed = invoke(&webview, "list_profiles", json!({}));
    let listed = listed.as_array().expect("profiles is an array");
    assert_eq!(listed.len(), 2);
    assert_focus_profile(
        find_named(listed, "focus"),
        &fixture,
        &focus_id,
        Some(GROUP),
    );
    assert_optional_profile(find_named(listed, "optional"), &optional_id, Some(GROUP));
    let rules = invoke(&webview, "list_tunnels", json!({}));
    assert_forward_rules(&rules, &focus_id);
}
