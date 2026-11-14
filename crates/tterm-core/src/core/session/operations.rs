//! Client-neutral operations on an already-created PTY session.

use crate::core::session::stop::{StopPtyContext, stop_pty_session};
use crate::core::state::{
    ActiveSession, PtyMap, ZmodemArmedSendMap, ZmodemManualDetectMap, ZmodemMap,
};
use crate::prompts::SessionKey;
use crate::ssh::context::ConnectionContext;
use std::io::Write;
use std::sync::PoisonError;
use tokio::sync::Mutex as TokioMutex;

/// Sends input to a tab's live local, SSH or serial channel.
pub fn write_pty(
    app: &ConnectionContext,
    state: &PtyMap,
    zmodem_state: &ZmodemMap,
    key: &SessionKey,
    data: &[u8],
) -> Result<(), String> {
    let map = state.blocking_read();
    let session = map
        .get(&key.tab_id)
        .ok_or_else(|| format!("PTY session {} not found", key.tab_id))?;
    if session.session_nonce != key.session_nonce {
        return Ok(());
    }

    if intercept_zmodem_input(
        zmodem_state,
        &key.tab_id,
        key.session_nonce,
        data,
        &session.active,
    ) {
        return Ok(());
    }

    let mut active_guard = session.active.blocking_lock();
    let active = active_guard
        .as_mut()
        .ok_or_else(|| format!("PTY session {} is reconnecting", key.tab_id))?;

    let result = match active {
        ActiveSession::Local(local) => local
            .input
            .send(data.to_vec())
            .map_err(|error| format!("Failed to write to PTY: {error}")),
        ActiveSession::Ssh(ssh) => ssh
            .input
            .send(ssh.encoding.encode_input(data).into_owned())
            .map_err(|_| format!("PTY session {} is not writable", key.tab_id)),
        ActiveSession::Serial(serial) => serial
            .input
            .send(serial.encode_input(data).into_owned())
            .map_err(|error| format!("Failed to write to the serial port: {error}")),
    };
    if result.is_ok() {
        session.sink.note_input();
        crate::session_log::record_input(&app.log, &key.tab_id, data);
    }
    result
}

/// Resizes a tab's local or SSH channel.
pub fn resize_pty(
    app: &ConnectionContext,
    state: &PtyMap,
    key: &SessionKey,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    let map = state.blocking_read();
    let session = map
        .get(&key.tab_id)
        .ok_or_else(|| format!("PTY session {} not found", key.tab_id))?;
    if session.session_nonce != key.session_nonce {
        return Ok(());
    }

    // Record the size while reconnecting so the next attempt adopts it.
    session.size.store(rows, cols);

    let mut active_guard = session.active.blocking_lock();
    let active = active_guard
        .as_mut()
        .ok_or_else(|| format!("PTY session {} is reconnecting", key.tab_id))?;

    let result = match active {
        ActiveSession::Local(local) => local
            .master
            .resize(portable_pty::PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| format!("Failed to resize PTY: {error}")),
        ActiveSession::Ssh(ssh) => ssh
            .resize_tx
            .send((rows, cols))
            .map_err(|_| format!("PTY session {} is not resizable", key.tab_id)),
        // A serial line has no terminal size, but the size is still logged.
        ActiveSession::Serial(_) => Ok(()),
    };
    if result.is_ok() {
        crate::session_log::record_resize(&app.log, &key.tab_id, rows, cols);
    }
    result
}

/// Stops a tab's session after checking that the caller owns its generation.
pub fn kill_pty(
    app: &ConnectionContext,
    state: &PtyMap,
    zmodem_state: &ZmodemMap,
    zmodem_armed_send_state: &ZmodemArmedSendMap,
    zmodem_manual_detect_state: &ZmodemManualDetectMap,
    key: &SessionKey,
) -> Result<(), String> {
    let session = {
        let mut sessions = state.blocking_write();
        if sessions
            .get(&key.tab_id)
            .is_some_and(|session| session.session_nonce == key.session_nonce)
        {
            sessions.remove(&key.tab_id)
        } else {
            None
        }
    };

    if let Some(session) = session {
        stop_pty_session(
            StopPtyContext {
                prompts: &app.prompts,
                tab_id: &key.tab_id,
            },
            session,
            zmodem_state,
            zmodem_armed_send_state,
            zmodem_manual_detect_state,
        );
        crate::session_log::end_session(&app.log, &key.tab_id);
    }

    Ok(())
}

/// While a ZMODEM transfer occupies the channel, suppresses ordinary input.
/// Ctrl+C cancels the transfer and sends its standard abort sequence.
fn intercept_zmodem_input(
    zmodem_state: &ZmodemMap,
    tab_id: &str,
    session_nonce: u32,
    data: &[u8],
    active: &std::sync::Arc<TokioMutex<Option<ActiveSession>>>,
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
