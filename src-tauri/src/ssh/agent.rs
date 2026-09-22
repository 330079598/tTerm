//! SSH-Agent authentication: sign publickey auth challenges through the
//! user's local `ssh-agent` (Unix socket / Windows OpenSSH named pipe /
//! Pageant) instead of reading a private key file directly. This also picks
//! up hardware-backed keys (YubiKey, FIDO2/`sk-*`, PIV) transparently,
//! since the agent process handles PIN/touch prompts itself.

use russh::client;
use russh::keys::agent::client::AgentClient;

use crate::ssh::types::SshConnectError;

#[cfg(unix)]
async fn connect_local_agent() -> Result<AgentClient<tokio::net::UnixStream>, SshConnectError> {
    AgentClient::connect_env().await.map_err(|e| {
        SshConnectError::Permanent(format!(
            "Could not connect to the local SSH agent (is ssh-agent running and SSH_AUTH_SOCK set?): {e}"
        ))
    })
}

#[cfg(windows)]
async fn connect_local_agent(
) -> Result<AgentClient<tokio::net::windows::named_pipe::NamedPipeClient>, SshConnectError> {
    // The OpenSSH agent service (Windows 10+) is what most hardware-key
    // tooling (e.g. YubiKey minidriver, Windows Hello) targets.
    AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent")
        .await
        .map_err(|e| {
            SshConnectError::Permanent(format!(
                "Could not connect to the local SSH agent (checked the OpenSSH agent service at \\\\.\\pipe\\openssh-ssh-agent): {e}"
            ))
        })
}

/// Open a raw duplex stream to the local agent socket, for bridging a
/// server-opened `auth-agent@openssh.com` forwarding channel. Unlike
/// [`connect_local_agent`], this isn't wrapped in the structured agent
/// protocol client: the bytes are just piped through unmodified, since the
/// remote side (not us) speaks the agent protocol over this connection.
#[cfg(unix)]
pub async fn connect_local_agent_socket() -> Result<tokio::net::UnixStream, SshConnectError> {
    let path = std::env::var_os("SSH_AUTH_SOCK").ok_or_else(|| {
        SshConnectError::Permanent(
            "Could not forward the SSH agent: SSH_AUTH_SOCK is not set".to_string(),
        )
    })?;
    tokio::net::UnixStream::connect(&path).await.map_err(|e| {
        SshConnectError::Permanent(format!(
            "Could not connect to the local SSH agent at {}: {e}",
            path.to_string_lossy()
        ))
    })
}

#[cfg(windows)]
pub async fn connect_local_agent_socket(
) -> Result<tokio::net::windows::named_pipe::NamedPipeClient, SshConnectError> {
    const ERROR_PIPE_BUSY: i32 = 231;
    const PIPE_PATH: &str = r"\\.\pipe\openssh-ssh-agent";

    loop {
        match tokio::net::windows::named_pipe::ClientOptions::new().open(PIPE_PATH) {
            Ok(client) => return Ok(client),
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) => {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            Err(e) => {
                return Err(SshConnectError::Permanent(format!(
                    "Could not connect to the local SSH agent (checked the OpenSSH agent service at {PIPE_PATH}): {e}"
                )))
            }
        }
    }
}

/// Authenticate `session` as `username` by trying every identity held by the
/// local SSH agent, in order, until one is accepted.
///
/// Signing can involve a hardware touch/PIN prompt handled by the agent
/// itself, so each attempt gets a generous timeout rather than the shorter
/// budget used for password/key-file auth.
pub async fn authenticate_via_agent<H>(
    session: &mut client::Handle<H>,
    username: &str,
) -> Result<(), SshConnectError>
where
    H: client::Handler,
{
    const SIGN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

    let mut agent = connect_local_agent().await?;
    let identities = agent.request_identities().await.map_err(|e| {
        SshConnectError::Permanent(format!("Failed to list SSH agent identities: {e}"))
    })?;

    if identities.is_empty() {
        return Err(SshConnectError::Permanent(
            "The SSH agent has no loaded identities (run `ssh-add` first)".to_string(),
        ));
    }

    // RSA identities default to the legacy SHA-1 `ssh-rsa` signature algorithm
    // unless told otherwise, and most modern servers (OpenSSH 8.8+) reject
    // that. Negotiate the best RFC 8332 rsa-sha2-* algorithm the server
    // advertises up front so RSA keys aren't silently rejected.
    let rsa_hash_alg = session.best_supported_rsa_hash().await.unwrap_or(None).flatten();

    for identity in identities {
        let public_key = identity.public_key().into_owned();
        let hash_alg = match public_key.algorithm() {
            russh::keys::Algorithm::Rsa { .. } => rsa_hash_alg,
            _ => None,
        };
        let attempt = tokio::time::timeout(
            SIGN_TIMEOUT,
            session.authenticate_publickey_with(username, public_key, hash_alg, &mut agent),
        )
        .await;

        match attempt {
            Ok(Ok(result)) if result.success() => return Ok(()),
            _ => continue,
        }
    }

    Err(SshConnectError::Permanent(
        "None of the SSH agent's identities were accepted by the server".to_string(),
    ))
}
