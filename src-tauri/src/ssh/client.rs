use super::shell_integration::ShellIntegrationStatus;
use super::types::{
    emit_connection_progress, ConnectionStatusOptions, SshClientHandler,
    SshConnectionProgressPayload,
};
use crate::core::session::types::SessionPlan;
use crate::core::state::{ChannelContext, HostPromptMap};
use crate::zmodem::session::{ZmodemReceiveDriver, ZmodemSettings};
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

/// Forwards one output batch to the webview, converted to UTF-8 when the
/// session uses another charset.
fn deliver_output(
    app: &tauri::AppHandle,
    tab_id: &str,
    sender: Option<&crate::terminal::io_batcher::TerminalOutputSender>,
    decoder: Option<&mut crate::terminal::encoding::OutputDecoder>,
    payload: Vec<u8>,
) {
    let payload = match decoder {
        Some(decoder) => {
            let decoded = decoder.decode(&payload);
            // Logged after decoding (see `record_raw_output`).
            crate::session_log::record_output(app, tab_id, &decoded);
            decoded
        }
        None => payload,
    };
    if payload.is_empty() {
        return;
    }
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

/// Opens a session channel with a PTY, ready for a shell or command.
async fn open_terminal_channel(
    session: &russh::client::Handle<SshClientHandler>,
    plan: &SessionPlan,
    rows: u16,
    cols: u16,
) -> Result<russh::Channel<russh::client::Msg>, String> {
    let channel = session
        .channel_open_session()
        .await
        .map_err(|err| format!("Failed to open SSH channel: {err}"))?;

    if plan.agent_forward {
        // Best-effort: a server with agent forwarding disabled just won't
        // open a channel back later, which isn't worth failing the session over.
        if let Err(err) = channel.agent_forward(false).await {
            eprintln!("SSH agent forwarding request failed: {err}");
        }
    }

    channel
        .request_pty(false, "xterm-256color", cols as u32, rows as u32, 0, 0, &[])
        .await
        .map_err(|err| format!("Failed to request SSH PTY: {err}"))?;
    // Remote TUIs otherwise quantize to 256 colors. Servers only take it when
    // their AcceptEnv lists it (recent Debian and Ubuntu do), so a refusal is
    // ignored rather than asked for.
    if let Err(err) = channel.set_env(false, "COLORTERM", "truecolor").await {
        eprintln!("SSH COLORTERM request failed: {err}");
    }
    Ok(channel)
}

/// Returns the newest size the terminal reported while the connection was
/// being set up, or `size` when it reported none. Opening the PTY at that size
/// spares the shell a resize while it prints its first prompt: readline
/// redraws the prompt for the old width then, and on an older bash that
/// lands on the line above and leaves a broken copy behind.
fn latest_queued_size(
    resize_rx: &mut mpsc::UnboundedReceiver<(u16, u16)>,
    size: (u16, u16),
) -> (u16, u16) {
    let mut latest = size;
    while let Ok((rows, cols)) = resize_rx.try_recv() {
        latest = (rows, cols);
    }
    latest
}

/// The channels a connection attempt is driven through.
pub struct SshChannelIo {
    pub stop_rx: watch::Receiver<bool>,
    pub input_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    pub resize_rx: mpsc::UnboundedReceiver<(u16, u16)>,
    pub sender: Option<crate::terminal::io_batcher::TerminalOutputSender>,
}

#[expect(
    clippy::too_many_lines,
    reason = "predates the 150-line limit; split it when it next changes"
)]
pub async fn run_single_ssh_connection(
    tab: ChannelContext,
    settings: ZmodemSettings,
    plan: SessionPlan,
    prompts: HostPromptMap,
    rows: u16,
    cols: u16,
    io: SshChannelIo,
) -> SshExitSignal {
    let SshChannelIo {
        mut stop_rx,
        mut input_rx,
        mut resize_rx,
        mut sender,
    } = io;
    let app = tab.app.clone();
    let tab_id = tab.tab_id.clone();
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
    let (jump_chain, session) = match crate::ssh::jump::connect::open_target_ssh_session(
        crate::ssh::jump::connect::ConnectContext {
            app: &app,
            tab_id: &tab_id,
            prompts: &prompts,
            status_options: ConnectionStatusOptions::VERBOSE,
            host_key_verification_mode:
                crate::ssh::types::HostKeyVerificationMode::PromptAndPersist,
            auth_prompter: &crate::ssh::auth::AuthPrompter::tab(&app, &tab_id),
        },
        crate::ssh::jump::connect::SshTarget::from_plan(&plan, &host, &username),
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

    // The command that starts the shell with shell integration, and how
    // integration went for the connection header.
    let unavailable = |reason| Some(ShellIntegrationStatus::Unavailable { reason });
    let (shell_integration, mut shell_integration_status) = if !plan.shell_integration {
        (None, None)
    } else {
        let target = format!("{username}@{host}:{}", plan.port);
        let marks = crate::config::command_marks_enabled();
        match super::shell_integration::prepare(&session, &target, marks).await {
            Ok(command) => (Some(command), None),
            Err(reason) => (None, unavailable(reason)),
        }
    };

    let (mut rows, mut cols) = latest_queued_size(&mut resize_rx, (rows, cols));
    let channel_open_started_at = tokio::time::Instant::now();
    let mut channel = match open_terminal_channel(&session, &plan, rows, cols).await {
        Ok(channel) => channel,
        Err(reason) => {
            finish_output!();
            return SshExitSignal {
                terminated: false,
                recoverable: true,
                reason: Some(reason),
                connected_duration: None,
            };
        }
    };
    let network_latency_ms = channel_open_started_at
        .elapsed()
        .as_millis()
        .min(u64::MAX as u128) as u64;

    // With shell integration: what the shell printed before the server
    // confirmed it started. `None` means a plain shell.
    let mut early_output = None;
    if let Some(command) = shell_integration {
        match super::shell_integration::start(&mut channel, &command).await {
            Ok(output) => {
                early_output = Some(output);
                shell_integration_status = Some(ShellIntegrationStatus::Active);
            }
            Err(reason) => shell_integration_status = unavailable(reason),
        }
        if early_output.is_none() {
            // That channel is closed; the plain shell gets a fresh one.
            (rows, cols) = latest_queued_size(&mut resize_rx, (rows, cols));
            channel = match open_terminal_channel(&session, &plan, rows, cols).await {
                Ok(channel) => channel,
                Err(reason) => {
                    finish_output!();
                    return SshExitSignal {
                        terminated: false,
                        recoverable: true,
                        reason: Some(reason),
                        connected_duration: None,
                    };
                }
            };
        }
    }

    if early_output.is_none() {
        if let Err(err) = channel.request_shell(false).await {
            finish_output!();
            return SshExitSignal {
                terminated: false,
                recoverable: true,
                reason: Some(format!("Failed to request SSH shell: {err}")),
                connected_duration: None,
            };
        }
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
        .network_latency(Some(network_latency_ms))
        .shell_integration(shell_integration_status),
    );

    // Asked once the shell is requested, so its answer travels alongside the
    // first prompt instead of ahead of it.
    if crate::config::tab_icons_enabled() {
        super::remote_os::probe(&app, &tab_id, &session).await;
    }

    let (mut reader, writer) = channel.split();
    let mut writer_stream = writer.make_writer();
    let mut ssh_query_pending = Vec::new();
    let connected_at = std::time::Instant::now();
    let mut zmodem_driver = ZmodemReceiveDriver::new(&tab, settings);
    // Once a `rz`-style trigger arms this tab for the send direction, every
    // subsequent byte is piped here instead of through `zmodem_driver` —
    // see `ZmodemSendPipe`'s doc comment.
    let mut zmodem_send_pipe = crate::zmodem::ZmodemSendPipe::default();
    let mut output_decoder = plan.encoding.output_decoder();
    // A UTF-8 session logs output as received. A transcoded one logs it in
    // `deliver_output` instead, since bytes in a foreign charset would be
    // unreadable in a plain-text log.
    let record_raw_output = output_decoder.is_none();

    if let Some(output) = early_output.filter(|output| !output.is_empty()) {
        if record_raw_output {
            crate::session_log::record_output(&app, &tab_id, &output);
        }
        deliver_output(
            &app,
            &tab_id,
            sender.as_ref(),
            output_decoder.as_mut(),
            output,
        );
    }

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
                        if record_raw_output {
                            crate::session_log::record_output(&app, &tab_id, data.as_ref());
                        }

                        if zmodem_send_pipe.try_forward(data.as_ref()) {
                            continue;
                        }

                        // Process terminal queries (vim t_u7, t_RV, etc.) before forwarding to UI
                        // This is critical for SSH connections where vim waits for responses
                        let processed = crate::terminal::ssh_query_handler::process_ssh_output_for_ui(
                            data.as_ref(),
                            &mut ssh_query_pending,
                            &mut writer_stream,
                        ).await;

                        let bytes = match processed {
                            Ok(bytes) => bytes,
                            Err(e) => {
                                // Fallback: use raw data if processing fails
                                eprintln!("SSH output processing failed: {}", e);
                                data.as_ref().to_vec()
                            }
                        };
                        if !bytes.is_empty() {
                            let outcome = zmodem_driver.process(&bytes);
                            if !outcome.outgoing.is_empty() {
                                if let Err(err) = writer_stream.write_all(&outcome.outgoing).await {
                                    finish_output!();
                                    return SshExitSignal {
                                        terminated: false,
                                        recoverable: true,
                                        reason: Some(format!("SSH write failed: {err}")),
                                        connected_duration: Some(connected_at.elapsed()),
                                    };
                                }
                            }
                            if let Some(remainder) = outcome.send_requested {
                                let tx = crate::zmodem::arm_send(&tab, remainder);
                                zmodem_send_pipe.arm(tx);
                                continue;
                            }
                            if !outcome.passthrough.is_empty() {
                                deliver_output(
                                    &app,
                                    &tab_id,
                                    sender.as_ref(),
                                    output_decoder.as_mut(),
                                    outcome.passthrough,
                                );
                            }
                        }
                    }
                    Some(ChannelMsg::ExtendedData { data, .. }) => {
                        if record_raw_output {
                            crate::session_log::record_output(&app, &tab_id, data.as_ref());
                        }
                        deliver_output(
                            &app,
                            &tab_id,
                            sender.as_ref(),
                            output_decoder.as_mut(),
                            data.as_ref().to_vec(),
                        );
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

#[cfg(test)]
mod tests {
    use super::latest_queued_size;
    use tokio::sync::mpsc;

    #[test]
    fn latest_queued_size_keeps_the_newest_resize() {
        let (resize_tx, mut resize_rx) = mpsc::unbounded_channel();
        assert_eq!(latest_queued_size(&mut resize_rx, (24, 80)), (24, 80));

        resize_tx.send((30, 100)).unwrap();
        resize_tx.send((40, 120)).unwrap();
        assert_eq!(latest_queued_size(&mut resize_rx, (24, 80)), (40, 120));
        assert!(resize_rx.try_recv().is_err());
    }
}
