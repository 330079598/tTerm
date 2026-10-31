//! Authenticates an established SSH transport.
//!
//! The profile's method runs first. If the server then asks for more
//! (`AuthenticationMethods publickey,keyboard-interactive`, a PAM stack that
//! only offers keyboard-interactive, an OTP after the password) the remaining
//! steps are followed here, asking the user through an [`AuthPrompter`] for
//! anything the profile cannot answer.
//!
//! [`AuthMethod::Auto`] has no configured method and tries what a plain
//! `ssh user@host` would: the agent, key files, then asking the user.

use auto::auto;
use methods::{keyboard_interactive, password_method, prompt_payload, publickey};
use prompter::lock;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::client::{self, AuthResult};
use russh::{MethodKind, MethodSet};
use serde::Serialize;
use tokio::sync::oneshot;

use crate::ssh::typed_passwords;
use crate::ssh::types::SshConnectError;

/// Budget for one request/response exchange with the server.
const EXCHANGE_TIMEOUT: Duration = Duration::from_secs(30);
/// How long the user has to answer a prompt. Servers drop unauthenticated
/// connections after their own grace period (OpenSSH: 120 s), so waiting
/// longer would only postpone the failure.
const PROMPT_TIMEOUT: Duration = Duration::from_secs(120);
/// Upper bound on follow-up methods after the first one, and on
/// keyboard-interactive rounds within one attempt. Real servers need two or
/// three; the limit only stops a misbehaving one from looping forever.
const MAX_STEPS: usize = 8;
/// How often [`AuthMethod::Auto`] asks again after a wrong password or key
/// passphrase, as OpenSSH's `NumberOfPasswordPrompts`.
const TYPING_ATTEMPTS: usize = 3;
/// Key files [`AuthMethod::Auto`] looks for in `~/.ssh`, in OpenSSH's order.
const DEFAULT_IDENTITY_FILES: [&str; 3] = ["id_ed25519", "id_ecdsa", "id_rsa"];

/// Pending prompts, keyed by request id. The sender receives the answers, or
/// `None` when the user cancels.
pub type AuthPromptMap = Arc<Mutex<HashMap<String, oneshot::Sender<Option<Vec<String>>>>>>;

/// How a profile or jump host is configured to authenticate.
#[derive(Debug, Clone, Copy)]
pub enum AuthMethod<'a> {
    Agent,
    Key {
        path: &'a str,
        passphrase: Option<&'a str>,
        /// The key's OpenSSH certificate; `<path>-cert.pub` when unset.
        certificate: Option<&'a str>,
    },
    Password(Option<&'a str>),
    /// Nothing stored: every answer comes from the user.
    KeyboardInteractive,
    /// Nothing configured: the agent, `key_path`, the default key files, then
    /// whatever the server asks the user.
    Auto {
        key_path: Option<&'a str>,
        /// Use the local agent and `~/.ssh/id_*`; tests turn this off.
        local_identities: bool,
        /// The tab whose typed passwords are remembered and tried first; see
        /// [`typed_passwords`].
        tab_id: Option<&'a str>,
    },
}

impl<'a> AuthMethod<'a> {
    /// Picks the method from the fields of a resolved plan, in the order the
    /// plans themselves give them precedence.
    pub fn from_plan(
        auto: bool,
        use_agent: bool,
        keyboard_interactive: bool,
        private_key_path: Option<&'a str>,
        private_key_passphrase: Option<&'a str>,
        certificate_path: Option<&'a str>,
        password: Option<&'a str>,
    ) -> Self {
        if auto {
            Self::Auto {
                key_path: private_key_path,
                local_identities: true,
                tab_id: None,
            }
        } else if use_agent {
            Self::Agent
        } else if keyboard_interactive {
            Self::KeyboardInteractive
        } else if let Some(path) = private_key_path {
            Self::Key {
                path,
                passphrase: private_key_passphrase,
                certificate: certificate_path,
            }
        } else {
            Self::Password(password)
        }
    }

    /// Automatic authentication on behalf of `tab_id`: passwords typed there
    /// are remembered for, and tried first by, its other connections.
    pub fn for_tab(self, tab_id: &'a str) -> Self {
        match self {
            Self::Auto {
                key_path,
                local_identities,
                ..
            } => Self::Auto {
                key_path,
                local_identities,
                tab_id: Some(tab_id),
            },
            other => other,
        }
    }
}

/// The method that got the user in, so a connection that tried
/// [`AuthMethod::Auto`] can remember what worked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthUsed {
    /// The server asked for nothing.
    NoneRequired,
    Agent,
    /// `passphrase_typed` when the user unlocked the key for this connection.
    Key {
        path: String,
        passphrase_typed: bool,
    },
    Password,
    KeyboardInteractive,
}

