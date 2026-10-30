use super::config::{compatibility_client_config, unverifiable_host_key_error};
use super::hop::ChainHops;
use super::pool::{chain_keys, Relay};
use crate::core::session::{types::JumpHostPlan, types::SessionPlan};
use crate::core::state::HostPromptMap;
use crate::proxy::command::CommandTarget;
use crate::proxy::connect::{open_stream, ProxyStream};
use crate::proxy::{ProxyEndpoint, ProxyKind};
use crate::ssh::auth::{authenticate, AuthMethod, AuthPrompter, AuthTarget};
use crate::ssh::types::{
    emit_connection_progress, ConnectionStatusOptions, HostKeyVerificationMode, SshClientHandler,
    SshConnectError, SshConnectionProgressPayload,
};
use russh::{client, Disconnect};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

/// What every hop of one connection shares: where it reports progress and how
/// it asks the user about host keys and credentials.
#[derive(Clone, Copy)]
pub struct ConnectContext<'a> {
    pub app: &'a AppHandle,
    pub tab_id: &'a str,
    pub prompts: &'a HostPromptMap,
    pub status_options: ConnectionStatusOptions,
    pub host_key_verification_mode: HostKeyVerificationMode,
    pub auth_prompter: &'a AuthPrompter,
}

/// The host a connection ends at, how to log in to it, and the jump hosts on
/// the way there.
#[derive(Clone, Copy)]
pub struct SshTarget<'a> {
    pub profile_id: Option<&'a str>,
    pub profile_name: &'a str,
    pub host: &'a str,
    pub port: u16,
    pub username: &'a str,
    pub auth: AuthMethod<'a>,
    pub agent_forward: bool,
    pub keepalive_interval_secs: u16,
    pub keepalive_count_max: u16,
    pub jump_hosts: &'a [JumpHostPlan],
    /// The proxy server the first host, a jump host or the target, is reached through.
    pub proxy: Option<&'a ProxyEndpoint>,
}

impl<'a> SshTarget<'a> {
    /// The target `plan` connects to. The plan leaves host and username
    /// optional, so the caller passes them in once it has checked them.
    pub fn from_plan(plan: &'a SessionPlan, host: &'a str, username: &'a str) -> Self {
        Self {
            profile_id: plan.profile_id.as_deref(),
            profile_name: &plan.profile_name,
            host,
            port: plan.port,
            username,
            auth: plan.auth_method(),
            agent_forward: plan.agent_forward,
            keepalive_interval_secs: plan.keepalive_interval_secs,
            keepalive_count_max: plan.keepalive_count_max,
            jump_hosts: &plan.jump_hosts,
            proxy: plan.proxy.as_ref(),
        }
    }
}

/// Opens the stream to the first host, `target`, through `proxy`, reporting
/// progress, or `None` when the first host is dialed directly.
pub(crate) async fn open_proxy_stream(
    ctx: ConnectContext<'_>,
    proxy: Option<&ProxyEndpoint>,
    target: &CommandTarget<'_>,
) -> Result<Option<ProxyStream>, SshConnectError> {
    let Some(proxy) = proxy else {
        return Ok(None);
    };
    let (host, port) = (target.host, target.port);
    let progress = SshConnectionProgressPayload::new(
        "proxy_connecting",
        format!(
            "Connecting to {host}:{port} through the {}",
            proxy.describe()
        ),
    );
    let progress = if proxy.kind == ProxyKind::Command {
        progress
    } else {
        progress.host(proxy.host.clone(), proxy.port)
    };
    emit_connection_progress(ctx.app, ctx.tab_id, ctx.status_options, progress);
    open_stream(ctx.app, proxy, target).await.map(Some)
}

/// The SSH handshake with the first host, over `stream` when a proxy opened
/// one, otherwise over a new TCP connection.
pub(crate) async fn connect_first_hop<H>(
    config: Arc<client::Config>,
    host: &str,
    port: u16,
    stream: Option<ProxyStream>,
    handler: H,
) -> Result<client::Handle<H>, russh::Error>
where
    H: client::Handler<Error = russh::Error> + Send + 'static,
{
    match stream {
        Some(stream) => client::connect_stream(config, stream, handler).await,
        None => client::connect(config, (host, port), handler).await,
    }
}

