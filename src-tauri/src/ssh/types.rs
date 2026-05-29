use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::Emitter;

pub const HOST_KEY_PROMPT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
pub const HOST_KEY_REJECTED_REASON: &str = "SSH host fingerprint rejected by user";

#[derive(Clone, Copy, Debug)]
pub struct ConnectionStatusOptions {
    pub emit_terminal_output: bool,
    pub emit_progress_events: bool,
}

impl ConnectionStatusOptions {
    pub const VERBOSE: Self = Self {
        emit_terminal_output: true,
        emit_progress_events: true,
    };

    pub const SILENT: Self = Self {
        emit_terminal_output: false,
        emit_progress_events: true,
    };
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshConnectionProgressPayload {
    pub phase: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hop_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_hops: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
}

impl SshConnectionProgressPayload {
    pub fn new(phase: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            phase: phase.into(),
            message: message.into(),
            hop_index: None,
            total_hops: None,
            host: None,
            port: None,
            username: None,
        }
    }

    pub fn host(mut self, host: impl Into<String>, port: u16) -> Self {
        self.host = Some(host.into());
        self.port = Some(port);
        self
    }

    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    pub fn hop(mut self, hop_index: usize, total_hops: usize) -> Self {
        self.hop_index = Some(hop_index);
        self.total_hops = Some(total_hops);
        self
    }
}

pub fn emit_connection_progress(
    app: &tauri::AppHandle,
    tab_id: &str,
    status_options: ConnectionStatusOptions,
    payload: SshConnectionProgressPayload,
) {
    if !status_options.emit_progress_events {
        return;
    }

    let event_name = format!("ssh-connection-progress-{tab_id}");
    let _ = app.emit_to(tauri::EventTarget::any(), &event_name, payload);
}

#[derive(Clone)]
pub struct SshClientHandler {
    pub app: tauri::AppHandle,
    pub tab_id: String,
    pub profile_id: Option<String>,
    pub profile_name: String,
    pub host: String,
    pub port: u16,
    pub prompts: crate::core::state::HostPromptMap,
    pub user_rejected_host_key: Arc<AtomicBool>,
    pub status_options: ConnectionStatusOptions,
}

impl russh::client::Handler for SshClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        use russh::keys::ssh_key::HashAlg;
        use tokio::sync::oneshot;

        let algorithm = server_public_key.algorithm().to_string();
        let fingerprint = server_public_key.fingerprint(HashAlg::Sha256).to_string();

        emit_connection_progress(
            &self.app,
            &self.tab_id,
            self.status_options,
            SshConnectionProgressPayload::new(
                "target_host_key_checking",
                format!(
                    "Checking target host fingerprint for {}:{}",
                    self.host, self.port
                ),
            )
            .host(self.host.clone(), self.port),
        );

        let known = match crate::ssh::store::load_known_host(
            &self.profile_name,
            self.profile_id.as_deref(),
            &self.host,
            self.port,
        ) {
            Ok(value) => value,
            Err(err) => {
                self.emit_status("31", &format!("Failed to read known host store: {err}"));
                return Ok(false);
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

        let payload = crate::ssh::store::SshHostKeyPromptPayload {
            request_id: request_id.clone(),
            profile_name: self.profile_name.clone(),
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
            "Waiting for user confirmation of SSH host fingerprint...",
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
            self.emit_status("31", HOST_KEY_REJECTED_REASON);
            return Err(russh::Error::Disconnect);
        }

        let save_result =
            crate::ssh::store::save_known_host_entry(crate::ssh::store::KnownHostRecord {
                profile_id: self.profile_id.clone(),
                profile_name: self.profile_name.clone(),
                host: self.host.clone(),
                port: self.port,
                algorithm: server_public_key.algorithm().to_string(),
                fingerprint,
                trusted_at: crate::ssh::store::now_unix_ms(),
            });

        if let Err(err) = save_result {
            self.emit_status("31", &format!("Failed to save known host: {err}"));
            return Ok(false);
        }

        Ok(true)
    }
}

impl SshClientHandler {
    fn emit_status(&self, color: &str, message: &str) {
        if !self.status_options.emit_terminal_output {
            return;
        }

        let payload = format!("\r\n\x1b[{}m[{}]\x1b[0m\r\n", color, message);
        let event_name = format!("pty-output-{}", self.tab_id);
        let _ = self
            .app
            .emit_to(tauri::EventTarget::any(), &event_name, payload);
    }
}
