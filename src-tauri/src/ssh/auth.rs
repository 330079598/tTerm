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

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::client::{self, AuthResult, KeyboardInteractiveAuthResponse};
use russh::keys::ssh_key::private::KeypairData;
use russh::keys::{HashAlg, PrivateKey, PrivateKeyWithHashAlg, PublicKey};
use russh::{MethodKind, MethodSet};
use serde::Serialize;
use tauri::{Emitter, Manager};
use tokio::sync::oneshot;

use zeroize::Zeroizing;

use crate::ssh::rsa_signer::RingRsaSigner;
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

/// [`AuthMethod::Auto`]: what `ssh user@host` tries, in its order. Returns
/// the first verdict that is not a plain rejection, with what produced it
/// and the password the user typed for it, if any.
async fn auto<H>(
    session: &mut client::Handle<H>,
    target: &AuthTarget<'_>,
    prompter: &AuthPrompter,
    key_path: Option<&str>,
    local_identities: bool,
    tab_id: Option<&str>,
) -> Result<(AuthResult, AuthUsed, Option<Zeroizing<String>>), SshConnectError>
where
    H: client::Handler,
{
    // `none` asks the server which methods it takes, and lets in the rare
    // server that needs nothing.
    let mut remaining = match exchange(
        target,
        "authentication",
        session.authenticate_none(target.username),
    )
    .await?
    {
        AuthResult::Failure {
            remaining_methods,
            partial_success: false,
        } => remaining_methods,
        verdict => return Ok((verdict, AuthUsed::NoneRequired, None)),
    };

    // A password typed in this tab before goes first: it is what got in last
    // time, and the tab's SFTP, monitoring and reconnects need not ask again.
    let remembered = tab_id.and_then(|tab_id| {
        typed_passwords::get(tab_id, target.host, target.port, target.username)
            .map(|password| (tab_id, password))
    });
    if let Some((tab_id, password)) = remembered {
        let verdict = if remaining.contains(&MethodKind::Password) {
            Some(password_method(session, target, &password).await?)
        } else if remaining.contains(&MethodKind::KeyboardInteractive) {
            let mut stored = Some(password.as_str());
            Some(keyboard_interactive(session, target, prompter, &mut stored).await?)
        } else {
            None
        };
        match verdict {
            Some(AuthResult::Failure {
                remaining_methods,
                partial_success: false,
            }) => {
                // Changed on the server since; ask again below.
                typed_passwords::forget(tab_id, target.host, target.port, target.username);
                remaining = remaining_methods;
            }
            Some(verdict) => return Ok((verdict, AuthUsed::Password, None)),
            None => {}
        }
    }

    let mut offered = Vec::new();
    if remaining.contains(&MethodKind::PublicKey) {
        if local_identities {
            if let Some(verdict) =
                crate::ssh::agent::try_agent_identities(session, target.username, &mut offered)
                    .await
            {
                return Ok((verdict, AuthUsed::Agent, None));
            }
        }
        for path in identity_files(key_path, local_identities) {
            if !remaining.contains(&MethodKind::PublicKey) {
                break;
            }
            let Some((verdict, passphrase_typed)) =
                try_key_file(session, target, prompter, &path, &mut offered).await?
            else {
                continue;
            };
            match verdict {
                AuthResult::Failure {
                    remaining_methods,
                    partial_success: false,
                } => remaining = remaining_methods,
                verdict => {
                    let path = path.to_string_lossy().into_owned();
                    return Ok((
                        verdict,
                        AuthUsed::Key {
                            path,
                            passphrase_typed,
                        },
                        None,
                    ));
                }
            }
        }
    }

    let mut typed = false;
    for attempt in 0..TYPING_ATTEMPTS {
        let (verdict, used, typed_password) =
            if remaining.contains(&MethodKind::KeyboardInteractive) {
                let mut typed_password = None;
                let verdict = keyboard_interactive_capturing(
                    session,
                    target,
                    prompter,
                    &mut None,
                    &mut typed_password,
                )
                .await?;
                // The server asked for the password through PAM: a saved profile
                // can answer that with the password method.
                let used = if typed_password.is_some() {
                    AuthUsed::Password
                } else {
                    AuthUsed::KeyboardInteractive
                };
                (verdict, used, typed_password)
            } else if remaining.contains(&MethodKind::Password) {
                let instructions = if attempt == 0 {
                    String::new()
                } else {
                    "Permission denied, please try again.".to_string()
                };
                let answers = prompter
                    .ask(
                        target,
                        prompt_payload(
                            target,
                            "password",
                            String::new(),
                            instructions,
                            vec![AuthPromptField {
                                text: "Password:".to_string(),
                                echo: false,
                            }],
                        ),
                    )
                    .await?;
                let password = Zeroizing::new(answers.into_iter().next().unwrap_or_default());
                let verdict = password_method(session, target, &password).await?;
                (verdict, AuthUsed::Password, Some(password))
            } else {
                break;
            };
        typed = true;
        match verdict {
            AuthResult::Failure {
                remaining_methods,
                partial_success: false,
            } => remaining = remaining_methods,
            verdict => return Ok((verdict, used, typed_password)),
        }
    }

    let subject = target.subject();
    if typed {
        return Err(SshConnectError::Permanent(format!(
            "{subject} authentication failed"
        )));
    }
    let methods = remaining
        .iter()
        .map(<&'static str>::from)
        .collect::<Vec<_>>()
        .join(", ");
    Err(SshConnectError::Permanent(format!(
        "{subject} authentication failed: no key from the SSH agent or ~/.ssh was accepted, and \
         the server offers no password login (it accepts: {})",
        if methods.is_empty() {
            "nothing"
        } else {
            &methods
        }
    )))
}

/// Key files for [`AuthMethod::Auto`]: the configured one, then the default
/// ones that exist.
fn identity_files(key_path: Option<&str>, local_identities: bool) -> Vec<PathBuf> {
    let mut files = key_path
        .map(crate::profiles::expand_home_path)
        .into_iter()
        .collect::<Vec<_>>();
    let ssh_dir = crate::profiles::home_dir()
        .filter(|_| local_identities)
        .map(|home| home.join(".ssh"));
    for path in ssh_dir
        .iter()
        .flat_map(|dir| DEFAULT_IDENTITY_FILES.map(|name| dir.join(name)))
    {
        if path.is_file() && !files.contains(&path) {
            files.push(path);
        }
    }
    files
}

/// Offers one key file, unless its key was offered already (by the agent).
/// The user unlocks a key that has a passphrase; cancelling skips the key.
/// `None` when the key was skipped or cannot be read; otherwise the verdict
/// and whether a passphrase was typed.
async fn try_key_file<H>(
    session: &mut client::Handle<H>,
    target: &AuthTarget<'_>,
    prompter: &AuthPrompter,
    path: &std::path::Path,
    offered: &mut Vec<PublicKey>,
) -> Result<Option<(AuthResult, bool)>, SshConnectError>
where
    H: client::Handler,
{
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(None);
    };
    let display = path.to_string_lossy();
    let Ok(key) = PrivateKey::from_openssh(&text) else {
        // Not in OpenSSH format (old PEM RSA): `publickey` reads the other
        // formats, as long as they need no passphrase.
        let verdict = publickey(session, target, &display, None).await.ok();
        return Ok(verdict.map(|verdict| (verdict, false)));
    };

    let key_data = key.public_key().key_data();
    if offered.iter().any(|known| known.key_data() == key_data) {
        return Ok(None);
    }
    offered.push(key.public_key().clone());

    if !key.is_encrypted() {
        let verdict = sign_in_with_key(session, target, &display, key).await?;
        return Ok(Some((verdict, false)));
    }

    for attempt in 0..TYPING_ATTEMPTS {
        let instructions = if attempt == 0 {
            "Cancel to skip this key."
        } else {
            "Wrong passphrase, try again. Cancel to skip this key."
        };
        let payload = prompt_payload(
            target,
            "passphrase",
            String::new(),
            instructions.to_string(),
            vec![AuthPromptField {
                text: format!("Passphrase for {display}:"),
                echo: false,
            }],
        );
        // Cancelled, or no one to ask: try what comes next instead.
        let Ok(answers) = prompter.ask(target, payload).await else {
            return Ok(None);
        };
        let passphrase = answers.into_iter().next().unwrap_or_default();
        if let Ok(decrypted) = key.decrypt(passphrase.as_bytes()) {
            let verdict = sign_in_with_key(session, target, &display, decrypted).await?;
            return Ok(Some((verdict, true)));
        }
    }
    Ok(None)
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
    let subject = target.subject();
    let key_error =
        |reason: String| SshConnectError::Permanent(format!("{subject} key {path}: {reason}"));

    match crate::ssh::key_file::load_private_key(path, passphrase, &format!("{subject} key")) {
        Ok(key) => sign_in_with_key(session, target, path, key).await,
        // Old PEM-format RSA keys are beyond `russh` in this build.
        Err(load_error) => {
            let (mut signer, public_key) = std::fs::read_to_string(path)
                .ok()
                .and_then(|pem| RingRsaSigner::from_pem(&pem))
                .ok_or(SshConnectError::Permanent(load_error))?
                .map_err(key_error)?;
            sign_in_with_rsa(session, target, &mut signer, public_key).await
        }
    }
}