/// Keeps a connection's jump hosts open. Its last hop holds on to the hops
/// before it, and to sessions other connections share with it.
pub struct JumpChain {
    last: Arc<dyn Relay>,
}

impl JumpChain {
    /// The session of the last hop, which the target is reached through.
    pub(crate) fn into_relay(self) -> Arc<dyn Relay> {
        self.last
    }
}

/// Writes a status line into the tab's terminal, when this connection shows them.
pub(super) fn emit_terminal_status(ctx: ConnectContext<'_>, status: &str) {
    if !ctx.status_options.emit_terminal_output {
        return;
    }
    let status_msg = format!("\r\n\x1b[33m[{status}]\x1b[0m\r\n");
    crate::session_log::record_output(ctx.app, ctx.tab_id, status_msg.as_bytes());
    let event_name = format!("pty-output-{}", ctx.tab_id);
    let _ = ctx
        .app
        .emit_to(tauri::EventTarget::any(), &event_name, status_msg);
}

/// Open an ordered ProxyJump-style chain, then establish the target SSH session
/// on top of the final direct-tcpip stream. The first jump host is reached
/// through `proxy` when one is set. The returned `JumpChain` must stay alive
/// for as long as the target session is used.
///
/// Hops already open, for another connection or as a terminal tab of their
/// own, are used as they are, so only the hops after them are logged in to.
/// A connection that trusts unknown host keys for itself alone, the
/// connection test, neither uses nor shares them.
pub async fn connect_via_jump_chain<H>(
    ctx: ConnectContext<'_>,
    jump_plans: &[JumpHostPlan],
    target_host: &str,
    target_port: u16,
    target_handler: H,
    target_config: Arc<client::Config>,
    proxy: Option<&ProxyEndpoint>,
) -> Result<(JumpChain, client::Handle<H>), SshConnectError>
where
    H: client::Handler<Error = russh::Error> + Send + 'static,
{
    let ConnectContext {
        app,
        tab_id,
        status_options,
        ..
    } = ctx;
    if jump_plans.is_empty() {
        return Err(SshConnectError::Permanent(
            "Jump host chain is empty".to_string(),
        ));
    }

    let shared = ctx.host_key_verification_mode == HostKeyVerificationMode::PromptAndPersist;
    let keys = chain_keys(
        proxy,
        jump_plans
            .iter()
            .map(|jump| (jump.username.as_str(), jump.host.as_str(), jump.port)),
    );
    let hops = ChainHops {
        plans: jump_plans,
        keys: shared.then_some(keys.as_slice()),
        proxy,
    };
    let (mut last, next) = match hops.farthest_open(ctx) {
        Some((index, session)) => (Some(session), index + 1),
        None => (None, 0),
    };
    for index in next..jump_plans.len() {
        last = Some(hops.open(ctx, index, last.take()).await?);
    }
    let last = last.ok_or_else(|| SshConnectError::Permanent("Jump chain is empty".to_string()))?;

    emit_terminal_status(
        ctx,
        &format!("Jump chain connected. Opening tunnel to {target_host}:{target_port}..."),
    );
    emit_connection_progress(
        app,
        tab_id,
        status_options,
        SshConnectionProgressPayload::new(
            "tunnel_opening",
            format!("Opening tunnel to target {}:{}", target_host, target_port),
        )
        .host(target_host.to_string(), target_port),
    );
    let tunnel_channel = last
        .open_direct_tcpip(target_host, target_port)
        .await
        .map_err(|e| {
            SshConnectError::Network(format!(
                "Failed to open tunnel to target through jump chain: {e}"
            ))
        })?;

    emit_connection_progress(
        app,
        tab_id,
        status_options,
        SshConnectionProgressPayload::new(
            "target_connecting",
            format!(
                "Connecting to target {}:{} through jump chain",
                target_host, target_port
            ),
        )
        .host(target_host.to_string(), target_port),
    );

    let target_session =
        client::connect_stream(target_config, tunnel_channel.into_stream(), target_handler)
            .await
            .map_err(|e| {
                unverifiable_host_key_error(&e).unwrap_or_else(|| {
                    SshConnectError::Network(format!(
                        "Failed to establish SSH session through jump chain: {e}"
                    ))
                })
            })?;

    Ok((JumpChain { last }, target_session))
}

