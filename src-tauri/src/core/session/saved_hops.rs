//! Jump hosts that log in as one of the user's saved connections.

use super::types::{JumpHostOptions, JumpHostPlan, SessionPlan};
use super::{load_saved_jump_host_password, load_saved_ssh_password, JumpHostSecretOwner};
use crate::profiles::types::SavedProfile;
use crate::ssh::secret_store::types::SecretStoreState;

/// Fills in each hop that names a saved connection from that connection, so
/// the hop goes where the connection points now and logs in the way it does.
pub fn fill_from_saved_connections(hops: &mut [JumpHostPlan]) -> Result<(), String> {
    if hops.iter().all(|hop| hop.profile_id.is_none()) {
        return Ok(());
    }
    let profiles = crate::profiles::storage::load_profiles()?;
    for (index, hop) in hops.iter_mut().enumerate() {
        let Some(id) = hop.profile_id.as_deref() else {
            continue;
        };
        let profile = profiles
            .iter()
            .find(|profile| profile.id == id)
            .ok_or_else(|| {
                format!(
                    "Jump host #{}: the saved connection it uses ({}@{}:{}) no longer exists. \
                     Choose another one for this hop.",
                    index + 1,
                    hop.username,
                    hop.host,
                    hop.port
                )
            })?;
        let typed = (hop.password.take(), hop.private_key_passphrase.take());
        *hop = hop_from_profile(profile, typed)
            .map_err(|error| format!("Jump host #{}: {error}", index + 1))?;
    }
    Ok(())
}

/// The hop that logs in as `profile`, with the password and key passphrase
/// typed for this connection, if any.
fn hop_from_profile(
    profile: &SavedProfile,
    (password, private_key_passphrase): (Option<String>, Option<String>),
) -> Result<JumpHostPlan, String> {
    if profile.connection_type != "ssh" {
        return Err(format!("'{}' is not an SSH connection", profile.name));
    }
    super::normalize_jump_host(JumpHostOptions {
        host: profile.host.clone(),
        port: profile.port,
        username: profile.username.clone(),
        password,
        auth_method: profile.auth_method.clone(),
        private_key_path: profile.private_key_path.clone(),
        private_key_passphrase,
        certificate_path: profile.certificate_path.clone(),
        profile_id: Some(profile.id.clone()),
    })
    .map_err(|_| format!("'{}' has no host or username", profile.name))
}

/// Fills a password hop that logs in as a saved connection with the password
/// saved for that connection.
pub(super) fn fill_saved_password(
    app: &tauri::AppHandle,
    secret_state: &SecretStoreState,
    hop: &mut JumpHostPlan,
) -> Result<(), String> {
    let Some(profile_id) = hop.profile_id.as_deref() else {
        return Ok(());
    };
    if hop.password.is_some() {
        return Ok(());
    }
    match load_saved_ssh_password(app, secret_state, Some(profile_id), None)? {
        Some(password) => hop.password = Some(password),
        None => {
            if let Some(reason) = secret_state.unreadable_reason(&[profile_id])? {
                return Err(reason.to_string());
            }
        }
    }
    Ok(())
}

/// The password saved for `hop` of `plan`: the saved connection's own when
/// the hop logs in as one, otherwise the one saved with `plan`'s profile.
pub fn load_saved_hop_password(
    app: &tauri::AppHandle,
    secret_state: &SecretStoreState,
    plan: &SessionPlan,
    hop: &JumpHostPlan,
) -> Result<Option<String>, String> {
    if let Some(profile_id) = hop.profile_id.as_deref() {
        return load_saved_ssh_password(app, secret_state, Some(profile_id), None);
    }
    load_saved_jump_host_password(
        app,
        secret_state,
        JumpHostSecretOwner {
            profile_id: plan.profile_id.as_deref(),
            profile_name: Some(plan.profile_name.as_str()),
            host: Some(hop.host.as_str()),
            port: Some(hop.port),
            username: Some(hop.username.as_str()),
        },
        plan.jump_hosts.len() == 1,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(auth_method: &str) -> SavedProfile {
        serde_json::from_value(serde_json::json!({
            "id": "bastion-id",
            "name": "Bastion",
            "connection_type": "ssh",
            "host": "bastion.example.com",
            "port": 2222,
            "username": "ops",
            "auth_method": auth_method,
            "private_key_path": "/keys/ops",
            "certificate_path": "/keys/ops-cert.pub",
        }))
        .expect("profile")
    }

    #[test]
    fn a_hop_takes_the_address_and_login_of_its_saved_connection() {
        let hop =
            hop_from_profile(&profile("key"), (None, Some("typed".to_string()))).expect("hop");
        assert_eq!(
            (hop.host.as_str(), hop.port, hop.username.as_str()),
            ("bastion.example.com", 2222, "ops")
        );
        assert_eq!(hop.private_key_path.as_deref(), Some("/keys/ops"));
        assert_eq!(hop.certificate_path.as_deref(), Some("/keys/ops-cert.pub"));
        assert_eq!(hop.private_key_passphrase.as_deref(), Some("typed"));
        assert_eq!(hop.profile_id.as_deref(), Some("bastion-id"));
    }

    #[test]
    fn a_password_hop_keeps_the_password_typed_for_it() {
        let hop =
            hop_from_profile(&profile("password"), (Some("pw".to_string()), None)).expect("hop");
        assert_eq!(hop.password.as_deref(), Some("pw"));
        assert!(hop.private_key_path.is_none());
    }

    #[test]
    fn only_an_ssh_connection_can_be_a_hop() {
        let mut serial = profile("password");
        serial.connection_type = "serial".to_string();
        assert!(hop_from_profile(&serial, (None, None)).is_err());
    }
}
