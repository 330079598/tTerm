//! Connecting and logging in to one jump host of a chain.

use super::config::{compatibility_client_config, unverifiable_host_key_error};
use super::connect::{connect_first_hop, emit_terminal_status, open_proxy_stream, ConnectContext};
use super::handler::JumpHostHandler;
use super::pool::{self, PooledSession, Relay, RouteKey};
use crate::core::session::types::JumpHostPlan;
use crate::proxy::command::CommandTarget;
use crate::proxy::ProxyEndpoint;
use crate::ssh::auth::{authenticate, AuthPrompter, AuthTarget};
use crate::ssh::types::{emit_connection_progress, SshConnectError, SshConnectionProgressPayload};
use russh::client;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Authenticates one hop of the chain with its configured method.
async fn authenticate_jump_host(
    session: &mut client::Handle<JumpHostHandler>,
    tab_id: &str,
    jump_plan: &JumpHostPlan,
    hop_index: usize,
    total_hops: usize,
    auth_prompter: &AuthPrompter,
) -> Result<(), SshConnectError> {
    authenticate(
        session,
        &AuthTarget {
            host: &jump_plan.host,
            port: jump_plan.port,
            username: &jump_plan.username,
            hop: Some((hop_index, total_hops)),
        },
        jump_plan.auth_method().for_tab(tab_id),
        auth_prompter,
    )
    .await
    .map(|_| ())
    .map_err(|e| e.with_prefix(&format!("Jump host #{hop_index}: ")))
}

fn format_jump_host_connect_error(error: &russh::Error) -> String {
    let detail = error.to_string();
    if detail.eq_ignore_ascii_case("disconnected") {
        return "Jump host connection failed: disconnected during SSH handshake. Check the jump host address/port, SSH service, network reachability, host key prompt, and server SSH algorithm compatibility.".to_string();
    }

    format!("Jump host connection failed: {detail}")
}

fn build_jump_handler(
    ctx: ConnectContext<'_>,
    jump_plan: &JumpHostPlan,
    hop_index: usize,
    total_hops: usize,
) -> JumpHostHandler {
    JumpHostHandler {
        app: ctx.app.clone(),
        tab_id: ctx.tab_id.to_string(),
        host: jump_plan.host.clone(),
        port: jump_plan.port,
        hop_index,
        total_hops,
        prompts: ctx.prompts.clone(),
        user_rejected_host_key: Arc::new(AtomicBool::new(false)),
        failure_reason: Arc::new(Mutex::new(None)),
        status_options: ctx.status_options,
        host_key_verification_mode: ctx.host_key_verification_mode,
    }
}

fn map_jump_connect_error(
    error: russh::Error,
    host_key_rejected: Arc<AtomicBool>,
    failure_reason: Arc<Mutex<Option<String>>>,
    hop_index: usize,
) -> SshConnectError {
    // The rejection flag is checked first so a user-refused host key is
    // classified correctly even though the handler also records a reason.
    if host_key_rejected.load(Ordering::Relaxed) {
        return SshConnectError::HostKeyRejected;
    }
    if let Some(unverifiable) = unverifiable_host_key_error(&error) {
        return unverifiable.with_prefix(&format!("Jump host #{hop_index}: "));
    }

    if let Ok(mut reason) = failure_reason.lock() {
        if let Some(reason) = reason.take() {
            return SshConnectError::Permanent(reason);
        }
    }

    SshConnectError::Network(format!(
        "Jump host #{hop_index}: {}",
        format_jump_host_connect_error(&error)
    ))
}

