use super::session::{normalize_connection, resolve_ssh_password};
use super::state::{ActiveSession, HostPromptMap, PtyMap, PtySession, SessionExitSignal};
use super::supervisor::spawn_supervisor;
use crate::terminal;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::sync::Arc;
use tauri::{AppHandle, State};
use tokio::sync::{mpsc, watch, Mutex as TokioMutex};

fn stop_pty_session(session: PtySession) {
    let _ = session.stop_tx.send(true);

    if let Some(active) = session.active.blocking_lock().take() {
        match active {
            ActiveSession::Local(mut local) => {
                let _ = local.child.kill();
            }
            ActiveSession::Ssh(ssh) => {
                ssh.task.abort();
            }
        }
    }

    session.supervisor.abort();
}

#[tauri::command]
pub fn create_pty(
    app: AppHandle,
    tab_id: String,
    session_nonce: u32,
    rows: u16,
    cols: u16,
    connection: Option<super::session::PtyConnectionOptions>,
    state: State<'_, PtyMap>,
    prompt_state: State<'_, HostPromptMap>,
    runtime_state: State<'_, crate::TokioRuntimeState>,
    secret_state: State<'_, crate::ssh::SecretStoreState>,
) -> Result<u32, String> {
    let mut plan = normalize_connection(connection)?;
    resolve_ssh_password(&app, &secret_state, &mut plan)?;

    let mut sessions = state.blocking_write();
    if let Some(session) = sessions.get(&tab_id) {
        if session.session_nonce == session_nonce {
            let mut active_guard = session.active.blocking_lock();
            if let Some(active) = active_guard.as_mut() {
                match active {
                    ActiveSession::Local(local) => {
                        let _ = local.master.resize(portable_pty::PtySize {
                            rows,
                            cols,
                            pixel_width: 0,
                            pixel_height: 0,
                        });
                    }
                    ActiveSession::Ssh(ssh) => {
                        let _ = ssh.resize_tx.send((rows, cols));
                    }
                }

                return Ok(session.pid);
            }
        }
    }

    if let Some(session) = sessions.remove(&tab_id) {
        stop_pty_session(session);
    }

    crate::session_log::start_session(&app, &tab_id, session_nonce, &plan)?;

    let (exit_tx, exit_rx) = mpsc::unbounded_channel::<SessionExitSignal>();
    let active = Arc::new(TokioMutex::new(None));
    let (stop_tx, stop_rx) = watch::channel(false);

    let runtime_handle = runtime_state.runtime.handle().clone();

    let (pid, initial_active) = match plan.kind {
        crate::core::SessionKind::Terminal => {
            let (pid, pty) = terminal::spawn_local_pty(rows, cols, plan.terminal_shell.clone())?;

            let reader = pty
                .master
                .try_clone_reader()
                .map_err(|e| format!("Failed to clone reader: {}", e))?;

            terminal::spawn_reader_thread(reader, app.clone(), tab_id.clone(), exit_tx.clone());

            (pid, ActiveSession::Local(pty))
        }
        crate::core::SessionKind::Ssh => {
            let (input_tx, _input_rx) = mpsc::unbounded_channel::<Vec<u8>>();
            let (resize_tx, _resize_rx) = mpsc::unbounded_channel::<(u16, u16)>();

            let _app_clone = app.clone();
            let _tab_id_clone = tab_id.clone();
            let _plan_clone = plan.clone();
            let _prompt_state_clone = prompt_state.inner().clone();
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
                            if reason == crate::ssh::HOST_KEY_REJECTED_REASON {
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

            let active = ActiveSession::Ssh(super::state::ActiveSsh {
                input_tx,
                resize_tx,
                task,
            });

            (0, active)
        }
    };

    {
        let mut guard = runtime_state.runtime.block_on(active.lock());
        *guard = Some(initial_active);
    }

    let supervisor = spawn_supervisor(
        app.clone(),
        tab_id.clone(),
        rows,
        cols,
        plan.clone(),
        exit_rx,
        exit_tx.clone(),
        active.clone(),
        stop_rx.clone(),
        prompt_state.inner().clone(),
        runtime_handle.clone(),
    );

    let session = PtySession {
        pid,
        session_nonce,
        active,
        stop_tx,
        supervisor,
    };

    sessions.insert(tab_id, session);

    Ok(pid)
}

#[tauri::command]
pub fn write_pty(
    app: AppHandle,
    tab_id: String,
    session_nonce: u32,
    data: String,
    state: State<'_, PtyMap>,
) -> Result<(), String> {
    let map = state.blocking_read();
    let session = map
        .get(&tab_id)
        .ok_or_else(|| format!("PTY session {} not found", tab_id))?;
    if session.session_nonce != session_nonce {
        return Ok(());
    }

    let mut active_guard = session.active.blocking_lock();
    let active = active_guard
        .as_mut()
        .ok_or_else(|| format!("PTY session {} is reconnecting", tab_id))?;

    let input = data.into_bytes();
    let result = match active {
        ActiveSession::Local(local) => local
            .writer
            .write_all(&input)
            .map_err(|e| format!("Failed to write to PTY: {}", e)),
        ActiveSession::Ssh(ssh) => ssh
            .input_tx
            .send(input.clone())
            .map_err(|_| format!("PTY session {} is not writable", tab_id)),
    };
    if result.is_ok() {
        crate::session_log::record_input(&app, &tab_id, &input);
    }
    result
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyWriteTarget {
    pub tab_id: String,
    pub session_nonce: u32,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PtyWriteStatus {
    Written,
    Stale,
    Missing,
    Reconnecting,
    Failed,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyWriteResult {
    pub tab_id: String,
    pub status: PtyWriteStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl PtyWriteResult {
    fn new(tab_id: String, status: PtyWriteStatus) -> Self {
        Self {
            tab_id,
            status,
            error: None,
        }
    }

    fn failed(tab_id: String, error: String) -> Self {
        Self {
            tab_id,
            status: PtyWriteStatus::Failed,
            error: Some(error),
        }
    }
}

enum BatchTarget {
    Ready {
        tab_id: String,
        active: Arc<TokioMutex<Option<ActiveSession>>>,
    },
    Result(PtyWriteResult),
}

fn snapshot_batch_targets(
    sessions: &HashMap<String, PtySession>,
    targets: Vec<PtyWriteTarget>,
) -> Vec<BatchTarget> {
    let mut seen = HashSet::with_capacity(targets.len());
    targets
        .into_iter()
        .filter(|target| seen.insert(target.tab_id.clone()))
        .map(|target| match sessions.get(&target.tab_id) {
            None => {
                BatchTarget::Result(PtyWriteResult::new(target.tab_id, PtyWriteStatus::Missing))
            }
            Some(session) if session.session_nonce != target.session_nonce => {
                BatchTarget::Result(PtyWriteResult::new(target.tab_id, PtyWriteStatus::Stale))
            }
            Some(session) => BatchTarget::Ready {
                tab_id: target.tab_id,
                active: session.active.clone(),
            },
        })
        .collect()
}

fn write_active_session(tab_id: String, active: &mut ActiveSession, data: &[u8]) -> PtyWriteResult {
    let write_result = match active {
        ActiveSession::Local(local) => local.writer.write_all(data),
        ActiveSession::Ssh(ssh) => ssh
            .input_tx
            .send(data.to_vec())
            .map_err(|_| std::io::Error::other("SSH input channel is closed")),
    };

    match write_result {
        Ok(()) => PtyWriteResult::new(tab_id, PtyWriteStatus::Written),
        Err(error) => PtyWriteResult::failed(tab_id, error.to_string()),
    }
}

fn write_batch_target(target: BatchTarget, data: &[u8]) -> PtyWriteResult {
    let (tab_id, active) = match target {
        BatchTarget::Result(result) => return result,
        BatchTarget::Ready { tab_id, active } => (tab_id, active),
    };
    let mut active_guard = active.blocking_lock();
    let Some(active) = active_guard.as_mut() else {
        return PtyWriteResult::new(tab_id, PtyWriteStatus::Reconnecting);
    };
    write_active_session(tab_id, active, data)
}

fn write_targets(targets: Vec<BatchTarget>, data: &[u8]) -> Vec<PtyWriteResult> {
    targets
        .into_iter()
        .map(|target| write_batch_target(target, data))
        .collect()
}

fn write_guarded_batch(
    guard_target: Option<BatchTarget>,
    targets: Vec<BatchTarget>,
    data: &[u8],
) -> Vec<PtyWriteResult> {
    let Some(guard_target) = guard_target else {
        return write_targets(targets, data);
    };
    let (tab_id, active) = match guard_target {
        BatchTarget::Result(result) => return vec![result],
        BatchTarget::Ready { tab_id, active } => (tab_id, active),
    };

    // Keep the source session active until every target write has completed.
    let mut active_guard = active.blocking_lock();
    let Some(active) = active_guard.as_mut() else {
        return vec![PtyWriteResult::new(tab_id, PtyWriteStatus::Reconnecting)];
    };
    let guard_result = write_active_session(tab_id, active, data);
    if guard_result.status != PtyWriteStatus::Written {
        return vec![guard_result];
    }

    let mut results = Vec::with_capacity(targets.len() + 1);
    results.push(guard_result);
    results.extend(write_targets(targets, data));
    results
}

#[tauri::command]
pub fn write_pty_batch(
    app: AppHandle,
    mut targets: Vec<PtyWriteTarget>,
    guard_target: Option<PtyWriteTarget>,
    data: String,
    state: State<'_, PtyMap>,
) -> Vec<PtyWriteResult> {
    if let Some(guard_tab_id) = guard_target.as_ref().map(|target| target.tab_id.as_str()) {
        targets.retain(|target| target.tab_id != guard_tab_id);
    }
    if (targets.is_empty() && guard_target.is_none()) || data.is_empty() {
        return Vec::new();
    }

    let map = state.blocking_read();
    let guard_target = guard_target.map(|target| {
        snapshot_batch_targets(&map, vec![target])
            .into_iter()
            .next()
            .expect("guard target snapshot")
    });
    let targets = snapshot_batch_targets(&map, targets);
    drop(map);

    let results = write_guarded_batch(guard_target, targets, data.as_bytes());
    for result in &results {
        if result.status == PtyWriteStatus::Written {
            crate::session_log::record_input(&app, &result.tab_id, data.as_bytes());
        }
    }
    results
}

#[cfg(test)]
mod batch_write_tests {
    use super::*;

    fn session(session_nonce: u32, runtime: &tokio::runtime::Runtime) -> PtySession {
        let (stop_tx, _) = watch::channel(false);
        PtySession {
            pid: 0,
            session_nonce,
            active: Arc::new(TokioMutex::new(None)),
            stop_tx,
            supervisor: runtime.spawn(async {}),
        }
    }

    fn writable_ssh_target(
        runtime: &tokio::runtime::Runtime,
    ) -> (
        BatchTarget,
        Arc<TokioMutex<Option<ActiveSession>>>,
        mpsc::UnboundedReceiver<Vec<u8>>,
    ) {
        let (input_tx, input_rx) = mpsc::unbounded_channel();
        let (resize_tx, _) = mpsc::unbounded_channel();
        let active = Arc::new(TokioMutex::new(Some(ActiveSession::Ssh(
            super::super::state::ActiveSsh {
                input_tx,
                resize_tx,
                task: runtime.spawn(async {}),
            },
        ))));
        (
            BatchTarget::Ready {
                tab_id: "target".to_string(),
                active: active.clone(),
            },
            active,
            input_rx,
        )
    }

    #[test]
    fn snapshot_deduplicates_and_classifies_targets_in_request_order() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let sessions = HashMap::from([
            ("ready".to_string(), session(7, &runtime)),
            ("stale".to_string(), session(9, &runtime)),
        ]);
        let targets = vec![
            PtyWriteTarget {
                tab_id: "missing".to_string(),
                session_nonce: 1,
            },
            PtyWriteTarget {
                tab_id: "ready".to_string(),
                session_nonce: 7,
            },
            PtyWriteTarget {
                tab_id: "stale".to_string(),
                session_nonce: 8,
            },
            PtyWriteTarget {
                tab_id: "ready".to_string(),
                session_nonce: 7,
            },
        ];

        let snapshot = snapshot_batch_targets(&sessions, targets);
        assert_eq!(snapshot.len(), 3);
        assert!(matches!(
            &snapshot[0],
            BatchTarget::Result(PtyWriteResult {
                tab_id,
                status: PtyWriteStatus::Missing,
                ..
            }) if tab_id == "missing"
        ));
        assert!(matches!(
            &snapshot[1],
            BatchTarget::Ready { tab_id, .. } if tab_id == "ready"
        ));
        assert!(matches!(
            &snapshot[2],
            BatchTarget::Result(PtyWriteResult {
                tab_id,
                status: PtyWriteStatus::Stale,
                ..
            }) if tab_id == "stale"
        ));
    }

    #[test]
    fn failed_guard_prevents_target_writes() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let guard = BatchTarget::Result(PtyWriteResult::new(
            "source".to_string(),
            PtyWriteStatus::Missing,
        ));
        let (target, target_active, mut input_rx) = writable_ssh_target(&runtime);

        let results = write_guarded_batch(Some(guard), vec![target], b"input");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tab_id, "source");
        assert_eq!(results[0].status, PtyWriteStatus::Missing);
        assert!(target_active.blocking_lock().is_some());
        assert!(matches!(
            input_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn disconnected_guard_prevents_target_writes() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let guard = BatchTarget::Ready {
            tab_id: "source".to_string(),
            active: Arc::new(TokioMutex::new(None)),
        };
        let (target, target_active, mut input_rx) = writable_ssh_target(&runtime);

        let results = write_guarded_batch(Some(guard), vec![target], b"input");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tab_id, "source");
        assert_eq!(results[0].status, PtyWriteStatus::Reconnecting);
        assert!(target_active.blocking_lock().is_some());
        assert!(matches!(
            input_rx.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn writable_guard_receives_input_without_targets() {
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let (guard, guard_active, mut input_rx) = writable_ssh_target(&runtime);

        let results = write_guarded_batch(Some(guard), Vec::new(), b"input");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, PtyWriteStatus::Written);
        assert!(guard_active.blocking_lock().is_some());
        assert_eq!(input_rx.try_recv().expect("guard input"), b"input");
    }
}

#[tauri::command]
pub fn resize_pty(
    app: AppHandle,
    tab_id: String,
    session_nonce: u32,
    rows: u16,
    cols: u16,
    state: State<'_, PtyMap>,
) -> Result<(), String> {
    let map = state.blocking_read();
    let session = map
        .get(&tab_id)
        .ok_or_else(|| format!("PTY session {} not found", tab_id))?;
    if session.session_nonce != session_nonce {
        return Ok(());
    }

    let mut active_guard = session.active.blocking_lock();
    let active = active_guard
        .as_mut()
        .ok_or_else(|| format!("PTY session {} is reconnecting", tab_id))?;

    let result = match active {
        ActiveSession::Local(local) => local
            .master
            .resize(portable_pty::PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to resize PTY: {}", e)),
        ActiveSession::Ssh(ssh) => ssh
            .resize_tx
            .send((rows, cols))
            .map_err(|_| format!("PTY session {} is not resizable", tab_id)),
    };
    if result.is_ok() {
        crate::session_log::record_resize(&app, &tab_id, rows, cols);
    }
    result
}

#[tauri::command]
pub fn kill_pty(
    app: AppHandle,
    tab_id: String,
    session_nonce: u32,
    state: State<'_, PtyMap>,
) -> Result<(), String> {
    let session = {
        let mut sessions = state.blocking_write();
        if sessions
            .get(&tab_id)
            .is_some_and(|session| session.session_nonce == session_nonce)
        {
            sessions.remove(&tab_id)
        } else {
            None
        }
    };

    if let Some(session) = session {
        stop_pty_session(session);
        crate::session_log::end_session(&app, &tab_id);
    }

    Ok(())
}

#[tauri::command]
pub fn respond_ssh_host_key_prompt(
    request_id: String,
    trust: bool,
    prompt_state: State<'_, HostPromptMap>,
) -> Result<(), String> {
    let sender = prompt_state
        .blocking_write()
        .remove(&request_id)
        .ok_or_else(|| "Host key prompt expired".to_string())?;

    sender
        .send(trust)
        .map_err(|_| "Host key prompt receiver is gone".to_string())
}

#[tauri::command]
pub fn has_saved_password(
    app: AppHandle,
    profile_id: Option<String>,
    profile_name: Option<String>,
    secret_state: State<'_, crate::ssh::SecretStoreState>,
) -> Result<bool, String> {
    Ok(super::session::load_saved_ssh_password(
        &app,
        &secret_state,
        profile_id.as_deref(),
        profile_name.as_deref(),
    )?
    .is_some())
}

#[tauri::command]
pub fn has_saved_jump_host_password(
    app: AppHandle,
    profile_id: Option<String>,
    profile_name: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    username: Option<String>,
    allow_legacy_fallback: Option<bool>,
    secret_state: State<'_, crate::ssh::SecretStoreState>,
) -> Result<bool, String> {
    Ok(super::session::load_saved_jump_host_password(
        &app,
        &secret_state,
        profile_id.as_deref(),
        profile_name.as_deref(),
        host.as_deref(),
        port,
        username.as_deref(),
        allow_legacy_fallback.unwrap_or(false),
    )?
    .is_some())
}

#[tauri::command]
pub fn write_saved_password_for_sudo(
    app: AppHandle,
    tab_id: String,
    session_nonce: u32,
    profile_id: Option<String>,
    profile_name: Option<String>,
    state: State<'_, PtyMap>,
    secret_state: State<'_, crate::ssh::SecretStoreState>,
) -> Result<bool, String> {
    let Some(password) = super::session::load_saved_ssh_password(
        &app,
        &secret_state,
        profile_id.as_deref(),
        profile_name.as_deref(),
    )?
    else {
        return Ok(false);
    };

    let map = state.blocking_read();
    let session = map
        .get(&tab_id)
        .ok_or_else(|| format!("PTY session {} not found", tab_id))?;
    if session.session_nonce != session_nonce {
        return Ok(false);
    }

    let mut active_guard = session.active.blocking_lock();
    let active = active_guard
        .as_mut()
        .ok_or_else(|| format!("PTY session {} is reconnecting", tab_id))?;

    let mut data = password.into_bytes();
    data.push(b'\n');

    let result = match active {
        ActiveSession::Local(_) => {
            Err("Saved SSH password cannot be written to a local terminal.".to_string())
        }
        ActiveSession::Ssh(ssh) => ssh
            .input_tx
            .send(data)
            .map(|_| true)
            .map_err(|_| format!("PTY session {} is not writable", tab_id)),
    };
    if matches!(result, Ok(true)) {
        crate::session_log::record_credential_injection(&app, &tab_id);
    }
    result
}
