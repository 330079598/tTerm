//! Authenticates an established SSH transport.
//!
//! The profile's method runs first. If the server then asks for more
//! (`AuthenticationMethods publickey,keyboard-interactive`, a PAM stack that
//! only offers keyboard-interactive, an OTP after the password) the remaining
//! steps are followed here, asking the user through an [`AuthPrompter`] for
//! anything the profile cannot answer.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::client::{self, AuthResult, KeyboardInteractiveAuthResponse};
use russh::keys::ssh_key::private::KeypairData;
use russh::keys::{HashAlg, PrivateKeyWithHashAlg};
use russh::{MethodKind, MethodSet};
use serde::Serialize;
use tauri::{Emitter, Manager};
use tokio::sync::oneshot;

use crate::ssh::rsa_signer::RingRsaSigner;
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
    },
    Password(Option<&'a str>),
    /// Nothing stored: every answer comes from the user.
    KeyboardInteractive,
}

impl<'a> AuthMethod<'a> {
    /// Picks the method from the fields of a resolved plan, in the order the
    /// plans themselves give them precedence.
    pub fn from_plan(
        use_agent: bool,
        keyboard_interactive: bool,
        private_key_path: Option<&'a str>,
        private_key_passphrase: Option<&'a str>,
        password: Option<&'a str>,
    ) -> Self {
        if use_agent {
            Self::Agent
        } else if keyboard_interactive {
            Self::KeyboardInteractive
        } else if let Some(path) = private_key_path {
            Self::Key {
                path,
                passphrase: private_key_passphrase,
            }
        } else {
            Self::Password(password)
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
    /// password method, `keyboard-interactive` for server-defined prompts.
    pub kind: &'static str,
    /// Server-supplied title and instructions; often empty.
    pub name: String,
    pub instructions: String,
    pub prompts: Vec<AuthPromptField>,
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

impl AuthPrompter {
    pub fn tab(app: &tauri::AppHandle, tab_id: &str) -> Self {
        Self::Tab {
            app: app.clone(),
            tab_id: tab_id.to_string(),
        }
    }

    async fn ask(
        &self,
        target: &AuthTarget<'_>,
        mut payload: AuthPromptPayload,
    ) -> Result<Vec<String>, SshConnectError> {
        let subject = target.subject();
        let cancelled =
            || SshConnectError::Permanent(format!("{subject} authentication was cancelled"));

        match self {
            Self::Unavailable => Err(SshConnectError::Permanent(format!(
                "{subject} authentication needs input from you ({}), which is not possible for \
                 this kind of connection",
                describe_prompts(&payload)
            ))),
            Self::Tab { app, tab_id } => {
                let Some(pending) = app.try_state::<AuthPromptMap>() else {
                    return Err(SshConnectError::Permanent(format!(
                        "{subject} authentication prompts are unavailable"
                    )));
                };
                let pending = pending.inner().clone();
                let request_id = uuid::Uuid::new_v4().to_string();
                payload.request_id = request_id.clone();
                let (tx, rx) = oneshot::channel();
                lock(&pending).insert(request_id.clone(), tx);

                let event_name = format!("ssh-auth-prompt-{tab_id}");
                let _ = app.emit_to(tauri::EventTarget::any(), &event_name, payload);
                crate::background::reveal_for_prompt(app);

                match tokio::time::timeout(PROMPT_TIMEOUT, rx).await {
                    Ok(Ok(Some(answers))) => Ok(answers),
                    Ok(_) => Err(cancelled()),
                    Err(_) => {
                        lock(&pending).remove(&request_id);
                        // Tell the frontend to close the dialog it is still showing.
                        let _ = app.emit_to(
                            tauri::EventTarget::any(),
                            &format!("ssh-auth-prompt-expired-{tab_id}"),
                            request_id,
                        );
                        Err(SshConnectError::Permanent(format!(
                            "{subject} authentication timed out waiting for your input"
                        )))
                    }
                }
            }
            #[cfg(test)]
            Self::Scripted(script) => {
                let mut script = script.lock().expect("scripted prompts lock");
                script.asked.push(payload);
                script.answers.pop_front().flatten().ok_or_else(cancelled)
            }
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}

fn describe_prompts(payload: &AuthPromptPayload) -> String {
    let texts = payload
        .prompts
        .iter()
        .map(|prompt| prompt.text.trim().trim_end_matches(':').trim())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();
    if texts.is_empty() {
        payload.kind.to_string()
    } else {
        texts.join(", ")
    }
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

/// Whether a hidden keyboard-interactive prompt is asking for the account
/// password, so a stored one may answer it. Anything else (verification
/// codes, tokens) is left for the user: sending the password there would
/// burn an attempt and hand it to the wrong question.
fn is_password_prompt(text: &str) -> bool {
    let text = text.to_lowercase();
    [
        "password",
        "passwort",
        "contraseña",
        "mot de passe",
        "密码",
        "密碼",
        "口令",
        "パスワード",
    ]
    .iter()
    .any(|needle| text.contains(needle))
        && ![
            "one-time",
            "one time",
            "otp",
            "verification",
            "token",
            "动态",
            "動態",
            "验证码",
        ]
        .iter()
        .any(|needle| text.contains(needle))
}

/// Runs the configured method and whatever the server requires afterwards.
///
/// Every failure is [`SshConnectError::Permanent`]: rejected credentials, an
/// unusable key or an unanswered prompt are not fixed by reconnecting.
pub async fn authenticate<H>(
    session: &mut client::Handle<H>,
    target: &AuthTarget<'_>,
    method: AuthMethod<'_>,
    prompter: &AuthPrompter,
) -> Result<(), SshConnectError>
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

    let mut result = match method {
        AuthMethod::Agent => {
            crate::ssh::agent::authenticate_via_agent(session, target.username).await?
        }
        AuthMethod::Key { path, passphrase } => {
            publickey(session, target, path, passphrase).await?
        }
        AuthMethod::Password(_) => {
            let password = stored_password.take().ok_or_else(|| {
                SshConnectError::Permanent(format!("{subject} password is required"))
            })?;
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
    };

    for _ in 0..MAX_STEPS {
        let remaining = match result {
            AuthResult::Success => return Ok(()),
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
        let password = answers.into_iter().next().unwrap_or_default();
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

fn prompt_payload(
    target: &AuthTarget<'_>,
    kind: &'static str,
    name: String,
    instructions: String,
    prompts: Vec<AuthPromptField>,
) -> AuthPromptPayload {
    AuthPromptPayload {
        request_id: String::new(),
        host: target.host.to_string(),
        port: target.port,
        username: target.username.to_string(),
        hop_index: target.hop.map(|(index, _)| index),
        total_hops: target.hop.map(|(_, total)| total),
        kind,
        name,
        instructions,
        prompts,
    }
}

async fn exchange<T>(
    target: &AuthTarget<'_>,
    what: &str,
    future: impl std::future::Future<Output = Result<T, russh::Error>>,
) -> Result<T, SshConnectError> {
    let subject = target.subject();
    tokio::time::timeout(EXCHANGE_TIMEOUT, future)
        .await
        .map_err(|_| SshConnectError::Permanent(format!("{subject} {what} timed out")))?
        .map_err(|error| SshConnectError::Permanent(format!("{subject} {what} failed: {error}")))
}

async fn password_method<H>(
    session: &mut client::Handle<H>,
    target: &AuthTarget<'_>,
    password: &str,
) -> Result<AuthResult, SshConnectError>
where
    H: client::Handler,
{
    exchange(
        target,
        "password authentication",
        session.authenticate_password(target.username, password),
    )
    .await
}

async fn publickey<H>(
    session: &mut client::Handle<H>,
    target: &AuthTarget<'_>,
    path: &str,
    passphrase: Option<&str>,
) -> Result<AuthResult, SshConnectError>
where
    H: client::Handler,
{
    const WHAT: &str = "key authentication";

    let subject = target.subject();
    let key_error =
        |reason: String| SshConnectError::Permanent(format!("{subject} key {path}: {reason}"));

    // RSA is signed by `ring` (see `rsa_signer`); every other key by `russh`.
    let (mut signer, public_key) =
        match crate::ssh::key_file::load_private_key(path, passphrase, &format!("{subject} key")) {
            Ok(key) => match key.key_data() {
                KeypairData::Rsa(rsa_keypair) => (
                    RingRsaSigner::new(rsa_keypair).map_err(key_error)?,
                    key.public_key().clone(),
                ),
                _ => {
                    return exchange(
                        target,
                        WHAT,
                        session.authenticate_publickey(
                            target.username,
                            PrivateKeyWithHashAlg::new(Arc::new(key), None),
                        ),
                    )
                    .await
                }
            },
            // Old PEM-format RSA keys are beyond `russh` in this build.
            Err(load_error) => std::fs::read_to_string(path)
                .ok()
                .and_then(|pem| RingRsaSigner::from_pem(&pem))
                .ok_or(SshConnectError::Permanent(load_error))?
                .map_err(key_error)?,
        };

    // Use the best rsa-sha2-* the server advertises. Plain `ssh-rsa` is
    // SHA-1, which OpenSSH 8.8+ rejects and `ring` does not produce.
    let hash_alg = match session.best_supported_rsa_hash().await.unwrap_or(None) {
        Some(Some(hash_alg)) => hash_alg,
        Some(None) => {
            return Err(SshConnectError::Permanent(format!(
                "{subject} server only accepts SHA-1 signatures for RSA keys, which tTerm \
                 does not produce. Use an Ed25519 or ECDSA key for this host."
            )))
        }
        // Not advertised (servers older than OpenSSH 7.2): those that know
        // rsa-sha2 at all accept SHA-256.
        None => HashAlg::Sha256,
    };

    tokio::time::timeout(
        EXCHANGE_TIMEOUT,
        session.authenticate_publickey_with(
            target.username,
            public_key,
            Some(hash_alg),
            &mut signer,
        ),
    )
    .await
    .map_err(|_| SshConnectError::Permanent(format!("{subject} {WHAT} timed out")))?
    .map_err(|error| SshConnectError::Permanent(format!("{subject} {WHAT} failed: {error}")))
}

/// Runs one keyboard-interactive attempt to its verdict. A hidden prompt for
/// the account password is answered once from `stored_password`; everything
/// else goes to the user.
async fn keyboard_interactive<H>(
    session: &mut client::Handle<H>,
    target: &AuthTarget<'_>,
    prompter: &AuthPrompter,
    stored_password: &mut Option<&str>,
) -> Result<AuthResult, SshConnectError>
where
    H: client::Handler,
{
    const WHAT: &str = "keyboard-interactive authentication";

    let mut response = exchange(
        target,
        WHAT,
        session.authenticate_keyboard_interactive_start(target.username, None),
    )
    .await?;

    for _ in 0..MAX_STEPS {
        let (name, instructions, prompts) = match response {
            KeyboardInteractiveAuthResponse::Success => return Ok(AuthResult::Success),
            KeyboardInteractiveAuthResponse::Failure {
                remaining_methods,
                partial_success,
            } => {
                return Ok(AuthResult::Failure {
                    remaining_methods,
                    partial_success,
                })
            }
            KeyboardInteractiveAuthResponse::InfoRequest {
                name,
                instructions,
                prompts,
            } => (name, instructions, prompts),
        };

        let answers = if prompts.is_empty() {
            // An informational round (e.g. a PAM message) expects an empty reply.
            Vec::new()
        } else if let Some(password) = stored_password
            .filter(|_| prompts.len() == 1 && !prompts[0].echo)
            .filter(|_| is_password_prompt(&prompts[0].prompt))
        {
            *stored_password = None;
            vec![password.to_string()]
        } else {
            let count = prompts.len();
            let fields = prompts
                .into_iter()
                .map(|prompt| AuthPromptField {
                    text: prompt.prompt,
                    echo: prompt.echo,
                })
                .collect();
            let mut answers = prompter
                .ask(
                    target,
                    prompt_payload(target, "keyboard-interactive", name, instructions, fields),
                )
                .await?;
            // The protocol requires exactly one answer per prompt.
            answers.resize(count, String::new());
            answers
        };

        response = exchange(
            target,
            WHAT,
            session.authenticate_keyboard_interactive_respond(answers),
        )
        .await?;
    }

    Err(SshConnectError::Permanent(format!(
        "{} {WHAT} did not finish after {MAX_STEPS} rounds",
        target.subject()
    )))
}

#[cfg(all(test, unix))]
mod real_sshd_tests;
#[cfg(test)]
mod tests;
