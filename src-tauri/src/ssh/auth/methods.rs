//! The single authentication methods, each one exchange with the server.

use super::{
    AuthPromptField, AuthPromptPayload, AuthPrompter, AuthTarget, EXCHANGE_TIMEOUT, MAX_STEPS,
};
use crate::ssh::agent::offered_before;
use crate::ssh::rsa_signer::RingRsaSigner;
use crate::ssh::types::SshConnectError;
use russh::client::{AuthResult, KeyboardInteractiveAuthResponse};
use russh::keys::agent::AgentIdentity;
use russh::keys::ssh_key::private::KeypairData;
use russh::keys::{Certificate, HashAlg, PrivateKey, PrivateKeyWithHashAlg, PublicKey};
use russh::{client, MethodKind};
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

pub(super) async fn exchange<T, E: std::fmt::Display>(
    target: &AuthTarget<'_>,
    what: &str,
    future: impl std::future::Future<Output = Result<T, E>>,
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

/// Signs in with the key file at `path`, and with `certificate_path` as its
/// certificate when one is configured; see [`key_file_identities`].
pub(super) async fn publickey<H>(
    session: &mut client::Handle<H>,
    target: &AuthTarget<'_>,
    path: &str,
    passphrase: Option<&str>,
    certificate_path: Option<&str>,
) -> Result<AuthResult, SshConnectError>
where
    H: client::Handler,
{
    let subject = target.subject();
    let key_error =
        |reason: String| SshConnectError::Permanent(format!("{subject} key {path}: {reason}"));
    let certificate = |public_key: &PublicKey| {
        certificate_path
            .map(|certificate_path| load_certificate(certificate_path, public_key))
            .transpose()
            .map_err(|reason| SshConnectError::Permanent(format!("{subject} {reason}")))
    };

    match crate::ssh::key_file::load_private_key(path, passphrase, &format!("{subject} key")) {
        Ok(key) => {
            let certificate = certificate(key.public_key())?;
            sign_in_with_key(session, target, path, key, certificate, &mut Vec::new()).await
        }
        // Old PEM-format RSA keys are beyond `russh` in this build.
        Err(load_error) => {
            let (mut signer, public_key) = std::fs::read_to_string(path)
                .ok()
                .and_then(|pem| RingRsaSigner::from_pem(&pem))
                .ok_or(SshConnectError::Permanent(load_error))?
                .map_err(key_error)?;
            let identities = key_file_identities(path, &public_key, certificate(&public_key)?);
            let mut offered = Vec::new();
            sign_in_with_rsa(session, target, &mut signer, path, identities, &mut offered).await
        }
    }
}

/// Reads the certificate configured for a key, which must certify that key
/// and must not have expired. Unlike the certificate `ssh` finds beside a
/// key, a configured one that cannot be used is an error, so the user learns
/// why instead of seeing the server turn the key down.
pub(super) fn load_certificate(path: &str, public_key: &PublicKey) -> Result<Certificate, String> {
    let text = std::fs::read_to_string(crate::profiles::parser::expand_home_path(path)).map_err(
        |error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                format!("certificate file not found: {path}")
            } else {
                format!("cannot read certificate file {path}: {error}")
            }
        },
    )?;
    let certificate = Certificate::from_openssh(text.trim())
        .map_err(|error| format!("{path} is not an OpenSSH certificate: {error}"))?;
    if certificate.public_key() != public_key.key_data() {
        return Err(format!(
            "certificate {path} certifies a different key than the private key"
        ));
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    if certificate.valid_before() <= now {
        let expired = i64::try_from(certificate.valid_before())
            .ok()
            .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
            .map(|time| {
                chrono::DateTime::<chrono::Local>::from(time)
                    .format("%Y-%m-%d %H:%M")
                    .to_string()
            })
            .unwrap_or_default();
        return Err(format!(
            "certificate {path} expired at {expired}; get a new one from your certificate authority"
        ));
    }
    Ok(certificate)
}

/// What a key file offers the server, in `ssh`'s order: its OpenSSH
/// certificate, then the key itself. The certificate is `certificate` when
/// one is configured, and otherwise the one stored beside the key as
/// `<key>-cert.pub`.
///
/// Servers that trust a certificate authority (`TrustedUserCAKeys`) list no
/// user keys, so the certificate is what gets the user in. A certificate
/// beside the key that cannot be read, or that certifies another key, is left
/// out, as `ssh` does.
pub(super) fn key_file_identities(
    path: &str,
    public_key: &PublicKey,
    certificate: Option<Certificate>,
) -> Vec<AgentIdentity> {
    let certificate = certificate.or_else(|| {
        std::fs::read_to_string(format!("{path}-cert.pub"))
            .ok()
            .and_then(|text| Certificate::from_openssh(text.trim()).ok())
            .filter(|certificate| certificate.public_key() == public_key.key_data())
    });
    certificate
        .map(AgentIdentity::from)
        .into_iter()
        .chain([AgentIdentity::from(public_key.clone())])
        .collect()
}

