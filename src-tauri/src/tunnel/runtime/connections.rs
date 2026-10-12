//! Accepting and forwarding a tunnel's connections.

use super::{CHANNEL_OPEN_TIMEOUT, DEST_CONNECT_TIMEOUT, MAX_CONNECTIONS, SOCKS_HANDSHAKE_TIMEOUT};
use crate::ssh::ForwardedTcpIp;
use crate::tunnel::hub::SshHandle;
use crate::tunnel::socks5;
use crate::tunnel::socks5::ConnectOutcome;
use crate::tunnel::types::TunnelStats;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// The sockets serving one bind address. `localhost` and `*` cover both IP
/// families, as OpenSSH does, so a client resolving to either one connects.
pub(super) struct Listeners {
    sockets: Vec<TcpListener>,
    /// Rotates which socket is polled first so none is starved.
    next: AtomicUsize,
}

impl Listeners {
    fn addresses_for(host: &str) -> Option<[IpAddr; 2]> {
        match host {
            "localhost" => Some([Ipv4Addr::LOCALHOST.into(), Ipv6Addr::LOCALHOST.into()]),
            "*" => Some([Ipv4Addr::UNSPECIFIED.into(), Ipv6Addr::UNSPECIFIED.into()]),
            _ => None,
        }
    }

    /// Binds every address for `host`. For the dual-family aliases one family
    /// may be unavailable (IPv6 disabled, or a dual-stack socket already
    /// covering it); that is fine as long as one socket comes up.
    pub(super) async fn bind(host: &str, port: u16) -> std::io::Result<Self> {
        let sockets = match Self::addresses_for(host) {
            Some(addresses) => {
                let mut sockets = Vec::new();
                let mut first_error = None;
                for address in addresses {
                    match TcpListener::bind((address, port)).await {
                        Ok(socket) => sockets.push(socket),
                        Err(err) => {
                            first_error.get_or_insert(err);
                        }
                    }
                }
                if sockets.is_empty() {
                    return Err(first_error
                        .unwrap_or_else(|| std::io::Error::other("no address to listen on")));
                }
                sockets
            }
            None => vec![TcpListener::bind((host, port)).await?],
        };
        Ok(Self {
            sockets,
            next: AtomicUsize::new(0),
        })
    }

    pub(super) fn local_port(&self) -> Option<u16> {
        self.sockets
            .first()
            .and_then(|socket| socket.local_addr().ok())
            .map(|addr| addr.port())
    }

    pub(super) async fn accept(&self) -> std::io::Result<(TcpStream, SocketAddr)> {
        let start = self.next.fetch_add(1, Ordering::Relaxed);
        std::future::poll_fn(|cx| {
            for offset in 0..self.sockets.len() {
                let socket = &self.sockets[(start + offset) % self.sockets.len()];
                if let Poll::Ready(result) = socket.poll_accept(cx) {
                    return Poll::Ready(result);
                }
            }
            Poll::Pending
        })
        .await
    }
}

/// Closes every connection that arrives while there is no SSH session to carry
/// it, so clients fail at once instead of hanging in the accept backlog and
/// being served stale after the tunnel recovers. Never completes.
pub(super) async fn refuse_connections(listeners: Option<&Listeners>) -> std::convert::Infallible {
    let Some(listeners) = listeners else {
        return std::future::pending().await;
    };
    loop {
        // The dropped stream is closed immediately.
        let _ = listeners.accept().await;
    }
}

/// Hands out slots for forwarded connections, refusing past the limit.
pub(super) struct ConnectionLimiter(Arc<Semaphore>);

impl ConnectionLimiter {
    pub(super) fn new(limit: usize) -> Self {
        Self(Arc::new(Semaphore::new(limit)))
    }

    /// A slot that frees itself when the connection task ends, or `None`
    /// (after noting the refusal) when the tunnel is full.
    pub(super) fn acquire(&self, stats: &TunnelStats) -> Option<OwnedSemaphorePermit> {
        let permit = self.0.clone().try_acquire_owned().ok();
        if permit.is_none() {
            stats.record_failure(format!(
                "Too many simultaneous connections (limit {MAX_CONNECTIONS}); refusing new ones"
            ));
        }
        permit
    }
}

/// Counts bytes crossing the local socket of a forwarded connection.
pub(super) struct Counted<S> {
    inner: S,
    stats: Arc<TunnelStats>,
    /// The local socket is the client side (local/dynamic forwards): what it
    /// reads travels up to the destination. For remote forwards it is the
    /// destination side, so the directions swap.
    local_is_client: bool,
}

impl<S> Counted<S> {
    pub(super) fn new(inner: S, stats: Arc<TunnelStats>, local_is_client: bool) -> Self {
        Self {
            inner,
            stats,
            local_is_client,
        }
    }

    fn read_counter(&self) -> &std::sync::atomic::AtomicU64 {
        if self.local_is_client {
            &self.stats.bytes_up
        } else {
            &self.stats.bytes_down
        }
    }

