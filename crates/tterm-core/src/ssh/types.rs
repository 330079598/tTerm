use crate::events::ssh::AuthUsedPayload;
use crate::events::ssh::SshConnectionProgressPayload;
use crate::events::ssh::SshHostKeyPromptPayload;
use crate::ssh::context::ConnectionContext;

use crate::ssh::openssh_known_hosts::verify::KnownHostVerdict;
use crate::ssh::openssh_known_hosts::{check_presented_key, listed_record};
use std::sync::{Arc, Mutex};

pub const HOST_KEY_PROMPT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
pub const HOST_KEY_REJECTED_REASON: &str = "SSH host fingerprint rejected by user";

/// Classified failure while establishing an SSH session. Callers use
/// [`SshConnectError::is_retryable`] to decide whether an automatic reconnect
/// is worthwhile: rejected credentials must not be retried (repeated attempts
/// risk a server-side lockout), while transient network failures may recover.
#[derive(Debug, Clone)]
pub enum SshConnectError {
    /// The user refused the host key (or a changed known host was rejected).
    HostKeyRejected,
    /// The connection operation ended before the user answered.
    Cancelled(String),
    /// A permanent failure — rejected credentials, an unusable key, or a local
    /// known_hosts store error. Retrying cannot succeed.
    Permanent(String),
    /// A transient network, handshake, or tunnel failure. A retry may succeed.
    Network(String),
}

impl SshConnectError {
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Network(_))
    }

    pub fn is_host_key_rejected(&self) -> bool {
        matches!(self, Self::HostKeyRejected)
    }

    /// The user-facing message, always non-empty.
    pub fn message(&self) -> &str {
        match self {
            Self::HostKeyRejected => HOST_KEY_REJECTED_REASON,
            Self::Cancelled(message) | Self::Permanent(message) | Self::Network(message) => message,
        }
    }

    /// Prefix the message while preserving the retry classification.
    pub fn with_prefix(self, prefix: &str) -> Self {
        match self {
            Self::HostKeyRejected => Self::HostKeyRejected,
            Self::Cancelled(message) => Self::Cancelled(format!("{prefix}{message}")),
            Self::Permanent(message) => Self::Permanent(format!("{prefix}{message}")),
            Self::Network(message) => Self::Network(format!("{prefix}{message}")),
        }
    }
}

impl std::fmt::Display for SshConnectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message())
    }
}

impl std::error::Error for SshConnectError {}