/// Whether the server turned an identity down but still takes another one.
fn offer_next(verdict: &AuthResult) -> bool {
    matches!(
        verdict,
        AuthResult::Failure {
            remaining_methods,
            partial_success: false,
        } if remaining_methods.contains(&MethodKind::PublicKey)
    )
}

/// The error for a key whose identities were all offered before. Callers
/// that keep an `offered` list skip such a key instead of signing with it.
fn already_offered(target: &AuthTarget<'_>, path: &str) -> SshConnectError {
    SshConnectError::Permanent(format!(
        "{} key {path} was already offered",
        target.subject()
    ))
}

/// Authenticates with a loaded, decrypted key, offering each of its
/// [`key_file_identities`] that is not in `offered` yet. Every identity
/// offered is added to `offered`. Returns the first verdict that is not a
/// plain rejection, or the last rejection.
pub(super) async fn sign_in_with_key<H>(
    session: &mut client::Handle<H>,
    target: &AuthTarget<'_>,
    path: &str,
    key: PrivateKey,
    certificate: Option<Certificate>,
    offered: &mut Vec<AgentIdentity>,
) -> Result<AuthResult, SshConnectError>
where
    H: client::Handler,
{
    let identities = key_file_identities(path, key.public_key(), certificate)
        .into_iter()
        .filter(|identity| !offered_before(offered, identity))
        .collect::<Vec<_>>();

    // RSA is signed by `ring` (see `rsa_signer`); every other key by `russh`.
    if let KeypairData::Rsa(rsa_keypair) = key.key_data() {
        let mut signer = RingRsaSigner::new(rsa_keypair).map_err(|reason| {
            SshConnectError::Permanent(format!("{} key {path}: {reason}", target.subject()))
        })?;
        return sign_in_with_rsa(session, target, &mut signer, path, identities, offered).await;
    }

    let key = Arc::new(key);
    let mut verdict = None;
    for identity in identities {
        offered.push(identity.clone());
        let result = match identity {
            AgentIdentity::Certificate { certificate, .. } => {
                exchange(
                    target,
                    "certificate authentication",
                    session.authenticate_openssh_cert(target.username, key.clone(), certificate),
                )
                .await?
            }
            AgentIdentity::PublicKey { .. } => {
                exchange(
                    target,
                    "key authentication",
                    session.authenticate_publickey(
                        target.username,
                        PrivateKeyWithHashAlg::new(key.clone(), None),
                    ),
                )
                .await?
            }
        };
        if !offer_next(&result) {
            return Ok(result);
        }
        verdict = Some(result);
    }
    verdict.ok_or_else(|| already_offered(target, path))
}

/// [`sign_in_with_key`] for an RSA key, whose `identities` are signed by
/// `signer`.
async fn sign_in_with_rsa<H>(
    session: &mut client::Handle<H>,
    target: &AuthTarget<'_>,
    signer: &mut RingRsaSigner,
    path: &str,
    identities: Vec<AgentIdentity>,
    offered: &mut Vec<AgentIdentity>,
) -> Result<AuthResult, SshConnectError>
where
    H: client::Handler,
{
    // Use the best rsa-sha2-* the server advertises. Plain `ssh-rsa` is
    // SHA-1, which OpenSSH 8.8+ rejects and `ring` does not produce.
    let hash_alg = match session.best_supported_rsa_hash().await.unwrap_or(None) {
        Some(Some(hash_alg)) => hash_alg,
        Some(None) => {
            return Err(SshConnectError::Permanent(format!(
                "{} server only accepts SHA-1 signatures for RSA keys, which tTerm \
                 does not produce. Use an Ed25519 or ECDSA key for this host.",
                target.subject()
            )))
        }
        // Not advertised (servers older than OpenSSH 7.2): those that know
        // rsa-sha2 at all accept SHA-256.
        None => HashAlg::Sha256,
    };

    let mut verdict = None;
    for identity in identities {
        offered.push(identity.clone());
        let result = match identity {
            AgentIdentity::Certificate { certificate, .. } => {
                exchange(
                    target,
                    "certificate authentication",
                    session.authenticate_certificate_with(
                        target.username,
                        certificate,
                        Some(hash_alg),
                        signer,
                    ),
                )
                .await?
            }
            AgentIdentity::PublicKey { key, .. } => {
                exchange(
                    target,
                    "key authentication",
                    session.authenticate_publickey_with(
                        target.username,
                        key,
                        Some(hash_alg),
                        signer,
                    ),
                )
                .await?
            }
        };
        if !offer_next(&result) {
            return Ok(result);
        }
        verdict = Some(result);
    }
    verdict.ok_or_else(|| already_offered(target, path))
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
