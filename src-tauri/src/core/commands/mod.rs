use std::sync::Arc;
use tauri::{AppHandle, State};
use tterm_core::core::arguments::GetSudoPasswordSourceInput;
use tterm_core::core::arguments::HasSavedPasswordInput;
use tterm_core::core::arguments::ResetPtyConsoleInput;
use tterm_core::core::arguments::RespondSshHostKeyPromptInput;
use tterm_core::core::arguments::SudoPasswordContext;
use tterm_core::core::arguments::WritePtyBatchInput;
use tterm_core::core::arguments::WriteSavedPasswordForSudoInput;
use tterm_core::core::batch_write::AckPtyOutputInput;
use tterm_core::core::batch_write::PtyWriteResult;
use tterm_core::core::batch_write::PtyWriteStatus;
use tterm_core::core::batch_write::PtyWriteTarget;
use tterm_core::core::batch_write::ack_pty_output_service;
use tterm_core::core::batch_write::snapshot_batch_targets;
use tterm_core::core::batch_write::write_guarded_batch;
use tterm_core::core::blocking::run_blocking;
use tterm_core::services::AppServices;
use tterm_core::ssh::context::ConnectionContext;
use tterm_core::ssh::secret_store::types::SecretStoreState;
use tterm_core::{core::state::ActiveSession, core::state::PtyMap};

use tterm_core::core::create::PtyIdentity;

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
    connection: Option<tterm_core::core::session::types::PtyConnectionOptions>,
    output_channel: tauri::ipc::JavaScriptChannelId,
    services: State<'_, Arc<AppServices>>,
) -> Result<u32, String> {
    let output_channel = crate::session_sink::output::new(output_channel.channel_on(webview));
    let sink: Arc<dyn tterm_core::session::sink::SessionSink> = Arc::new(
        crate::session_sink::TauriSessionSink::new(tab_id.clone(), app, output_channel.clone()),
    );
    let services = Arc::clone(&services);
    run_blocking(move || {
        services.create_pty_blocking(
            PtyIdentity {
                tab_id,
                session_nonce,
                rows,
                cols,
            },
            connection,
            output_channel,
            sink,
        )
    })
    .await
}

#[tauri::command]
pub fn write_pty(
    tab_id: String,
    session_nonce: u32,
    data: String,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<(), String> {
    services.write_pty(
        &tterm_core::prompts::SessionKey {
            tab_id,
            session_nonce,
        },
        data.as_bytes(),
    )
}

/// Soft-resets the console behind a local Windows session, which interprets
/// the shell's output before xterm.js sees it (see `console_reset`). A no-op
/// for SSH sessions and on other platforms, where xterm.js holds all state.
#[tauri::command]
pub async fn reset_pty_console(
    tab_id: String,
    session_nonce: u32,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
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
    ResetPtyConsoleInput {
        tab_id,
        session_nonce,
    }: ResetPtyConsoleInput,
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
        return tokio::task::spawn_blocking(move || {
            tterm_core::terminal::console_reset::reset_console(pid)
        })
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
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
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
#[tauri::command]
pub fn write_pty_batch(
    mut targets: Vec<PtyWriteTarget>,
    guard_target: Option<PtyWriteTarget>,
    data: String,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Vec<PtyWriteResult> {
    let targets = std::mem::take(&mut targets);
    let app = tterm_core::connection_context::connection(&services);
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
    app: ConnectionContext,
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
            tterm_core::session_log::record_input(&app.log, &result.tab_id, data.as_bytes());
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
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<(), String> {
    services.resize_pty(
        &tterm_core::prompts::SessionKey {
            tab_id,
            session_nonce,
        },
        rows,
        cols,
    )
}

#[tauri::command]
pub fn kill_pty(
    tab_id: String,
    session_nonce: u32,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<(), String> {
    services.kill_pty(&tterm_core::prompts::SessionKey {
        tab_id,
        session_nonce,
    })
}

#[tauri::command]
pub fn respond_ssh_host_key_prompt(
    request_id: String,
    trust: bool,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
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
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
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
    secret_state: SecretStoreState,
    arguments: HasSavedPasswordInput,
) -> Result<bool, String> {
    let HasSavedPasswordInput {
        profile_id,
        profile_name,
    } = arguments;

    let secret_state = secret_state.clone();
    run_blocking(move || {
        Ok(tterm_core::core::session::load_saved_ssh_password(
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
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
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
    secret_state: SecretStoreState,
    profile_id: Option<String>,
    profile_name: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    username: Option<String>,
    allow_legacy_fallback: Option<bool>,
) -> Result<bool, String> {
    let secret_state = secret_state.clone();
    run_blocking(move || {
        Ok(tterm_core::core::session::load_saved_jump_host_password(
            &secret_state,
            tterm_core::core::session::JumpHostSecretOwner {
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
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<Option<tterm_core::core::session::SudoPasswordSource>, String> {
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
    secret_state: SecretStoreState,
    arguments: GetSudoPasswordSourceInput,
) -> Result<Option<tterm_core::core::session::SudoPasswordSource>, String> {
    let GetSudoPasswordSourceInput {
        profile_id,
        profile_name,
    } = arguments;

    let secret_state = secret_state.clone();
    run_blocking(move || {
        Ok(tterm_core::core::session::load_saved_sudo_password(
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
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<bool, String> {
    let secret_state = services.shared.secrets.clone();
    has_saved_sudo_password_service(secret_state, profile_id).await
}
async fn has_saved_sudo_password_service(
    secret_state: SecretStoreState,
    profile_id: String,
) -> Result<bool, String> {
    let secret_state = secret_state.clone();
    run_blocking(move || {
        Ok(secret_state
            .get_password(&tterm_core::core::session::types::sudo_secret_key(
                profile_id.trim(),
            ))?
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
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<bool, String> {
    let app = tterm_core::connection_context::connection(&services);
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
    app: ConnectionContext,
    secret_state: SecretStoreState,
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
            SudoPasswordContext {
                app: &app,
                secret_state: &secret_state,
            },
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
    context: SudoPasswordContext<'_>,
    tab_id: &str,
    session_nonce: u32,
    profile_id: Option<&str>,
    profile_name: Option<&str>,
    prompt: &str,
) -> Result<bool, String> {
    let SudoPasswordContext { app, secret_state } = context;
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

    let Some((password, _)) = tterm_core::core::session::load_saved_sudo_password(
        secret_state,
        profile_id,
        profile_name,
    )?
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
    tterm_core::session_log::record_credential_injection(&app.log, tab_id);
    Ok(true)
}
