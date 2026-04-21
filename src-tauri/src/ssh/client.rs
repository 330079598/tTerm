use super::store::{
    load_known_host, save_known_host_entry, now_unix_ms, KnownHostRecord,
    SshHostKeyPromptPayload,
};
use super::types::{HOST_KEY_PROMPT_TIMEOUT, HOST_KEY_REJECTED_REASON, SshClientHandler};
use crate::core::session::SessionPlan;
use crate::core::state::HostPromptMap;
use russh::client;
use russh::keys::ssh_key::HashAlg;
use russh::{ChannelMsg, Disconnect};
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, oneshot, watch};

pub struct SshExitSignal {
    pub terminated: bool,
    pub recoverable: bool,
    pub reason: Option<String>,
}

impl russh::client::Handler for SshClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        let algorithm = server_public_key.algorithm().to_string();
        let fingerprint = server_public_key.fingerprint(HashAlg::Sha256).to_string();

        let known = match load_known_host(&self.profile_name, self.profile_id.as_deref()) {
            Ok(value) => value,
            Err(err) => {
                emit_status_line(
                    &self.app,
                    &self.tab_id,
                    "31",
                    &format!("Failed to read known host store: {err}"),
                );
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

        let payload = SshHostKeyPromptPayload {
            request_id: request_id.clone(),
            profile_name: self.profile_name.clone(),
            host: self.host.clone(),
            port: self.port,
            algorithm,
            fingerprint: fingerprint.clone(),
            reason: reason.clone(),
            known_fingerprint,
        };

        let event_name = format!("ssh-hostkey-prompt-{}", self.tab_id);
        let _ = self
            .app
            .emit_to(tauri::EventTarget::any(), &event_name, payload);

        emit_status_line(
            &self.app,
            &self.tab_id,
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
            emit_status_line(&self.app, &self.tab_id, "31", HOST_KEY_REJECTED_REASON);
            return Err(russh::Error::Disconnect);
        }

        let save_result = save_known_host_entry(KnownHostRecord {
            profile_id: self.profile_id.clone(),
            profile_name: self.profile_name.clone(),
            host: self.host.clone(),
            port: self.port,
            algorithm: server_public_key.algorithm().to_string(),
            fingerprint,
            trusted_at: now_unix_ms(),
        });

        if let Err(err) = save_result {
            emit_status_line(
                &self.app,
                &self.tab_id,
                "31",
                &format!("Failed to save known host: {err}"),
            );
            return Ok(false);
        }

        Ok(true)
    }
}

fn emit_status_line(app: &AppHandle, tab_id: &str, color: &str, message: &str) {
    let payload = format!("\r\n\x1b[{}m[{}]\x1b[0m\r\n", color, message);
    emit_pty_output(app, tab_id, payload);
}

fn emit_pty_output(app: &AppHandle, tab_id: &str, payload: String) {
    let event_name = format!("pty-output-{}", tab_id);
    let _ = app.emit_to(tauri::EventTarget::any(), &event_name, payload);
}

