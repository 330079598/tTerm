use russh::keys::ssh_key::HashAlg;
use russh::keys::PrivateKeyWithHashAlg;
use russh::Disconnect;
use russh::{cipher, client, kex, mac, Preferred};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;

use crate::core::session::JumpHostPlan;
use crate::core::state::HostPromptMap;
use crate::ssh::store::{
    load_known_host, now_unix_ms, save_known_host_entry, KnownHostRecord, SshHostKeyPromptPayload,
};
use crate::ssh::types::{SshClientHandler, HOST_KEY_PROMPT_TIMEOUT, HOST_KEY_REJECTED_REASON};

/// russh handler for the jump host leg of the connection.
/// Performs the same host-key verification as `SshClientHandler`:
/// known_hosts lookup → user prompt on unknown/mismatch → save on approve.
pub struct JumpHostHandler {
    pub app: AppHandle,
    pub tab_id: String,
    pub host: String,
    pub port: u16,
    pub hop_index: usize,
    pub prompts: HostPromptMap,
    pub user_rejected_host_key: Arc<AtomicBool>,
    pub failure_reason: Arc<Mutex<Option<String>>>,
}

impl client::Handler for JumpHostHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        let algorithm = server_public_key.algorithm().to_string();
        let fingerprint = server_public_key.fingerprint(HashAlg::Sha256).to_string();

        // Use a synthetic profile name to key known_hosts entries for jump hosts.
        let synthetic_name = format!("jump:{}:{}", self.host, self.port);

        let known = match load_known_host(&synthetic_name, None) {
            Ok(value) => value,
            Err(err) => {
                let reason = format!("Failed to read jump host known_hosts store: {err}");
                self.set_failure_reason(reason.clone());
                self.emit_status("31", &reason);
                return Err(russh::Error::Disconnect);
            }
        };

        if let Some(record) = &known {
            if record.host == self.host
                && record.port == self.port
                && record.fingerprint == fingerprint
            {
                return Ok(true);
            }
        }

        let reason = if known.is_some() {
            "mismatch".to_string()
        } else {
            "unknown".to_string()
        };
        let known_fingerprint = known.as_ref().map(|r| r.fingerprint.clone());

        let request_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel::<bool>();
        self.prompts.write().await.insert(request_id.clone(), tx);

        let payload = SshHostKeyPromptPayload {
            request_id: request_id.clone(),
            profile_name: synthetic_name.clone(),
            host: self.host.clone(),
            port: self.port,
            algorithm,
            fingerprint: fingerprint.clone(),
            reason,
            known_fingerprint,
        };

        let event_name = format!("ssh-hostkey-prompt-{}", self.tab_id);
        let _ = self
            .app
            .emit_to(tauri::EventTarget::any(), &event_name, payload);

        self.emit_status(
            "33",
            &format!(
                "Waiting for user confirmation of jump host #{} fingerprint...",
                self.hop_index
            ),
        );

        let approved = match tokio::time::timeout(HOST_KEY_PROMPT_TIMEOUT, rx).await {
            Ok(Ok(value)) => value,
            Ok(Err(_)) => false,
            Err(_) => {
                let _ = self.prompts.write().await.remove(&request_id);
                false
            }
        };

        if !approved {
            self.user_rejected_host_key.store(true, Ordering::Relaxed);
            self.set_failure_reason(HOST_KEY_REJECTED_REASON.to_string());
            self.emit_status("31", HOST_KEY_REJECTED_REASON);
            return Err(russh::Error::Disconnect);
        }

        let save_result = save_known_host_entry(KnownHostRecord {
            profile_id: None,
            profile_name: synthetic_name,
            host: self.host.clone(),
            port: self.port,
            algorithm: server_public_key.algorithm().to_string(),
            fingerprint,
            trusted_at: now_unix_ms(),
        });

        if let Err(err) = save_result {
            let reason = format!("Failed to save jump host known_hosts entry: {err}");
            self.set_failure_reason(reason.clone());
            self.emit_status("31", &reason);
            return Err(russh::Error::Disconnect);
        }

        Ok(true)
    }
}

impl JumpHostHandler {
    fn set_failure_reason(&self, reason: String) {
        if let Ok(mut current) = self.failure_reason.lock() {
            *current = Some(reason);
        }
    }

