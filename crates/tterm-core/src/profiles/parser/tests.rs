use super::host::build_import_host;
use super::parse::{SshConfig, parse_ssh_config};
use super::resolve::import_entries;
use crate::profiles::storage::sanitize_profile;
use crate::profiles::types::{SavedProfile, SshConfigImportHost};

/// The preview entries of a config, with the warnings about the whole file.
pub(super) fn import(config: &str) -> (Vec<SshConfigImportHost>, Vec<String>) {
    import_as(config, "local-user")
}

/// The preview entries of a config read by the local user `local_user`.
pub(super) fn import_as(config: &str, local_user: &str) -> (Vec<SshConfigImportHost>, Vec<String>) {
    let (mut config, warnings) = parse_ssh_config(config);
    config.local_user = local_user.to_string();
    config.local_host = "local-host".to_string();
    (entries(&config), warnings)
}

/// The preview entries of a config that was read already.
pub(super) fn entries(config: &SshConfig) -> Vec<SshConfigImportHost> {
    import_entries(config)
        .into_iter()
        .map(|raw| build_import_host(raw, config, &[]))
        .collect()
}

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
Host prod
  HostName prod.example.com
  User deploy
  Port 2202
  IdentityFile ~/.ssh/prod_key
  ServerAliveInterval 15
  ServerAliveCountMax 4
  ProxyJump ops@bastion.example.com:2222

Host *
  User default-user
"#;

    let (hosts, warnings) = import(config);
    assert!(warnings.is_empty());
    assert_eq!(hosts.len(), 1);

    let host = &hosts[0];
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
    assert!(
        host.warnings
            .iter()
            .any(|warning| warning.contains("verify each jump host authentication setting"))
    );
}

#[test]
fn parse_ssh_config_skips_wildcard_hosts_and_tracks_unsupported_options() {
    let config = r#"
Host *.internal
  User deploy
  ControlMaster auto
"#;

    let (hosts, warnings) = import(config);
    assert!(warnings.is_empty());
    assert_eq!(hosts.len(), 1);

    let host = &hosts[0];
    assert!(host.skipped);
    assert_eq!(
        host.skip_reason.as_deref(),
        Some(
            "Wildcard Host patterns are not imported as saved profiles; their options apply to the hosts they match."
        )
    );
    assert_eq!(host.unsupported_options, vec!["controlmaster".to_string()]);
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

    let (hosts, _) = import(config);
    let host = &hosts[0];

    assert_eq!(host.forwards.len(), 4);
    assert_eq!(host.forwards[0].dest_host, "db.internal");
    assert_eq!(host.forwards[2].bind_port, 9000);
    assert_eq!(host.forwards[3].bind_port, 1080);
    assert!(
        host.warnings
            .iter()
            .any(|warning| warning.contains("LocalForward 'nonsense' was skipped"))
    );
    assert!(
        host.unsupported_options.is_empty(),
        "forwards are supported now"
    );
}

#[test]
fn parse_ssh_config_imports_forward_agent_independent_of_auth() {
    let config = r#"
Host keyed
  HostName keyed.example.com
  IdentityFile ~/.ssh/id_ed25519

Host opted-out
  HostName out.example.com
  ForwardAgent no

Host custom-sock
  HostName sock.example.com
  ForwardAgent ~/.1password/agent.sock

Host *
  ForwardAgent yes
"#;

    let (hosts, _) = import(config);

    assert_eq!(hosts[0].auth_method, "key");
    assert!(hosts[0].agent_forward, "Host * default applies to key auth");
    assert!(!hosts[1].agent_forward, "host-level no comes before Host *");
    assert!(!hosts[2].agent_forward);
    assert!(
        hosts[2]
            .warnings
            .iter()
            .any(|warning| warning
                .contains("ForwardAgent '~/.1password/agent.sock' was not enabled"))
    );
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

    let (hosts, _) = import(config);

    let home = crate::profiles::parser::paths::home_dir().expect("home directory");
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
    assert!(
        hosts[2]
            .warnings
            .iter()
            .any(|warning| warning.contains("has no IdentityFile"))
    );
}