/// Authenticates with a loaded, decrypted key.
async fn sign_in_with_key<H>(
    session: &mut client::Handle<H>,
    target: &AuthTarget<'_>,
    path: &str,
    key: PrivateKey,
) -> Result<AuthResult, SshConnectError>
where
    H: client::Handler,
{
    // RSA is signed by `ring` (see `rsa_signer`); every other key by `russh`.
    match key.key_data() {
        KeypairData::Rsa(rsa_keypair) => {
            let mut signer = RingRsaSigner::new(rsa_keypair).map_err(|reason| {
                SshConnectError::Permanent(format!("{} key {path}: {reason}", target.subject()))
            })?;
            sign_in_with_rsa(session, target, &mut signer, key.public_key().clone()).await
        }
        _ => {
            exchange(
                target,
                "key authentication",
                session.authenticate_publickey(
                    target.username,
                    PrivateKeyWithHashAlg::new(Arc::new(key), None),
                ),
            )
            .await
        }
    }
}

async fn sign_in_with_rsa<H>(
    session: &mut client::Handle<H>,
    target: &AuthTarget<'_>,
    signer: &mut RingRsaSigner,
    public_key: PublicKey,
) -> Result<AuthResult, SshConnectError>
where
    H: client::Handler,
{
    const WHAT: &str = "key authentication";
    let subject = target.subject();

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
        session.authenticate_publickey_with(target.username, public_key, Some(hash_alg), signer),
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
    keyboard_interactive_capturing(session, target, prompter, stored_password, &mut None).await
}

/// [`keyboard_interactive`], also handing back the last answer the user typed
/// to a lone password prompt.
async fn keyboard_interactive_capturing<H>(
    session: &mut client::Handle<H>,
    target: &AuthTarget<'_>,
    prompter: &AuthPrompter,
    stored_password: &mut Option<&str>,
    typed_password: &mut Option<Zeroizing<String>>,
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
            let asks_password =
                count == 1 && !prompts[0].echo && is_password_prompt(&prompts[0].prompt);
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
            if asks_password {
                *typed_password = Some(Zeroizing::new(answers[0].clone()));
            }
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
