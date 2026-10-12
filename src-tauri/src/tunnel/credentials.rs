//! Works out which secrets a tunnel still needs from the user before it can
//! connect, so the UI can ask for them instead of failing.

use crate::core::session::{types::JumpHostPlan, types::PtyConnectionOptions, types::SessionPlan};
use russh::keys::{self, ssh_key};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Secrets entered by the user in answer to a [`CredentialRequest`].
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelCredentials {
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub key_passphrase: Option<String>,
    /// Save the entered passwords in secure storage.
    #[serde(default)]
    pub remember: bool,
    #[serde(default)]
    pub jump_hosts: Vec<JumpCredentials>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JumpCredentials {
    pub index: usize,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub key_passphrase: Option<String>,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CredentialKind {
    Password,
    Passphrase,
}

/// One secret the UI must collect.
#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CredentialRequest {
    /// `None` for the target host, otherwise the jump host's position.
    pub hop: Option<usize>,
    pub kind: CredentialKind,
    /// `user@host:port` the secret is for.
    pub label: String,
    /// A passphrase was supplied but did not unlock the key.
    pub incorrect: bool,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum StartOutcome {
    Started,
    NeedsCredentials { requests: Vec<CredentialRequest> },
}

/// Copies user-entered secrets into the connection options for a tunnel.
pub fn apply_credentials(options: &mut PtyConnectionOptions, credentials: &TunnelCredentials) {
    if credentials.password.is_some() {
        options.password = credentials.password.clone();
    }
    if credentials.key_passphrase.is_some() {
        options.private_key_passphrase = credentials.key_passphrase.clone();
    }
    options.remember_password = Some(credentials.remember);
    for entry in &credentials.jump_hosts {
        if let Some(jump) = options.jump_hosts.get_mut(entry.index) {
            jump.password = entry.password.clone();
            jump.private_key_passphrase = entry.key_passphrase.clone();
        }
    }
}

enum KeyCheck {
    Usable,
    NeedsPassphrase { incorrect: bool },
}

fn check_key(path: &str, passphrase: Option<&str>) -> Result<KeyCheck, String> {
    match keys::load_secret_key(Path::new(path), passphrase) {
        Ok(_) => Ok(KeyCheck::Usable),
        Err(keys::Error::KeyIsEncrypted) => Ok(KeyCheck::NeedsPassphrase { incorrect: false }),
        Err(keys::Error::SshKey(ssh_key::Error::Crypto)) if passphrase.is_some() => {
            Ok(KeyCheck::NeedsPassphrase { incorrect: true })
        }
        // Old PEM-format RSA keys are loaded when connecting, not by `russh`.
        Err(_) if crate::ssh::rsa_signer::pem_key_file_is_usable(path) => Ok(KeyCheck::Usable),
        Err(err) => Err(crate::ssh::key_file::describe_key_error(
            "SSH key", path, &err,
        )),
    }
}

fn label(username: &str, host: &str, port: u16) -> String {
    format!("{username}@{host}:{port}")
}

/// Lists the secrets missing from `plan`. Saved passwords found through the
/// `saved_*` lookups are filled into the plan; anything still absent becomes a
/// request. An unusable key file (missing, not a key) is an error, not a prompt.
pub fn collect_requests(
    plan: &mut SessionPlan,
    mut saved_target: impl FnMut(&SessionPlan) -> Result<Option<String>, String>,
    mut saved_jump: impl FnMut(&SessionPlan, &JumpHostPlan) -> Result<Option<String>, String>,
) -> Result<Vec<CredentialRequest>, String> {
    let mut requests = Vec::new();
    let target_label = label(
        plan.username.as_deref().unwrap_or_default(),
        plan.host.as_deref().unwrap_or_default(),
        plan.port,
    );

    if plan.use_agent || plan.keyboard_interactive {
        // Nothing to collect up front: the agent signs, and keyboard-interactive
        // answers cannot be known before the server asks.
    } else if let Some(key_path) = plan.private_key_path.clone() {
        if let KeyCheck::NeedsPassphrase { incorrect } =
            check_key(&key_path, plan.private_key_passphrase.as_deref())?
        {
            requests.push(CredentialRequest {
                hop: None,
                kind: CredentialKind::Passphrase,
                label: target_label,
                incorrect,
            });
        }
    } else if plan.password.is_none() {
        match saved_target(plan)? {
            Some(password) => plan.password = Some(password),
            None => requests.push(CredentialRequest {
                hop: None,
                kind: CredentialKind::Password,
                label: target_label,
                incorrect: false,
            }),
        }
    }

    for index in 0..plan.jump_hosts.len() {
        let jump = &plan.jump_hosts[index];
        let jump_label = label(&jump.username, &jump.host, jump.port);
        if jump.use_agent || jump.keyboard_interactive {
            // Nothing to collect up front, as for the target above.
        } else if let Some(key_path) = jump.private_key_path.clone() {
            if let KeyCheck::NeedsPassphrase { incorrect } =
                check_key(&key_path, jump.private_key_passphrase.as_deref())?
            {
                requests.push(CredentialRequest {
                    hop: Some(index),
                    kind: CredentialKind::Passphrase,
                    label: jump_label,
                    incorrect,
                });
            }
        } else if jump.password.is_none() {
            match saved_jump(plan, jump)? {
                Some(password) => plan.jump_hosts[index].password = Some(password),
                None => requests.push(CredentialRequest {
                    hop: Some(index),
                    kind: CredentialKind::Password,
                    label: jump_label,
                    incorrect: false,
                }),
            }
        }
    }

    Ok(requests)
}

#[cfg(test)]
mod tests;
