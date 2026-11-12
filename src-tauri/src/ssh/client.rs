use crate::ssh::context::ConnectionContext;
mod output;
mod reader;
mod setup;

use super::types::{ConnectionStatusOptions, SshClientHandler};
use crate::core::session::types::SessionPlan;
use crate::core::state::ChannelContext;
use crate::ssh::jump::connect::JumpChain;
use crate::ssh::jump::pool::{self, PooledSession, RouteKey, chain_keys};
use crate::zmodem::session::ZmodemSettings;
use output::run_blocking;
use reader::{ConnectedSession, read_connection};
use russh::Disconnect;
use setup::{PreparedTerminal, prepare_terminal};
use std::sync::Arc;
use tokio::sync::{mpsc, watch};
use tterm_core::events::ssh::SshConnectionProgressPayload;

const LATENCY_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Measures an SSH round trip by timing the server's channel-open confirmation.
pub async fn measure_ssh_latency(
    session: &russh::client::Handle<SshClientHandler>,
) -> Result<u64, String> {
    tokio::time::timeout(LATENCY_PROBE_TIMEOUT, async {
        let started_at = tokio::time::Instant::now();
        let channel = session
            .channel_open_session()
            .await
            .map_err(|err| format!("Failed to open latency probe channel: {err}"))?;
        let elapsed = started_at.elapsed();
        let _ = channel.close().await;
        Ok(elapsed.as_millis().min(u64::MAX as u128) as u64)
    })
    .await
    .map_err(|_| "Latency probe timed out".to_string())?
}

/// Where a tab's session ends up and the way it gets there.
fn target_route(plan: &SessionPlan, host: &str, username: &str) -> Option<RouteKey> {
    let hops = plan
        .jump_hosts
        .iter()
        .map(|jump| (jump.username.as_str(), jump.host.as_str(), jump.port));
    chain_keys(
        plan.proxy.as_ref(),
        hops.chain(std::iter::once((username, host, plan.port))),
    )
    .pop()
}

/// Lets connections that jump through the tab's host use its session, which
/// keeps the jump hosts it was reached through open with it.
fn share_session(
    route: Option<&RouteKey>,
    session: russh::client::Handle<SshClientHandler>,
    jump_chain: Option<JumpChain>,
) -> Arc<PooledSession<SshClientHandler>> {
    let session = Arc::new(PooledSession::new(
        session,
        jump_chain.map(JumpChain::into_relay),
    ));
    if let Some(route) = route {
        pool::offer(route.clone(), &session);
    }
    session
}

/// Ends the tab's session, unless connections still jump through it; it then
/// closes with the last of them.
async fn close_session(
    route: Option<&RouteKey>,
    session: &Arc<PooledSession<SshClientHandler>>,
    reason: &str,
) {
    let unshared = route.is_none_or(|route| pool::withdraw_if_unshared(route, session));
    if unshared {
        let _ = session
            .handle
            .disconnect(Disconnect::ByApplication, reason, "en")
            .await;
    }
}

pub struct SshExitSignal {
    pub terminated: bool,
    pub recoverable: bool,
    pub reason: Option<String>,
    pub connected_duration: Option<std::time::Duration>,
}

/// The channels a connection attempt is driven through.
pub struct SshChannelIo {
    pub stop_rx: watch::Receiver<bool>,
    pub input_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    pub resize_rx: mpsc::UnboundedReceiver<(u16, u16)>,
    pub sender: Option<crate::terminal::io_batcher::TerminalOutputSender>,
}

pub(crate) struct SshAttemptContext {
    pub(crate) channel: ChannelContext,
    pub(crate) connection: ConnectionContext,
}

