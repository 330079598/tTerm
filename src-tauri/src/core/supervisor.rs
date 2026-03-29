use super::session::{next_backoff_delay, SessionPlan};
use super::state::{ActiveSession, ActiveSsh, HostPromptMap, SessionExitSignal};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, watch, Mutex as TokioMutex};

const HOST_KEY_REJECTED_REASON: &str = "SSH host fingerprint rejected by user";

pub fn emit_pty_exit(app: &AppHandle, tab_id: &str, reason: Option<&str>) {
    let event_name = format!("pty-exit-{}", tab_id);
    let _ = app.emit_to(tauri::EventTarget::any(), &event_name, reason);
}

fn emit_status_line(app: &AppHandle, tab_id: &str, color: &str, message: &str) {
    let payload = format!("\r\n\x1b[{}m[{}]\x1b[0m\r\n", color, message);
    emit_pty_output(app, tab_id, payload);
}

fn emit_pty_output(app: &AppHandle, tab_id: &str, payload: String) {
    let event_name = format!("pty-output-{}", tab_id);
    let _ = app.emit_to(tauri::EventTarget::any(), &event_name, payload);
}

pub fn spawn_supervisor(
    app: AppHandle,
    tab_id: String,
    rows: u16,
    cols: u16,
    plan: SessionPlan,
    mut exit_rx: mpsc::UnboundedReceiver<SessionExitSignal>,
    exit_tx: mpsc::UnboundedSender<SessionExitSignal>,
    active: Arc<TokioMutex<Option<ActiveSession>>>,
    mut stop_rx: watch::Receiver<bool>,
    prompt_state: HostPromptMap,
    runtime_handle: tokio::runtime::Handle,
) -> tokio::task::JoinHandle<()> {
    runtime_handle.clone().spawn(async move {
        let mut attempt: u32 = 0;
        let mut delay = plan.reconnect_initial_delay;

        while let Some(signal) = exit_rx.recv().await {
            if *stop_rx.borrow() {
                break;
            }

            let host_key_rejected = matches!(
                &signal,
                SessionExitSignal::NonRecoverable(reason) if reason == HOST_KEY_REJECTED_REASON
            );

            {
                let mut guard = active.lock().await;
                *guard = None;
            }

            let should_reconnect = (matches!(signal, SessionExitSignal::Recoverable(_))
                || host_key_rejected)
                && matches!(plan.kind, crate::core::SessionKind::Ssh)
                && plan.reconnect;

            if !should_reconnect {
                let exit_reason = if let SessionExitSignal::Recoverable(reason)
                | SessionExitSignal::NonRecoverable(reason) = &signal
                {
                    emit_status_line(&app, &tab_id, "31", reason);
                    Some(reason.clone())
                } else {
                    None
                };
                emit_pty_exit(&app, &tab_id, exit_reason.as_deref());
                break;
            }

            if let Some(max_retries) = plan.reconnect_max_retries {
                if attempt >= max_retries {
                    emit_status_line(
                        &app,
                        &tab_id,
                        "31",
                        "SSH reconnect exhausted retry budget",
                    );
                    emit_pty_exit(
                        &app,
                        &tab_id,
                        Some("SSH reconnect exhausted retry budget"),
                    );
                    break;
                }
            }

            attempt += 1;

            emit_status_line(
                &app,
                &tab_id,
                "33",
                &format!(
                    "SSH disconnected. Reconnect attempt {attempt} in {}s...",
                    delay.as_secs()
                ),
            );

            loop {
                tokio::select! {
                    _ = stop_rx.changed() => {
                        return;
                    }
                    _ = tokio::time::sleep(delay) => {
                        if *stop_rx.borrow() {
                            return;
                        }

                        match spawn_process(
                            &app,
                            &tab_id,
                            rows,
                            cols,
                            &plan,
                            stop_rx.clone(),
                            exit_tx.clone(),
                            prompt_state.clone(),
                            &runtime_handle,
                        ) {
                            Ok(new_active) => {
                                let mut guard = active.lock().await;
                                *guard = Some(new_active);
                                emit_status_line(&app, &tab_id, "32", "SSH reconnected");
                                if !host_key_rejected {
                                    attempt = 0;
                                    delay = plan.reconnect_initial_delay;
                                }
                                break;
                            }
                            Err(err) => {
                                delay = next_backoff_delay(delay, plan.reconnect_max_delay);
                                emit_status_line(
                                    &app,
                                    &tab_id,
                                    "31",
                                    &format!("Reconnect failed: {}", err),
                                );
                            }
                        }
                    }
                }
            }
        }
    })
}

fn spawn_process(
    app: &AppHandle,
    tab_id: &str,
    rows: u16,
    cols: u16,
    plan: &SessionPlan,
    stop_rx: watch::Receiver<bool>,
    exit_tx: mpsc::UnboundedSender<SessionExitSignal>,
    prompt_state: HostPromptMap,
    runtime_handle: &tokio::runtime::Handle,
) -> Result<ActiveSession, String> {
    let (input_tx, _input_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (resize_tx, _resize_rx) = mpsc::unbounded_channel::<(u16, u16)>();

    let _app_clone = app.clone();
    let _tab_id_clone = tab_id.to_string();
    let _plan_clone = plan.clone();
    let _prompt_state_clone = prompt_state;
    let _exit_tx_clone = exit_tx.clone();
    let _stop_rx_clone = stop_rx.clone();

    let task = runtime_handle.spawn(async move {
        let _ssh_result = crate::ssh::run_single_ssh_connection(
            _app_clone,
            _tab_id_clone,
            rows,
            cols,
            _plan_clone,
            _prompt_state_clone,
            _stop_rx_clone,
            _input_rx,
            _resize_rx,
        )
        .await;

        let signal = match _ssh_result {
            r if r.terminated && !r.recoverable => {
                if let Some(reason) = r.reason {
                    if reason == HOST_KEY_REJECTED_REASON {
                        SessionExitSignal::NonRecoverable(reason)
                    } else {
                        SessionExitSignal::Terminated
                    }
                } else {
                    SessionExitSignal::Terminated
                }
            }
            r if !r.terminated && r.recoverable => {
                SessionExitSignal::Recoverable(r.reason.unwrap_or_default())
            }
            r if !r.terminated && !r.recoverable => {
                SessionExitSignal::NonRecoverable(r.reason.unwrap_or_default())
            }
            _ => SessionExitSignal::Terminated,
        };
        let _ = _exit_tx_clone.send(signal);
    });

    let active = ActiveSession::Ssh(ActiveSsh {
        input_tx,
        resize_tx,
        task,
    });

    Ok(active)
}