    fn emit_status(&self, color: &str, message: &str) {
        let payload = format!(
            "\r\n\x1b[{}m[Jump #{}: {}]\x1b[0m\r\n",
            color, self.hop_index, message
        );
        let event_name = format!("pty-output-{}", self.tab_id);
        let _ = self
            .app
            .emit_to(tauri::EventTarget::any(), &event_name, payload);
    }
}

/// Authenticate a russh session using either a private key or a password.
async fn authenticate_session(
    session: &mut client::Handle<JumpHostHandler>,
    username: &str,
    private_key_path: Option<&str>,
    private_key_passphrase: Option<&str>,
    password: Option<&str>,
) -> Result<(), String> {
    let auth_result = if let Some(key_path) = private_key_path {
        let key_pair = russh::keys::load_secret_key(Path::new(key_path), private_key_passphrase)
            .map_err(|e| format!("Failed to load jump host SSH key: {e}"))?;

        session
            .authenticate_publickey(
                username,
                PrivateKeyWithHashAlg::new(Arc::new(key_pair), None),
            )
            .await
            .map_err(|e| format!("Jump host key authentication failed: {e}"))?
    } else {
        let pw = password.ok_or_else(|| "Jump host password is required".to_string())?;
        session
            .authenticate_password(username, pw)
            .await
            .map_err(|e| format!("Jump host password authentication failed: {e}"))?
    };

    if !auth_result.success() {
        return Err("Jump host authentication failed".to_string());
    }

    Ok(())
}

fn compatibility_preferred_algorithms() -> Preferred {
    Preferred {
        kex: std::borrow::Cow::Owned(vec![
            kex::MLKEM768X25519_SHA256,
            kex::CURVE25519,
            kex::CURVE25519_PRE_RFC_8731,
            kex::ECDH_SHA2_NISTP256,
            kex::ECDH_SHA2_NISTP384,
            kex::ECDH_SHA2_NISTP521,
            kex::DH_GEX_SHA256,
            kex::DH_G14_SHA256,
            kex::DH_G14_SHA1,
            kex::DH_GEX_SHA1,
            kex::DH_G1_SHA1,
            kex::EXTENSION_SUPPORT_AS_CLIENT,
            kex::EXTENSION_OPENSSH_STRICT_KEX_AS_CLIENT,
        ]),
        cipher: std::borrow::Cow::Owned(vec![
            cipher::CHACHA20_POLY1305,
            cipher::AES_256_GCM,
            cipher::AES_128_GCM,
            cipher::AES_256_CTR,
            cipher::AES_192_CTR,
            cipher::AES_128_CTR,
            cipher::AES_256_CBC,
            cipher::AES_192_CBC,
            cipher::AES_128_CBC,
        ]),
        mac: std::borrow::Cow::Owned(vec![
            mac::HMAC_SHA512_ETM,
            mac::HMAC_SHA256_ETM,
            mac::HMAC_SHA1_ETM,
            mac::HMAC_SHA512,
            mac::HMAC_SHA256,
            mac::HMAC_SHA1,
        ]),
        ..Preferred::default()
    }
}