impl AuthUsed {
    /// The `auth_method` name a profile would store for it.
    pub fn method_name(&self) -> &'static str {
        match self {
            Self::NoneRequired | Self::KeyboardInteractive => "interactive",
            Self::Agent => "agent",
            Self::Key { .. } => "key",
            Self::Password => "password",
        }
    }

    /// Whether connecting the same way again needs nothing from the user.
    pub fn reusable(&self) -> bool {
        matches!(
            self,
            Self::Agent
                | Self::Key {
                    passphrase_typed: false,
                    ..
                }
        )
    }

    pub fn key_path(&self) -> Option<&str> {
        match self {
            Self::Key { path, .. } => Some(path),
            _ => None,
        }
    }
}

/// Who is being authenticated, for messages and prompts.
#[derive(Debug, Clone, Copy)]
pub struct AuthTarget<'a> {
    pub host: &'a str,
    pub port: u16,
    pub username: &'a str,
    /// `(hop, total)` when this is a jump host rather than the target.
    pub hop: Option<(usize, usize)>,
}

impl AuthTarget<'_> {
    /// Prefix of user-facing messages: the target is "SSH", a hop "Jump host".
    fn subject(&self) -> &'static str {
        if self.hop.is_some() {
            "Jump host"
        } else {
            "SSH"
        }
    }
}

/// A question for the user, as sent to the frontend.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthPromptPayload {
    pub request_id: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hop_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_hops: Option<usize>,
    /// `password` when the server asks for the account password through the
    /// password method, `keyboard-interactive` for server-defined prompts,
    /// `sudo` for the password sudo wants in the file manager's sudo mode.
    pub kind: &'static str,
    /// Server-supplied title and instructions; often empty.
    pub name: String,
    pub instructions: String,
    pub prompts: Vec<AuthPromptField>,
    /// The answer given to the same question just before was wrong.
    pub retry: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthPromptField {
    pub text: String,
    /// Whether the answer may be shown while typing (false for secrets).
    pub echo: bool,
}

/// How to reach the user during authentication.
#[derive(Clone)]
pub enum AuthPrompter {
    /// Emits `ssh-auth-prompt-{tab_id}`; the frontend answers through
    /// [`respond_ssh_auth_prompt`].
    Tab {
        app: tauri::AppHandle,
        tab_id: String,
    },
    /// Background sessions (monitoring, tunnels) have nowhere to show a
    /// prompt, so anything that needs one fails with an explanation.
    Unavailable,
    /// Answers prompts from a queue, recording what was asked.
    #[cfg(test)]
    Scripted(Arc<Mutex<ScriptedPrompts>>),
}

#[cfg(test)]
#[derive(Default)]
pub struct ScriptedPrompts {
    /// One entry per expected prompt; `None` cancels.
    pub answers: std::collections::VecDeque<Option<Vec<String>>>,
    pub asked: Vec<AuthPromptPayload>,
}

/// Delivers the user's answers to a waiting prompt; `responses: None` cancels.
#[tauri::command]
pub fn respond_ssh_auth_prompt(
    request_id: String,
    responses: Option<Vec<String>>,
    pending: tauri::State<'_, AuthPromptMap>,
) -> Result<(), String> {
    let sender = lock(pending.inner())
        .remove(&request_id)
        .ok_or_else(|| "Authentication prompt expired".to_string())?;
    sender
        .send(responses)
        .map_err(|_| "Authentication prompt receiver is gone".to_string())
}