pub async fn run_single_ssh_connection(
    app: AppHandle,
    tab_id: String,
    rows: u16,
    cols: u16,
    plan: SessionPlan,
    prompts: HostPromptMap,
    mut stop_rx: watch::Receiver<bool>,
    mut input_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    mut resize_rx: mpsc::UnboundedReceiver<(u16, u16)>,
) -> SshExitSignal {
    let host: String = match &plan.host {
        Some(host) => host.clone(),
        None => return SshExitSignal {
            terminated: true,
            recoverable: false,
            reason: None,
        },
    };
    let username: String = match &plan.username {
        Some(username) => username.clone(),
        None => return SshExitSignal {
            terminated: true,
            recoverable: false,
            reason: None,
        },
    };
    let private_key_path = plan.private_key_path.clone();
    let private_key_passphrase = plan.private_key_passphrase.clone();
    let password: String = if private_key_path.is_none() {
        match &plan.password {
            Some(password) => password.clone(),
            None => return SshExitSignal {
                terminated: true,
                recoverable: false,
                reason: None,
            },
        }
    } else {
        String::new()
    };

    let mut config = client::Config::default();
    config.keepalive_interval = Some(Duration::from_secs(plan.keepalive_interval_secs as u64));
    config.keepalive_max = plan.keepalive_count_max as usize;

    let handler = SshClientHandler {
        app: app.clone(),
        tab_id: tab_id.clone(),
        profile_id: plan.profile_id.clone(),
        profile_name: plan.profile_name.clone(),
        host: host.clone(),
        port: plan.port,
        prompts,
        user_rejected_host_key: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    };

    let host_key_rejected_flag = handler.user_rejected_host_key.clone();

    let mut session = match client::connect(
        std::sync::Arc::new(config),
        (host.as_str(), plan.port),
        handler,
    )
    .await
    {
        Ok(session) => session,
        Err(err) => {
            if host_key_rejected_flag.load(Ordering::Relaxed) {
                return SshExitSignal {
                    terminated: true,
                    recoverable: false,
                    reason: Some(HOST_KEY_REJECTED_REASON.to_string()),
                };
            }
            return SshExitSignal {
                terminated: false,
                recoverable: true,
                reason: Some(format!("SSH connect failed: {err}")),
            };
        }
    };

    let auth_result = if let Some(key_path) = private_key_path {
        let key_path = std::path::Path::new(&key_path);
        let key_pair =
            match russh::keys::load_secret_key(key_path, private_key_passphrase.as_deref()) {
                Ok(kp) => kp,
                Err(err) => {
                    return SshExitSignal {
                        terminated: true,
                        recoverable: false,
                        reason: Some(format!("Failed to load SSH key: {err}")),
                    };
                }
            };
        match session
            .authenticate_publickey(
                username,
                russh::keys::PrivateKeyWithHashAlg::new(std::sync::Arc::new(key_pair), None),
            )
            .await
        {
            Ok(result) => result,
            Err(err) => {
                return SshExitSignal {
                    terminated: true,
                    recoverable: false,
                    reason: Some(format!("SSH key authentication failed: {err}")),
                };
            }
        }
    } else {
        match session.authenticate_password(username, password).await {
            Ok(result) => result,
            Err(err) => {
                return SshExitSignal {
                    terminated: true,
                    recoverable: false,
                    reason: Some(format!("SSH authentication failed: {err}")),
                };
            }
        }
    };

    if !auth_result.success() {
        emit_status_line(&app, &tab_id, "31", "SSH authentication failed");
        let _ = session
            .disconnect(Disconnect::ByApplication, "Authentication failed", "en")
            .await;
        return SshExitSignal {
            terminated: true,
            recoverable: false,
            reason: Some("SSH authentication failed".to_string()),
        };
    }

    let channel = match session.channel_open_session().await {
        Ok(channel) => channel,
        Err(err) => {
            return SshExitSignal {
                terminated: false,
                recoverable: true,
                reason: Some(format!("Failed to open SSH channel: {err}")),
            };
        }
    };

    if let Err(err) = channel
        .request_pty(false, "xterm-256color", cols as u32, rows as u32, 0, 0, &[])
        .await
    {
        return SshExitSignal {
            terminated: false,
            recoverable: true,
            reason: Some(format!("Failed to request SSH PTY: {err}")),
        };
    }

    if let Err(err) = channel.request_shell(false).await {
        return SshExitSignal {
            terminated: false,
            recoverable: true,
            reason: Some(format!("Failed to request SSH shell: {err}")),
        };
    }

    let (mut reader, writer) = channel.split();
    let mut writer_stream = writer.make_writer();
    let mut ssh_query_pending = Vec::new();

    loop {
        tokio::select! {
            _ = stop_rx.changed() => {
                if *stop_rx.borrow() {
                    let _ = writer.close().await;
                    let _ = session.disconnect(Disconnect::ByApplication, "Session closed", "en").await;
                    return SshExitSignal {
                        terminated: true,
                        recoverable: false,
                        reason: None,
                    };
                }
            }
            incoming = input_rx.recv() => {
                if let Some(data) = incoming {
                    if let Err(err) = writer_stream.write_all(&data).await {
                        return SshExitSignal {
                            terminated: false,
                            recoverable: true,
                            reason: Some(format!("SSH write failed: {err}")),
                        };
                    }
                }
            }
            resize = resize_rx.recv() => {
                if let Some((next_rows, next_cols)) = resize {
                    if let Err(err) = writer.window_change(next_cols as u32, next_rows as u32, 0, 0).await {
                        return SshExitSignal {
                            terminated: false,
                            recoverable: true,
                            reason: Some(format!("SSH resize failed: {err}")),
                        };
                    }
                }
            }
            event = reader.wait() => {
                match event {
                    Some(ChannelMsg::Data { data }) => {
                        // Process terminal queries (vim t_u7, t_RV, etc.) before forwarding to UI
                        // This is critical for SSH connections where vim waits for responses
                        let processed = crate::terminal::process_ssh_output_for_ui(
                            data.as_ref(),
                            &mut ssh_query_pending,
                            &mut writer_stream,
                        ).await;
                        
                        match processed {
                            Ok(text) => {
                                if !text.is_empty() {
                                    emit_pty_output(&app, &tab_id, text);
                                }
                            }
                            Err(e) => {
                                // Fallback: send raw data if processing fails
                                eprintln!("SSH output processing failed: {}", e);
                                let text = String::from_utf8_lossy(data.as_ref()).to_string();
                                emit_pty_output(&app, &tab_id, text);
                            }
                        }
                    }
                    Some(ChannelMsg::ExtendedData { data, .. }) => {
                        let text = String::from_utf8_lossy(data.as_ref()).to_string();
                        emit_pty_output(&app, &tab_id, text);
                    }
                    Some(ChannelMsg::ExitStatus { .. }) => {
                        let _ = session.disconnect(Disconnect::ByApplication, "Shell exited", "en").await;
                        return SshExitSignal {
                            terminated: true,
                            recoverable: false,
                            reason: None,
                        };
                    }
                    Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => {
                        return SshExitSignal {
                            terminated: false,
                            recoverable: true,
                            reason: Some("SSH channel closed".to_string()),
                        };
                    }
                    _ => {}
                }
            }
        }
    }
}