pub(super) async fn connect_jump_direct(
    ctx: ConnectContext<'_>,
    jump_plan: &JumpHostPlan,
    hop_index: usize,
    total_hops: usize,
    proxy: Option<&ProxyEndpoint>,
) -> Result<client::Handle<JumpHostHandler>, SshConnectError> {
    let ConnectContext {
        app,
        tab_id,
        status_options,
        auth_prompter,
        ..
    } = ctx;
    let jump_handler = build_jump_handler(ctx, jump_plan, hop_index, total_hops);
    let host_key_rejected = jump_handler.user_rejected_host_key.clone();
    let failure_reason = jump_handler.failure_reason.clone();
    let jump_config = Arc::new(compatibility_client_config(15, 3));

    jump_handler.emit_status(
        "33",
        &format!(
            "Connecting to {}@{}:{}...",
            jump_plan.username, jump_plan.host, jump_plan.port
        ),
    );
    emit_connection_progress(
        app,
        tab_id,
        status_options,
        SshConnectionProgressPayload::new(
            "jump_connecting",
            format!(
                "Connecting to jump host #{} {}@{}:{}",
                hop_index, jump_plan.username, jump_plan.host, jump_plan.port
            ),
        )
        .host(jump_plan.host.clone(), jump_plan.port)
        .username(jump_plan.username.clone())
        .hop(hop_index, total_hops),
    );

    let stream = open_proxy_stream(
        ctx,
        proxy,
        &CommandTarget {
            host: &jump_plan.host,
            port: jump_plan.port,
            username: &jump_plan.username,
        },
    )
    .await?;
    let mut session = tokio::time::timeout(
        Duration::from_secs(15),
        connect_first_hop(
            jump_config,
            &jump_plan.host,
            jump_plan.port,
            stream,
            jump_handler,
        ),
    )
    .await
    .map_err(|_| SshConnectError::Network(format!("Jump host #{hop_index} connection timed out")))?
    .map_err(|e| map_jump_connect_error(e, host_key_rejected, failure_reason, hop_index))?;

    emit_connection_progress(
        app,
        tab_id,
        status_options,
        SshConnectionProgressPayload::new(
            "jump_authenticating",
            format!(
                "Authenticating jump host #{} as {}",
                hop_index, jump_plan.username
            ),
        )
        .host(jump_plan.host.clone(), jump_plan.port)
        .username(jump_plan.username.clone())
        .hop(hop_index, total_hops),
    );

    authenticate_jump_host(
        &mut session,
        tab_id,
        jump_plan,
        hop_index,
        total_hops,
        auth_prompter,
    )
    .await?;

    emit_connection_progress(
        app,
        tab_id,
        status_options,
        SshConnectionProgressPayload::new(
            "jump_connected",
            format!("Jump host #{} connected", hop_index),
        )
        .host(jump_plan.host.clone(), jump_plan.port)
        .username(jump_plan.username.clone())
        .hop(hop_index, total_hops),
    );

    Ok(session)
}

pub(super) async fn connect_jump_over_stream<S>(
    ctx: ConnectContext<'_>,
    jump_plan: &JumpHostPlan,
    hop_index: usize,
    total_hops: usize,
    stream: S,
) -> Result<client::Handle<JumpHostHandler>, SshConnectError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let ConnectContext {
        app,
        tab_id,
        status_options,
        auth_prompter,
        ..
    } = ctx;
    let jump_handler = build_jump_handler(ctx, jump_plan, hop_index, total_hops);
    let host_key_rejected = jump_handler.user_rejected_host_key.clone();
    let failure_reason = jump_handler.failure_reason.clone();
    let jump_config = Arc::new(compatibility_client_config(15, 3));

    jump_handler.emit_status(
        "33",
        &format!(
            "Connecting to {}@{}:{} through tunnel...",
            jump_plan.username, jump_plan.host, jump_plan.port
        ),
    );
    emit_connection_progress(
        app,
        tab_id,
        status_options,
        SshConnectionProgressPayload::new(
            "jump_connecting",
            format!(
                "Connecting to jump host #{} {}@{}:{} through tunnel",
                hop_index, jump_plan.username, jump_plan.host, jump_plan.port
            ),
        )
        .host(jump_plan.host.clone(), jump_plan.port)
        .username(jump_plan.username.clone())
        .hop(hop_index, total_hops),
    );

    let mut session = tokio::time::timeout(
        Duration::from_secs(15),
        client::connect_stream(jump_config, stream, jump_handler),
    )
    .await
    .map_err(|_| SshConnectError::Network(format!("Jump host #{hop_index} connection timed out")))?
    .map_err(|e| map_jump_connect_error(e, host_key_rejected, failure_reason, hop_index))?;

    emit_connection_progress(
        app,
        tab_id,
        status_options,
        SshConnectionProgressPayload::new(
            "jump_authenticating",
            format!(
                "Authenticating jump host #{} as {}",
                hop_index, jump_plan.username
            ),
        )
        .host(jump_plan.host.clone(), jump_plan.port)
        .username(jump_plan.username.clone())
        .hop(hop_index, total_hops),
    );

    authenticate_jump_host(
        &mut session,
        tab_id,
        jump_plan,
        hop_index,
        total_hops,
        auth_prompter,
    )
    .await?;

    emit_connection_progress(
        app,
        tab_id,
        status_options,
        SshConnectionProgressPayload::new(
            "jump_connected",
            format!("Jump host #{} connected", hop_index),
        )
        .host(jump_plan.host.clone(), jump_plan.port)
        .username(jump_plan.username.clone())
        .hop(hop_index, total_hops),
    );

    Ok(session)
}

