use super::config::compatibility_client_config;
use super::handler::JumpHostHandler;
use crate::core::session::JumpHostPlan;
use crate::core::state::HostPromptMap;
use crate::ssh::types::{
    emit_connection_progress, ConnectionStatusOptions, HostKeyVerificationMode, SshClientHandler,
    SshConnectionProgressPayload, HOST_KEY_REJECTED_REASON,
};
use russh::keys::PrivateKeyWithHashAlg;
use russh::{client, Disconnect};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// Authenticate a russh session using either a private key or a password.
async fn authenticate_session(
    session: &mut client::Handle<JumpHostHandler>,
    username: &str,
    private_key_path: Option<&str>,
    private_key_passphrase: Option<&str>,
    password: Option<&str>,
) -> Result<(), String> {
    const AUTH_TIMEOUT: Duration = Duration::from_secs(30);

    let auth_result = if let Some(key_path) = private_key_path {
        let key_pair = russh::keys::load_secret_key(Path::new(key_path), private_key_passphrase)
            .map_err(|e| format!("Failed to load jump host SSH key: {e}"))?;

        tokio::time::timeout(
            AUTH_TIMEOUT,
            session.authenticate_publickey(
                username,
                PrivateKeyWithHashAlg::new(Arc::new(key_pair), None),
            ),
        )
        .await
        .map_err(|_| "Jump host key authentication timed out".to_string())?
        .map_err(|e| format!("Jump host key authentication failed: {e}"))?
    } else {
        let pw = password.ok_or_else(|| "Jump host password is required".to_string())?;
        tokio::time::timeout(AUTH_TIMEOUT, session.authenticate_password(username, pw))
            .await
            .map_err(|_| "Jump host password authentication timed out".to_string())?
            .map_err(|e| format!("Jump host password authentication failed: {e}"))?
    };

    if !auth_result.success() {
        return Err("Jump host authentication failed".to_string());
    }

    Ok(())
}

fn format_jump_host_connect_error(error: &russh::Error) -> String {
    let detail = error.to_string();
    if detail.eq_ignore_ascii_case("disconnected") {
        return "Jump host connection failed: disconnected during SSH handshake. Check the jump host address/port, SSH service, network reachability, host key prompt, and server SSH algorithm compatibility.".to_string();
    }

    format!("Jump host connection failed: {detail}")
}

fn build_jump_handler(
    app: &AppHandle,
    tab_id: &str,
    jump_plan: &JumpHostPlan,
    hop_index: usize,
    total_hops: usize,
    prompts: HostPromptMap,
    status_options: ConnectionStatusOptions,
    host_key_verification_mode: HostKeyVerificationMode,
) -> JumpHostHandler {
    JumpHostHandler {
        app: app.clone(),
        tab_id: tab_id.to_string(),
        host: jump_plan.host.clone(),
        port: jump_plan.port,
        hop_index,
        total_hops,
        prompts,
        user_rejected_host_key: Arc::new(AtomicBool::new(false)),
        failure_reason: Arc::new(Mutex::new(None)),
        status_options,
        host_key_verification_mode,
    }
}

fn map_jump_connect_error(
    error: russh::Error,
    host_key_rejected: Arc<AtomicBool>,
    failure_reason: Arc<Mutex<Option<String>>>,
    hop_index: usize,
) -> String {
    if let Ok(mut reason) = failure_reason.lock() {
        if let Some(reason) = reason.take() {
            return reason;
        }
    }

    if host_key_rejected.load(Ordering::Relaxed) {
        HOST_KEY_REJECTED_REASON.to_string()
    } else {
        format!(
            "Jump host #{hop_index}: {}",
            format_jump_host_connect_error(&error)
        )
    }
}

