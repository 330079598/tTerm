//! Opens the stream an SSH session starts on through a proxy: a TCP stream
//! through a proxy server, or the input and output of a proxy command.

use super::command::{self, CommandStream, CommandTarget};
use super::{http_connect, socks5, ProxyEndpoint, ProxyKind};
use crate::ssh::types::SshConnectError;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tauri::AppHandle;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;

/// How long reaching the proxy and its handshake may take together.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);

/// Why a proxy could not open the tunnel, before it is worded for the user.
#[derive(Debug)]
pub(super) enum ProxyError {
    /// The proxy itself could not be reached.
    Unreachable(std::io::Error),
    /// The connection to the proxy failed during the handshake.
    Io(std::io::Error),
    /// The proxy answered in a way its protocol does not allow, which usually
    /// means it speaks another protocol than the one configured.
    Protocol(String),
    /// The proxy wants a user name and password and none is set.
    AuthRequired,
    /// The proxy refused the user name or password.
    AuthRejected,
    /// The proxy could not or would not connect to the target.
    Refused { reason: String, retryable: bool },
    /// The request cannot be sent, e.g. a user name too long for SOCKS5.
    Invalid(String),
}

impl From<std::io::Error> for ProxyError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl ProxyError {
    fn into_connect_error(self, proxy: &ProxyEndpoint, host: &str, port: u16) -> SshConnectError {
        let name = proxy.describe();
        match self {
            Self::Unreachable(error) => SshConnectError::Network(format!(
                "Could not reach the {name}: {error}. Check its address and that it is running."
            )),
            Self::Io(error) => SshConnectError::Network(format!(
                "The {name} closed the connection during its handshake: {error}"
            )),
            Self::Protocol(detail) => SshConnectError::Permanent(format!(
                "The {name} answered unexpectedly ({detail}). Check that it is a {} proxy.",
                proxy.kind.label()
            )),
            Self::AuthRequired => SshConnectError::Permanent(format!(
                "The {name} requires a user name and password. Set them in Settings > Proxies."
            )),
            Self::AuthRejected => SshConnectError::Permanent(format!(
                "The {name} rejected the user name or password. Check them in Settings > Proxies."
            )),
            Self::Refused { reason, retryable } => {
                let message = format!("The {name} could not connect to {host}:{port}: {reason}");
                if retryable {
                    SshConnectError::Network(message)
                } else {
                    SshConnectError::Permanent(message)
                }
            }
            Self::Invalid(detail) => {
                SshConnectError::Permanent(format!("Cannot use the {name}: {detail}"))
            }
        }
    }
}

/// Connects to the proxy and asks it for a tunnel to `host:port`.
async fn open_tunnel(
    proxy: &ProxyEndpoint,
    host: &str,
    port: u16,
) -> Result<TcpStream, ProxyError> {
    let mut stream = TcpStream::connect((proxy.host.as_str(), proxy.port))
        .await
        .map_err(ProxyError::Unreachable)?;
    // SSH sends small packets that should not wait for more; `russh` sets
    // the same on the sockets it opens itself.
    let _ = stream.set_nodelay(true);
    match proxy.kind {
        ProxyKind::Http => http_connect::handshake(&mut stream, proxy, host, port).await?,
        ProxyKind::Socks5 => socks5::handshake(&mut stream, proxy, host, port).await?,
        ProxyKind::Command => {
            return Err(ProxyError::Invalid(
                "a proxy command is not a server to dial".to_string(),
            ))
        }
    }
    Ok(stream)
}

pub(super) async fn connect_with_timeout(
    proxy: &ProxyEndpoint,
    host: &str,
    port: u16,
    timeout: Duration,
) -> Result<TcpStream, SshConnectError> {
    match tokio::time::timeout(timeout, open_tunnel(proxy, host, port)).await {
        Ok(Ok(stream)) => Ok(stream),
        Ok(Err(error)) => Err(error.into_connect_error(proxy, host, port)),
        Err(_) => Err(SshConnectError::Network(format!(
            "The {} did not answer within {} seconds.",
            proxy.describe(),
            timeout.as_secs()
        ))),
    }
}

/// A TCP stream to `host:port` through `proxy`, ready for the SSH handshake.
pub async fn connect_through_proxy(
    proxy: &ProxyEndpoint,
    host: &str,
    port: u16,
) -> Result<TcpStream, SshConnectError> {
    connect_with_timeout(proxy, host, port, HANDSHAKE_TIMEOUT).await
}

/// The stream to the first host that a proxy opened.
pub enum ProxyStream {
    Tcp(TcpStream),
    Command(CommandStream),
}

/// Opens the stream to `target` through `proxy`. A proxy command this device
/// has not allowed yet is shown to the user first.
pub async fn open_stream(
    app: &AppHandle,
    proxy: &ProxyEndpoint,
    target: &CommandTarget<'_>,
) -> Result<ProxyStream, SshConnectError> {
    let Some(template) = proxy.command.as_deref() else {
        return connect_through_proxy(proxy, target.host, target.port)
            .await
            .map(ProxyStream::Tcp);
    };
    let command_line = command::expand(template, target).map_err(SshConnectError::Permanent)?;
    super::approval::ensure_allowed(app, &proxy.name, template).await?;
    command::spawn(&command_line)
        .await
        .map(ProxyStream::Command)
}

impl AsyncRead for ProxyStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Tcp(stream) => Pin::new(stream).poll_read(cx, buf),
            Self::Command(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for ProxyStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Self::Tcp(stream) => Pin::new(stream).poll_write(cx, data),
            Self::Command(stream) => Pin::new(stream).poll_write(cx, data),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Tcp(stream) => Pin::new(stream).poll_flush(cx),
            Self::Command(stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Tcp(stream) => Pin::new(stream).poll_shutdown(cx),
            Self::Command(stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }
}
