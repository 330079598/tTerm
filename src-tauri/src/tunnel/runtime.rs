use super::socks5::{self, ConnectOutcome};
use super::types::{TunnelKind, TunnelRule, TunnelState, TunnelStats, TunnelStatus};
use crate::core::session::SessionPlan;
use crate::core::state::HostPromptMap;
use crate::ssh::{
    open_target_ssh_session_with_forwarding, ConnectionStatusOptions, ForwardedTcpIp,
    HostKeyVerificationMode, SshClientHandler,
};
use russh::client::Handle;
use russh::Disconnect;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;

/// Event tab id shared by every tunnel; host-key prompts arrive on
/// `ssh-hostkey-prompt-tunnels`.
pub const TUNNEL_EVENT_TAB_ID: &str = "tunnels";
pub const STATUS_EVENT: &str = "tunnel-status";

const CONNECT_TIMEOUT: Duration = Duration::from_secs(45);
const CHANNEL_OPEN_TIMEOUT: Duration = Duration::from_secs(15);
const DEST_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const SOCKS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const HEALTH_TICK: Duration = Duration::from_secs(1);
const MAX_BACKOFF_SECS: u64 = 30;
/// Tunnels sit idle for long stretches, so a silent drop (NAT timeout, sleeping
/// laptop) must be noticed quickly whatever the profile's terminal keepalive
/// says: at most 3 missed probes 30 seconds apart.
const KEEPALIVE_INTERVAL_CEILING_SECS: u16 = 30;
const KEEPALIVE_COUNT_CEILING: u16 = 3;
/// A session that lasted this long counts as healthy and resets the backoff.
const STABLE_SESSION: Duration = Duration::from_secs(30);

type SshHandle = Handle<SshClientHandler>;
/// active, total, failed, bytes up, bytes down.
type Counters = (u64, u64, u64, u64, u64);

struct StatusInner {
    state: TunnelState,
    message: Option<String>,
    bound_port: Option<u16>,
    connected_at: Option<u64>,
    retry_attempt: u32,
    last_emitted_counters: Counters,
}

/// Holds a tunnel's live state and pushes changes to the UI.
pub struct TunnelReporter {
    app: AppHandle,
    id: String,
    pub stats: Arc<TunnelStats>,
    inner: Mutex<StatusInner>,
}

impl TunnelReporter {
    pub fn new(app: AppHandle, id: String) -> Arc<Self> {
        Arc::new(Self {
            app,
            id,
            stats: Arc::new(TunnelStats::default()),
            inner: Mutex::new(StatusInner {
                state: TunnelState::Stopped,
                message: None,
                bound_port: None,
                connected_at: None,
                retry_attempt: 0,
                last_emitted_counters: (0, 0, 0, 0, 0),
            }),
        })
    }

    fn counters(&self) -> Counters {
        (
            self.stats.active_connections.load(Ordering::Relaxed),
            self.stats.total_connections.load(Ordering::Relaxed),
            self.stats.failed_connections.load(Ordering::Relaxed),
            self.stats.bytes_up.load(Ordering::Relaxed),
            self.stats.bytes_down.load(Ordering::Relaxed),
        )
    }

    pub fn snapshot(&self) -> TunnelStatus {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let (active, total, failed, up, down) = self.counters();
        TunnelStatus {
            id: self.id.clone(),
            state: inner.state,
            message: inner.message.clone(),
            bound_port: inner.bound_port,
            active_connections: active,
            total_connections: total,
            failed_connections: failed,
            bytes_up: up,
            bytes_down: down,
            connected_at: inner.connected_at,
            retry_attempt: inner.retry_attempt,
            last_failure: self.stats.last_failure(),
        }
    }

    fn emit(&self) {
        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.last_emitted_counters = self.counters();
        }
        let _ = self.app.emit(STATUS_EVENT, self.snapshot());
    }

    pub fn set_state(&self, state: TunnelState, message: Option<String>) {
        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.state = state;
            inner.message = message;
            if state != TunnelState::Running {
                inner.connected_at = None;
            }
            if matches!(state, TunnelState::Stopped | TunnelState::Starting) {
                inner.retry_attempt = 0;
            }
            if matches!(state, TunnelState::Stopped | TunnelState::Error) {
                inner.bound_port = None;
            }
        }
        if state == TunnelState::Starting {
            self.stats.reset();
        }
        self.emit();
    }

    fn set_retry_attempt(&self, attempt: u32) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retry_attempt = attempt;
    }

    fn set_bound_port(&self, port: Option<u16>) {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .bound_port = port;
    }

    fn set_running(&self) {
        {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.state = TunnelState::Running;
            inner.message = None;
            inner.connected_at = Some(crate::ssh::now_unix_ms().max(0) as u64);
        }
        self.emit();
    }

    /// Emits a status update only when traffic counters moved.
    fn tick(&self) {
        let changed = {
            let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            inner.last_emitted_counters != self.counters()
        };
        if changed {
            self.emit();
        }
    }
}