/// Runs the configured method and whatever the server requires afterwards,
/// and says which method let the user in.
///
/// Every failure is [`SshConnectError::Permanent`]: rejected credentials, an
/// unusable key or an unanswered prompt are not fixed by reconnecting.
pub async fn authenticate<H>(
    session: &mut client::Handle<H>,
    target: &AuthTarget<'_>,
    method: AuthMethod<'_>,
    prompter: &AuthPrompter,
) -> Result<AuthUsed, SshConnectError>
where
    H: client::Handler,
{
    let subject = target.subject();
    let rejected = || SshConnectError::Permanent(format!("{subject} authentication failed"));

    // The stored password, until it has been sent once.
    let mut stored_password = match method {
        AuthMethod::Password(password) => password,
        _ => None,
    };

    // A password the user typed during automatic authentication.
    let mut typed_password = None;
    let mut used = match method {
        AuthMethod::Agent => AuthUsed::Agent,
        AuthMethod::Key { path, .. } => AuthUsed::Key {
            path: path.to_string(),
            passphrase_typed: false,
        },
        AuthMethod::Password(_) => AuthUsed::Password,
        AuthMethod::KeyboardInteractive | AuthMethod::Auto { .. } => AuthUsed::KeyboardInteractive,
    };

    let mut result = match method {
        AuthMethod::Agent => {
            crate::ssh::agent::authenticate_via_agent(session, target.username).await?
        }
        AuthMethod::Key {
            path,
            passphrase,
            certificate,
        } => publickey(session, target, path, passphrase, certificate).await?,
        AuthMethod::Password(_) => {
            // Nothing saved, as for a saved connection used as a jump host
            // that keeps no password: the user types it, as ssh asks.
            let typed;
            let password = match stored_password.take() {
                Some(password) => password,
                None => {
                    typed = ask_password(target, prompter).await?;
                    typed.as_str()
                }
            };
            let result = password_method(session, target, password).await?;
            // Servers with `PasswordAuthentication no` take the same password
            // through keyboard-interactive (PAM) instead. When `password` is
            // still on offer the password itself was wrong, and guessing again
            // would only burn a second attempt.
            match &result {
                AuthResult::Failure {
                    remaining_methods,
                    partial_success: false,
                } if remaining_methods.contains(&MethodKind::KeyboardInteractive)
                    && !remaining_methods.contains(&MethodKind::Password) =>
                {
                    let mut retry_password = Some(password);
                    keyboard_interactive(session, target, prompter, &mut retry_password).await?
                }
                _ => result,
            }
        }
        AuthMethod::KeyboardInteractive => {
            keyboard_interactive(session, target, prompter, &mut stored_password).await?
        }
        AuthMethod::Auto {
            key_path,
            local_identities,
            tab_id,
        } => {
            let (result, auto_used, typed) = auto(
                session,
                target,
                prompter,
                key_path,
                local_identities,
                tab_id,
            )
            .await?;
            used = auto_used;
            typed_password = typed;
            result
        }
    };

    for _ in 0..MAX_STEPS {
        let remaining = match result {
            AuthResult::Success => {
                if let (
                    AuthMethod::Auto {
                        tab_id: Some(tab_id),
                        ..
                    },
                    Some(password),
                ) = (method, &typed_password)
                {
                    typed_passwords::remember(
                        tab_id,
                        target.host,
                        target.port,
                        target.username,
                        password,
                    );
                }
                return Ok(used);
            }
            AuthResult::Failure {
                partial_success: false,
                ..
            } => return Err(rejected()),
            AuthResult::Failure {
                remaining_methods,
                partial_success: true,
            } => remaining_methods,
        };

        result = next_step(session, target, prompter, &remaining, &mut stored_password).await?;
    }

    Err(SshConnectError::Permanent(format!(
        "{subject} authentication did not complete after {MAX_STEPS} additional steps"
    )))
}

/// One more method after a partial success, chosen from what the server offers.
async fn next_step<H>(
    session: &mut client::Handle<H>,
    target: &AuthTarget<'_>,
    prompter: &AuthPrompter,
    remaining: &MethodSet,
    stored_password: &mut Option<&str>,
) -> Result<AuthResult, SshConnectError>
where
    H: client::Handler,
{
    let offers = |kind| remaining.contains(&kind);

    if offers(MethodKind::Password) {
        if let Some(password) = stored_password.take() {
            return password_method(session, target, password).await;
        }
    }
    if offers(MethodKind::KeyboardInteractive) {
        return keyboard_interactive(session, target, prompter, stored_password).await;
    }
    if offers(MethodKind::Password) {
        let password = ask_password(target, prompter).await?;
        return password_method(session, target, &password).await;
    }

    let methods = remaining
        .iter()
        .map(<&'static str>::from)
        .collect::<Vec<_>>()
        .join(", ");
    Err(SshConnectError::Permanent(format!(
        "{} server requires an additional authentication step that is not supported ({})",
        target.subject(),
        if methods.is_empty() {
            "none offered"
        } else {
            &methods
        }
    )))
}

/// Asks the user for `target`'s password.
async fn ask_password(
    target: &AuthTarget<'_>,
    prompter: &AuthPrompter,
) -> Result<String, SshConnectError> {
    let answers = prompter
        .ask(
            target,
            prompt_payload(
                target,
                "password",
                String::new(),
                String::new(),
                vec![AuthPromptField {
                    text: "Password:".to_string(),
                    echo: false,
                }],
            ),
        )
        .await?;
    Ok(answers.into_iter().next().unwrap_or_default())
}

mod auto;
mod methods;
mod prompter;

#[cfg(all(test, unix))]
mod real_sshd_tests;
#[cfg(test)]
mod tests;
