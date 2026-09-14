use super::session::SessionPlan;
use super::state::{ActiveSession, ActiveSsh, HostPromptMap, SessionExitSignal, SessionKind};
use crate::ssh::{
    emit_connection_progress, ConnectionStatusOptions, SshConnectionProgressPayload, SshExitSignal,
};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, watch, Mutex as TokioMutex};

/// Nominal delay before the first reconnect attempt.
const RECONNECT_INITIAL_DELAY_SECS: u64 = 1;
/// Upper bound for the (pre-jitter) backoff delay.
const RECONNECT_MAX_DELAY_SECS: u64 = 30;
/// A session that stayed up at least this long resets the attempt counter, so
/// one flappy outage cannot exhaust the retry budget of a later real drop.
const RECONNECT_STABLE_RESET: Duration = Duration::from_secs(60);

pub fn emit_pty_exit(app: &AppHandle, tab_id: &str, reason: Option<&str>) {
    let event_name = format!("pty-exit-{}", tab_id);
    let _ = app.emit_to(tauri::EventTarget::any(), &event_name, reason);
}

fn emit_status_line(app: &AppHandle, tab_id: &str, color: &str, message: &str) {
    let payload = format!("\r\n\x1b[{}m[{}]\x1b[0m\r\n", color, message);
    emit_pty_output(app, tab_id, payload);
}

fn emit_pty_output(app: &AppHandle, tab_id: &str, payload: String) {
    crate::session_log::record_output(app, tab_id, payload.as_bytes());
    let event_name = format!("pty-output-{}", tab_id);
    let _ = app.emit_to(tauri::EventTarget::any(), &event_name, payload);
}

/// Map the raw SSH task outcome onto the supervisor's exit taxonomy.
pub(crate) fn map_ssh_exit_signal(result: SshExitSignal) -> SessionExitSignal {
    match (result.terminated, result.recoverable) {
        (false, true) => SessionExitSignal::Recoverable {
            reason: result.reason.unwrap_or_default(),
            connected_duration: result.connected_duration,
        },
        (false, false) => SessionExitSignal::NonRecoverable(result.reason.unwrap_or_default()),
        _ => SessionExitSignal::Terminated,
    }
}

