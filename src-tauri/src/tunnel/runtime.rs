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
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::atomic::Ordering;
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
const MAX_RECONNECT_ATTEMPTS: u32 = 8;
const MAX_BACKOFF_SECS: u64 = 30;
/// A session that lasted this long counts as healthy and resets the backoff.
const STABLE_SESSION: Duration = Duration::from_secs(30);

type SshHandle = Handle<SshClientHandler>;

struct StatusInner {
    state: TunnelState,
    message: Option<String>,
    bound_port: Option<u16>,
    connected_at: Option<u64>,
    retry_attempt: u32,
    last_emitted_counters: (u64, u64, u64, u64),
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
                last_emitted_counters: (0, 0, 0, 0),
            }),
        })
    }

    fn counters(&self) -> (u64, u64, u64, u64) {
        (
            self.stats.active_connections.load(Ordering::Relaxed),
            self.stats.total_connections.load(Ordering::Relaxed),
            self.stats.bytes_up.load(Ordering::Relaxed),
            self.stats.bytes_down.load(Ordering::Relaxed),
        )
    }

    pub fn snapshot(&self) -> TunnelStatus {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let (active, total, up, down) = self.counters();
        TunnelStatus {
            id: self.id.clone(),
            state: inner.state,
            message: inner.message.clone(),
            bound_port: inner.bound_port,
            active_connections: active,
            total_connections: total,
            bytes_up: up,
            bytes_down: down,
            connected_at: inner.connected_at,
            retry_attempt: inner.retry_attempt,
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

async fn open_direct_channel(
    session: &SshHandle,
    host: &str,
    port: u16,
    peer: SocketAddr,
) -> Option<russh::Channel<russh::client::Msg>> {
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
        Ok(Ok(channel)) => Some(channel),
        _ => None,
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
    if let Some(channel) = open_direct_channel(&session, &dest_host, dest_port, peer).await {
        pipe(tcp, channel.into_stream(), &stats, true).await;
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
        Some(channel) => {
            if socks5::write_outcome(&mut tcp, ConnectOutcome::Success)
                .await
                .is_ok()
            {
                pipe(tcp, channel.into_stream(), &stats, true).await;
            }
        }
        None => {
            let _ = socks5::write_outcome(&mut tcp, ConnectOutcome::Refused).await;
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
        _ => {
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
    listener: &TcpListener,
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

async fn run_remote_session(
    session: Arc<SshHandle>,
    forwarded_rx: &mut mpsc::UnboundedReceiver<ForwardedTcpIp>,
    rule: &TunnelRule,
    reporter: &TunnelReporter,
    stop_rx: &mut watch::Receiver<bool>,
) -> SessionEnd {
    let bound_port = match session
        .tcpip_forward(rule.bind_host.clone(), rule.bind_port as u32)
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
            .cancel_tcpip_forward(rule.bind_host.clone(), bound_port as u32)
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

/// Sleeps for `delay`, returning `false` if a stop arrived first.
async fn sleep_unless_stopped(delay: Duration, stop_rx: &mut watch::Receiver<bool>) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(delay) => true,
        _ = wait_for_stop(stop_rx) => false,
    }
}

/// Drives one tunnel until it is stopped or fails permanently, reconnecting
/// with capped backoff while the SSH connection is lost.
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
        match TcpListener::bind((rule.bind_host.as_str(), rule.bind_port)).await {
            Ok(listener) => {
                reporter.set_bound_port(listener.local_addr().ok().map(|addr| addr.port()));
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
            plan.keepalive_interval_secs,
            plan.keepalive_count_max,
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
        };

        let (jump_chain, session) = match connected {
            Ok(Ok(pair)) => pair,
            failed => {
                let (retryable, message) = match failed {
                    Ok(Err(err)) => (err.is_retryable(), err.to_string()),
                    _ => (true, "Timed out connecting to the SSH server".to_string()),
                };
                attempt += 1;
                if !retryable || attempt > MAX_RECONNECT_ATTEMPTS {
                    reporter.set_state(TunnelState::Error, Some(message));
                    return;
                }
                let delay = backoff_delay(attempt);
                reporter.set_retry_attempt(attempt);
                reporter.set_state(
                    TunnelState::Reconnecting,
                    Some(format!("{message} — retrying in {}s", delay.as_secs())),
                );
                if !sleep_unless_stopped(delay, &mut stop_rx).await {
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
                attempt += 1;
                if attempt > MAX_RECONNECT_ATTEMPTS {
                    reporter.set_state(TunnelState::Error, Some(message));
                    return;
                }
                let delay = backoff_delay(attempt);
                reporter.set_retry_attempt(attempt);
                reporter.set_state(
                    TunnelState::Reconnecting,
                    Some(format!("{message} — retrying in {}s", delay.as_secs())),
                );
                if !sleep_unless_stopped(delay, &mut stop_rx).await {
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
