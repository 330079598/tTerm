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
            "Waiting for user confirmation of jump host fingerprint...",
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
        let payload = format!("\r\n\x1b[{}m[{}]\x1b[0m\r\n", color, message);
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

/// Open a `direct-tcpip` channel through an already-authenticated jump host
/// session, then establish a second SSH session on top of that tunnel.
///
/// Returns `(jump_session, target_session)`.  The caller **must** keep
/// `jump_session` alive for as long as `target_session` is in use, because
/// the tunnel channel lives inside the jump session.
pub async fn connect_via_jump<H>(
    app: &AppHandle,
    tab_id: &str,
    jump_plan: &JumpHostPlan,
    target_host: &str,
    target_port: u16,
    target_handler: H,
    target_config: Arc<client::Config>,
    prompts: HostPromptMap,
) -> Result<(client::Handle<JumpHostHandler>, client::Handle<H>), String>
where
    H: client::Handler + Send + 'static,
    H::Error: std::fmt::Display,
{
    // ── Step 1: connect to the jump host ────────────────────────────────────
    let jump_handler = JumpHostHandler {
        app: app.clone(),
        tab_id: tab_id.to_string(),
        host: jump_plan.host.clone(),
        port: jump_plan.port,
        prompts,
        user_rejected_host_key: Arc::new(AtomicBool::new(false)),
        failure_reason: Arc::new(Mutex::new(None)),
    };
    let host_key_rejected = jump_handler.user_rejected_host_key.clone();
    let failure_reason = jump_handler.failure_reason.clone();

    let jump_config = Arc::new(compatibility_client_config(15, 3));

    jump_handler.emit_status(
        "33",
        &format!(
            "Connecting to jump host {}@{}:{}...",
            jump_plan.username, jump_plan.host, jump_plan.port
        ),
    );

    let mut jump_session = tokio::time::timeout(
        Duration::from_secs(15),
        client::connect(
            jump_config,
            (jump_plan.host.as_str(), jump_plan.port),
            jump_handler,
        ),
    )
    .await
    .map_err(|_| "Jump host connection timed out".to_string())?
    .map_err(|e| {
        if let Ok(mut reason) = failure_reason.lock() {
            if let Some(reason) = reason.take() {
                return reason;
            }
        }

        if host_key_rejected.load(Ordering::Relaxed) {
            HOST_KEY_REJECTED_REASON.to_string()
        } else {
            format_jump_host_connect_error(&e)
        }
    })?;

    // ── Step 2: authenticate on the jump host ───────────────────────────────
    authenticate_session(
        &mut jump_session,
        &jump_plan.username,
        jump_plan.private_key_path.as_deref(),
        jump_plan.private_key_passphrase.as_deref(),
        jump_plan.password.as_deref(),
    )
    .await?;

    let status_msg = format!(
        "\r\n\x1b[33m[Jump host connected. Opening tunnel to {}:{}...]\x1b[0m\r\n",
        target_host, target_port
    );
    let event_name = format!("pty-output-{}", tab_id);
    let _ = app.emit_to(tauri::EventTarget::any(), &event_name, status_msg);

    // ── Step 3: open a direct-tcpip channel to the target ───────────────────
    // The originator address/port are informational only (RFC 4254 §7.2).
    let tunnel_channel = jump_session
        .channel_open_direct_tcpip(target_host, target_port as u32, "127.0.0.1", 0)
        .await
        .map_err(|e| format!("Failed to open tunnel to target: {e}"))?;

    // ── Step 4: run a second SSH session over the tunnel stream ─────────────
    // `into_stream()` gives us an `AsyncRead + AsyncWrite` backed by the channel.
    let target_session =
        client::connect_stream(target_config, tunnel_channel.into_stream(), target_handler)
            .await
            .map_err(|e| format!("Failed to establish SSH session through tunnel: {e}"))?;

    Ok((jump_session, target_session))
}

/// Build a `SshClientHandler` for the target host and open an authenticated
/// SSH session, routing through the jump host when `jump_plan` is `Some`.
///
/// Returns `(Option<jump_session>, target_session)`.  The jump session must
/// be kept alive alongside the target session.
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
    jump_plan: Option<&JumpHostPlan>,
    prompts: HostPromptMap,
) -> Result<
    (
        Option<client::Handle<JumpHostHandler>>,
        client::Handle<SshClientHandler>,
    ),
    String,
> {
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

    let (jump_session_opt, mut target_session) = if let Some(jump) = jump_plan {
        // Route through jump host
        let (jump_sess, target_sess) = connect_via_jump(
            app,
            tab_id,
            jump,
            target_host,
            target_port,
            handler,
            target_config,
            prompts,
        )
        .await?;
        (Some(jump_sess), target_sess)
    } else {
        // Direct connection
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
    };

    // ── Authenticate on the target ───────────────────────────────────────────
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

    Ok((jump_session_opt, target_session))
}