impl From<SshConnectError> for String {
    fn from(error: SshConnectError) -> Self {
        error.to_string()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostKeyVerificationMode {
    PromptAndPersist,
    TrustUnknownForSession,
}

#[derive(Debug, Eq, PartialEq)]
enum HostKeyCheckOutcome {
    TrustedKnown,
    TrustUnknownForSession,
    PromptUser,
    RejectChangedKnownHost,
}

fn resolve_host_key_check(
    mode: HostKeyVerificationMode,
    known: Option<&crate::ssh::store::KnownHostRecord>,
    host: &str,
    port: u16,
    fingerprint: &str,
) -> HostKeyCheckOutcome {
    if let Some(record) = known
        && record.host == host
        && record.port == port
        && record.fingerprint == fingerprint
    {
        return HostKeyCheckOutcome::TrustedKnown;
    }

    match (mode, known.is_some()) {
        (HostKeyVerificationMode::TrustUnknownForSession, false) => {
            HostKeyCheckOutcome::TrustUnknownForSession
        }
        (HostKeyVerificationMode::TrustUnknownForSession, true) => {
            HostKeyCheckOutcome::RejectChangedKnownHost
        }
        (HostKeyVerificationMode::PromptAndPersist, _) => HostKeyCheckOutcome::PromptUser,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ConnectionStatusOptions {
    pub emit_terminal_output: bool,
    pub emit_progress_events: bool,
}

impl ConnectionStatusOptions {
    pub const VERBOSE: Self = Self {
        emit_terminal_output: true,
        emit_progress_events: true,
    };

    pub const SILENT: Self = Self {
        emit_terminal_output: false,
        emit_progress_events: true,
    };

    pub const QUIET: Self = Self {
        emit_terminal_output: false,
        emit_progress_events: false,
    };
}

/// Attaches the authentication result without coupling the shared payload to SSH services.
pub fn with_auth_used(
    mut payload: SshConnectionProgressPayload,
    used: &super::auth::AuthUsed,
) -> SshConnectionProgressPayload {
    payload.auth = Some(AuthUsedPayload {
        method: used.method_name(),
        private_key_path: used.key_path().map(str::to_string),
        reusable: used.reusable(),
    });
    payload
}

pub fn emit_connection_progress(
    events: &dyn crate::events::EventSink,
    tab_id: &str,
    status_options: ConnectionStatusOptions,
    payload: SshConnectionProgressPayload,
) {
    if !status_options.emit_progress_events {
        return;
    }

    let _ = events.emit(crate::events::AppEvent::Ssh(
        crate::events::ssh::SshEvent::ConnectionProgress {
            tab_id: tab_id.to_string(),
            payload,
        },
    ));
}

/// A connection the server opened back to us for a remote (`-R`) port forward.
pub struct ForwardedTcpIp {
    pub channel: russh::Channel<russh::client::Msg>,
    /// The server-side port the connection arrived on; it identifies which
    /// remote forward the connection belongs to.
    pub connected_port: u16,
}

#[derive(Clone)]
pub struct SshClientHandler {
    pub app: ConnectionContext,
    pub tab_id: String,
    pub profile_id: Option<String>,
    pub profile_name: String,
    pub host: String,
    pub port: u16,
    pub prompts: std::sync::Arc<crate::prompts::PromptBroker>,
    pub prompt_owner: crate::prompts::PromptOwner,
    pub host_key_failure: Arc<Mutex<Option<SshConnectError>>>,
    pub status_options: ConnectionStatusOptions,
    pub host_key_verification_mode: HostKeyVerificationMode,
    /// Receives channels the server opens for remote port forwards. `None`
    /// for sessions that never request one; such channels are dropped.
    pub forwarded_tcpip_tx: Option<tokio::sync::mpsc::UnboundedSender<ForwardedTcpIp>>,
    /// Whether this session requested SSH-agent forwarding (`agent_forward`
    /// on the resolved plan, independent of the auth method). Gates
    /// [`server_channel_open_agent_forward`](russh::client::Handler::server_channel_open_agent_forward):
    /// a server could open an `auth-agent@openssh.com` channel even when we
    /// never asked for one, and bridging it unconditionally would let an
    /// untrusted host sign challenges through the local agent it was never
    /// granted access to.
    pub agent_forward_enabled: bool,
}

impl russh::client::Handler for SshClientHandler {
    type Error = russh::Error;

    async fn server_channel_open_agent_forward(
        &mut self,
        channel: russh::Channel<russh::client::Msg>,
        open: russh::client::ChannelOpenHandle,
        _session: &mut russh::client::Session,
    ) -> Result<(), Self::Error> {
        if !self.agent_forward_enabled {
            open.reject(russh::ChannelOpenFailure::AdministrativelyProhibited)
                .await;
            return Ok(());
        }

        // Since russh 0.60 an unanswered open is rejected on drop, so it must
        // be confirmed explicitly before the channel is bridged.
        open.accept().await;
        tokio::spawn(async move {
            let mut agent_stream = match crate::ssh::agent::connect_local_agent_socket().await {
                Ok(stream) => stream,
                Err(_) => {
                    let _ = channel.close().await;
                    return;
                }
            };
            let mut channel_stream = channel.into_stream();
            let _ = tokio::io::copy_bidirectional(&mut channel_stream, &mut agent_stream).await;
        });

        Ok(())
    }

    async fn server_channel_open_forwarded_tcpip(
        &mut self,
        channel: russh::Channel<russh::client::Msg>,
        _connected_address: &str,
        connected_port: u32,
        _originator_address: &str,
        _originator_port: u32,
        open: russh::client::ChannelOpenHandle,
        _session: &mut russh::client::Session,
    ) -> Result<(), Self::Error> {
        let Some(tx) = &self.forwarded_tcpip_tx else {
            open.reject(russh::ChannelOpenFailure::AdministrativelyProhibited)
                .await;
            return Ok(());
        };
        open.accept().await;
        let _ = tx.send(ForwardedTcpIp {
            channel,
            connected_port: u16::try_from(connected_port).unwrap_or(0),
        });
        Ok(())
    }

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        use russh::keys::ssh_key::HashAlg;

        // What `ssh` trusts or refuses decides first; a host certificate is
        // only asked for when `known_hosts` trusts an authority for the host.
        let (server_public_key, listed_fingerprint) = match check_presented_key(
            &self.host,
            self.port,
            Some(&self.profile_name),
            server_public_key,
        ) {
            KnownHostVerdict::Trusted => return Ok(true),
            KnownHostVerdict::Rejected(reason) => {
                self.emit_status("31", &reason);
                return Ok(false);
            }
            KnownHostVerdict::Undecided {
                key,
                listed_fingerprint,
            } => (key, listed_fingerprint),
        };

        let algorithm = server_public_key.algorithm().to_string();
        let fingerprint = server_public_key.fingerprint(HashAlg::Sha256).to_string();

        emit_connection_progress(
            self.app.events.as_ref(),
            &self.tab_id,
            self.status_options,
            SshConnectionProgressPayload::new(
                "target_host_key_checking",
                format!(
                    "Checking target host fingerprint for {}:{}",
                    self.host, self.port
                ),
            )
            .host(self.host.clone(), self.port),
        );

        let known = match crate::ssh::store::load_known_host(
            &self.profile_name,
            self.profile_id.as_deref(),
            &self.host,
            self.port,
        ) {
            Ok(value) => value,
            Err(err) => {
                self.emit_status("31", &format!("Failed to read known host store: {err}"));
                return Ok(false);
            }
        };
        let known = known.or_else(|| {
            listed_fingerprint
                .map(|listed| listed_record(&self.host, self.port, &algorithm, listed))
        });

        match resolve_host_key_check(
            self.host_key_verification_mode,
            known.as_ref(),
            &self.host,
            self.port,
            &fingerprint,
        ) {
            HostKeyCheckOutcome::TrustedKnown => return Ok(true),
            HostKeyCheckOutcome::TrustUnknownForSession => {
                emit_connection_progress(
                    self.app.events.as_ref(),
                    &self.tab_id,
                    self.status_options,
                    SshConnectionProgressPayload::new(
                        "target_host_key_trusted_for_test",
                        format!(
                            "Temporarily trusting target host fingerprint for {}:{}",
                            self.host, self.port
                        ),
                    )
                    .host(self.host.clone(), self.port),
                );
                return Ok(true);
            }
            HostKeyCheckOutcome::RejectChangedKnownHost => {
                self.emit_status(
                    "31",
                    "SSH host fingerprint changed; connect normally to review the new fingerprint.",
                );
                return Ok(false);
            }
            HostKeyCheckOutcome::PromptUser => {}
        }

        let reason = if known.is_some() {
            "mismatch".to_string()
        } else {
            "unknown".to_string()
        };
        let known_fingerprint = known.as_ref().map(|r| r.fingerprint.clone());

        let request_id = uuid::Uuid::new_v4().to_string();
        let payload = SshHostKeyPromptPayload {
            request_id: request_id.clone(),
            profile_name: self.profile_name.clone(),
            host: self.host.clone(),
            port: self.port,
            algorithm,
            fingerprint: fingerprint.clone(),
            reason,
            known_fingerprint,
        };

        let wait =
            crate::prompt_scope::host_key(&self.prompts, &self.prompt_owner, &self.tab_id, payload);
        crate::prompt_scope::reveal(self.app.window.as_ref());

        self.emit_status(
            "33",
            "Waiting for user confirmation of SSH host fingerprint...",
        );

        if let Err(error) = crate::ssh::host_key_prompt::resolve(wait, &self.host_key_failure).await
        {
            self.emit_status("31", error.message());
            return Err(russh::Error::Disconnect);
        }

        let save_result =
            crate::ssh::store::save_known_host_entry(crate::ssh::store::KnownHostRecord {
                profile_id: self.profile_id.clone(),
                profile_name: self.profile_name.clone(),
                host: self.host.clone(),
                port: self.port,
                algorithm: server_public_key.algorithm().to_string(),
                fingerprint,
                trusted_at: crate::ssh::store::now_unix_ms(),
            });

        if let Err(err) = save_result {
            self.emit_status("31", &format!("Failed to save known host: {err}"));
            return Ok(false);
        }

        Ok(true)
    }
}

impl SshClientHandler {
    fn emit_status(&self, color: &str, message: &str) {
        if !self.status_options.emit_terminal_output {
            return;
        }

        let payload = crate::terminal::legacy_events::status_line(color, message);
        crate::session_log::record_output(&self.app.log, &self.tab_id, payload.as_bytes());
        crate::terminal::legacy_events::emit_output(
            self.app.events.as_ref(),
            &self.tab_id,
            payload,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HOST_KEY_REJECTED_REASON, HostKeyCheckOutcome, HostKeyVerificationMode, SshConnectError,
        resolve_host_key_check,
    };

    fn known_host(fingerprint: &str) -> crate::ssh::store::KnownHostRecord {
        crate::ssh::store::KnownHostRecord {
            profile_id: Some("profile-1".to_string()),
            profile_name: "demo".to_string(),
            host: "example.com".to_string(),
            port: 22,
            algorithm: "ssh-ed25519".to_string(),
            fingerprint: fingerprint.to_string(),
            trusted_at: 1,
        }
    }

    #[test]
    fn test_mode_trusts_unknown_host_key_for_session() {
        assert_eq!(
            resolve_host_key_check(
                HostKeyVerificationMode::TrustUnknownForSession,
                None,
                "example.com",
                22,
                "SHA256:new"
            ),
            HostKeyCheckOutcome::TrustUnknownForSession
        );
    }

    #[test]
    fn test_mode_rejects_changed_known_host_key() {
        let known = known_host("SHA256:old");

        assert_eq!(
            resolve_host_key_check(
                HostKeyVerificationMode::TrustUnknownForSession,
                Some(&known),
                "example.com",
                22,
                "SHA256:new"
            ),
            HostKeyCheckOutcome::RejectChangedKnownHost
        );
    }

    #[test]
    fn normal_mode_prompts_for_unknown_host_key() {
        assert_eq!(
            resolve_host_key_check(
                HostKeyVerificationMode::PromptAndPersist,
                None,
                "example.com",
                22,
                "SHA256:new"
            ),
            HostKeyCheckOutcome::PromptUser
        );
    }

    #[test]
    fn connect_error_classifies_retryability() {
        assert!(SshConnectError::Network("timeout".to_string()).is_retryable());
        assert!(!SshConnectError::Permanent("bad password".to_string()).is_retryable());
        assert!(!SshConnectError::HostKeyRejected.is_retryable());

        assert_eq!(
            SshConnectError::HostKeyRejected.to_string(),
            HOST_KEY_REJECTED_REASON
        );
        assert_eq!(
            SshConnectError::Network("boom".to_string())
                .with_prefix("Jump host #1: ")
                .to_string(),
            "Jump host #1: boom"
        );
        // Prefixing must not change the retry classification.
        assert!(
            SshConnectError::HostKeyRejected
                .with_prefix("ignored: ")
                .is_host_key_rejected()
        );
    }
}

#[cfg(test)]
#[path = "types_legacy_tests.rs"]
mod legacy_contract_tests;
