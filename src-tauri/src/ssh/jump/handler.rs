use crate::core::state::HostPromptMap;
use crate::ssh::store::{
    load_known_host, now_unix_ms, save_known_host_entry, KnownHostRecord, SshHostKeyPromptPayload,
};
use crate::ssh::types::{
    emit_connection_progress, ConnectionStatusOptions, HostKeyVerificationMode,
    SshConnectionProgressPayload, HOST_KEY_PROMPT_TIMEOUT, HOST_KEY_REJECTED_REASON,
};
use russh::client;
use russh::keys::ssh_key::HashAlg;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;

/// russh handler for the jump host leg of the connection.
/// Performs the same host-key verification as `SshClientHandler`:
/// known_hosts lookup → user prompt on unknown/mismatch → save on approve.
pub struct JumpHostHandler {
    pub app: AppHandle,
    pub tab_id: String,
    pub host: String,
    pub port: u16,
    pub hop_index: usize,
    pub total_hops: usize,
    pub prompts: HostPromptMap,
    pub user_rejected_host_key: Arc<AtomicBool>,
    pub failure_reason: Arc<Mutex<Option<String>>>,
    pub status_options: ConnectionStatusOptions,
    pub host_key_verification_mode: HostKeyVerificationMode,
}

impl client::Handler for JumpHostHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        let algorithm = server_public_key.algorithm().to_string();
        let fingerprint = server_public_key.fingerprint(HashAlg::Sha256).to_string();

        emit_connection_progress(
            &self.app,
            &self.tab_id,
            self.status_options,
            SshConnectionProgressPayload::new(
                "jump_host_key_checking",
                format!(
                    "Checking jump host #{} fingerprint for {}:{}",
                    self.hop_index, self.host, self.port
                ),
            )
            .host(self.host.clone(), self.port)
            .hop(self.hop_index, self.total_hops),
        );

        let synthetic_name = format!("jump:{}:{}", self.host, self.port);

        let known = match load_known_host(&synthetic_name, None, &self.host, self.port) {
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

        if self.host_key_verification_mode == HostKeyVerificationMode::TrustUnknownForSession {
            if known.is_none() {
                emit_connection_progress(
                    &self.app,
                    &self.tab_id,
                    self.status_options,
                    SshConnectionProgressPayload::new(
                        "jump_host_key_trusted_for_test",
                        format!(
                            "Temporarily trusting jump host #{} fingerprint for {}:{}",
                            self.hop_index, self.host, self.port
                        ),
                    )
                    .host(self.host.clone(), self.port)
                    .hop(self.hop_index, self.total_hops),
                );
                return Ok(true);
            }

            let reason = format!(
                "Jump host #{} fingerprint changed; connect normally to review the new fingerprint.",
                self.hop_index
            );
            self.set_failure_reason(reason.clone());
            self.emit_status("31", &reason);
            return Err(russh::Error::Disconnect);
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
    pub(crate) fn set_failure_reason(&self, reason: String) {
        if let Ok(mut current) = self.failure_reason.lock() {
            *current = Some(reason);
        }
    }

    pub(crate) fn emit_status(&self, color: &str, message: &str) {
        if !self.status_options.emit_terminal_output {
            return;
        }

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