/// The hops of one chain and, when this connection shares them, the route of
/// each.
pub(super) struct ChainHops<'a> {
    pub plans: &'a [JumpHostPlan],
    /// `None` when this connection keeps its hops to itself.
    pub keys: Option<&'a [RouteKey]>,
    pub proxy: Option<&'a ProxyEndpoint>,
}

impl ChainHops<'_> {
    /// The farthest hop that is already open, and its position.
    pub fn farthest_open(&self, ctx: ConnectContext<'_>) -> Option<(usize, Arc<dyn Relay>)> {
        let (index, session) = self
            .keys?
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, key)| pool::find(key).map(|session| (index, session)))?;
        self.report_reused(ctx, index);
        Some((index, session))
    }

    /// Logs in to hop `index`, through `previous` or, for the first hop,
    /// directly, and offers it to the connections that come after.
    pub async fn open(
        &self,
        ctx: ConnectContext<'_>,
        index: usize,
        previous: Option<Arc<dyn Relay>>,
    ) -> Result<Arc<dyn Relay>, SshConnectError> {
        let plan = self
            .plans
            .get(index)
            .ok_or_else(|| SshConnectError::Permanent("Jump chain lost a hop".to_string()))?;
        let key = self.keys.and_then(|keys| keys.get(index));
        let _connecting = match key {
            Some(key) => {
                let guard = pool::connecting(key).await;
                // Whoever held the place before may have just opened this hop.
                if let Some(session) = pool::find(key) {
                    self.report_reused(ctx, index);
                    return Ok(session);
                }
                Some(guard)
            }
            None => None,
        };

        let (hop_index, total_hops) = (index + 1, self.plans.len());
        let handle = match &previous {
            None => connect_jump_direct(ctx, plan, hop_index, total_hops, self.proxy).await?,
            Some(previous) => {
                emit_terminal_status(
                    ctx,
                    &format!(
                        "Opening tunnel to jump #{hop_index} {}:{}...",
                        plan.host, plan.port
                    ),
                );
                emit_connection_progress(
                    ctx.app,
                    ctx.tab_id,
                    ctx.status_options,
                    SshConnectionProgressPayload::new(
                        "tunnel_opening",
                        format!(
                            "Opening tunnel to jump host #{hop_index} {}:{}",
                            plan.host, plan.port
                        ),
                    )
                    .host(plan.host.clone(), plan.port)
                    .hop(hop_index, total_hops),
                );
                let channel = previous
                    .open_direct_tcpip(&plan.host, plan.port)
                    .await
                    .map_err(|e| {
                        SshConnectError::Network(format!(
                            "Jump host #{index} failed to open tunnel to jump #{hop_index}: {e}"
                        ))
                    })?;
                connect_jump_over_stream(ctx, plan, hop_index, total_hops, channel.into_stream())
                    .await?
            }
        };
        let session = Arc::new(PooledSession::new(handle, previous));
        if let Some(key) = key {
            pool::offer(key.clone(), &session);
        }
        Ok(session)
    }

    /// Tells the user hop `index` goes over a session that is already open.
    fn report_reused(&self, ctx: ConnectContext<'_>, index: usize) {
        let Some(plan) = self.plans.get(index) else {
            return;
        };
        let address = format!("{}@{}:{}", plan.username, plan.host, plan.port);
        emit_terminal_status(ctx, &format!("Using the open connection to {address}"));
        emit_connection_progress(
            ctx.app,
            ctx.tab_id,
            ctx.status_options,
            SshConnectionProgressPayload::new(
                "jump_connected",
                format!(
                    "Using the open connection to jump host #{} {address}",
                    index + 1
                ),
            )
            .host(plan.host.clone(), plan.port)
            .username(plan.username.clone())
            .hop(index + 1, self.plans.len()),
        );
    }
}