#[test]
fn parse_ssh_config_imports_proxy_commands() {
    use crate::profiles::types::SshConfigImportProxy;
    let config = r#"
Host tunnel
  HostName db.internal
  ProxyCommand=sh -c "cloudflared access ssh --hostname %h"
Host office
  HostName 10.0.0.5
  ProxyCommand nc -X connect -x proxy.corp:8080 %h %p
Host behind
  HostName 10.0.0.6
  ProxyCommand ssh -q -W %h:%p ops@bastion
Host both
  HostName 10.0.0.7
  ProxyJump gateway
  ProxyCommand nc %h %p
Host plain
  HostName 10.0.0.8
  ProxyCommand none
Host fallback
  HostName 10.0.0.9
Host hashed
  HostName 10.0.0.10
  ProxyCommand ssh -S ~/.ssh/cm-%C -W %h:%p bastion
Host *
  ProxyCommand cloudflared access ssh --hostname %h
"#;
    let (hosts, _) = import(config);
    let host = |name: &str| hosts.iter().find(|host| host.name == name).unwrap();

    // The shell gets the command with its quotes.
    assert_eq!(
        host("tunnel").proxy,
        Some(SshConfigImportProxy::Command {
            command: r#"sh -c "cloudflared access ssh --hostname %h""#.to_string()
        })
    );
    assert_eq!(
        host("office").proxy,
        Some(SshConfigImportProxy::Server {
            kind: crate::proxy::ProxyKind::Http,
            host: "proxy.corp".to_string(),
            port: 8080
        })
    );
    let behind = host("behind");
    assert_eq!(behind.proxy, None);
    assert_eq!(behind.jump_hosts.len(), 1);
    assert_eq!(
        (
            behind.jump_hosts[0].username.as_str(),
            behind.jump_hosts[0].host.as_str()
        ),
        ("ops", "bastion")
    );
    let both = host("both");
    assert_eq!(both.proxy, None);
    assert_eq!(both.jump_hosts[0].host, "gateway");
    assert!(
        both.warnings
            .iter()
            .any(|w| w.contains("ProxyCommand was not imported"))
    );
    assert_eq!(host("plain").proxy, Some(SshConfigImportProxy::Direct));
    assert!(matches!(
        host("fallback").proxy,
        Some(SshConfigImportProxy::Command { .. })
    ));
    // OpenSSH tokens tTerm cannot fill leave the host without a proxy.
    let hashed = host("hashed");
    assert_eq!(hashed.proxy, None);
    assert!(hashed.warnings.iter().any(|w| w.contains("%C")));
}

#[test]
fn wildcard_host_blocks_apply_to_the_hosts_they_match() {
    let config = r#"
Host *.corp.example.com !legacy.corp.example.com
  User alice
  ProxyJump bastion
  IdentityFile ~/.ssh/corp_key
  LocalForward 5432 db:5432

Host DB1.corp.example.com
  IdentityFile ~/.ssh/db1_key
  LocalForward 8080 web:80

Host legacy.corp.example.com
  User root

Host web.other.example.com
  Port 2222

Host bastion
  HostName 203.0.113.5
  User ops
"#;
    let (hosts, _) = import(config);
    let host = |name: &str| hosts.iter().find(|host| host.name == name).unwrap();
    let home = crate::profiles::parser::paths::home_dir().expect("home directory");
    let key = |name: &str| {
        home.join(format!(".ssh/{name}"))
            .to_string_lossy()
            .to_string()
    };

    assert!(host("*.corp.example.com").skipped);

    let db1 = host("DB1.corp.example.com");
    assert!(!db1.skipped);
    assert_eq!(db1.host.as_deref(), Some("DB1.corp.example.com"));
    assert_eq!(db1.username.as_deref(), Some("alice"));
    assert_eq!(
        db1.private_key_path.as_deref(),
        Some(key("db1_key").as_str()),
        "the host's own key file comes before the wildcard one"
    );
    assert_eq!(db1.forwards.len(), 2, "forwards gather from every block");
    assert_eq!(db1.jump_hosts.len(), 1);
    assert_eq!(db1.jump_hosts[0].host, "203.0.113.5");
    assert_eq!(
        db1.jump_hosts[0].username, "ops",
        "the jump host's user comes from its own Host block"
    );

    let legacy = host("legacy.corp.example.com");
    assert_eq!(
        legacy.username.as_deref(),
        Some("root"),
        "!pattern rules it out"
    );
    assert!(legacy.jump_hosts.is_empty());

    let other = host("web.other.example.com");
    assert_eq!(other.port, 2222);
    assert!(other.username.is_none());
    assert!(other.jump_hosts.is_empty());
}

#[test]
fn the_first_value_in_file_order_wins_as_in_ssh() {
    let config = r#"
User top-user

Host *
  Port 2200
  ServerAliveInterval 30

Host prod
  User prod-user
  Port 22
  HostName prod.example.com

Host prod
  HostName other.example.com
  ServerAliveCountMax 9
"#;
    let (hosts, _) = import(config);

    assert_eq!(hosts.len(), 1, "a repeated Host name is one entry");
    let prod = &hosts[0];
    assert_eq!(prod.username.as_deref(), Some("top-user"));
    assert_eq!(prod.port, 2200);
    assert_eq!(prod.host.as_deref(), Some("prod.example.com"));
    assert_eq!(prod.keepalive_interval_secs, 30);
    assert_eq!(prod.keepalive_count_max, 9);
}

