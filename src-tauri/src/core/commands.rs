use super::blocking::run_blocking;
use super::session::{normalize_connection, resolve_ssh_password};
use super::state::{
    ActiveSession, AtomicTerminalSize, ChannelContext, HostPromptMap, PtyMap, PtySession,
    SessionExitSignal, TabZmodem, ZmodemArmedSendMap, ZmodemManualDetectMap, ZmodemMap,
};
use super::supervisor::{spawn_ssh_attempt, spawn_supervisor, TabConnection};
use crate::terminal;
use crate::zmodem::session::ZmodemSettings;
use batch_write::{
    snapshot_batch_targets, write_guarded_batch, PtyWriteResult, PtyWriteStatus, PtyWriteTarget,
};
use std::io::Write;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::PoisonError;
use tauri::{AppHandle, Manager, State};
use tokio::sync::{mpsc, watch, Mutex as TokioMutex};

mod batch_write;

fn stop_pty_session(
    tab_id: &str,
    session: PtySession,
    zmodem_state: &ZmodemMap,
    zmodem_armed_send_state: &ZmodemArmedSendMap,
    zmodem_manual_detect_state: &ZmodemManualDetectMap,
) {
    let _ = session.stop_tx.send(true);
    // Nobody will acknowledge this channel's output again.
    session.output.flow().close();

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

    // The peer's rz/sz process (if any) dies with this channel; a ZMODEM
    // session can't meaningfully survive it.
    zmodem_state
        .write()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(tab_id);
    zmodem_armed_send_state
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(tab_id);
    zmodem_manual_detect_state
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(tab_id);

    session.supervisor.abort();
}

/// Async so the work below runs off the main thread: reading saved passwords,
/// waiting for the session map, and starting the shell (ConPTY, or the login
/// shell environment on macOS) would otherwise freeze the window.
#[tauri::command]
#[allow(
    clippy::too_many_arguments,
    reason = "a Tauri command's arguments are its IPC fields and injected state"
)]
pub async fn create_pty(
    app: AppHandle,
    webview: tauri::Webview,
    tab_id: String,
    session_nonce: u32,
    rows: u16,
    cols: u16,
    connection: Option<super::session::types::PtyConnectionOptions>,
    output_channel: tauri::ipc::JavaScriptChannelId,
) -> Result<u32, String> {
    let output_channel =
        terminal::io_batcher::TerminalOutput::new(output_channel.channel_on(webview));
    run_blocking(move || {
        create_pty_blocking(
            app,
            tab_id,
            session_nonce,
            rows,
            cols,
            connection,
            output_channel,
        )
    })
    .await
}

fn create_pty_blocking(
    app: AppHandle,
    tab_id: String,
    session_nonce: u32,
    rows: u16,
    cols: u16,
    connection: Option<super::session::types::PtyConnectionOptions>,
    output_channel: terminal::io_batcher::TerminalOutput,
) -> Result<u32, String> {
    let state = app.state::<PtyMap>().inner().clone();
    let prompt_state = app.state::<HostPromptMap>().inner().clone();
    let secret_state = app
        .state::<crate::ssh::secret_store::types::SecretStoreState>()
        .inner()
        .clone();
    let zmodem_state = app.state::<ZmodemMap>().inner().clone();
    let zmodem_armed_send_state = app.state::<ZmodemArmedSendMap>().inner().clone();
    let zmodem_manual_detect_state = app.state::<ZmodemManualDetectMap>().inner().clone();
    let mut plan = normalize_connection(connection)?;
    resolve_ssh_password(&app, &secret_state, &mut plan)?;
    let runtime_state = app.state::<crate::TokioRuntimeState>();

    let mut sessions = state.blocking_write();
    let reattached = sessions
        .get(&tab_id)
        .and_then(|session| session.reattach(session_nonce, rows, cols, &output_channel));
    if let Some(pid) = reattached {
        return Ok(pid);
    }

    if let Some(session) = sessions.remove(&tab_id) {
        stop_pty_session(
            &tab_id,
            session,
            &zmodem_state,
            &zmodem_armed_send_state,
            &zmodem_manual_detect_state,
        );
    }

    crate::session_log::start_session(&app, &tab_id, session_nonce, &plan, rows, cols);

    let (exit_tx, exit_rx) = mpsc::unbounded_channel::<SessionExitSignal>();
    let active = Arc::new(TokioMutex::new(None));
    let (stop_tx, stop_rx) = watch::channel(false);
    let size = Arc::new(AtomicTerminalSize::new(rows, cols));
    // Lives for the tab's whole lifetime (unlike `ZmodemMap`, only present
    // while a transfer is in flight) so the manual "Send/Receive Files via
    // ZMODEM" keymap actions can force detection on later, even across a
    // reconnect.
    let zmodem_manual_override = Arc::new(AtomicBool::new(false));
    zmodem_manual_detect_state
        .lock()
        .unwrap()
        .insert(tab_id.clone(), zmodem_manual_override.clone());

    let runtime_handle = runtime_state.runtime.handle().clone();
    let tab = TabConnection {
        channel: ChannelContext {
            app: app.clone(),
            tab_id: tab_id.clone(),
            session_nonce,
            active: active.clone(),
            zmodem: TabZmodem {
                map: zmodem_state.clone(),
                armed_send_map: zmodem_armed_send_state.clone(),
                manual_override: zmodem_manual_override,
            },
        },
        plan: plan.clone(),
        prompt_state,
        stop_rx: stop_rx.clone(),
        exit_tx,
        output_channel: output_channel.clone(),
        runtime_handle,
    };

    let (pid, initial_active) = spawn_initial_session(&tab, rows, cols)?;

    {
        let mut guard = runtime_state.runtime.block_on(active.lock());
        *guard = Some(initial_active);
    }

    let supervisor = spawn_supervisor(tab, exit_rx, size.clone());

    let session = PtySession {
        pid,
        session_nonce,
        active,
        stop_tx,
        supervisor,
        size,
        output: output_channel,
    };

    sessions.insert(tab_id, session);

    Ok(pid)
}

