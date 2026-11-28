//! Connection options for a quick connection opened from the launcher.
//!
//! The launcher keeps a [`QuickConnection`] until the user confirms it. This
//! module turns that display model into the options the SSH session starts
//! with, without reading storage or prompting for credentials.

use crate::connections::quick_connect::{QuickConnection, SshConfigHost};
use crate::core::session::types::{JumpHostOptions, PtyConnectionOptions};
use crate::profiles::types::SshConfigImportHost;

/// Builds the SSH options a quick connection starts with.
///
/// Authentication stays `auto`, so the SSH client tries the agent and the
/// usual key files before asking for a password. Secrets are never copied
/// here; the SSH layer resolves them when the tab opens.
pub fn connection_options_for_quick(connection: &QuickConnection) -> PtyConnectionOptions {
    let jump_hosts = connection
        .jump_hosts
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|jump| JumpHostOptions {
            host: Some(jump.host.clone()),
            port: Some(jump.port),
            username: Some(jump.username.clone()),
            password: None,
            auth_method: Some(jump.auth_method.clone()),
            private_key_path: jump.private_key_path.clone(),
            private_key_passphrase: None,
            certificate_path: None,
            profile_id: None,
        })
        .collect();
    PtyConnectionOptions {
        connection_type: Some("ssh".to_owned()),
        profile_name: Some(connection.title.clone()),
        host: Some(connection.host.clone()),
        port: Some(connection.port),
        username: Some(connection.username.clone()),
        auth_method: Some("auto".to_owned()),
        private_key_path: connection.private_key_path.clone(),
        agent_forward: connection.agent_forward.unwrap_or(false),
        keepalive_interval_secs: connection
            .keepalive_interval_secs
            .map(|value| value.min(u16::MAX as u32) as u16),
        keepalive_count_max: connection
            .keepalive_count_max
            .map(|value| value.min(u16::MAX as u32) as u16),
        jump_hosts,
        ..PtyConnectionOptions::default()
    }
}

/// The quick-connect view of one host imported from `~/.ssh/config`.
///
/// The importer keeps fields the launcher never reads, such as forwards and
/// warnings; the launcher only needs what `ssh <alias>` would apply.
pub fn launcher_config_host(host: SshConfigImportHost) -> SshConfigHost {
    SshConfigHost {
        host_pattern: host.host_pattern,
        name: host.name,
        host: host.host,
        port: host.port,
        username: host.username,
        private_key_path: host.private_key_path,
        agent_forward: host.agent_forward,
        keepalive_interval_secs: host.keepalive_interval_secs,
        keepalive_count_max: host.keepalive_count_max,
        jump_hosts: host.jump_hosts,
        skipped: host.skipped,
    }
}

#[cfg(test)]
mod tests {
    use super::connection_options_for_quick;
    use crate::connections::quick_connect::{JumpHostConnection, QuickConnection};

    fn connection() -> QuickConnection {
        QuickConnection {
            title: "deploy@web.example".into(),
            username: "deploy".into(),
            host: "web.example".into(),
            port: 2222,
            private_key_path: Some("/keys/work".into()),
            agent_forward: Some(true),
            keepalive_interval_secs: Some(30),
            keepalive_count_max: Some(90_000),
            jump_hosts: Some(vec![JumpHostConnection {
                host: "bastion.example".into(),
                port: 22,
                username: "ops".into(),
                auth_method: "auto".into(),
                private_key_path: None,
            }]),
        }
    }

    #[test]
    fn options_keep_the_typed_target_and_resolve_auth_automatically() {
        let options = connection_options_for_quick(&connection());
        assert_eq!(options.connection_type.as_deref(), Some("ssh"));
        assert_eq!(options.host.as_deref(), Some("web.example"));
        assert_eq!(options.port, Some(2222));
        assert_eq!(options.username.as_deref(), Some("deploy"));
        assert_eq!(options.profile_name.as_deref(), Some("deploy@web.example"));
        assert_eq!(options.auth_method.as_deref(), Some("auto"));
        assert_eq!(options.private_key_path.as_deref(), Some("/keys/work"));
        assert!(options.agent_forward);
    }

    #[test]
    fn jump_hosts_and_keepalive_values_are_converted() {
        let options = connection_options_for_quick(&connection());
        assert_eq!(options.keepalive_interval_secs, Some(30));
        assert_eq!(options.keepalive_count_max, Some(u16::MAX));
        assert_eq!(options.jump_hosts.len(), 1);
        assert_eq!(
            options.jump_hosts[0].host.as_deref(),
            Some("bastion.example")
        );
        assert_eq!(options.jump_hosts[0].port, Some(22));
        assert_eq!(options.jump_hosts[0].username.as_deref(), Some("ops"));
        assert_eq!(options.jump_hosts[0].auth_method.as_deref(), Some("auto"));
    }
}
