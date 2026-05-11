use super::types::HOST_KEY_REJECTED_REASON;
use crate::core::session::SessionPlan;
use crate::core::state::HostPromptMap;
use russh::{ChannelMsg, Disconnect};
use tauri::Emitter;
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, watch};

pub struct SshExitSignal {
    pub terminated: bool,
    pub recoverable: bool,
    pub reason: Option<String>,
}

fn emit_pty_output(app: &tauri::AppHandle, tab_id: &str, payload: String) {
    let event_name = format!("pty-output-{}", tab_id);
    let _ = app.emit_to(tauri::EventTarget::any(), &event_name, payload);
}

pub async fn run_single_ssh_connection(
    app: tauri::AppHandle,
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
        None => {
            return SshExitSignal {
                terminated: true,
                recoverable: false,
                reason: None,
            }
        }
    };
    let username: String = match &plan.username {
        Some(username) => username.clone(),
        None => {
            return SshExitSignal {
                terminated: true,
                recoverable: false,
                reason: None,
            }
        }
    };

    // Use open_target_ssh_session for connection + auth (supports jump hosts)
    let (jump_session, session) = match crate::ssh::open_target_ssh_session(
        &app,
        &tab_id,
        plan.profile_id.as_deref(),
        &plan.profile_name,
        &host,
        plan.port,
        &username,
        plan.private_key_path.as_deref(),
        plan.private_key_passphrase.as_deref(),
        plan.password.as_deref(),
        plan.keepalive_interval_secs,
        plan.keepalive_count_max,
        plan.jump_host.as_ref(),
        prompts,
    )
    .await
    {
        Ok(result) => result,
        Err(err) => {
            if err == HOST_KEY_REJECTED_REASON {
                return SshExitSignal {
                    terminated: true,
                    recoverable: false,
                    reason: Some(err),
                };
            }
            return SshExitSignal {
                terminated: false,
                recoverable: true,
                reason: Some(err),
            };
        }
    };

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
                    drop(jump_session);
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
                        drop(jump_session);
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