/// Build a `SshClientHandler` for the target host and open an authenticated
/// SSH session, routing through an ordered jump chain when configured.
///
/// Returns `(Option<jump_chain>, target_session)`.  The jump chain must be kept
/// alive alongside the target session.
pub async fn open_target_ssh_session(
    ctx: ConnectContext<'_>,
    target: SshTarget<'_>,
) -> Result<(Option<JumpChain>, client::Handle<SshClientHandler>), SshConnectError> {
    open_target_ssh_session_with_forwarding(ctx, target, None).await
}

pub async fn open_target_ssh_session_with_forwarding(
    ctx: ConnectContext<'_>,
    target: SshTarget<'_>,
    forwarded_tcpip_tx: Option<
        tokio::sync::mpsc::UnboundedSender<crate::ssh::types::ForwardedTcpIp>,
    >,
) -> Result<(Option<JumpChain>, client::Handle<SshClientHandler>), SshConnectError> {
    let ConnectContext {
        app,
        tab_id,
        status_options,
        auth_prompter,
        ..
    } = ctx;
    let SshTarget {
        host: target_host,
        port: target_port,
        username: target_username,
        auth: target_auth,
        ..
    } = target;

    let target_config = Arc::new(compatibility_client_config(
        target.keepalive_interval_secs as u64,
        target.keepalive_count_max as usize,
    ));

    let handler = SshClientHandler {
        app: app.clone(),
        tab_id: tab_id.to_string(),
        profile_id: target.profile_id.map(str::to_string),
        profile_name: target.profile_name.to_string(),
        host: target_host.to_string(),
        port: target_port,
        prompts: ctx.prompts.clone(),
        user_rejected_host_key: Arc::new(AtomicBool::new(false)),
        status_options,
        host_key_verification_mode: ctx.host_key_verification_mode,
        forwarded_tcpip_tx,
        agent_forward_enabled: target.agent_forward,
    };
    let host_key_rejected = handler.user_rejected_host_key.clone();

    let (jump_chain_opt, mut target_session) = if target.jump_hosts.is_empty() {
        emit_connection_progress(
            app,
            tab_id,
            status_options,
            SshConnectionProgressPayload::new(
                "target_connecting",
                format!("Connecting to target {}:{}", target_host, target_port),
            )
            .host(target_host.to_string(), target_port)
            .username(target_username.to_string()),
        );
        let stream = open_proxy_stream(
            ctx,
            target.proxy,
            &CommandTarget {
                host: target_host,
                port: target_port,
                username: target_username,
            },
        )
        .await?;
        let sess = connect_first_hop(target_config, target_host, target_port, stream, handler)
            .await
            .map_err(|e| {
                if host_key_rejected.load(Ordering::Relaxed) {
                    SshConnectError::HostKeyRejected
                } else if let Some(unverifiable) = unverifiable_host_key_error(&e) {
                    unverifiable
                } else {
                    SshConnectError::Network(format!("SSH connect failed: {e}"))
                }
            })?;

        (None, sess)
    } else {
        let (chain, target_sess) = connect_via_jump_chain(
            ctx,
            target.jump_hosts,
            target_host,
            target_port,
            handler,
            target_config,
            target.proxy,
        )
        .await?;
        (Some(chain), target_sess)
    };

    emit_connection_progress(
        app,
        tab_id,
        status_options,
        SshConnectionProgressPayload::new(
            "target_authenticating",
            format!("Authenticating target as {}", target_username),
        )
        .host(target_host.to_string(), target_port)
        .username(target_username.to_string()),
    );

    let auth_result = authenticate(
        &mut target_session,
        &AuthTarget {
            host: target_host,
            port: target_port,
            username: target_username,
            hop: None,
        },
        target_auth.for_tab(tab_id),
        auth_prompter,
    )
    .await;
    let used = match auth_result {
        Ok(used) => used,
        Err(error) => {
            let _ = target_session
                .disconnect(Disconnect::ByApplication, "Authentication failed", "en")
                .await;
            return Err(error);
        }
    };
    if matches!(target_auth, AuthMethod::Auto { .. }) {
        emit_connection_progress(
            app,
            tab_id,
            status_options,
            SshConnectionProgressPayload::new(
                "target_authenticated",
                format!("Authenticated target as {}", target_username),
            )
            .host(target_host.to_string(), target_port)
            .username(target_username.to_string())
            .auth_used(&used),
        );
    }

    Ok((jump_chain_opt, target_session))
}
