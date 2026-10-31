//! Opens the channels an SFTP connection works over: its SFTP sessions and
//! the command channels for the work the protocol cannot do itself. Every
//! SFTP feature goes through one [`ChannelLauncher`], so that how a channel
//! starts on the host is decided in one place for the whole connection.

use russh::client::{Handle, Handler};
use russh_sftp::client::rawsession::Limits;
use russh_sftp::client::{RawSftpSession, SftpSession};
use russh_sftp::extensions::LimitsExtension;
use std::sync::Arc;
use std::time::Duration;

use crate::sftp::internal::connection::map_sftp_error;
use crate::sftp::internal::exec::RemoteScript;
use crate::ssh::types::SshClientHandler;

const SFTP_REQUEST_TIMEOUT_SECS: u64 = 120;
/// Bound on opening an SSH channel and negotiating the SFTP subsystem on it.
/// `russh_sftp` only times out its own protocol requests (via
/// `set_timeout`), which only start once a channel exists — the
/// `channel_open_session`/`request_subsystem` calls that precede it are
/// plain `russh` futures with no timeout of their own. A server that accepts
/// a channel but never answers it (e.g. a session limit that is enforced by
/// silence rather than a refusal, or a transport that stalls without
/// resetting the socket) would otherwise hang this forever — freezing a
/// fresh transfer at 0% with no error, since `prepare_transfer` opens a new
/// batch of channels for every upload/download call and waits for all of
/// them before doing any I/O.
const SFTP_CHANNEL_SETUP_TIMEOUT: Duration = Duration::from_secs(30);

const SETUP_TIMED_OUT: &str = "Timed out opening SFTP channel";

/// Starts the channels of one pooled SFTP connection on its host.
#[derive(Clone, Default)]
pub struct ChannelLauncher;

impl ChannelLauncher {
    /// Opens the connection's control session, the one listings, edits and
    /// the other single requests go over.
    pub async fn open_sftp_session(
        &self,
        ssh: &Handle<SshClientHandler>,
    ) -> Result<Arc<SftpSession>, String> {
        tokio::time::timeout(SFTP_CHANNEL_SETUP_TIMEOUT, open_control_session(ssh))
            .await
            .map_err(|_| SETUP_TIMED_OUT.to_string())?
    }

    /// Opens one more SFTP channel on the connection and wraps it as a raw
    /// session for a transfer lane, negotiating `limits@openssh.com` when
    /// the server offers it.
    pub async fn open_raw_session(
        &self,
        ssh: &Handle<SshClientHandler>,
    ) -> Result<(Arc<RawSftpSession>, Option<LimitsExtension>), String> {
        tokio::time::timeout(SFTP_CHANNEL_SETUP_TIMEOUT, open_raw_session(ssh))
            .await
            .map_err(|_| SETUP_TIMED_OUT.to_string())?
    }

    /// Starts `script` on its own command channel, as
    /// [`RemoteScript::start`] describes.
    pub async fn start_script<H: Handler>(
        &self,
        ssh: &Handle<H>,
        script: &str,
    ) -> Result<RemoteScript, String> {
        RemoteScript::start(ssh, script).await
    }
}

async fn open_control_session(ssh: &Handle<SshClientHandler>) -> Result<Arc<SftpSession>, String> {
    let channel = ssh
        .channel_open_session()
        .await
        .map_err(|err| format!("Failed to open SSH channel: {err}"))?;

    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|err| format!("Failed to start SFTP subsystem: {err}"))?;

    let sftp = SftpSession::new(channel.into_stream())
        .await
        .map_err(map_sftp_error)?;
    sftp.set_timeout(SFTP_REQUEST_TIMEOUT_SECS);
    Ok(Arc::new(sftp))
}

async fn open_raw_session(
    ssh: &Handle<SshClientHandler>,
) -> Result<(Arc<RawSftpSession>, Option<LimitsExtension>), String> {
    let channel = ssh
        .channel_open_session()
        .await
        .map_err(|err| format!("Failed to open SFTP channel: {err}"))?;

    channel
        .request_subsystem(true, "sftp")
        .await
        .map_err(|err| format!("Failed to start SFTP subsystem: {err}"))?;

    let mut session = RawSftpSession::new(channel.into_stream());
    let version = session
        .init()
        .await
        .map_err(|err| format!("Failed to initialize SFTP channel: {err}"))?;

    session.set_timeout(SFTP_REQUEST_TIMEOUT_SECS);

    let limits = if version
        .extensions
        .get(russh_sftp::extensions::LIMITS)
        .is_some_and(|value| value == "1")
    {
        let extension = session
            .limits()
            .await
            .map_err(|err| format!("Failed to query server SFTP limits: {err}"))?;
        session.set_limits(Limits {
            packet_len: None,
            read_len: (extension.max_read_len > 0).then_some(extension.max_read_len),
            write_len: (extension.max_write_len > 0).then_some(extension.max_write_len),
            open_handles: None,
        });
        Some(extension)
    } else {
        None
    };

    Ok((Arc::new(session), limits))
}