/// Starts a tab's first connection attempt, returning the pid of its local
/// shell (0 when there is none) and the live session.
fn spawn_initial_session(
    tab: &TabConnection,
    rows: u16,
    cols: u16,
) -> Result<(u32, ActiveSession), String> {
    let channel = &tab.channel;
    match tab.plan.kind {
        crate::core::state::SessionKind::Terminal => {
            let (pid, pty) =
                terminal::pty::spawn_local_pty(rows, cols, tab.plan.terminal_shell.clone())?;

            let reader = pty.input.track_output(
                pty.master
                    .try_clone_reader()
                    .map_err(|e| format!("Failed to clone reader: {}", e))?,
            );

            let sender = crate::terminal::io_batcher::TerminalOutputSender::spawn(
                &channel.tab_id,
                tab.output_channel.clone(),
            );
            terminal::pty::spawn_reader_thread(
                reader,
                channel.clone(),
                tab.exit_tx.clone(),
                Some(sender),
                ZmodemSettings::load(),
            );

            #[cfg(unix)]
            terminal::cwd_watch::spawn_cwd_watcher(
                channel.app.clone(),
                channel.tab_id.clone(),
                channel.session_nonce,
                pid,
                tab.stop_rx.clone(),
            );

            Ok((pid, ActiveSession::Local(pty)))
        }
        crate::core::state::SessionKind::Ssh => {
            let ssh = spawn_ssh_attempt(tab.clone(), rows, cols, ZmodemSettings::load());

            Ok((0, ActiveSession::Ssh(ssh)))
        }
    }
}

/// While a ZMODEM transfer occupies `tab_id`'s channel, ordinary keystrokes
/// must not reach it (they'd corrupt the binary stream) — except Ctrl-C,
/// which this treats as "cancel" the same way it would abort any other
/// foreground program: flags the transfer so the reader thread/task cleans
/// up on its next read, and best-effort writes the standard abort sequence
/// straight to the peer. Returns `true` if the input was handled here (the
/// caller's normal write must be skipped either way).
fn intercept_zmodem_input(
    zmodem_state: &ZmodemMap,
    tab_id: &str,
    session_nonce: u32,
    data: &[u8],
    active: &Arc<TokioMutex<Option<ActiveSession>>>,
) -> bool {
    let cancel_requested = {
        let map = zmodem_state.read().unwrap_or_else(PoisonError::into_inner);
        let handle = match map.get(tab_id) {
            Some(handle) if handle.session_nonce == session_nonce => handle,
            _ => return false,
        };
        data.contains(&0x03)
            .then(|| handle.cancel_requested.clone())
    };
    if let Some(cancel_requested) = cancel_requested {
        cancel_requested.store(true, std::sync::atomic::Ordering::Relaxed);
        let mut guard = active.blocking_lock();
        match guard.as_mut() {
            Some(ActiveSession::Local(local)) => {
                let _ = local
                    .writer
                    .write_all(&crate::zmodem::session::abort_sequence());
            }
            Some(ActiveSession::Ssh(ssh)) => {
                let _ = ssh.input_tx.send(crate::zmodem::session::abort_sequence());
            }
            None => {}
        }
    }
    true
}