pub fn compatibility_client_config(
    keepalive_interval_secs: u64,
    keepalive_max: usize,
) -> client::Config {
    client::Config {
        client_id: russh::SshId::Standard(std::borrow::Cow::Borrowed("SSH-2.0-OpenSSH_9.6")),
        keepalive_interval: Some(Duration::from_secs(keepalive_interval_secs)),
        keepalive_max,
        preferred: compatibility_preferred_algorithms(),
        nodelay: true,
        ..Default::default()
    }
}
fn format_jump_host_connect_error(error: &russh::Error) -> String {
    let detail = error.to_string();
    if detail.eq_ignore_ascii_case("disconnected") {
        return "Jump host connection failed: disconnected during SSH handshake. Check the jump host address/port, SSH service, network reachability, host key prompt, and server SSH algorithm compatibility.".to_string();
    }

    format!("Jump host connection failed: {detail}")
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

fn build_jump_handler(
    app: &AppHandle,
    tab_id: &str,
    jump_plan: &JumpHostPlan,
    hop_index: usize,
    _total_hops: usize,
    prompts: HostPromptMap,
) -> JumpHostHandler {
    JumpHostHandler {
        app: app.clone(),
        tab_id: tab_id.to_string(),
        host: jump_plan.host.clone(),
        port: jump_plan.port,
        hop_index,
        prompts,
        user_rejected_host_key: Arc::new(AtomicBool::new(false)),
        failure_reason: Arc::new(Mutex::new(None)),
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
) -> Result<client::Handle<JumpHostHandler>, String> {
    let jump_handler = build_jump_handler(app, tab_id, jump_plan, hop_index, total_hops, prompts);
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

    authenticate_session(
        &mut session,
        &jump_plan.username,
        jump_plan.private_key_path.as_deref(),
        jump_plan.private_key_passphrase.as_deref(),
        jump_plan.password.as_deref(),
    )
    .await
    .map_err(|e| format!("Jump host #{hop_index}: {e}"))?;

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
) -> Result<client::Handle<JumpHostHandler>, String>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let jump_handler = build_jump_handler(app, tab_id, jump_plan, hop_index, total_hops, prompts);
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

    let mut session = tokio::time::timeout(
        Duration::from_secs(15),
        client::connect_stream(jump_config, stream, jump_handler),
    )
    .await
    .map_err(|_| format!("Jump host #{hop_index} connection timed out"))?
    .map_err(|e| map_jump_connect_error(e, host_key_rejected, failure_reason, hop_index))?;

    authenticate_session(
        &mut session,
        &jump_plan.username,
        jump_plan.private_key_path.as_deref(),
        jump_plan.private_key_passphrase.as_deref(),
        jump_plan.password.as_deref(),
    )
    .await
    .map_err(|e| format!("Jump host #{hop_index}: {e}"))?;

    Ok(session)
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

    let first =
        connect_jump_direct(app, tab_id, &jump_plans[0], 1, total_hops, prompts.clone()).await?;
    sessions.push(first);

    for (index, jump_plan) in jump_plans.iter().enumerate().skip(1) {
        let hop_index = index + 1;
        let previous = sessions
            .last()
            .ok_or_else(|| "Jump chain lost its previous session".to_string())?;

        let status_msg = format!(
            "\r\n\x1b[33m[Opening tunnel to jump #{} {}:{}...]\x1b[0m\r\n",
            hop_index, jump_plan.host, jump_plan.port
        );
        let event_name = format!("pty-output-{}", tab_id);
        let _ = app.emit_to(tauri::EventTarget::any(), &event_name, status_msg);

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
        )
        .await?;
        sessions.push(next);
    }

    let status_msg = format!(
        "\r\n\x1b[33m[Jump chain connected. Opening tunnel to {}:{}...]\x1b[0m\r\n",
        target_host, target_port
    );
    let event_name = format!("pty-output-{}", tab_id);
    let _ = app.emit_to(tauri::EventTarget::any(), &event_name, status_msg);

    let last = sessions
        .last()
        .ok_or_else(|| "Jump chain is empty".to_string())?;
    let tunnel_channel = last
        .channel_open_direct_tcpip(target_host, target_port as u32, "127.0.0.1", 0)
        .await
        .map_err(|e| format!("Failed to open tunnel to target through jump chain: {e}"))?;

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
    };
    let host_key_rejected = handler.user_rejected_host_key.clone();

    let (jump_chain_opt, mut target_session) = if jump_plans.is_empty() {
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
        )
        .await?;
        (Some(chain), target_sess)
    };

    let auth_result = if let Some(key_path) = target_private_key_path {
        let key_pair =
            russh::keys::load_secret_key(Path::new(key_path), target_private_key_passphrase)
                .map_err(|e| format!("Failed to load SSH key: {e}"))?;

        target_session
            .authenticate_publickey(
                target_username,
                PrivateKeyWithHashAlg::new(Arc::new(key_pair), None),
            )
            .await
            .map_err(|e| format!("SSH key authentication failed: {e}"))?
    } else {
        let pw = target_password.ok_or_else(|| "SSH password is required".to_string())?;
        target_session
            .authenticate_password(target_username, pw)
            .await
            .map_err(|e| format!("SSH authentication failed: {e}"))?
    };

    if !auth_result.success() {
        let _ = target_session
            .disconnect(Disconnect::ByApplication, "Authentication failed", "en")
            .await;
        return Err("SSH authentication failed".to_string());
    }

    Ok((jump_chain_opt, target_session))
}
