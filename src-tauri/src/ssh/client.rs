use super::types::{
    emit_connection_progress, ConnectionStatusOptions, SshClientHandler,
    SshConnectionProgressPayload,
};
use crate::core::session::SessionPlan;
use crate::core::state::HostPromptMap;
use russh::{ChannelMsg, Disconnect};
use tauri::Emitter;
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, watch};

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

pub struct SshExitSignal {
    pub terminated: bool,
    pub recoverable: bool,
    pub reason: Option<String>,
    pub connected_duration: Option<std::time::Duration>,
}

/// Runs a call that may block on the batcher's bounded queue (send, or the
/// join inside finish) off the async worker: `block_in_place` on a
/// multi-thread runtime keeps the connection's select! loop responsive.
/// `block_in_place` panics outside a multi-thread runtime, so fall back to
/// running inline on a current-thread runtime or plain thread (the sender
/// runs on its own OS thread, so this cannot deadlock).
fn run_blocking<T>(f: impl FnOnce() -> T) -> T {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread => {
            tokio::task::block_in_place(f)
        }
        _ => f(),
    }
}

/// Forwards one output batch to the webview.
fn deliver_output(
    app: &tauri::AppHandle,
    tab_id: &str,
    sender: Option<&crate::terminal::TerminalOutputSender>,
    payload: Vec<u8>,
) {
    match sender {
        Some(sender) => run_blocking(|| sender.send(payload)),
        None => {
            emit_pty_output(app, tab_id, String::from_utf8_lossy(&payload).into_owned());
        }
    }
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
    mut sender: Option<crate::terminal::TerminalOutputSender>,
) -> SshExitSignal {
    macro_rules! finish_output {
        () => {
            if let Some(sender) = sender.as_mut() {
                run_blocking(|| sender.finish());
            }
        };
    }
    let host: String = match &plan.host {
        Some(host) => host.clone(),
        None => {
            finish_output!();
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
            finish_output!();
            return SshExitSignal {
                terminated: true,
                recoverable: false,
                reason: Some("SSH username is required".to_string()),
                connected_duration: None,
            };
        }
    };

    // Use open_target_ssh_session for connection + auth (supports jump hosts)
    let (jump_chain, session) = match crate::ssh::open_target_ssh_session(
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
        plan.use_agent,
        plan.agent_forward,
        plan.keepalive_interval_secs,
        plan.keepalive_count_max,
        &plan.jump_hosts,
        prompts,
        ConnectionStatusOptions::VERBOSE,
        crate::ssh::HostKeyVerificationMode::PromptAndPersist,
    )
    .await
    {
        Ok(result) => result,
        Err(err) => {
            finish_output!();
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

    let channel_open_started_at = tokio::time::Instant::now();
    let channel = match session.channel_open_session().await {
        Ok(channel) => channel,
        Err(err) => {
            finish_output!();
            return SshExitSignal {
                terminated: false,
                recoverable: true,
                reason: Some(format!("Failed to open SSH channel: {err}")),
                connected_duration: None,
            };
        }
    };
    let network_latency_ms = channel_open_started_at
        .elapsed()
        .as_millis()
        .min(u64::MAX as u128) as u64;

    if plan.use_agent && plan.agent_forward {
        // Best-effort: a server with agent forwarding disabled just won't
        // open a channel back later, which isn't worth failing the session over.
        if let Err(err) = channel.agent_forward(false).await {
            eprintln!("SSH agent forwarding request failed: {err}");
        }
    }

    if let Err(err) = channel
        .request_pty(false, "xterm-256color", cols as u32, rows as u32, 0, 0, &[])
        .await
    {
        finish_output!();
        return SshExitSignal {
            terminated: false,
            recoverable: true,
            reason: Some(format!("Failed to request SSH PTY: {err}")),
            connected_duration: None,
        };
    }

    if let Err(err) = channel.request_shell(false).await {
        finish_output!();
        return SshExitSignal {
            terminated: false,
            recoverable: true,
            reason: Some(format!("Failed to request SSH shell: {err}")),
            connected_duration: None,
        };
    }

    emit_connection_progress(
        &app,
        &tab_id,
        ConnectionStatusOptions::VERBOSE,
        SshConnectionProgressPayload::new(
            "ready",
            format!("Connected to {}@{}:{}", username, host, plan.port),
        )
        .host(host.clone(), plan.port)
        .username(username.clone())
        .network_latency(Some(network_latency_ms)),
    );

    let (mut reader, writer) = channel.split();
    let mut writer_stream = writer.make_writer();
    let mut ssh_query_pending = Vec::new();
    let connected_at = std::time::Instant::now();

    loop {
        tokio::select! {
            _ = stop_rx.changed() => {
                if *stop_rx.borrow() {
                    let _ = writer.close().await;
                    let _ = session.disconnect(Disconnect::ByApplication, "Session closed", "en").await;
                    drop(jump_chain);
                    finish_output!();
                    return SshExitSignal {
                        terminated: true,
                        recoverable: false,
                        reason: None,
                        connected_duration: Some(connected_at.elapsed()),
                    };
                }
            }
            incoming = input_rx.recv() => {
                if let Some(data) = incoming {
                    if let Err(err) = writer_stream.write_all(&data).await {
                        finish_output!();
                        return SshExitSignal {
                            terminated: false,
                            recoverable: true,
                            reason: Some(format!("SSH write failed: {err}")),
                            connected_duration: Some(connected_at.elapsed()),
                        };
                    }
                }
            }
            resize = resize_rx.recv() => {
                if let Some((next_rows, next_cols)) = resize {
                    if let Err(err) = writer.window_change(next_cols as u32, next_rows as u32, 0, 0).await {
                        finish_output!();
                        return SshExitSignal {
                            terminated: false,
                            recoverable: true,
                            reason: Some(format!("SSH resize failed: {err}")),
                            connected_duration: Some(connected_at.elapsed()),
                        };
                    }
                }
            }
            event = reader.wait() => {
                match event {
                    Some(ChannelMsg::Data { data }) => {
                        crate::session_log::record_output(&app, &tab_id, data.as_ref());
                        // Process terminal queries (vim t_u7, t_RV, etc.) before forwarding to UI
                        // This is critical for SSH connections where vim waits for responses
                        let processed = crate::terminal::process_ssh_output_for_ui(
                            data.as_ref(),
                            &mut ssh_query_pending,
                            &mut writer_stream,
                        ).await;

                        match processed {
                            Ok(bytes) => {
                                if !bytes.is_empty() {
                                    deliver_output(&app, &tab_id, sender.as_ref(), bytes);
                                }
                            }
                            Err(e) => {
                                // Fallback: send raw data if processing fails
                                eprintln!("SSH output processing failed: {}", e);
                                deliver_output(&app, &tab_id, sender.as_ref(), data.as_ref().to_vec());
                            }
                        }
                    }
                    Some(ChannelMsg::ExtendedData { data, .. }) => {
                        crate::session_log::record_output(&app, &tab_id, data.as_ref());
                        deliver_output(&app, &tab_id, sender.as_ref(), data.as_ref().to_vec());
                    }
                    Some(ChannelMsg::ExitStatus { .. }) => {
                        let _ = session.disconnect(Disconnect::ByApplication, "Shell exited", "en").await;
                        drop(jump_chain);
                        finish_output!();
                        return SshExitSignal {
                            terminated: true,
                            recoverable: false,
                            reason: None,
                            connected_duration: Some(connected_at.elapsed()),
                        };
                    }
                    Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => {
                        finish_output!();
                        return SshExitSignal {
                            terminated: false,
                            recoverable: true,
                            reason: Some("SSH channel closed".to_string()),
                            connected_duration: Some(connected_at.elapsed()),
                        };
                    }
                    _ => {}
                }
            }
        }
    }
}