/// Spawn one SSH connection attempt, returning the live handle for `active`.
/// Used for the first attempt and again for every automatic reconnect — the
/// output `Channel` is cloned per attempt because `TerminalOutputSender` closes
/// its worker on drop.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_ssh_attempt(
    app: AppHandle,
    tab_id: String,
    rows: u16,
    cols: u16,
    plan: SessionPlan,
    prompt_state: HostPromptMap,
    stop_rx: watch::Receiver<bool>,
    exit_tx: mpsc::UnboundedSender<SessionExitSignal>,
    output_channel: tauri::ipc::Channel<tauri::ipc::InvokeResponseBody>,
    runtime_handle: tokio::runtime::Handle,
) -> ActiveSsh {
    let (input_tx, input_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (resize_tx, resize_rx) = mpsc::unbounded_channel::<(u16, u16)>();
    let sender = crate::terminal::TerminalOutputSender::spawn(&tab_id, output_channel);

    let task = runtime_handle.spawn(async move {
        let result = crate::ssh::run_single_ssh_connection(
            app,
            tab_id,
            rows,
            cols,
            plan,
            prompt_state,
            stop_rx,
            input_rx,
            resize_rx,
            Some(sender),
        )
        .await;
        let _ = exit_tx.send(map_ssh_exit_signal(result));
    });

    ActiveSsh {
        input_tx,
        resize_tx,
        task,
    }
}

/// Nominal (pre-jitter) backoff before the 1-indexed `attempt`-th reconnect:
/// 1, 2, 4, 8, 16, 30, 30, …
fn reconnect_nominal_secs(attempt: u32) -> u64 {
    let exponent = attempt.saturating_sub(1).min(31);
    (RECONNECT_INITIAL_DELAY_SECS << exponent).min(RECONNECT_MAX_DELAY_SECS)
}

/// Equal jitter: keep half the nominal delay fixed and randomize the rest, so
/// retries spread out without ever collapsing to a near-zero sleep.
fn equal_jitter(nominal_secs: u64) -> Duration {
    let nominal_ms = nominal_secs.saturating_mul(1000);
    let half_ms = nominal_ms / 2;
    let spread_ms = nominal_ms - half_ms;
    let extra_ms = if spread_ms > 0 {
        rand::random::<u64>() % (spread_ms + 1)
    } else {
        0
    };
    Duration::from_millis(half_ms + extra_ms)
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_supervisor(
    app: AppHandle,
    tab_id: String,
    plan: SessionPlan,
    mut exit_rx: mpsc::UnboundedReceiver<SessionExitSignal>,
    exit_tx: mpsc::UnboundedSender<SessionExitSignal>,
    active: Arc<TokioMutex<Option<ActiveSession>>>,
    mut stop_rx: watch::Receiver<bool>,
    prompt_state: HostPromptMap,
    size: Arc<super::state::AtomicTerminalSize>,
    output_channel: tauri::ipc::Channel<tauri::ipc::InvokeResponseBody>,
    runtime_handle: tokio::runtime::Handle,
) -> tokio::task::JoinHandle<()> {
    runtime_handle.clone().spawn(async move {
        let reconnect_enabled = plan.reconnect_enabled && plan.kind == SessionKind::Ssh;
        let max_attempts = plan.reconnect_max_attempts;
        let mut attempt: u32 = 0;

        while let Some(signal) = exit_rx.recv().await {
            if *stop_rx.borrow() {
                break;
            }

            {
                let mut guard = active.lock().await;
                *guard = None;
            }

            let recoverable_info = match &signal {
                SessionExitSignal::Recoverable {
                    reason,
                    connected_duration,
                } => Some((reason.clone(), *connected_duration)),
                _ => None,
            };

            let should_retry = reconnect_enabled && recoverable_info.is_some();
            if !should_retry {
                let exit_reason = match &signal {
                    SessionExitSignal::Recoverable { reason, .. }
                    | SessionExitSignal::NonRecoverable(reason) => {
                        emit_status_line(&app, &tab_id, "31", reason);
                        Some(reason.clone())
                    }
                    SessionExitSignal::Terminated => None,
                };
                emit_pty_exit(&app, &tab_id, exit_reason.as_deref());
                crate::session_log::end_session(&app, &tab_id);
                break;
            }

            let (reason, connected_duration) = recoverable_info.unwrap_or_default();
            // Reset the exponential backoff attempt counter only when the connection
            // was established and stayed up for at least RECONNECT_STABLE_RESET.
            if connected_duration.unwrap_or_default() >= RECONNECT_STABLE_RESET {
                attempt = 0;
            }
            attempt += 1;

            // Retry budget exhausted: report the disconnect. The localized
            // terminal line comes from the frontend via the progress event, so
            // no status line is written here.
            if attempt > max_attempts {
                emit_connection_progress(
                    &app,
                    &tab_id,
                    ConnectionStatusOptions::VERBOSE,
                    SshConnectionProgressPayload::new(
                        "retry_exhausted",
                        format!("Reconnect attempts exhausted ({reason})"),
                    )
                    .reason(reason.clone())
                    .retry_max(max_attempts),
                );
                emit_pty_exit(&app, &tab_id, Some(&reason));
                crate::session_log::end_session(&app, &tab_id);
                break;
            }

            let nominal_secs = reconnect_nominal_secs(attempt);
            let delay = equal_jitter(nominal_secs);
            let delay_secs = delay.as_secs_f64();
            // Structured retry status: the frontend renders the localized
            // message from these fields instead of the English `message`.
            emit_connection_progress(
                &app,
                &tab_id,
                ConnectionStatusOptions::VERBOSE,
                SshConnectionProgressPayload::new(
                    "retrying",
                    format!("Reconnecting in {delay_secs:.0}s (attempt {attempt})"),
                )
                .reason(reason)
                .retry_attempt(attempt, delay_secs)
                .retry_max(max_attempts),
            );

            tokio::select! {
                _ = tokio::time::sleep(delay) => {}
                _ = stop_rx.changed() => {}
            }
            if *stop_rx.borrow() {
                break;
            }

            let (rows, cols) = size.load();
            let next = spawn_ssh_attempt(
                app.clone(),
                tab_id.clone(),
                rows,
                cols,
                plan.clone(),
                prompt_state.clone(),
                stop_rx.clone(),
                exit_tx.clone(),
                output_channel.clone(),
                runtime_handle.clone(),
            );
            {
                let mut guard = active.lock().await;
                *guard = Some(ActiveSession::Ssh(next));
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{equal_jitter, reconnect_nominal_secs};

    #[test]
    fn nominal_backoff_doubles_then_caps() {
        let schedule: Vec<u64> = (1..=8).map(reconnect_nominal_secs).collect();
        assert_eq!(schedule, vec![1, 2, 4, 8, 16, 30, 30, 30]);
    }

    #[test]
    fn nominal_backoff_never_exceeds_cap_for_large_attempts() {
        assert_eq!(reconnect_nominal_secs(1_000), 30);
        assert_eq!(reconnect_nominal_secs(u32::MAX), 30);
    }

    #[test]
    fn equal_jitter_stays_within_the_nominal_window() {
        for nominal in 1..=30u64 {
            for _ in 0..200 {
                let millis = equal_jitter(nominal).as_millis() as u64;
                assert!(
                    millis >= nominal * 500 && millis <= nominal * 1000,
                    "delay {millis}ms outside [{}, {}] for nominal {nominal}s",
                    nominal * 500,
                    nominal * 1000
                );
            }
        }
    }
}
