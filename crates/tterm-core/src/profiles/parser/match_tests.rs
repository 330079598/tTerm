//! `Match` blocks, checked the way `ssh` checks them.

use super::tests::{import, import_as};

fn key_path(name: &str) -> String {
    crate::profiles::parser::paths::home_dir()
        .expect("home directory")
        .join(format!(".ssh/{name}"))
        .to_string_lossy()
        .to_string()
}

#[test]
fn a_match_block_applies_with_what_the_lines_above_it_set() {
    let config = r#"
User global-user

Match host prod
  User match-user
  IdentityFile ~/.ssh/match_key

Host prod
  HostName prod.example.com
"#;
    let (hosts, warnings) = import(config);

    assert!(warnings.is_empty());
    assert_eq!(hosts.len(), 1, "a Match line is not a host to import");
    let prod = &hosts[0];
    assert_eq!(
        prod.username.as_deref(),
        Some("global-user"),
        "the User above the Match line comes first"
    );
    assert_eq!(
        prod.private_key_path.as_deref(),
        Some(key_path("match_key").as_str())
    );
}

#[test]
fn match_host_compares_the_host_name_set_so_far() {
    let config = r#"
Host db web
  HostName %h.corp.example.com

Host jump
  HostName 203.0.113.5

Match host *.corp.example.com !host web.*
  User alice

Match originalhost jump
  Port 2222
"#;
    let (hosts, _) = import(config);
    let host = |name: &str| hosts.iter().find(|host| host.name == name).unwrap();

    assert_eq!(host("db").username.as_deref(), Some("alice"));
    assert_eq!(host("jump").port, 2222);
    assert!(host("jump").username.is_none());
}

#[test]
fn match_user_and_localuser_compare_the_users() {
    let config = r#"
Host admin
  HostName admin.example.com
  User root

Host plain
  HostName plain.example.com

Match user root
  IdentityFile ~/.ssh/root_key

Match localuser alice,bob
  Port 2200

Match !localuser alice
  ServerAliveInterval 99
"#;
    let (hosts, _) = import_as(config, "alice");
    let host = |name: &str| hosts.iter().find(|host| host.name == name).unwrap();

    assert_eq!(
        host("admin").private_key_path.as_deref(),
        Some(key_path("root_key").as_str())
    );
    assert!(
        host("plain").private_key_path.is_none(),
        "without a User, Match user compares the local user"
    );
    assert_eq!(host("plain").port, 2200);
    assert_ne!(host("plain").keepalive_interval_secs, 99);

    let (hosts, _) = import_as(config, "carol");
    assert_eq!(hosts[1].port, 22);
    assert_eq!(hosts[1].keepalive_interval_secs, 99);
}

#[test]
fn match_final_reads_the_blocks_again_for_the_host_name() {
    let config = r#"
Host web
  HostName web.corp.example.com

Host *.corp.example.com
  ProxyJump bastion
  IdentityFile ~/.ssh/corp_key

Match final
  ServerAliveInterval 40

Match !final
  ServerAliveCountMax 7
"#;
    let (hosts, _) = import(config);
    let web = &hosts[0];

    assert_eq!(web.jump_hosts.len(), 1);
    assert_eq!(web.jump_hosts[0].host, "bastion");
    assert_eq!(
        web.private_key_path.as_deref(),
        Some(key_path("corp_key").as_str())
    );
    assert_eq!(web.keepalive_interval_secs, 40);
    assert_eq!(web.keepalive_count_max, 7, "the first pass is not final");
}

#[test]
fn a_match_block_that_cannot_be_checked_is_not_applied() {
    let config = r#"
Host prod
  HostName prod.example.com

Match exec "test -f ~/.vpn-up" host prod
  ProxyJump bastion

Match originalhost prod sessiontype shell
  User deploy

Match bogus value
  Port 2222
"#;
    let (hosts, warnings) = import(config);
    let prod = &hosts[0];

    assert!(prod.jump_hosts.is_empty());
    assert_eq!(prod.username.as_deref(), Some("deploy"));
    assert_eq!(prod.port, 22);
    assert!(warnings.iter().any(|warning| {
        warning.contains("Line 5: Match exec cannot be checked during import, so the block")
    }));
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("Match criterion 'bogus' is not known"))
    );
}
