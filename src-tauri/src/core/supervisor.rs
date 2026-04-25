use super::session::SessionPlan;
use super::state::{ActiveSession, HostPromptMap, SessionExitSignal};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, watch, Mutex as TokioMutex};

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
    _rows: u16,
    _cols: u16,
    _plan: SessionPlan,
    mut exit_rx: mpsc::UnboundedReceiver<SessionExitSignal>,
    _exit_tx: mpsc::UnboundedSender<SessionExitSignal>,
    active: Arc<TokioMutex<Option<ActiveSession>>>,
    stop_rx: watch::Receiver<bool>,
    _prompt_state: HostPromptMap,
    runtime_handle: tokio::runtime::Handle,
) -> tokio::task::JoinHandle<()> {
    runtime_handle.clone().spawn(async move {
        if let Some(signal) = exit_rx.recv().await {
            if *stop_rx.borrow() {
                return;
            }

            {
                let mut guard = active.lock().await;
                *guard = None;
            }

            // Always exit on disconnect, no automatic reconnection
            let exit_reason = if let SessionExitSignal::Recoverable(reason)
            | SessionExitSignal::NonRecoverable(reason) = &signal
            {
                emit_status_line(&app, &tab_id, "31", reason);
                Some(reason.clone())
            } else {
                None
            };
            emit_pty_exit(&app, &tab_id, exit_reason.as_deref());
        }
    })
}
