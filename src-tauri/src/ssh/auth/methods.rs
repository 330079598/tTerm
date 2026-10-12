//! The single authentication methods, each one exchange with the server.

use super::{
    AuthPromptField, AuthPromptPayload, AuthPrompter, AuthTarget, EXCHANGE_TIMEOUT, MAX_STEPS,
};
use crate::ssh::rsa_signer::RingRsaSigner;
use crate::ssh::types::SshConnectError;
use russh::client;
use russh::client::{AuthResult, KeyboardInteractiveAuthResponse};
use russh::keys::ssh_key::private::KeypairData;
use russh::keys::{HashAlg, PrivateKey, PrivateKeyWithHashAlg, PublicKey};
use std::sync::Arc;
use zeroize::Zeroizing;

/// Whether a hidden keyboard-interactive prompt is asking for the account
/// password, so a stored one may answer it. Anything else (verification
/// codes, tokens) is left for the user: sending the password there would
/// burn an attempt and hand it to the wrong question.
pub(super) fn is_password_prompt(text: &str) -> bool {
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

pub(super) fn prompt_payload(
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

pub(super) async fn exchange<T>(
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

pub(super) async fn password_method<H>(
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

pub(super) async fn publickey<H>(
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
pub(super) async fn sign_in_with_key<H>(
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

pub(super) async fn sign_in_with_rsa<H>(
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
pub(super) async fn keyboard_interactive<H>(
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
pub(super) async fn keyboard_interactive_capturing<H>(
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
