use tterm_core::events::ssh::SshConnectionProgressPayload;
use tterm_core::events::ssh::SshHostKeyPromptPayload;

use crate::ssh::context::ConnectionContext;
use crate::ssh::openssh_known_hosts::verify::KnownHostVerdict;
use crate::ssh::openssh_known_hosts::{check_presented_key, listed_record};
use crate::ssh::types::{
    ConnectionStatusOptions, HostKeyVerificationMode, SshConnectError, emit_connection_progress,
};
use russh::client;
use russh::keys::ssh_key::HashAlg;
use std::sync::{Arc, Mutex};
use tterm_core::ssh::store::{
    JUMP_HOST_PROFILE_PREFIX, KnownHostRecord, load_known_host, now_unix_ms, save_known_host_entry,
};

/// russh handler for the jump host leg of the connection.
/// Performs the same host-key verification as `SshClientHandler`:
/// known_hosts lookup → user prompt on unknown/mismatch → save on approve.
pub struct JumpHostHandler {
    pub app: ConnectionContext,
    pub tab_id: String,
    pub host: String,
    pub port: u16,
    pub hop_index: usize,
    pub total_hops: usize,
    pub prompts: std::sync::Arc<tterm_core::prompts::PromptBroker>,
    pub prompt_owner: tterm_core::prompts::PromptOwner,
    pub host_key_failure: Arc<Mutex<Option<SshConnectError>>>,
    pub failure_reason: Arc<Mutex<Option<String>>>,
    pub status_options: ConnectionStatusOptions,
    pub host_key_verification_mode: HostKeyVerificationMode,
}

impl client::Handler for JumpHostHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        // What `ssh` trusts or refuses decides first; a host certificate is
        // only asked for when `known_hosts` trusts an authority for the host.
        let (server_public_key, listed_fingerprint) =
            match check_presented_key(&self.host, self.port, None, server_public_key) {
                KnownHostVerdict::Trusted => return Ok(true),
                KnownHostVerdict::Rejected(reason) => {
                    self.set_failure_reason(reason.clone());
                    self.emit_status("31", &reason);
                    return Ok(false);
                }
                KnownHostVerdict::Undecided {
                    key,
                    listed_fingerprint,
                } => (key, listed_fingerprint),
            };
        let algorithm = server_public_key.algorithm().to_string();
        let fingerprint = server_public_key.fingerprint(HashAlg::Sha256).to_string();

        emit_connection_progress(
            self.app.events.as_ref(),
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

        let synthetic_name = format!("{JUMP_HOST_PROFILE_PREFIX}{}:{}", self.host, self.port);

        let known = match load_known_host(&synthetic_name, None, &self.host, self.port) {
            Ok(value) => value,
            Err(err) => {
                let reason = format!("Failed to read jump host known_hosts store: {err}");
                self.set_failure_reason(reason.clone());
                self.emit_status("31", &reason);
                return Err(russh::Error::Disconnect);
            }
        };
        let known = known.or_else(|| {
            listed_fingerprint
                .map(|listed| listed_record(&self.host, self.port, &algorithm, listed))
        });

        if let Some(record) = &known
            && record.host == self.host
            && record.port == self.port
            && record.fingerprint == fingerprint
        {
            return Ok(true);
        }

        if self.host_key_verification_mode == HostKeyVerificationMode::TrustUnknownForSession {
            if known.is_none() {
                emit_connection_progress(
                    self.app.events.as_ref(),
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

        let wait =
            crate::prompt_scope::host_key(&self.prompts, &self.prompt_owner, &self.tab_id, payload);
        crate::prompt_scope::reveal(self.app.window.as_ref());

        self.emit_status(
            "33",
            &format!(
                "Waiting for user confirmation of jump host #{} fingerprint...",
                self.hop_index
            ),
        );

        if let Err(error) = crate::ssh::host_key_prompt::resolve(wait, &self.host_key_failure).await
        {
            self.set_failure_reason(error.to_string());
            self.emit_status("31", error.message());
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

        let payload =
            crate::terminal::legacy_events::jump_status_line(color, self.hop_index, message);
        crate::session_log::record_output(&self.app.log, &self.tab_id, payload.as_bytes());
        crate::terminal::legacy_events::emit_output(
            self.app.events.as_ref(),
            &self.tab_id,
            payload,
        );
    }
}