#[test]
fn a_wildcard_hostname_names_the_host_with_percent_h() {
    let config = r#"
Host app-web app-api
  User deploy

Host app-*
  HostName %h.internal.example.com
"#;
    let (hosts, _) = import(config);

    assert_eq!(hosts.len(), 2);
    assert_eq!(hosts[0].name, "app-web");
    assert_eq!(
        hosts[0].host.as_deref(),
        Some("app-web.internal.example.com")
    );
    assert!(
        hosts[0]
            .warnings
            .iter()
            .any(|warning| warning.contains("only 'app-web' is imported"))
    );
    assert!(hosts[1].skipped);
}

#[test]
fn proxy_jump_and_proxy_command_from_any_block_compete() {
    use crate::profiles::types::SshConfigImportProxy;
    let config = r#"
Host gated
  HostName 10.0.0.1
  ProxyCommand none

Host jumped
  HostName 10.0.0.2
  ProxyJump gateway

Host *
  ProxyCommand nc -X 5 -x socks.corp:1080 %h %p
  ProxyJump unused
"#;
    let (hosts, _) = import(config);

    assert_eq!(hosts[0].proxy, Some(SshConfigImportProxy::Direct));
    assert!(hosts[0].jump_hosts.is_empty());
    assert!(hosts[0].warnings.iter().any(|warning| {
        warning.contains("ProxyJump was not imported because ProxyCommand comes first")
    }));

    assert_eq!(hosts[1].proxy, None);
    assert_eq!(hosts[1].jump_hosts[0].host, "gateway");
    assert!(
        hosts[1]
            .warnings
            .iter()
            .any(|warning| warning.contains("ProxyCommand was not imported"))
    );
}

#[test]
fn key_file_paths_fill_in_the_host_and_user() {
    let config = r#"
Host db
  HostName db.corp.example.com
  User deploy
  ProxyJump bastion

Host bastion
  HostName bastion.example.com

Host *
  IdentityFile ~/.ssh/%Z
  IdentityFile ~/.ssh/%h_%r
"#;
    let (hosts, _) = import(config);
    let db = &hosts[0];
    let key = |name: &str| {
        crate::profiles::parser::paths::home_dir()
            .expect("home directory")
            .join(format!(".ssh/{name}"))
            .to_string_lossy()
            .to_string()
    };

    assert_eq!(
        db.private_key_path.as_deref(),
        Some(key("db.corp.example.com_deploy").as_str())
    );
    assert!(
        db.warnings
            .iter()
            .any(|warning| warning.contains("cannot fill in %Z"))
    );
    assert_eq!(
        db.jump_hosts[0].private_key_path.as_deref(),
        Some(key("bastion.example.com_local-user").as_str()),
        "a jump host without a User fills in the local user, as ssh does"
    );
}

#[test]
fn a_host_the_config_does_not_list_still_gets_its_wildcard_blocks() {
    let config = r#"
Host *.corp.example.com
  User alice
  ProxyJump bastion

Match originalhost db9.*
  Port 2222
"#;
    let (config, _) = parse_ssh_config(config);
    let host = super::config_host_for(&config, "db9.corp.example.com");

    assert!(!host.skipped);
    assert_eq!(host.host.as_deref(), Some("db9.corp.example.com"));
    assert_eq!(host.username.as_deref(), Some("alice"));
    assert_eq!(host.port, 2222);
    assert_eq!(host.jump_hosts[0].host, "bastion");

    let other = super::config_host_for(&config, "10.0.0.1");
    assert!(other.username.is_none());
    assert!(other.jump_hosts.is_empty());
}

#[test]
fn the_known_hosts_files_a_host_is_given_are_listed_with_their_tokens() {
    let config = r#"
Host db
  HostName db.corp.example.com
  UserKnownHostsFile "~/.ssh/known hosts/%h" none

Host *.corp.example.com db
  UserKnownHostsFile ~/.ssh/ignored
  GlobalKnownHostsFile /etc/ssh/corp_known_hosts ~/.ssh/%Z

Host *
  UserKnownHostsFile ~/.ssh/known_hosts_%n
"#;
    let (config, _) = parse_ssh_config(config);
    let home =
        |name: &str| crate::profiles::parser::paths::expand_home_path(&format!("~/.ssh/{name}"));

    assert_eq!(
        super::known_hosts::named_known_hosts_files(&config, &["My DB", "db"], 22),
        [
            home("known_hosts_My DB"),
            home("known hosts/db.corp.example.com"),
            std::path::PathBuf::from("/etc/ssh/corp_known_hosts"),
        ],
        "the first value of each option wins, quotes keep a path with a space whole, and none and unfilled tokens are left out"
    );
}
