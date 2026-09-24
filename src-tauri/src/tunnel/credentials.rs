//! Works out which secrets a tunnel still needs from the user before it can
//! connect, so the UI can ask for them instead of failing.

use crate::core::session::{JumpHostPlan, PtyConnectionOptions, SessionPlan};
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

    if plan.use_agent {
        // Nothing to prompt for: the local SSH agent supplies the signature.
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
        if jump.use_agent {
            // Nothing to prompt for: the local SSH agent supplies the signature.
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
mod tests {
    use super::*;
    use crate::core::session::{normalize_connection, JumpHostOptions};
    use russh::keys::ssh_key::{Algorithm, LineEnding, PrivateKey};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn write_key(passphrase: Option<&str>) -> String {
        let mut rng = rand::rngs::OsRng;
        let mut key = PrivateKey::random(&mut rng, Algorithm::Ed25519).unwrap();
        if let Some(passphrase) = passphrase {
            key = key.encrypt(&mut rng, passphrase).unwrap();
        }
        let dir = std::env::temp_dir().join(format!(
            "tterm-cred-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("id");
        std::fs::write(&path, key.to_openssh(LineEnding::LF).unwrap().as_bytes()).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn plan(
        key: Option<&str>,
        passphrase: Option<&str>,
        password: Option<&str>,
        jumps: Vec<JumpHostOptions>,
    ) -> SessionPlan {
        normalize_connection(Some(PtyConnectionOptions {
            connection_type: Some("ssh".into()),
            host: Some("target".into()),
            port: Some(22),
            username: Some("deploy".into()),
            private_key_path: key.map(str::to_string),
            private_key_passphrase: passphrase.map(str::to_string),
            password: password.map(str::to_string),
            jump_hosts: jumps,
            ..PtyConnectionOptions::default()
        }))
        .unwrap()
    }

    fn jump(key: Option<&str>) -> JumpHostOptions {
        JumpHostOptions {
            host: Some("bastion".into()),
            port: Some(22),
            username: Some("ops".into()),
            password: None,
            auth_method: Some(if key.is_some() { "key" } else { "password" }.into()),
            private_key_path: key.map(str::to_string),
            private_key_passphrase: None,
        }
    }

    fn none_saved() -> (
        impl FnMut(&SessionPlan) -> Result<Option<String>, String>,
        impl FnMut(&SessionPlan, &JumpHostPlan) -> Result<Option<String>, String>,
    ) {
        (
            |_: &SessionPlan| Ok(None),
            |_: &SessionPlan, _: &JumpHostPlan| Ok(None),
        )
    }

    #[test]
    fn missing_password_is_requested_unless_saved_or_provided() {
        let (t, j) = none_saved();
        let mut missing = plan(None, None, None, vec![]);
        let requests = collect_requests(&mut missing, t, j).unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].kind, CredentialKind::Password);
        assert_eq!(requests[0].hop, None);
        assert_eq!(requests[0].label, "deploy@target:22");

        let (t, j) = none_saved();
        let mut provided = plan(None, None, Some("pw"), vec![]);
        assert!(collect_requests(&mut provided, t, j).unwrap().is_empty());

        let mut saved = plan(None, None, None, vec![]);
        let requests =
            collect_requests(&mut saved, |_| Ok(Some("stored".into())), |_, _| Ok(None)).unwrap();
        assert!(requests.is_empty());
        assert_eq!(saved.password.as_deref(), Some("stored"));
    }

    #[test]
    fn encrypted_keys_request_a_passphrase_and_flag_a_wrong_one() {
        let key = write_key(Some("right"));

        let (t, j) = none_saved();
        let requests = collect_requests(&mut plan(Some(&key), None, None, vec![]), t, j).unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].kind, CredentialKind::Passphrase);
        assert!(!requests[0].incorrect);

        let (t, j) = none_saved();
        let requests =
            collect_requests(&mut plan(Some(&key), Some("wrong"), None, vec![]), t, j).unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].incorrect);

        let (t, j) = none_saved();
        let requests =
            collect_requests(&mut plan(Some(&key), Some("right"), None, vec![]), t, j).unwrap();
        assert!(requests.is_empty());
    }

    #[test]
    fn plain_keys_need_nothing() {
        let key = write_key(None);
        let (t, j) = none_saved();
        assert!(
            collect_requests(&mut plan(Some(&key), None, None, vec![]), t, j)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn unusable_key_files_are_errors_not_prompts() {
        let (t, j) = none_saved();
        let err = collect_requests(
            &mut plan(Some("/definitely/not/here"), None, None, vec![]),
            t,
            j,
        )
        .unwrap_err();
        assert!(err.contains("/definitely/not/here"));
    }

    #[test]
    fn jump_hosts_are_checked_independently() {
        let key = write_key(Some("pw"));
        let (t, j) = none_saved();
        let mut p = plan(
            None,
            None,
            Some("target-pw"),
            vec![jump(None), jump(Some(&key))],
        );
        let requests = collect_requests(&mut p, t, j).unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(
            (requests[0].hop, requests[0].kind),
            (Some(0), CredentialKind::Password)
        );
        assert_eq!(
            (requests[1].hop, requests[1].kind),
            (Some(1), CredentialKind::Passphrase)
        );
        assert_eq!(requests[0].label, "ops@bastion:22");
    }

    #[test]
    fn saved_jump_passwords_are_filled_in() {
        let mut p = plan(None, None, Some("pw"), vec![jump(None)]);
        let requests =
            collect_requests(&mut p, |_| Ok(None), |_, _| Ok(Some("jump-pw".into()))).unwrap();
        assert!(requests.is_empty());
        assert_eq!(p.jump_hosts[0].password.as_deref(), Some("jump-pw"));
    }

    #[test]
    fn apply_credentials_maps_secrets_onto_the_right_hop() {
        let mut options = PtyConnectionOptions {
            jump_hosts: vec![jump(None), jump(None)],
            ..PtyConnectionOptions::default()
        };
        let credentials: TunnelCredentials = serde_json::from_value(serde_json::json!({
            "password": "pw", "keyPassphrase": "pp", "remember": true,
            "jumpHosts": [{"index": 1, "password": "jp"}, {"index": 9, "password": "ignored"}]
        }))
        .unwrap();
        apply_credentials(&mut options, &credentials);
        assert_eq!(options.password.as_deref(), Some("pw"));
        assert_eq!(options.private_key_passphrase.as_deref(), Some("pp"));
        assert_eq!(options.remember_password, Some(true));
        assert_eq!(options.jump_hosts[0].password, None);
        assert_eq!(options.jump_hosts[1].password.as_deref(), Some("jp"));
    }

    #[test]
    fn outcomes_serialize_with_a_status_tag() {
        assert_eq!(
            serde_json::to_value(StartOutcome::Started).unwrap(),
            serde_json::json!({"status": "started"})
        );
        let value = serde_json::to_value(StartOutcome::NeedsCredentials {
            requests: vec![CredentialRequest {
                hop: Some(0),
                kind: CredentialKind::Password,
                label: "a@b:22".into(),
                incorrect: false,
            }],
        })
        .unwrap();
        assert_eq!(value["status"], "needsCredentials");
        assert_eq!(value["requests"][0]["kind"], "password");
        assert_eq!(value["requests"][0]["hop"], 0);
    }
}