pub async fn run_single_ssh_connection(
    context: SshAttemptContext,
    settings: ZmodemSettings,
    plan: SessionPlan,
    prompts: std::sync::Arc<tterm_core::prompts::PromptBroker>,
    rows: u16,
    cols: u16,
    io: SshChannelIo,
) -> SshExitSignal {
    let SshChannelIo {
        stop_rx,
        input_rx,
        mut resize_rx,
        mut sender,
    } = io;
    let SshAttemptContext {
        channel: tab,
        connection: app,
    } = context;
    let tab_id = tab.tab_id.clone();
    let host: String = match &plan.host {
        Some(host) => host.clone(),
        None => {
            finish_output(&mut sender);
            return SshExitSignal {
                terminated: true,
                recoverable: false,
                reason: Some("SSH host is required".to_string()),
                connected_duration: None,
            };
        }
    };
    let username: String = match &plan.username {
        Some(username) => username.clone(),
        None => {
            finish_output(&mut sender);
            return SshExitSignal {
                terminated: true,
                recoverable: false,
                reason: Some("SSH username is required".to_string()),
                connected_duration: None,
            };
        }
    };

    let prompt_scope = match crate::prompt_scope::PromptScope::new(
        prompts.clone(),
        Some(tterm_core::prompts::SessionKey {
            tab_id: tab_id.clone(),
            session_nonce: tab.session_nonce,
        }),
    ) {
        Ok(scope) => scope,
        Err(reason) => {
            finish_output(&mut sender);
            return SshExitSignal {
                terminated: true,
                recoverable: false,
                reason: Some(reason),
                connected_duration: None,
            };
        }
    };
    // Use open_target_ssh_session for connection + auth (supports jump hosts)
    let (jump_chain, session) = match crate::ssh::jump::connect::open_target_ssh_session(
        crate::ssh::jump::connect::ConnectContext {
            app: &app,
            tab_id: &tab_id,
            prompts: &prompts,
            status_options: ConnectionStatusOptions::VERBOSE,
            host_key_verification_mode:
                crate::ssh::types::HostKeyVerificationMode::PromptAndPersist,
            prompt_owner: Some(&prompt_scope.owner),
            auth_prompter: &crate::ssh::auth::AuthPrompter::tab(&app, &tab_id),
        },
        crate::ssh::jump::connect::SshTarget::from_plan(&plan, &host, &username),
    )
    .await
    {
        Ok(result) => result,
        Err(err) => {
            finish_output(&mut sender);
            // Auth failures and host-key rejections are permanent: automatic
            // reconnect must not loop on them.
            let recoverable = err.is_retryable();
            return SshExitSignal {
                terminated: false,
                recoverable,
                reason: Some(err.to_string()),
                connected_duration: None,
            };
        }
    };

    let route = target_route(&plan, &host, &username);
    let session = share_session(route.as_ref(), session, jump_chain);

    let PreparedTerminal {
        channel,
        early_output,
        shell_integration_status,
        network_latency_ms,
    } = match prepare_terminal(
        &session,
        &plan,
        &host,
        &username,
        (rows, cols),
        &mut resize_rx,
        &mut sender,
    )
    .await
    {
        Ok(prepared) => prepared,
        Err(signal) => return signal,
    };

    let _ = tab.sink.state_changed(
        SshConnectionProgressPayload::new(
            "ready",
            format!("Connected to {}@{}:{}", username, host, plan.port),
        )
        .host(host.clone(), plan.port)
        .username(username.clone())
        .network_latency(Some(network_latency_ms))
        .shell_integration(shell_integration_status),
    );

    // Asked once the shell is requested, so its answer travels alongside the
    // first prompt instead of ahead of it.
    if crate::config::tab_icons_enabled() {
        super::remote_os::probe(app.events.clone(), &tab_id, &session.handle).await;
    }

    read_connection(ConnectedSession {
        tab,
        app: app.log,
        settings,
        plan,
        route,
        session,
        channel,
        early_output,
        io: SshChannelIo {
            stop_rx,
            input_rx,
            resize_rx,
            sender,
        },
    })
    .await
}

fn finish_output(sender: &mut Option<crate::terminal::io_batcher::TerminalOutputSender>) {
    if let Some(sender) = sender.as_mut() {
        run_blocking(|| sender.finish());
    }
}

#[cfg(test)]
mod tests;
