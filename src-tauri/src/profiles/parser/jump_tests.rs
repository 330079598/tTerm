//! Jump hosts read through their own `Host` blocks, the way `ssh` reads them.

use super::tests::import;

fn key_path(name: &str) -> String {
    tterm_core::profiles::parser::paths::home_dir()
        .expect("home directory")
        .join(format!(".ssh/{name}"))
        .to_string_lossy()
        .to_string()
}

#[test]
fn a_jump_host_takes_its_own_host_block() {
    let config = r#"
Host app
  HostName app.internal
  IdentityFile ~/.ssh/app_key
  ProxyJump bastion,admin@relay:2022

Host bastion
  HostName bastion.example.com
  Port 2200
  User ops
  IdentityFile ~/.ssh/bastion_key

Host relay
  HostName relay.example.com
  User nobody
  Port 22
"#;
    let (hosts, _) = import(config);
    let app = &hosts[0];

    assert_eq!(app.jump_hosts.len(), 2);
    let bastion = &app.jump_hosts[0];
    assert_eq!(
        (
            bastion.host.as_str(),
            bastion.port,
            bastion.username.as_str()
        ),
        ("bastion.example.com", 2200, "ops")
    );
    assert_eq!(bastion.auth_method, "key");
    assert_eq!(
        bastion.private_key_path.as_deref(),
        Some(key_path("bastion_key").as_str())
    );

    let relay = &app.jump_hosts[1];
    assert_eq!(
        (relay.host.as_str(), relay.port, relay.username.as_str()),
        ("relay.example.com", 2022, "admin"),
        "the user and port in ProxyJump come first"
    );
    assert_eq!(
        relay.private_key_path.as_deref(),
        Some(key_path("app_key").as_str()),
        "a jump host without a key file uses the target's"
    );
    assert!(
        app.warnings
            .iter()
            .any(|warning| warning.contains("imported with the target host private key"))
    );
}

#[test]
fn the_first_jump_host_brings_its_own_jump_hosts() {
    let config = r#"
Host app
  ProxyJump inner,second

Host inner
  HostName inner.example.com
  ProxyJump outer

Host second
  HostName second.example.com
  ProxyJump ignored

Host outer
  HostName outer.example.com
"#;
    let (hosts, _) = import(config);
    let chain: Vec<_> = hosts[0]
        .jump_hosts
        .iter()
        .map(|hop| hop.host.as_str())
        .collect();

    assert_eq!(
        chain,
        [
            "outer.example.com",
            "inner.example.com",
            "second.example.com"
        ],
        "ssh passes the hosts after the first as -J, which overrides their own ProxyJump"
    );
}

#[test]
fn a_jump_host_that_leads_back_is_skipped() {
    let config = r#"
Host bastion
  HostName bastion.example.com

Host db
  HostName db.internal

Host *
  ProxyJump bastion
"#;
    let (hosts, _) = import(config);
    let host = |name: &str| hosts.iter().find(|host| host.name == name).unwrap();

    let db = host("db");
    assert_eq!(db.jump_hosts.len(), 1, "bastion's own ProxyJump is itself");
    assert_eq!(db.jump_hosts[0].host, "bastion.example.com");

    let bastion = host("bastion");
    assert!(bastion.jump_hosts.is_empty());
    assert!(
        bastion
            .warnings
            .iter()
            .any(|warning| warning.contains("leads back to a host already on the way"))
    );
}

#[test]
fn a_jump_host_without_a_user_signs_in_as_the_local_user() {
    let config = r#"
Host app
  User deploy
  ProxyJump bastion

Host bastion
  HostName bastion.example.com
"#;
    let (hosts, _) = super::tests::import_as(config, "alice");
    let app = &hosts[0];

    assert_eq!(
        app.jump_hosts[0].username, "alice",
        "ssh signs in to a jump host as the local user, not as the target's user"
    );
    assert!(app.warnings.iter().any(|warning| {
        warning.contains("without a User were imported with the user name 'alice'")
    }));
}
