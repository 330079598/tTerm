use super::host::build_import_host;
use super::include::read_ssh_config_with_includes;
use super::parse::parse_ssh_config;
use crate::profiles::storage::sanitize_profile;
use crate::profiles::types::SavedProfile;
use std::collections::HashSet;
use std::fs;

#[test]
fn sanitize_profile_migrates_legacy_jump_host_to_chain() {
    let mut profile: SavedProfile = serde_json::from_value(serde_json::json!({
        "id": "profile-1",
        "name": "demo",
        "group": "",
        "connection_type": "ssh",
        "remember_password": false,
        "keepalive_interval_secs": 30,
        "keepalive_count_max": 3,
        "server_monitor_visible": false,
        "jump_host": {
            "host": "bastion",
            "port": 22,
            "username": "stone",
            "auth_method": "password",
            "password": "secret"
        }
    }))
    .expect("profile should deserialize");

    sanitize_profile(&mut profile);

    assert_eq!(profile.jump_hosts.len(), 1);
    assert_eq!(profile.jump_hosts[0].host, "bastion");
    assert!(profile.jump_hosts[0].password.is_none());
    let serialized = serde_json::to_value(&profile).expect("profile should serialize");
    assert!(serialized.get("jump_host").is_none());
}

#[test]
fn parse_ssh_config_maps_supported_host_fields() {
    let config = r#"
User default-user

Host prod
  HostName prod.example.com
  User deploy
  Port 2202
  IdentityFile ~/.ssh/prod_key
  ServerAliveInterval 15
  ServerAliveCountMax 4
  ProxyJump ops@bastion.example.com:2222
"#;

    let (raw_hosts, defaults, warnings) = parse_ssh_config(config);
    assert!(warnings.is_empty());
    assert_eq!(raw_hosts.len(), 1);

    let host = build_import_host(raw_hosts[0].clone(), &defaults, &[]);
    assert!(!host.skipped);
    assert_eq!(host.name, "prod");
    assert_eq!(host.host.as_deref(), Some("prod.example.com"));
    assert_eq!(host.username.as_deref(), Some("deploy"));
    assert_eq!(host.port, 2202);
    assert_eq!(host.auth_method, "key");
    assert_eq!(host.keepalive_interval_secs, 15);
    assert_eq!(host.keepalive_count_max, 4);
    assert_eq!(host.jump_hosts.len(), 1);
    assert_eq!(host.jump_hosts[0].host, "bastion.example.com");
    assert_eq!(host.jump_hosts[0].port, 2222);
    assert_eq!(host.jump_hosts[0].username, "ops");
    assert_eq!(host.jump_hosts[0].auth_method, "key");
    assert_eq!(
        host.jump_hosts[0].private_key_path.as_deref(),
        host.private_key_path.as_deref()
    );
    assert!(host
        .warnings
        .iter()
        .any(|warning| warning.contains("verify each jump host authentication setting")));
}

#[test]
fn parse_ssh_config_skips_wildcard_hosts_and_tracks_unsupported_options() {
    let config = r#"
Host *.internal
  User deploy
  ProxyCommand ssh gateway nc %h %p
"#;

    let (raw_hosts, defaults, warnings) = parse_ssh_config(config);
    assert!(warnings.is_empty());
    assert_eq!(raw_hosts.len(), 1);

    let host = build_import_host(raw_hosts[0].clone(), &defaults, &[]);
    assert!(host.skipped);
    assert_eq!(
        host.skip_reason.as_deref(),
        Some("Wildcard Host patterns are not imported as saved profiles.")
    );
    assert_eq!(host.unsupported_options, vec!["proxycommand".to_string()]);
}

#[test]
fn parse_ssh_config_collects_forwards_and_reports_bad_ones() {
    let config = r#"
Host prod
  HostName prod.example.com
  LocalForward 5432 db.internal:5432
  LocalForward 8080 web:80
  RemoteForward 9000 localhost:3000
  DynamicForward 1080
  LocalForward nonsense
"#;

    let (raw_hosts, defaults, _) = parse_ssh_config(config);
    let host = build_import_host(raw_hosts[0].clone(), &defaults, &[]);

    assert_eq!(host.forwards.len(), 4);
    assert_eq!(host.forwards[0].dest_host, "db.internal");
    assert_eq!(host.forwards[2].bind_port, 9000);
    assert_eq!(host.forwards[3].bind_port, 1080);
    assert!(host
        .warnings
        .iter()
        .any(|warning| warning.contains("LocalForward 'nonsense' was skipped")));
    assert!(
        host.unsupported_options.is_empty(),
        "forwards are supported now"
    );
}