/// The sockets serving one bind address. `localhost` and `*` cover both IP
/// families, as OpenSSH does, so a client resolving to either one connects.
struct Listeners {
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
    async fn bind(host: &str, port: u16) -> std::io::Result<Self> {
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

    fn local_port(&self) -> Option<u16> {
        self.sockets
            .first()
            .and_then(|socket| socket.local_addr().ok())
            .map(|addr| addr.port())
    }

    async fn accept(&self) -> std::io::Result<(TcpStream, SocketAddr)> {
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
async fn refuse_connections(listeners: Option<&Listeners>) -> std::convert::Infallible {
    let Some(listeners) = listeners else {
        return std::future::pending().await;
    };
    loop {
        // The dropped stream is closed immediately.
        let _ = listeners.accept().await;
    }
}

/// Counts bytes crossing the local socket of a forwarded connection.
struct Counted<S> {
    inner: S,
    stats: Arc<TunnelStats>,
    /// The local socket is the client side (local/dynamic forwards): what it
    /// reads travels up to the destination. For remote forwards it is the
    /// destination side, so the directions swap.
    local_is_client: bool,
}

impl<S> Counted<S> {
    fn new(inner: S, stats: Arc<TunnelStats>, local_is_client: bool) -> Self {
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
struct ConnectionGuard(Arc<TunnelStats>);

impl ConnectionGuard {
    fn new(stats: &Arc<TunnelStats>) -> Self {
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
enum ChannelFailure {
    Prohibited,
    ConnectFailed,
    ResourceShortage,
    TimedOut,
    Other(String),
}

impl ChannelFailure {
    fn from_error(err: russh::Error) -> Self {
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

    fn describe(&self, host: &str, port: u16) -> String {
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

    fn socks_outcome(&self) -> ConnectOutcome {
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

async fn forward_local_connection(
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

async fn forward_socks_connection(
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

async fn forward_remote_connection(
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

/// Resolves once a stop is requested (or the controlling side is gone).
async fn wait_for_stop(stop_rx: &mut watch::Receiver<bool>) {
    loop {
        if *stop_rx.borrow() {
            return;
        }
        if stop_rx.changed().await.is_err() {
            return;
        }
    }
}

enum SessionEnd {
    Stopped,
    /// The SSH connection dropped; reconnecting may help.
    Lost(String),
    /// Retrying cannot help (e.g. the server denied forwarding).
    Fatal(String),
}

async fn run_listener_session(
    session: Arc<SshHandle>,
    listener: &Listeners,
    rule: &TunnelRule,
    reporter: &TunnelReporter,
    stop_rx: &mut watch::Receiver<bool>,
) -> SessionEnd {
    let mut connections: JoinSet<()> = JoinSet::new();
    let mut tick = tokio::time::interval(HEALTH_TICK);
    loop {
        tokio::select! {
            _ = wait_for_stop(stop_rx) => return SessionEnd::Stopped,
            _ = tick.tick() => {
                if session.is_closed() {
                    return SessionEnd::Lost("SSH connection closed".to_string());
                }
                reporter.tick();
            }
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
            accepted = listener.accept() => {
                let Ok((tcp, peer)) = accepted else { continue };
                let session = session.clone();
                let stats = reporter.stats.clone();
                if rule.kind == TunnelKind::Dynamic {
                    connections.spawn(forward_socks_connection(session, tcp, peer, stats));
                } else {
                    connections.spawn(forward_local_connection(
                        session,
                        tcp,
                        peer,
                        rule.dest_host.clone(),
                        rule.dest_port,
                        stats,
                    ));
                }
            }
        }
    }
}

/// The SSH protocol spells "all interfaces" as an empty address; `*` is the
/// friendlier form users know from OpenSSH's `-R`.
fn remote_bind_address(host: &str) -> String {
    if host == "*" {
        String::new()
    } else {
        host.to_string()
    }
}

async fn run_remote_session(
    session: Arc<SshHandle>,
    forwarded_rx: &mut mpsc::UnboundedReceiver<ForwardedTcpIp>,
    rule: &TunnelRule,
    reporter: &TunnelReporter,
    stop_rx: &mut watch::Receiver<bool>,
) -> SessionEnd {
    let bind_host = remote_bind_address(&rule.bind_host);
    let bound_port = match session
        .tcpip_forward(bind_host.clone(), rule.bind_port as u32)
        .await
    {
        Ok(port) => port,
        Err(russh::Error::RequestDenied) => {
            return SessionEnd::Fatal(format!(
                "The server refused to listen on {}:{} (check AllowTcpForwarding and GatewayPorts)",
                rule.bind_host, rule.bind_port
            ));
        }
        Err(err) => return SessionEnd::Lost(format!("Remote forward request failed: {err}")),
    };
    // A requested port of 0 lets the server choose; otherwise it echoes ours.
    let bound_port = if bound_port == 0 {
        rule.bind_port
    } else {
        bound_port as u16
    };
    reporter.set_bound_port(Some(bound_port));
    reporter.set_running();

    let mut connections: JoinSet<()> = JoinSet::new();
    let mut tick = tokio::time::interval(HEALTH_TICK);
    let end = loop {
        tokio::select! {
            _ = wait_for_stop(stop_rx) => break SessionEnd::Stopped,
            _ = tick.tick() => {
                if session.is_closed() {
                    break SessionEnd::Lost("SSH connection closed".to_string());
                }
                reporter.tick();
            }
            Some(_) = connections.join_next(), if !connections.is_empty() => {}
            incoming = forwarded_rx.recv() => {
                match incoming {
                    Some(forwarded) => {
                        connections.spawn(forward_remote_connection(
                            forwarded,
                            rule.dest_host.clone(),
                            rule.dest_port,
                            reporter.stats.clone(),
                        ));
                    }
                    None => break SessionEnd::Lost("SSH connection closed".to_string()),
                }
            }
        }
    };

    if matches!(end, SessionEnd::Stopped) {
        let _ = session
            .cancel_tcpip_forward(bind_host, bound_port as u32)
            .await;
    }
    end
}

pub struct RunContext {
    pub app: AppHandle,
    pub rule: TunnelRule,
    pub plan: SessionPlan,
    pub prompts: HostPromptMap,
    pub reporter: Arc<TunnelReporter>,
}

fn backoff_delay(attempt: u32) -> Duration {
    Duration::from_secs((1u64 << attempt.min(6)).min(MAX_BACKOFF_SECS))
}

/// Clamps a profile's keepalive settings to what a tunnel needs; zero means
/// "unset" and gets the ceiling.
fn tunnel_keepalive(interval_secs: u16, count_max: u16) -> (u16, u16) {
    let clamp = |value: u16, ceiling: u16| {
        if value == 0 {
            ceiling
        } else {
            value.min(ceiling)
        }
    };
    (
        clamp(interval_secs, KEEPALIVE_INTERVAL_CEILING_SECS),
        clamp(count_max, KEEPALIVE_COUNT_CEILING),
    )
}

/// Sleeps for `delay`, returning `false` if a stop arrived first.
/// Connections arriving on `listeners` meanwhile are refused.
async fn sleep_unless_stopped(
    delay: Duration,
    stop_rx: &mut watch::Receiver<bool>,
    listeners: Option<&Listeners>,
) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(delay) => true,
        _ = wait_for_stop(stop_rx) => false,
        never = refuse_connections(listeners) => match never {},
    }
}

/// Drives one tunnel until it is stopped or fails permanently. While the SSH
/// connection is lost or unreachable it reconnects indefinitely with capped
/// backoff; only errors that retrying cannot fix (rejected credentials, a
/// server that forbids forwarding) end it.
pub async fn run_tunnel(ctx: RunContext, mut stop_rx: watch::Receiver<bool>) {
    let RunContext {
        app,
        rule,
        plan,
        prompts,
        reporter,
    } = ctx;

    reporter.set_state(TunnelState::Starting, None);

    let listener = if rule.kind == TunnelKind::Remote {
        None
    } else {
        match Listeners::bind(&rule.bind_host, rule.bind_port).await {
            Ok(listener) => {
                reporter.set_bound_port(listener.local_port());
                Some(listener)
            }
            Err(err) => {
                reporter.set_state(
                    TunnelState::Error,
                    Some(format!(
                        "Cannot listen on {}:{}: {err}",
                        rule.bind_host, rule.bind_port
                    )),
                );
                return;
            }
        }
    };

    let (keepalive_interval_secs, keepalive_count_max) =
        tunnel_keepalive(plan.keepalive_interval_secs, plan.keepalive_count_max);
    let mut attempt: u32 = 0;
    loop {
        if attempt > 0 {
            reporter.set_retry_attempt(attempt);
        }

        let (forwarded_tx, mut forwarded_rx) = mpsc::unbounded_channel();
        let connect = open_target_ssh_session_with_forwarding(
            &app,
            TUNNEL_EVENT_TAB_ID,
            plan.profile_id.as_deref(),
            &plan.profile_name,
            plan.host.as_deref().unwrap_or_default(),
            plan.port,
            plan.username.as_deref().unwrap_or_default(),
            plan.private_key_path.as_deref(),
            plan.private_key_passphrase.as_deref(),
            plan.password.as_deref(),
            keepalive_interval_secs,
            keepalive_count_max,
            &plan.jump_hosts,
            prompts.clone(),
            ConnectionStatusOptions::QUIET,
            HostKeyVerificationMode::PromptAndPersist,
            (rule.kind == TunnelKind::Remote).then_some(forwarded_tx),
        );
        let connected = tokio::select! {
            _ = wait_for_stop(&mut stop_rx) => {
                reporter.set_state(TunnelState::Stopped, None);
                return;
            }
            result = tokio::time::timeout(CONNECT_TIMEOUT, connect) => result,
            never = refuse_connections(listener.as_ref()) => match never {},
        };

        let (jump_chain, session) = match connected {
            Ok(Ok(pair)) => pair,
            failed => {
                let (retryable, message) = match failed {
                    Ok(Err(err)) => (err.is_retryable(), err.to_string()),
                    _ => (true, "Timed out connecting to the SSH server".to_string()),
                };
                if !retryable {
                    reporter.set_state(TunnelState::Error, Some(message));
                    return;
                }
                attempt = attempt.saturating_add(1);
                let delay = backoff_delay(attempt);
                reporter.set_retry_attempt(attempt);
                reporter.set_state(
                    TunnelState::Reconnecting,
                    Some(format!(
                        "{message} — retrying in {}s (attempt {attempt})",
                        delay.as_secs()
                    )),
                );
                if !sleep_unless_stopped(delay, &mut stop_rx, listener.as_ref()).await {
                    reporter.set_state(TunnelState::Stopped, None);
                    return;
                }
                continue;
            }
        };

        let session = Arc::new(session);
        let started = tokio::time::Instant::now();
        let end = match &listener {
            Some(listener) => {
                reporter.set_running();
                run_listener_session(session.clone(), listener, &rule, &reporter, &mut stop_rx)
                    .await
            }
            None => {
                run_remote_session(
                    session.clone(),
                    &mut forwarded_rx,
                    &rule,
                    &reporter,
                    &mut stop_rx,
                )
                .await
            }
        };

        let _ = session
            .disconnect(Disconnect::ByApplication, "Tunnel closed", "en")
            .await;
        drop(session);
        drop(jump_chain);

        match end {
            SessionEnd::Stopped => {
                reporter.set_state(TunnelState::Stopped, None);
                return;
            }
            SessionEnd::Fatal(message) => {
                reporter.set_state(TunnelState::Error, Some(message));
                return;
            }
            SessionEnd::Lost(message) => {
                if started.elapsed() >= STABLE_SESSION {
                    attempt = 0;
                }
                attempt = attempt.saturating_add(1);
                let delay = backoff_delay(attempt);
                reporter.set_retry_attempt(attempt);
                reporter.set_state(
                    TunnelState::Reconnecting,
                    Some(format!(
                        "{message} — retrying in {}s (attempt {attempt})",
                        delay.as_secs()
                    )),
                );
                if !sleep_unless_stopped(delay, &mut stop_rx, listener.as_ref()).await {
                    reporter.set_state(TunnelState::Stopped, None);
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn backoff_grows_and_is_capped() {
        assert_eq!(backoff_delay(1), Duration::from_secs(2));
        assert_eq!(backoff_delay(3), Duration::from_secs(8));
        assert_eq!(backoff_delay(5), Duration::from_secs(30));
        assert_eq!(backoff_delay(40), Duration::from_secs(30));
    }

    #[test]
    fn remote_star_means_all_interfaces() {
        assert_eq!(remote_bind_address("*"), "");
        assert_eq!(remote_bind_address("localhost"), "localhost");
        assert_eq!(remote_bind_address("0.0.0.0"), "0.0.0.0");
    }

    async fn free_port() -> u16 {
        let probe = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        probe.local_addr().unwrap().port()
    }

    #[tokio::test]
    async fn localhost_listens_on_ipv4_and_serves_accepts() {
        let port = free_port().await;
        let listeners = Listeners::bind("localhost", port).await.unwrap();
        assert_eq!(listeners.local_port(), Some(port));

        let _client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let (_, peer) = listeners.accept().await.unwrap();
        assert!(peer.ip().is_loopback());
    }

    #[tokio::test]
    async fn binding_a_taken_port_fails() {
        let port = free_port().await;
        let _first = Listeners::bind("127.0.0.1", port).await.unwrap();
        assert!(Listeners::bind("127.0.0.1", port).await.is_err());
    }

    #[tokio::test]
    async fn refused_connections_are_closed_at_once() {
        use tokio::io::AsyncReadExt;
        let port = free_port().await;
        let listeners = Listeners::bind("127.0.0.1", port).await.unwrap();
        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();

        let mut buf = [0u8; 1];
        let read = tokio::select! {
            never = refuse_connections(Some(&listeners)) => match never {},
            read = tokio::time::timeout(Duration::from_secs(2), client.read(&mut buf)) => read,
        };
        assert_eq!(read.expect("client must not hang").unwrap_or(0), 0);
    }

    #[test]
    fn keepalive_is_clamped_for_tunnels() {
        assert_eq!(tunnel_keepalive(0, 0), (30, 3));
        assert_eq!(tunnel_keepalive(120, 10), (30, 3));
        assert_eq!(tunnel_keepalive(10, 2), (10, 2));
    }

    #[test]
    fn channel_failures_are_classified_and_described() {
        use russh::ChannelOpenFailure as Reason;
        let classify =
            |reason| ChannelFailure::from_error(russh::Error::ChannelOpenFailure(reason));

        assert_eq!(
            classify(Reason::AdministrativelyProhibited),
            ChannelFailure::Prohibited
        );
        assert_eq!(
            classify(Reason::ConnectFailed),
            ChannelFailure::ConnectFailed
        );
        assert_eq!(
            classify(Reason::ResourceShortage),
            ChannelFailure::ResourceShortage
        );
        assert!(matches!(
            classify(Reason::Unknown),
            ChannelFailure::Other(_)
        ));

        let message = ChannelFailure::ConnectFailed.describe("db.internal", 5432);
        assert!(message.starts_with("db.internal:5432: "));
    }

    #[test]
    fn channel_failures_map_to_socks_replies() {
        assert!(matches!(
            ChannelFailure::Prohibited.socks_outcome(),
            ConnectOutcome::NotAllowed
        ));
        assert!(matches!(
            ChannelFailure::ConnectFailed.socks_outcome(),
            ConnectOutcome::Refused
        ));
        assert!(matches!(
            ChannelFailure::TimedOut.socks_outcome(),
            ConnectOutcome::TimedOut
        ));
        assert!(matches!(
            ChannelFailure::Other("x".into()).socks_outcome(),
            ConnectOutcome::Failed
        ));
    }

    #[test]
    fn connection_guard_tracks_active_and_total() {
        let stats = Arc::new(TunnelStats::default());
        {
            let _a = ConnectionGuard::new(&stats);
            let _b = ConnectionGuard::new(&stats);
            assert_eq!(stats.active_connections.load(Ordering::Relaxed), 2);
        }
        assert_eq!(stats.active_connections.load(Ordering::Relaxed), 0);
        assert_eq!(stats.total_connections.load(Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn counted_stream_attributes_bytes_by_direction() {
        for (local_is_client, up, down) in [(true, 5, 3), (false, 3, 5)] {
            let stats = Arc::new(TunnelStats::default());
            let (near, mut far) = tokio::io::duplex(64);
            let mut counted = Counted::new(near, stats.clone(), local_is_client);

            far.write_all(b"hello").await.unwrap();
            let mut buf = [0u8; 5];
            counted.read_exact(&mut buf).await.unwrap();
            counted.write_all(b"abc").await.unwrap();

            assert_eq!(stats.bytes_up.load(Ordering::Relaxed), up);
            assert_eq!(stats.bytes_down.load(Ordering::Relaxed), down);
        }
    }

    #[tokio::test]
    async fn wait_for_stop_resolves_on_signal_and_when_sender_drops() {
        let (tx, mut rx) = watch::channel(false);
        tx.send(true).unwrap();
        wait_for_stop(&mut rx).await;

        let (tx, mut rx) = watch::channel(false);
        drop(tx);
        wait_for_stop(&mut rx).await;
    }
}
