mod arguments;
use super::blocking::run_blocking;
use super::state::{
    ActiveSession, PtyMap, PtySession, ZmodemArmedSendMap, ZmodemManualDetectMap, ZmodemMap,
};
use crate::services::AppServices;
use crate::ssh::context::ConnectionContext;
#[cfg(target_os = "windows")]
use crate::terminal;
use arguments::{
    AckPtyOutputInput, GetSudoPasswordSourceInput, HasSavedPasswordInput, KillPtyInput,
    ResetPtyConsoleInput, ResizePtyInput, RespondSshHostKeyPromptInput, WritePtyBatchInput,
    WritePtyInput, WriteSavedPasswordForSudoInput,
};
use batch_write::{
    snapshot_batch_targets, write_guarded_batch, PtyWriteResult, PtyWriteStatus, PtyWriteTarget,
};
use std::io::Write;
use std::sync::Arc;
use std::sync::PoisonError;
use tauri::{AppHandle, State};
use tokio::sync::Mutex as TokioMutex;

mod batch_write;
mod create;
use create::{create_pty_blocking, CreateContext};

fn stop_pty_session(
    context: (&tterm_core::prompts::PromptBroker, &str),
    session: PtySession,
    zmodem_state: &ZmodemMap,
    zmodem_armed_send_state: &ZmodemArmedSendMap,
    zmodem_manual_detect_state: &ZmodemManualDetectMap,
) {
    let (prompts, tab_id) = context;
    prompts.cancel_session(&tterm_core::prompts::SessionKey {
        tab_id: tab_id.to_string(),
        session_nonce: session.session_nonce,
    });
    let _ = session.stop_tx.send(true);
    // Nobody will acknowledge this channel's output again.
    session.sink.close_output();

    if let Some(active) = session.active.blocking_lock().take() {
        match active {
            ActiveSession::Local(mut local) => {
                let _ = local.child.kill();
            }
            ActiveSession::Ssh(ssh) => {
                ssh.task.abort();
            }
            // Dropping it closes the port.
            ActiveSession::Serial(_) => {}
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
    services: State<'_, Arc<AppServices>>,
) -> Result<u32, String> {
    let output_channel = crate::session_sink::output::new(output_channel.channel_on(webview));
    let context = CreateContext {
        connection: crate::connection_context::connection(&services),
        ptys: services.session.ptys.clone(),
        secrets: services.shared.secrets.clone(),
        runtime: services.shared.runtime.clone(),
        zmodem: services.session.zmodem.clone(),
        armed: services.session.zmodem_armed_send.clone(),
        manual: services.session.zmodem_manual_detect.clone(),
    };
    let sink: Arc<dyn tterm_core::session::sink::SessionSink> = Arc::new(
        crate::session_sink::TauriSessionSink::new(tab_id.clone(), app, output_channel.clone()),
    );
    run_blocking(move || {
        create_pty_blocking(
            context,
            (tab_id, session_nonce, rows, cols),
            connection,
            output_channel,
            sink,
        )
    })
    .await
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
            Some(ActiveSession::Serial(serial)) => {
                let _ = serial
                    .writer
                    .write_all(&crate::zmodem::session::abort_sequence());
            }
            None => {}
        }
    }
    true
}

