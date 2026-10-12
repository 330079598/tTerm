//! [`AuthMethod::Auto`]: what a plain `ssh user@host` would try.

use super::methods::{
    exchange, keyboard_interactive, keyboard_interactive_capturing, password_method,
    prompt_payload, publickey, sign_in_with_key,
};
use super::{
    AuthPromptField, AuthPrompter, AuthTarget, AuthUsed, DEFAULT_IDENTITY_FILES, TYPING_ATTEMPTS,
};
use crate::ssh::agent::offered_before;
use crate::ssh::typed_passwords;
use crate::ssh::types::SshConnectError;
use russh::client::AuthResult;
use russh::keys::agent::AgentIdentity;
use russh::keys::PrivateKey;
use russh::{client, MethodKind};
use std::path::PathBuf;
use zeroize::Zeroizing;

/// [`AuthMethod::Auto`]: what `ssh user@host` tries, in its order. Returns
/// the first verdict that is not a plain rejection, with what produced it
/// and the password the user typed for it, if any.
#[expect(
    clippy::too_many_lines,
    reason = "predates the 150-line limit; split it when it next changes"
)]
pub(super) async fn auto<H>(
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
pub(super) fn identity_files(key_path: Option<&str>, local_identities: bool) -> Vec<PathBuf> {
    let mut files = key_path
        .map(crate::profiles::parser::expand_home_path)
        .into_iter()
        .collect::<Vec<_>>();
    let ssh_dir = crate::profiles::parser::home_dir()
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
    offered: &mut Vec<AgentIdentity>,
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

    let identity = AgentIdentity::from(key.public_key().clone());
    if offered_before(offered, &identity) {
        return Ok(None);
    }
    offered.push(identity);

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
