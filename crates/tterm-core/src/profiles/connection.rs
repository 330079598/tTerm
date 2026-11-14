//! Connection options built from a saved profile for clients that open sessions.

use crate::core::session::types::{JumpHostOptions, PtyConnectionOptions};
use crate::profiles::types::SavedProfile;

/// Builds the connection a saved SSH profile describes.
///
/// Secrets are left unset; `resolve_ssh_password` fills them from the store.
pub fn connection_options_for_profile(
    profile: &SavedProfile,
) -> Result<PtyConnectionOptions, String> {
    if profile.connection_type != "ssh" {
        return Err(format!("'{}' is not an SSH connection", profile.name));
    }
    let uses_key = profile.auth_method.as_deref() == Some("key");
    let jump_hosts = if profile.uses_jump_host() {
        profile
            .jump_hosts
            .iter()
            .map(|jump| JumpHostOptions {
                host: Some(jump.host.clone()),
                port: Some(jump.port),
                username: Some(jump.username.clone()),
                password: None,
                auth_method: Some(jump.auth_method.clone()),
                private_key_path: (jump.auth_method == "key")
                    .then(|| jump.private_key_path.clone())
                    .flatten(),
                private_key_passphrase: None,
                certificate_path: (jump.auth_method == "key")
                    .then(|| jump.certificate_path.clone())
                    .flatten(),
                profile_id: jump.profile_id.clone(),
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(PtyConnectionOptions {
        connection_type: Some("ssh".to_string()),
        profile_id: Some(profile.id.clone()),
        profile_name: Some(profile.name.clone()),
        host: profile.host.clone(),
        port: profile.port,
        username: profile.username.clone(),
        ignore_saved_password: profile.ignore_saved_password,
        keepalive_interval_secs: Some(profile.keepalive_interval_secs.min(u16::MAX as u32) as u16),
        keepalive_count_max: Some(profile.keepalive_count_max.min(u16::MAX as u32) as u16),
        private_key_path: uses_key.then(|| profile.private_key_path.clone()).flatten(),
        certificate_path: uses_key.then(|| profile.certificate_path.clone()).flatten(),
        auth_method: profile.auth_method.clone(),
        agent_forward: profile.agent_forward,
        jump_hosts,
        encoding: profile.encoding.clone(),
        ssh_shell_integration: profile.ssh_shell_integration,
        session_log: profile.session_log.clone(),
        proxy: profile.proxy.clone(),
        ..PtyConnectionOptions::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(json: serde_json::Value) -> SavedProfile {
        serde_json::from_value(json).expect("profile should deserialize")
    }

    #[test]
    fn options_carry_key_auth_and_enabled_jump_hosts() {
        let profile = profile(serde_json::json!({
            "id": "p1", "name": "prod", "connection_type": "ssh",
            "host": "10.0.0.5", "port": 2222, "username": "deploy",
            "auth_method": "key", "private_key_path": "/keys/id",
            "use_jump_host": true,
            "jump_hosts": [{"host": "bastion", "port": 22, "username": "ops", "auth_method": "password"}]
        }));
        let options = connection_options_for_profile(&profile).expect("connection options");
        assert_eq!(options.host.as_deref(), Some("10.0.0.5"));
        assert_eq!(options.port, Some(2222));
        assert_eq!(options.private_key_path.as_deref(), Some("/keys/id"));
        assert_eq!(options.jump_hosts.len(), 1);
        assert_eq!(options.jump_hosts[0].host.as_deref(), Some("bastion"));
    }

    #[test]
    fn options_skip_a_disabled_jump_chain_and_password_profiles_carry_no_key() {
        let profile = profile(serde_json::json!({
            "id": "p1", "name": "prod", "connection_type": "ssh",
            "host": "h", "username": "u", "auth_method": "password",
            "private_key_path": "/stale/key",
            "use_jump_host": false,
            "jump_hosts": [{"host": "bastion", "port": 22, "username": "ops"}]
        }));
        let options = connection_options_for_profile(&profile).expect("connection options");
        assert!(options.jump_hosts.is_empty());
        assert!(options.private_key_path.is_none());
    }

    #[test]
    fn non_ssh_profiles_are_rejected() {
        let profile = profile(serde_json::json!({
            "id": "p1", "name": "local", "connection_type": "terminal"
        }));
        assert!(connection_options_for_profile(&profile).is_err());
    }

    #[test]
    fn options_keep_the_profiles_connection_and_session_choices() {
        let profile = profile(serde_json::json!({
            "id": "p1", "name": "prod", "connection_type": "ssh",
            "host": "h", "username": "u", "auth_method": "agent",
            "agent_forward": true, "encoding": "gbk",
            "ssh_shell_integration": false, "session_log": "always"
        }));

        let options = connection_options_for_profile(&profile).expect("connection options");

        assert!(options.agent_forward);
        assert_eq!(options.encoding.as_deref(), Some("gbk"));
        assert_eq!(options.ssh_shell_integration, Some(false));
        assert_eq!(options.session_log.as_deref(), Some("always"));
    }
}