#[tauri::command]
pub fn write_pty(
    app: AppHandle,
    tab_id: String,
    session_nonce: u32,
    data: String,
    state: State<'_, PtyMap>,
    zmodem_state: State<'_, ZmodemMap>,
) -> Result<(), String> {
    let map = state.blocking_read();
    let session = map
        .get(&tab_id)
        .ok_or_else(|| format!("PTY session {} not found", tab_id))?;
    if session.session_nonce != session_nonce {
        return Ok(());
    }

    let input = data.into_bytes();
    if intercept_zmodem_input(
        &zmodem_state,
        &tab_id,
        session_nonce,
        &input,
        &session.active,
    ) {
        return Ok(());
    }

    let mut active_guard = session.active.blocking_lock();
    let active = active_guard
        .as_mut()
        .ok_or_else(|| format!("PTY session {} is reconnecting", tab_id))?;

    let result = match active {
        ActiveSession::Local(local) => local
            .input
            .send(input.clone())
            .map_err(|e| format!("Failed to write to PTY: {}", e)),
        ActiveSession::Ssh(ssh) => ssh
            .input
            .send(ssh.encoding.encode_input(&input).into_owned())
            .map_err(|_| format!("PTY session {} is not writable", tab_id)),
    };
    if result.is_ok() {
        crate::session_log::record_input(&app, &tab_id, &input);
    }
    result
}

/// Soft-resets the console behind a local Windows session, which interprets
/// the shell's output before xterm.js sees it (see `console_reset`). A no-op
/// for SSH sessions and on other platforms, where xterm.js holds all state.
#[tauri::command]
pub async fn reset_pty_console(
    tab_id: String,
    session_nonce: u32,
    state: State<'_, PtyMap>,
) -> Result<(), String> {
    let pid = {
        let map = state.read().await;
        let Some(session) = map.get(&tab_id) else {
            return Ok(());
        };
        if session.session_nonce != session_nonce
            || !matches!(*session.active.lock().await, Some(ActiveSession::Local(_)))
        {
            return Ok(());
        }
        session.pid
    };
    #[cfg(target_os = "windows")]
    if pid != 0 {
        return tokio::task::spawn_blocking(move || terminal::console_reset::reset_console(pid))
            .await
            .map_err(|err| format!("Console reset task failed: {err}"))?;
    }
    let _ = pid;
    Ok(())
}

/// Reports output bytes xterm.js has finished parsing, letting a paused
/// output sender continue. Async so it runs off the main thread.
#[tauri::command]
pub async fn ack_pty_output(
    tab_id: String,
    session_nonce: u32,
    bytes: usize,
    state: State<'_, PtyMap>,
) -> Result<(), String> {
    let map = state.read().await;
    if let Some(session) = map.get(&tab_id) {
        if session.session_nonce == session_nonce {
            session.output.flow().ack(bytes);
        }
    }
    Ok(())
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

    // Record the size even while reconnecting so the next attempt adopts it.
    session.size.store(rows, cols);

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
    zmodem_state: State<'_, ZmodemMap>,
    zmodem_armed_send_state: State<'_, ZmodemArmedSendMap>,
    zmodem_manual_detect_state: State<'_, ZmodemManualDetectMap>,
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
        stop_pty_session(
            &tab_id,
            session,
            zmodem_state.inner(),
            zmodem_armed_send_state.inner(),
            zmodem_manual_detect_state.inner(),
        );
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
pub async fn has_saved_password(
    app: AppHandle,
    profile_id: Option<String>,
    profile_name: Option<String>,
    secret_state: State<'_, crate::ssh::secret_store::types::SecretStoreState>,
) -> Result<bool, String> {
    let secret_state = secret_state.inner().clone();
    run_blocking(move || {
        Ok(super::session::load_saved_ssh_password(
            &app,
            &secret_state,
            profile_id.as_deref(),
            profile_name.as_deref(),
        )?
        .is_some())
    })
    .await
}

#[tauri::command]
#[allow(
    clippy::too_many_arguments,
    reason = "a Tauri command's arguments are its IPC fields and injected state"
)]
pub async fn has_saved_jump_host_password(
    app: AppHandle,
    profile_id: Option<String>,
    profile_name: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    username: Option<String>,
    allow_legacy_fallback: Option<bool>,
    secret_state: State<'_, crate::ssh::secret_store::types::SecretStoreState>,
) -> Result<bool, String> {
    let secret_state = secret_state.inner().clone();
    run_blocking(move || {
        Ok(super::session::load_saved_jump_host_password(
            &app,
            &secret_state,
            super::session::JumpHostSecretOwner {
                profile_id: profile_id.as_deref(),
                profile_name: profile_name.as_deref(),
                host: host.as_deref(),
                port,
                username: username.as_deref(),
            },
            allow_legacy_fallback.unwrap_or(false),
        )?
        .is_some())
    })
    .await
}

