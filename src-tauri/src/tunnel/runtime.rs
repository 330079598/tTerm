use super::hub::{Lease, RouteGuard, SessionHub, SessionKey, SshConnection, SshHandle};
use super::types::{TunnelKind, TunnelRule, TunnelState};
use crate::core::session::SessionPlan;
use crate::core::state::HostPromptMap;
use crate::ssh::{
    open_target_ssh_session_with_forwarding, ConnectContext, ConnectionStatusOptions,
    ForwardedTcpIp, HostKeyVerificationMode, SshTarget,
};
use connections::{
    forward_local_connection, forward_remote_connection, forward_socks_connection,
    refuse_connections, ConnectionLimiter, Listeners,
};
use reporter::TunnelReporter;
use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;
use tokio::sync::watch;
use tokio::task::JoinSet;

mod connections;
pub(super) mod reporter;

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
/// Simultaneous forwarded connections per tunnel; further ones are refused so
/// a port scan or a runaway client cannot exhaust tasks and file descriptors.
const MAX_CONNECTIONS: usize = 256;
/// Tunnels sit idle for long stretches, so a silent drop (NAT timeout, sleeping
/// laptop) must be noticed quickly whatever the profile's terminal keepalive
/// says: at most 3 missed probes 30 seconds apart.
const KEEPALIVE_INTERVAL_CEILING_SECS: u16 = 30;
const KEEPALIVE_COUNT_CEILING: u16 = 3;
/// A session that lasted this long counts as healthy and resets the backoff.
const STABLE_SESSION: Duration = Duration::from_secs(30);
/// How often a running tunnel saves its traffic, bounding what a crash loses.
const TRAFFIC_SAVE_INTERVAL: Duration = Duration::from_secs(30);

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
    let limiter = ConnectionLimiter::new(MAX_CONNECTIONS);
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
                let Some(permit) = limiter.acquire(&reporter.stats) else { continue };
                let session = session.clone();
                let stats = reporter.stats.clone();
                if rule.kind == TunnelKind::Dynamic {
                    connections.spawn(async move {
                        let _permit = permit;
                        forward_socks_connection(session, tcp, peer, stats).await;
                    });
                } else {
                    let dest_host = rule.dest_host.clone();
                    let dest_port = rule.dest_port;
                    connections.spawn(async move {
                        let _permit = permit;
                        forward_local_connection(session, tcp, peer, dest_host, dest_port, stats)
                            .await;
                    });
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

/// A `-R` listener on the server. The connection is shared with other
/// tunnels and outlives this one, so the server must be told to stop
/// listening however this ends.
struct RemoteForward {
    session: Arc<SshHandle>,
    bind_host: String,
    port: u16,
    route: Option<RouteGuard<ForwardedTcpIp>>,
    cancelled: bool,
}

impl RemoteForward {
    /// Stops the server-side listener and waits for it, so a restart right
    /// after can claim the same port.
    async fn cancel(mut self) {
        self.cancelled = true;
        self.route.take();
        let _ = self
            .session
            .cancel_tcpip_forward(self.bind_host.clone(), self.port as u32)
            .await;
    }
}

impl Drop for RemoteForward {
    /// Covers the ways out that skip `cancel` (a task abort, an early return).
    fn drop(&mut self) {
        if self.cancelled || self.session.is_closed() {
            return;
        }
        let session = self.session.clone();
        let bind_host = self.bind_host.clone();
        let port = self.port as u32;
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _ = session.cancel_tcpip_forward(bind_host, port).await;
            });
        }
    }
}

async fn run_remote_session(
    lease: &Lease<SshConnection>,
    rule: &TunnelRule,
    reporter: &TunnelReporter,
    stop_rx: &mut watch::Receiver<bool>,
) -> SessionEnd {
    let session = lease.connection().handle.clone();
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
    let Some((route, mut forwarded_rx)) = lease.register_route(bound_port) else {
        let _ = session
            .cancel_tcpip_forward(bind_host, bound_port as u32)
            .await;
        return SessionEnd::Fatal(format!(
            "Port {bound_port} on the server is already forwarded by another tunnel"
        ));
    };
    let forward = RemoteForward {
        session: session.clone(),
        bind_host,
        port: bound_port,
        route: Some(route),
        cancelled: false,
    };
    reporter.set_bound_port(Some(bound_port));
    reporter.set_running();

    let mut connections: JoinSet<()> = JoinSet::new();
    let limiter = ConnectionLimiter::new(MAX_CONNECTIONS);
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
                        let Some(permit) = limiter.acquire(&reporter.stats) else {
                            let _ = forwarded.channel.close().await;
                            continue;
                        };
                        let dest_host = rule.dest_host.clone();
                        let dest_port = rule.dest_port;
                        let stats = reporter.stats.clone();
                        connections.spawn(async move {
                            let _permit = permit;
                            forward_remote_connection(forwarded, dest_host, dest_port, stats)
                                .await;
                        });
                    }
                    None => break SessionEnd::Lost("SSH connection closed".to_string()),
                }
            }
        }
    };

    if matches!(end, SessionEnd::Stopped) {
        forward.cancel().await;
    }
    end
}

pub struct RunContext {
    pub app: AppHandle,
    pub rule: TunnelRule,
    pub plan: SessionPlan,
    pub prompts: HostPromptMap,
    pub reporter: Arc<TunnelReporter>,
    /// Shared with every other tunnel, so those to the same host share a connection.
    pub hub: Arc<SessionHub<SshConnection>>,
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
        hub,
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
    let session_key = SessionKey::for_plan(&plan);
    let mut attempt: u32 = 0;
    loop {
        if attempt > 0 {
            reporter.set_retry_attempt(attempt);
        }

        // Joins the connection another tunnel to this host already has, or
        // opens it for the others to join.
        let connect = hub.acquire(&session_key, |forwarded_tx| async {
            let (jump_chain, handle) = open_target_ssh_session_with_forwarding(
                ConnectContext {
                    app: &app,
                    tab_id: TUNNEL_EVENT_TAB_ID,
                    prompts: &prompts,
                    status_options: ConnectionStatusOptions::QUIET,
                    host_key_verification_mode: HostKeyVerificationMode::PromptAndPersist,
                    // Tunnels reconnect on their own, so there is no moment at
                    // which a prompt is expected.
                    auth_prompter: &crate::ssh::AuthPrompter::Unavailable,
                },
                SshTarget {
                    keepalive_interval_secs,
                    keepalive_count_max,
                    ..SshTarget::from_plan(
                        &plan,
                        plan.host.as_deref().unwrap_or_default(),
                        plan.username.as_deref().unwrap_or_default(),
                    )
                },
                Some(forwarded_tx),
            )
            .await?;
            Ok(SshConnection::new(handle, jump_chain))
        });
        let connected = tokio::select! {
            _ = wait_for_stop(&mut stop_rx) => {
                reporter.set_state(TunnelState::Stopped, None);
                return;
            }
            result = tokio::time::timeout(CONNECT_TIMEOUT, connect) => result,
            never = refuse_connections(listener.as_ref()) => match never {},
        };

        let lease = match connected {
            Ok(Ok(lease)) => lease,
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

        let started = tokio::time::Instant::now();
        let end = match &listener {
            Some(listener) => {
                reporter.set_running();
                run_listener_session(
                    lease.connection().handle.clone(),
                    listener,
                    &rule,
                    &reporter,
                    &mut stop_rx,
                )
                .await
            }
            None => run_remote_session(&lease, &rule, &reporter, &mut stop_rx).await,
        };

        // Other tunnels may still be using the connection; the last one out
        // closes it.
        drop(lease);

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
mod tests;