async fn connect_jump_direct(
    app: &AppHandle,
    tab_id: &str,
    jump_plan: &JumpHostPlan,
    hop_index: usize,
    total_hops: usize,
    prompts: HostPromptMap,
    status_options: ConnectionStatusOptions,
    host_key_verification_mode: HostKeyVerificationMode,
) -> Result<client::Handle<JumpHostHandler>, String> {
    let jump_handler = build_jump_handler(
        app,
        tab_id,
        jump_plan,
        hop_index,
        total_hops,
        prompts,
        status_options,
        host_key_verification_mode,
    );
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

    let mut session = tokio::time::timeout(
        Duration::from_secs(15),
        client::connect(
            jump_config,
            (jump_plan.host.as_str(), jump_plan.port),
            jump_handler,
        ),
    )
    .await
    .map_err(|_| format!("Jump host #{hop_index} connection timed out"))?
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

    authenticate_session(
        &mut session,
        &jump_plan.username,
        jump_plan.private_key_path.as_deref(),
        jump_plan.private_key_passphrase.as_deref(),
        jump_plan.password.as_deref(),
    )
    .await
    .map_err(|e| format!("Jump host #{hop_index}: {e}"))?;

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

async fn connect_jump_over_stream<S>(
    app: &AppHandle,
    tab_id: &str,
    jump_plan: &JumpHostPlan,
    hop_index: usize,
    total_hops: usize,
    stream: S,
    prompts: HostPromptMap,
    status_options: ConnectionStatusOptions,
    host_key_verification_mode: HostKeyVerificationMode,
) -> Result<client::Handle<JumpHostHandler>, String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let jump_handler = build_jump_handler(
        app,
        tab_id,
        jump_plan,
        hop_index,
        total_hops,
        prompts,
        status_options,
        host_key_verification_mode,
    );
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
    .map_err(|_| format!("Jump host #{hop_index} connection timed out"))?
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

    authenticate_session(
        &mut session,
        &jump_plan.username,
        jump_plan.private_key_path.as_deref(),
        jump_plan.private_key_passphrase.as_deref(),
        jump_plan.password.as_deref(),
    )
    .await
    .map_err(|e| format!("Jump host #{hop_index}: {e}"))?;

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

/// Holds the authenticated jump sessions that keep every direct-tcpip hop alive.
pub struct JumpChain {
    pub sessions: Vec<client::Handle<JumpHostHandler>>,
}

impl Drop for JumpChain {
    fn drop(&mut self) {
        let _ = self.sessions.len();
    }
}

/// Open an ordered ProxyJump-style chain, then establish the target SSH session
/// on top of the final direct-tcpip stream. The returned `JumpChain` must stay
/// alive for as long as the target session is used.
pub async fn connect_via_jump_chain<H>(
    app: &AppHandle,
    tab_id: &str,
    jump_plans: &[JumpHostPlan],
    target_host: &str,
    target_port: u16,
    target_handler: H,
    target_config: Arc<client::Config>,
    prompts: HostPromptMap,
    status_options: ConnectionStatusOptions,
    host_key_verification_mode: HostKeyVerificationMode,
) -> Result<(JumpChain, client::Handle<H>), String>
where
    H: client::Handler + Send + 'static,
    H::Error: std::fmt::Display,
{
    if jump_plans.is_empty() {
        return Err("Jump host chain is empty".to_string());
    }

    let total_hops = jump_plans.len();
    let mut sessions = Vec::with_capacity(total_hops);

    let first = connect_jump_direct(
        app,
        tab_id,
        &jump_plans[0],
        1,
        total_hops,
        prompts.clone(),
        status_options,
        host_key_verification_mode,
    )
    .await?;
    sessions.push(first);

    for (index, jump_plan) in jump_plans.iter().enumerate().skip(1) {
        let hop_index = index + 1;
        let previous = sessions
            .last()
            .ok_or_else(|| "Jump chain lost its previous session".to_string())?;

        if status_options.emit_terminal_output {
            let status_msg = format!(
                "\r\n\x1b[33m[Opening tunnel to jump #{} {}:{}...]\x1b[0m\r\n",
                hop_index, jump_plan.host, jump_plan.port
            );
            let event_name = format!("pty-output-{}", tab_id);
            let _ = app.emit_to(tauri::EventTarget::any(), &event_name, status_msg);
        }

        emit_connection_progress(
            app,
            tab_id,
            status_options,
            SshConnectionProgressPayload::new(
                "tunnel_opening",
                format!(
                    "Opening tunnel to jump host #{} {}:{}",
                    hop_index, jump_plan.host, jump_plan.port
                ),
            )
            .host(jump_plan.host.clone(), jump_plan.port)
            .hop(hop_index, total_hops),
        );
        let tunnel_channel = previous
            .channel_open_direct_tcpip(
                jump_plan.host.as_str(),
                jump_plan.port as u32,
                "127.0.0.1",
                0,
            )
            .await
            .map_err(|e| {
                format!(
                    "Jump host #{} failed to open tunnel to jump #{}: {e}",
                    index, hop_index
                )
            })?;

        let next = connect_jump_over_stream(
            app,
            tab_id,
            jump_plan,
            hop_index,
            total_hops,
            tunnel_channel.into_stream(),
            prompts.clone(),
            status_options,
            host_key_verification_mode,
        )
        .await?;
        sessions.push(next);
    }

    if status_options.emit_terminal_output {
        let status_msg = format!(
            "\r\n\x1b[33m[Jump chain connected. Opening tunnel to {}:{}...]\x1b[0m\r\n",
            target_host, target_port
        );
        let event_name = format!("pty-output-{}", tab_id);
        let _ = app.emit_to(tauri::EventTarget::any(), &event_name, status_msg);
    }

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
    let last = sessions
        .last()
        .ok_or_else(|| "Jump chain is empty".to_string())?;
    let tunnel_channel = last
        .channel_open_direct_tcpip(target_host, target_port as u32, "127.0.0.1", 0)
        .await
        .map_err(|e| format!("Failed to open tunnel to target through jump chain: {e}"))?;

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
            .map_err(|e| format!("Failed to establish SSH session through jump chain: {e}"))?;

    Ok((JumpChain { sessions }, target_session))
}

/// Build a `SshClientHandler` for the target host and open an authenticated
/// SSH session, routing through an ordered jump chain when configured.
///
/// Returns `(Option<jump_chain>, target_session)`.  The jump chain must be kept
/// alive alongside the target session.
pub async fn open_target_ssh_session(
    app: &AppHandle,
    tab_id: &str,
    profile_id: Option<&str>,
    profile_name: &str,
    target_host: &str,
    target_port: u16,
    target_username: &str,
    target_private_key_path: Option<&str>,
    target_private_key_passphrase: Option<&str>,
    target_password: Option<&str>,
    keepalive_interval_secs: u16,
    keepalive_count_max: u16,
    jump_plans: &[JumpHostPlan],
    prompts: HostPromptMap,
    status_options: ConnectionStatusOptions,
    host_key_verification_mode: HostKeyVerificationMode,
) -> Result<(Option<JumpChain>, client::Handle<SshClientHandler>), String> {
    let target_config = Arc::new(compatibility_client_config(
        keepalive_interval_secs as u64,
        keepalive_count_max as usize,
    ));

    let handler = SshClientHandler {
        app: app.clone(),
        tab_id: tab_id.to_string(),
        profile_id: profile_id.map(str::to_string),
        profile_name: profile_name.to_string(),
        host: target_host.to_string(),
        port: target_port,
        prompts: prompts.clone(),
        user_rejected_host_key: Arc::new(AtomicBool::new(false)),
        status_options,
        host_key_verification_mode,
    };
    let host_key_rejected = handler.user_rejected_host_key.clone();

    let (jump_chain_opt, mut target_session) = if jump_plans.is_empty() {
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
        let sess = client::connect(target_config, (target_host, target_port), handler)
            .await
            .map_err(|e| {
                if host_key_rejected.load(Ordering::Relaxed) {
                    HOST_KEY_REJECTED_REASON.to_string()
                } else {
                    format!("SSH connect failed: {e}")
                }
            })?;

        (None, sess)
    } else {
        let (chain, target_sess) = connect_via_jump_chain(
            app,
            tab_id,
            jump_plans,
            target_host,
            target_port,
            handler,
            target_config,
            prompts,
            status_options,
            host_key_verification_mode,
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

    const TARGET_AUTH_TIMEOUT: Duration = Duration::from_secs(30);

    let auth_result = if let Some(key_path) = target_private_key_path {
        let key_pair =
            russh::keys::load_secret_key(Path::new(key_path), target_private_key_passphrase)
                .map_err(|e| format!("Failed to load SSH key: {e}"))?;

        tokio::time::timeout(
            TARGET_AUTH_TIMEOUT,
            target_session.authenticate_publickey(
                target_username,
                PrivateKeyWithHashAlg::new(Arc::new(key_pair), None),
            ),
        )
        .await
        .map_err(|_| "SSH key authentication timed out".to_string())?
        .map_err(|e| format!("SSH key authentication failed: {e}"))?
    } else {
        let pw = target_password.ok_or_else(|| "SSH password is required".to_string())?;
        tokio::time::timeout(
            TARGET_AUTH_TIMEOUT,
            target_session.authenticate_password(target_username, pw),
        )
        .await
        .map_err(|_| "SSH password authentication timed out".to_string())?
        .map_err(|e| format!("SSH authentication failed: {e}"))?
    };

    if !auth_result.success() {
        let _ = target_session
            .disconnect(Disconnect::ByApplication, "Authentication failed", "en")
            .await;
        return Err("SSH authentication failed".to_string());
    }

    emit_connection_progress(
        app,
        tab_id,
        status_options,
        SshConnectionProgressPayload::new(
            "ready",
            format!(
                "Connected to {}@{}:{}",
                target_username, target_host, target_port
            ),
        )
        .host(target_host.to_string(), target_port)
        .username(target_username.to_string()),
    );

    Ok((jump_chain_opt, target_session))
}
