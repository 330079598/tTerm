use super::session::{normalize_connection, resolve_ssh_password};
use super::state::{ActiveSession, HostPromptMap, PtyMap, PtySession, SessionExitSignal};
use super::supervisor::spawn_supervisor;
use crate::terminal;
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

    match active {
        ActiveSession::Local(local) => local
            .writer
            .write_all(data.as_bytes())
            .map_err(|e| format!("Failed to write to PTY: {}", e)),
        ActiveSession::Ssh(ssh) => ssh
            .input_tx
            .send(data.into_bytes())
            .map_err(|_| format!("PTY session {} is not writable", tab_id)),
    }
}

#[tauri::command]
pub fn resize_pty(
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

    match active {
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
    }
}

#[tauri::command]
pub fn kill_pty(
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

    match active {
        ActiveSession::Local(_) => Err("Saved SSH password cannot be written to a local terminal.".to_string()),
        ActiveSession::Ssh(ssh) => ssh
            .input_tx
            .send(data)
            .map(|_| true)
            .map_err(|_| format!("PTY session {} is not writable", tab_id)),
    }
}