#[test]
fn parse_ssh_config_imports_forward_agent_independent_of_auth() {
    let config = r#"
Host *
  ForwardAgent yes

Host keyed
  HostName keyed.example.com
  IdentityFile ~/.ssh/id_ed25519

Host opted-out
  HostName out.example.com
  ForwardAgent no

Host custom-sock
  HostName sock.example.com
  ForwardAgent ~/.1password/agent.sock
"#;

    let (raw_hosts, defaults, _) = parse_ssh_config(config);
    let hosts: Vec<_> = raw_hosts
        .into_iter()
        .map(|raw| build_import_host(raw, &defaults, &[]))
        .collect();

    assert_eq!(hosts[0].auth_method, "key");
    assert!(hosts[0].agent_forward, "Host * default applies to key auth");
    assert!(!hosts[1].agent_forward, "host-level no overrides Host *");
    assert!(!hosts[2].agent_forward);
    assert!(hosts[2]
        .warnings
        .iter()
        .any(|warning| warning.contains("ForwardAgent '~/.1password/agent.sock' was not enabled")));
    assert!(hosts.iter().all(|host| host.unsupported_options.is_empty()));
}

#[test]
fn parse_ssh_config_imports_a_certificate_with_its_identity_file() {
    let config = r#"
CertificateFile ~/.ssh/default-cert.pub

Host certified
  HostName certified.example.com
  IdentityFile ~/.ssh/id_ed25519
  CertificateFile ~/.ssh/id_ed25519-ca-cert.pub
  ProxyJump bastion.example.com

Host inherits
  HostName inherits.example.com
  IdentityFile ~/.ssh/id_ed25519

Host keyless
  HostName keyless.example.com
  CertificateFile ~/.ssh/orphan-cert.pub
"#;

    let (raw_hosts, defaults, _) = parse_ssh_config(config);
    let hosts: Vec<_> = raw_hosts
        .into_iter()
        .map(|raw| build_import_host(raw, &defaults, &[]))
        .collect();

    let home = super::paths::home_dir().expect("home directory");
    let expected = home
        .join(".ssh/id_ed25519-ca-cert.pub")
        .to_string_lossy()
        .to_string();
    assert_eq!(
        hosts[0].certificate_path.as_deref(),
        Some(expected.as_str())
    );
    assert_eq!(
        hosts[0].jump_hosts[0].certificate_path.as_deref(),
        Some(expected.as_str()),
        "jump hosts take the target's key, and so its certificate"
    );
    assert!(hosts[0].unsupported_options.is_empty());

    let default = home
        .join(".ssh/default-cert.pub")
        .to_string_lossy()
        .to_string();
    assert_eq!(hosts[1].certificate_path.as_deref(), Some(default.as_str()));

    assert_eq!(hosts[2].auth_method, "password");
    assert!(hosts[2].certificate_path.is_none());
    assert!(hosts[2]
        .warnings
        .iter()
        .any(|warning| warning.contains("has no IdentityFile")));
}

#[test]
fn parse_ssh_config_ignores_match_block_options_until_next_host() {
    let config = r#"
User global-user

Match host prod
  User match-user
  IdentityFile ~/.ssh/match_key

Host prod
  HostName prod.example.com
"#;

    let (raw_hosts, defaults, warnings) = parse_ssh_config(config);
    assert_eq!(raw_hosts.len(), 1);
    assert!(warnings
        .iter()
        .any(|warning| warning.contains("Match block 'host prod' is ignored")));

    let host = build_import_host(raw_hosts[0].clone(), &defaults, &[]);
    assert_eq!(host.username.as_deref(), Some("global-user"));
    assert_eq!(host.auth_method, "password");
    assert!(host.private_key_path.is_none());
}

#[test]
fn read_ssh_config_expands_include_files() {
    let root =
        std::env::temp_dir().join(format!("tterm-ssh-config-include-{}", uuid::Uuid::new_v4()));
    let include_dir = root.join("config.d");
    fs::create_dir_all(&include_dir).expect("include dir should be created");
    let config_path = root.join("config");
    let included_path = include_dir.join("prod.conf");

    fs::write(
        &config_path,
        r#"
User default-user
Include config.d/*.conf
"#,
    )
    .expect("root config should be written");
    fs::write(
        &included_path,
        r#"
Host prod
  HostName prod.example.com
"#,
    )
    .expect("included config should be written");

    let (content, warnings) = read_ssh_config_with_includes(&config_path, &mut HashSet::new(), 0)
        .expect("include expansion should succeed");
    assert!(warnings.is_empty());
    assert!(content.contains("Host prod"));

    let (raw_hosts, defaults, parse_warnings) = parse_ssh_config(&content);
    assert!(parse_warnings.is_empty());
    let host = build_import_host(raw_hosts[0].clone(), &defaults, &[]);
    assert_eq!(host.username.as_deref(), Some("default-user"));
    assert_eq!(host.host.as_deref(), Some("prod.example.com"));

    let _ = fs::remove_dir_all(root);
}