    fn write_counter(&self) -> &std::sync::atomic::AtomicU64 {
        if self.local_is_client {
            &self.stats.bytes_down
        } else {
            &self.stats.bytes_up
        }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for Counted<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        let poll = Pin::new(&mut self.inner).poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = &poll {
            let read = (buf.filled().len() - before) as u64;
            self.read_counter().fetch_add(read, Ordering::Relaxed);
        }
        poll
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Counted<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let poll = Pin::new(&mut self.inner).poll_write(cx, data);
        if let Poll::Ready(Ok(written)) = &poll {
            self.write_counter()
                .fetch_add(*written as u64, Ordering::Relaxed);
        }
        poll
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

/// Keeps `active_connections` correct even when a connection task is aborted.
pub(super) struct ConnectionGuard(Arc<TunnelStats>);

impl ConnectionGuard {
    pub(super) fn new(stats: &Arc<TunnelStats>) -> Self {
        stats.active_connections.fetch_add(1, Ordering::Relaxed);
        stats.total_connections.fetch_add(1, Ordering::Relaxed);
        Self(stats.clone())
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.0.active_connections.fetch_sub(1, Ordering::Relaxed);
    }
}

async fn pipe<R>(local: TcpStream, remote: R, stats: &Arc<TunnelStats>, local_is_client: bool)
where
    R: AsyncRead + AsyncWrite + Unpin,
{
    let _ = local.set_nodelay(true);
    let mut local = Counted::new(local, stats.clone(), local_is_client);
    let mut remote = remote;
    let _ = tokio::io::copy_bidirectional(&mut local, &mut remote).await;
}

/// Why the server could not open a channel to a destination.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum ChannelFailure {
    Prohibited,
    ConnectFailed,
    ResourceShortage,
    TimedOut,
    Other(String),
}

impl ChannelFailure {
    pub(super) fn from_error(err: russh::Error) -> Self {
        match err {
            russh::Error::ChannelOpenFailure(reason) => match reason {
                russh::ChannelOpenFailure::AdministrativelyProhibited => Self::Prohibited,
                russh::ChannelOpenFailure::ConnectFailed => Self::ConnectFailed,
                russh::ChannelOpenFailure::ResourceShortage => Self::ResourceShortage,
                _ => Self::Other("the server rejected the channel".to_string()),
            },
            other => Self::Other(format!("SSH error: {other}")),
        }
    }

    pub(super) fn describe(&self, host: &str, port: u16) -> String {
        let reason = match self {
            Self::Prohibited => "the server does not allow forwarding to this destination",
            Self::ConnectFailed => {
                "the server could not connect (connection refused or host unreachable)"
            }
            Self::ResourceShortage => "the server is out of resources for new channels",
            Self::TimedOut => "timed out waiting for the server to open the connection",
            Self::Other(reason) => reason.as_str(),
        };
        format!("{host}:{port}: {reason}")
    }

    pub(super) fn socks_outcome(&self) -> ConnectOutcome {
        match self {
            Self::Prohibited => ConnectOutcome::NotAllowed,
            Self::ConnectFailed => ConnectOutcome::Refused,
            Self::TimedOut => ConnectOutcome::TimedOut,
            Self::ResourceShortage | Self::Other(_) => ConnectOutcome::Failed,
        }
    }
}

async fn open_direct_channel(
    session: &SshHandle,
    host: &str,
    port: u16,
    peer: SocketAddr,
) -> Result<russh::Channel<russh::client::Msg>, ChannelFailure> {
    match tokio::time::timeout(
        CHANNEL_OPEN_TIMEOUT,
        session.channel_open_direct_tcpip(
            host.to_string(),
            port as u32,
            peer.ip().to_string(),
            peer.port() as u32,
        ),
    )
    .await
    {
        Ok(Ok(channel)) => Ok(channel),
        Ok(Err(err)) => Err(ChannelFailure::from_error(err)),
        Err(_) => Err(ChannelFailure::TimedOut),
    }
}

pub(super) async fn forward_local_connection(
    session: Arc<SshHandle>,
    tcp: TcpStream,
    peer: SocketAddr,
    dest_host: String,
    dest_port: u16,
    stats: Arc<TunnelStats>,
) {
    let _guard = ConnectionGuard::new(&stats);
    match open_direct_channel(&session, &dest_host, dest_port, peer).await {
        Ok(channel) => pipe(tcp, channel.into_stream(), &stats, true).await,
        Err(failure) => stats.record_failure(failure.describe(&dest_host, dest_port)),
    }
}

pub(super) async fn forward_socks_connection(
    session: Arc<SshHandle>,
    mut tcp: TcpStream,
    peer: SocketAddr,
    stats: Arc<TunnelStats>,
) {
    let _guard = ConnectionGuard::new(&stats);
    let request = tokio::time::timeout(
        SOCKS_HANDSHAKE_TIMEOUT,
        socks5::read_connect_request(&mut tcp),
    )
    .await;
    let Ok(Ok((host, port))) = request else {
        return;
    };
    match open_direct_channel(&session, &host, port, peer).await {
        Ok(channel) => {
            if socks5::write_outcome(&mut tcp, ConnectOutcome::Success)
                .await
                .is_ok()
            {
                pipe(tcp, channel.into_stream(), &stats, true).await;
            }
        }
        Err(failure) => {
            stats.record_failure(failure.describe(&host, port));
            let _ = socks5::write_outcome(&mut tcp, failure.socks_outcome()).await;
        }
    }
}

pub(super) async fn forward_remote_connection(
    forwarded: ForwardedTcpIp,
    dest_host: String,
    dest_port: u16,
    stats: Arc<TunnelStats>,
) {
    let _guard = ConnectionGuard::new(&stats);
    let connect = tokio::time::timeout(
        DEST_CONNECT_TIMEOUT,
        TcpStream::connect((dest_host.as_str(), dest_port)),
    )
    .await;
    match connect {
        Ok(Ok(tcp)) => pipe(tcp, forwarded.channel.into_stream(), &stats, false).await,
        failed => {
            let reason = match failed {
                Ok(Err(err)) => err.to_string(),
                _ => "timed out".to_string(),
            };
            stats.record_failure(format!("{dest_host}:{dest_port}: {reason}"));
            let _ = forwarded.channel.close().await;
        }
    }
}