/// The saved password a sudo prompt in this profile would be answered with.
#[tauri::command]
pub async fn get_sudo_password_source(
    app: AppHandle,
    profile_id: Option<String>,
    profile_name: Option<String>,
    secret_state: State<'_, crate::ssh::secret_store::types::SecretStoreState>,
) -> Result<Option<super::session::SudoPasswordSource>, String> {
    let secret_state = secret_state.inner().clone();
    run_blocking(move || {
        Ok(super::session::load_saved_sudo_password(
            &app,
            &secret_state,
            profile_id.as_deref(),
            profile_name.as_deref(),
        )?
        .map(|(_, source)| source))
    })
    .await
}

#[tauri::command]
pub async fn has_saved_sudo_password(
    app: AppHandle,
    profile_id: String,
    secret_state: State<'_, crate::ssh::secret_store::types::SecretStoreState>,
) -> Result<bool, String> {
    let secret_state = secret_state.inner().clone();
    run_blocking(move || {
        Ok(secret_state
            .get_password(
                &app,
                &super::session::types::sudo_secret_key(profile_id.trim()),
            )?
            .is_some())
    })
    .await
}

/// Writes the saved sudo password to an SSH session, but only while `prompt`
/// is still the session's last output line. Returns false when there is no
/// saved password, the session moved on, or the prompt is no longer waiting.
///
/// The session map and the saved password are read on a worker thread: the
/// locks are taken with blocking calls and the password may sit behind the
/// credential store.
#[tauri::command]
pub async fn write_saved_password_for_sudo(
    app: AppHandle,
    tab_id: String,
    session_nonce: u32,
    profile_id: Option<String>,
    profile_name: Option<String>,
    prompt: String,
) -> Result<bool, String> {
    run_blocking(move || {
        write_saved_password_for_sudo_blocking(
            &app,
            &tab_id,
            session_nonce,
            profile_id.as_deref(),
            profile_name.as_deref(),
            &prompt,
        )
    })
    .await
}

fn write_saved_password_for_sudo_blocking(
    app: &AppHandle,
    tab_id: &str,
    session_nonce: u32,
    profile_id: Option<&str>,
    profile_name: Option<&str>,
    prompt: &str,
) -> Result<bool, String> {
    let state = app.state::<PtyMap>();
    let secret_state = app.state::<crate::ssh::secret_store::types::SecretStoreState>();
    let map = state.blocking_read();
    let session = map
        .get(tab_id)
        .ok_or_else(|| format!("PTY session {} not found", tab_id))?;
    if session.session_nonce != session_nonce {
        return Ok(false);
    }

    let mut active_guard = session.active.blocking_lock();
    let active = active_guard
        .as_mut()
        .ok_or_else(|| format!("PTY session {} is reconnecting", tab_id))?;
    let ActiveSession::Ssh(ssh) = active else {
        return Err("Saved SSH password cannot be written to a local terminal.".to_string());
    };
    if !ssh.output_tail.awaits_prompt(prompt) {
        return Ok(false);
    }

    let Some((password, _)) =
        super::session::load_saved_sudo_password(app, &secret_state, profile_id, profile_name)?
    else {
        return Ok(false);
    };

    // The channel takes ownership, so only this copy can be wiped here.
    let encoded = ssh.encoding.encode_input(password.as_bytes());
    let mut data = zeroize::Zeroizing::new(Vec::with_capacity(encoded.len() + 1));
    data.extend_from_slice(&encoded);
    data.push(b'\n');
    if let std::borrow::Cow::Owned(mut converted) = encoded {
        zeroize::Zeroize::zeroize(&mut converted);
    }

    // Behind any paste still being sent, like the keys typed before it.
    ssh.input
        .send(data.to_vec())
        .map_err(|_| format!("PTY session {} is not writable", tab_id))?;
    crate::session_log::record_credential_injection(app, tab_id);
    Ok(true)
}