#[tauri::command]
pub fn write_pty(
    tab_id: String,
    session_nonce: u32,
    data: String,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    let app = crate::connection_context::connection(&services);
    let state = services.session.ptys.clone();
    let zmodem_state = services.session.zmodem.clone();
    write_pty_service(
        app,
        state,
        zmodem_state,
        WritePtyInput {
            tab_id,
            session_nonce,
            data,
        },
    )
}
fn write_pty_service(
    app: crate::ssh::context::ConnectionContext,
    state: PtyMap,
    zmodem_state: ZmodemMap,
    arguments: WritePtyInput,
) -> Result<(), String> {
    let WritePtyInput {
        tab_id,
        session_nonce,
        data,
    } = arguments;

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
        ActiveSession::Serial(serial) => serial
            .input
            .send(serial.encode_input(&input).into_owned())
            .map_err(|e| format!("Failed to write to the serial port: {}", e)),
    };
    if result.is_ok() {
        session.sink.note_input();
        crate::session_log::record_input(&app.log, &tab_id, &input);
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
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    let state = services.session.ptys.clone();
    reset_pty_console_service(
        state,
        ResetPtyConsoleInput {
            tab_id,
            session_nonce,
        },
    )
    .await
}
async fn reset_pty_console_service(
    state: PtyMap,
    arguments: ResetPtyConsoleInput,
) -> Result<(), String> {
    let ResetPtyConsoleInput {
        tab_id,
        session_nonce,
    } = arguments;

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
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    let state = services.session.ptys.clone();
    ack_pty_output_service(
        state,
        AckPtyOutputInput {
            tab_id,
            session_nonce,
            bytes,
        },
    )
    .await
}
async fn ack_pty_output_service(state: PtyMap, arguments: AckPtyOutputInput) -> Result<(), String> {
    let AckPtyOutputInput {
        tab_id,
        session_nonce,
        bytes,
    } = arguments;

    let map = state.read().await;
    if let Some(session) = map.get(&tab_id) {
        if session.session_nonce == session_nonce {
            session.sink.ack_output(bytes);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn write_pty_batch(
    mut targets: Vec<PtyWriteTarget>,
    guard_target: Option<PtyWriteTarget>,
    data: String,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Vec<PtyWriteResult> {
    let targets = std::mem::take(&mut targets);
    let app = crate::connection_context::connection(&services);
    let state = services.session.ptys.clone();
    write_pty_batch_service(
        app,
        state,
        WritePtyBatchInput {
            targets,
            guard_target,
            data,
        },
    )
}
fn write_pty_batch_service(
    app: crate::ssh::context::ConnectionContext,
    state: PtyMap,
    arguments: WritePtyBatchInput,
) -> Vec<PtyWriteResult> {
    let WritePtyBatchInput {
        mut targets,
        guard_target,
        data,
    } = arguments;

    if let Some(guard_tab_id) = guard_target.as_ref().map(|target| target.tab_id.as_str()) {
        targets.retain(|target| target.tab_id != guard_tab_id);
    }
    if (targets.is_empty() && guard_target.is_none()) || data.is_empty() {
        return Vec::new();
    }

    let map = state.blocking_read();
    let guard_target = guard_target.and_then(|target| {
        snapshot_batch_targets(&map, vec![target])
            .into_iter()
            .next()
    });
    let targets = snapshot_batch_targets(&map, targets);
    drop(map);

    let results = write_guarded_batch(guard_target, targets, data.as_bytes());
    for result in &results {
        if result.status == PtyWriteStatus::Written {
            crate::session_log::record_input(&app.log, &result.tab_id, data.as_bytes());
        }
    }
    results
}

#[tauri::command]
pub fn resize_pty(
    tab_id: String,
    session_nonce: u32,
    rows: u16,
    cols: u16,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    let app = crate::connection_context::connection(&services);
    let state = services.session.ptys.clone();
    resize_pty_service(
        app,
        state,
        ResizePtyInput {
            tab_id,
            session_nonce,
            rows,
            cols,
        },
    )
}
fn resize_pty_service(
    app: crate::ssh::context::ConnectionContext,
    state: PtyMap,
    arguments: ResizePtyInput,
) -> Result<(), String> {
    let ResizePtyInput {
        tab_id,
        session_nonce,
        rows,
        cols,
    } = arguments;

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
        // A serial line has no terminal size; the size is still logged.
        ActiveSession::Serial(_) => Ok(()),
    };
    if result.is_ok() {
        crate::session_log::record_resize(&app.log, &tab_id, rows, cols);
    }
    result
}

#[tauri::command]
pub fn kill_pty(
    tab_id: String,
    session_nonce: u32,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    let app = crate::connection_context::connection(&services);
    let state = services.session.ptys.clone();
    let zmodem_state = services.session.zmodem.clone();
    let zmodem_armed_send_state = services.session.zmodem_armed_send.clone();
    let zmodem_manual_detect_state = services.session.zmodem_manual_detect.clone();
    kill_pty_service(
        app,
        state,
        zmodem_state,
        zmodem_armed_send_state,
        zmodem_manual_detect_state,
        KillPtyInput {
            tab_id,
            session_nonce,
        },
    )
}
fn kill_pty_service(
    app: crate::ssh::context::ConnectionContext,
    state: PtyMap,
    zmodem_state: ZmodemMap,
    zmodem_armed_send_state: ZmodemArmedSendMap,
    zmodem_manual_detect_state: ZmodemManualDetectMap,
    arguments: KillPtyInput,
) -> Result<(), String> {
    let KillPtyInput {
        tab_id,
        session_nonce,
    } = arguments;

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
            (&app.prompts, &tab_id),
            session,
            &zmodem_state,
            &zmodem_armed_send_state,
            &zmodem_manual_detect_state,
        );
        crate::session_log::end_session(&app.log, &tab_id);
    }

    Ok(())
}

#[tauri::command]
pub fn respond_ssh_host_key_prompt(
    request_id: String,
    trust: bool,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    let prompt_state = services.shared.prompts.clone();
    respond_ssh_host_key_prompt_service(
        prompt_state,
        RespondSshHostKeyPromptInput { request_id, trust },
    )
}
fn respond_ssh_host_key_prompt_service(
    prompt_state: std::sync::Arc<tterm_core::prompts::PromptBroker>,
    arguments: RespondSshHostKeyPromptInput,
) -> Result<(), String> {
    let RespondSshHostKeyPromptInput { request_id, trust } = arguments;

    prompt_state.reply_host_key(&request_id, trust)
}

#[tauri::command]
pub async fn has_saved_password(
    profile_id: Option<String>,
    profile_name: Option<String>,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<bool, String> {
    let secret_state = services.shared.secrets.clone();
    has_saved_password_service(
        secret_state,
        HasSavedPasswordInput {
            profile_id,
            profile_name,
        },
    )
    .await
}
async fn has_saved_password_service(
    secret_state: crate::ssh::secret_store::types::SecretStoreState,
    arguments: HasSavedPasswordInput,
) -> Result<bool, String> {
    let HasSavedPasswordInput {
        profile_id,
        profile_name,
    } = arguments;

    let secret_state = secret_state.clone();
    run_blocking(move || {
        Ok(super::session::load_saved_ssh_password(
            &secret_state,
            profile_id.as_deref(),
            profile_name.as_deref(),
        )?
        .is_some())
    })
    .await
}

#[tauri::command]
pub async fn has_saved_jump_host_password(
    profile_id: Option<String>,
    profile_name: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    username: Option<String>,
    allow_legacy_fallback: Option<bool>,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<bool, String> {
    let secret_state = services.shared.secrets.clone();
    has_saved_jump_host_password_service(
        secret_state,
        profile_id,
        profile_name,
        host,
        port,
        username,
        allow_legacy_fallback,
    )
    .await
}
async fn has_saved_jump_host_password_service(
    secret_state: crate::ssh::secret_store::types::SecretStoreState,
    profile_id: Option<String>,
    profile_name: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    username: Option<String>,
    allow_legacy_fallback: Option<bool>,
) -> Result<bool, String> {
    let secret_state = secret_state.clone();
    run_blocking(move || {
        Ok(super::session::load_saved_jump_host_password(
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
    profile_id: Option<String>,
    profile_name: Option<String>,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<Option<super::session::SudoPasswordSource>, String> {
    let secret_state = services.shared.secrets.clone();
    get_sudo_password_source_service(
        secret_state,
        GetSudoPasswordSourceInput {
            profile_id,
            profile_name,
        },
    )
    .await
}
async fn get_sudo_password_source_service(
    secret_state: crate::ssh::secret_store::types::SecretStoreState,
    arguments: GetSudoPasswordSourceInput,
) -> Result<Option<super::session::SudoPasswordSource>, String> {
    let GetSudoPasswordSourceInput {
        profile_id,
        profile_name,
    } = arguments;

    let secret_state = secret_state.clone();
    run_blocking(move || {
        Ok(super::session::load_saved_sudo_password(
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
    profile_id: String,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<bool, String> {
    let secret_state = services.shared.secrets.clone();
    has_saved_sudo_password_service(secret_state, profile_id).await
}
async fn has_saved_sudo_password_service(
    secret_state: crate::ssh::secret_store::types::SecretStoreState,
    profile_id: String,
) -> Result<bool, String> {
    let secret_state = secret_state.clone();
    run_blocking(move || {
        Ok(secret_state
            .get_password(&super::session::types::sudo_secret_key(profile_id.trim()))?
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
    tab_id: String,
    session_nonce: u32,
    profile_id: Option<String>,
    profile_name: Option<String>,
    prompt: String,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<bool, String> {
    let app = crate::connection_context::connection(&services);
    let secret_state = services.shared.secrets.clone();
    write_saved_password_for_sudo_service(
        app,
        secret_state,
        WriteSavedPasswordForSudoInput {
            tab_id,
            session_nonce,
            profile_id,
            profile_name,
            prompt,
        },
    )
    .await
}
async fn write_saved_password_for_sudo_service(
    app: crate::ssh::context::ConnectionContext,
    secret_state: crate::ssh::secret_store::types::SecretStoreState,
    arguments: WriteSavedPasswordForSudoInput,
) -> Result<bool, String> {
    let WriteSavedPasswordForSudoInput {
        tab_id,
        session_nonce,
        profile_id,
        profile_name,
        prompt,
    } = arguments;

    run_blocking(move || {
        write_saved_password_for_sudo_blocking(
            (&app, &secret_state),
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
    context: (
        &ConnectionContext,
        &crate::ssh::secret_store::types::SecretStoreState,
    ),
    tab_id: &str,
    session_nonce: u32,
    profile_id: Option<&str>,
    profile_name: Option<&str>,
    prompt: &str,
) -> Result<bool, String> {
    let (app, secret_state) = context;
    let state = app.ptys.upgrade().ok_or("Prompt session is closed")?;
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
        super::session::load_saved_sudo_password(secret_state, profile_id, profile_name)?
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
    crate::session_log::record_credential_injection(&app.log, tab_id);
    Ok(true)
}
