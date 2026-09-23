//! Tauri commands for manual ZMODEM control: `zmodem_cancel` covers both
//! directions; `zmodem_start_send` starts an upload once the frontend has
//! resolved which local files to send in response to a
//! `zmodem-send-requested-{tabId}` event; `zmodem_arm_manual_detect` backs
//! the "Send/Receive Files via ZMODEM" keymap actions, the fallback for
//! when auto-detection is off or missed a trigger.

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, State};

use crate::core::state::ActiveSession;
use crate::core::{PtyMap, ZmodemArmedSendMap, ZmodemManualDetectMap, ZmodemMap};

/// Cancels an in-progress or armed-but-not-yet-started ZMODEM transfer for
/// `tab_id`: flags it so a running send/receive loop stops treating further
/// bytes as protocol on its next iteration, best-effort writes the standard
/// abort sequence straight to the peer so a real `rz`/`sz` notices
/// immediately instead of waiting out its own timeout, and — if a send was
/// armed but the user never finished picking files — tears down that
/// registration directly, since nothing is polling `cancel_requested` yet.
#[tauri::command]
pub fn zmodem_cancel(
    tab_id: String,
    session_nonce: u32,
    pty_state: State<'_, PtyMap>,
    zmodem_state: State<'_, ZmodemMap>,
    armed_send_state: State<'_, ZmodemArmedSendMap>,
) -> Result<(), String> {
    {
        let map = zmodem_state.read().unwrap();
        match map.get(&tab_id) {
            Some(handle) if handle.session_nonce == session_nonce => {
                handle.cancel_requested.store(true, Ordering::Relaxed);
            }
            _ => return Ok(()),
        }
    }

    {
        let mut armed = armed_send_state.lock().unwrap();
        if armed
            .get(&tab_id)
            .is_some_and(|entry| entry.session_nonce == session_nonce)
        {
            armed.remove(&tab_id);
            zmodem_state.write().unwrap().remove(&tab_id);
        }
    }

    let sessions = pty_state.blocking_read();
    if let Some(session) = sessions.get(&tab_id) {
        if session.session_nonce == session_nonce {
            let mut active_guard = session.active.blocking_lock();
            match active_guard.as_mut() {
                Some(ActiveSession::Local(local)) => {
                    let _ = local.writer.write_all(&super::session::abort_sequence());
                }
                Some(ActiveSession::Ssh(ssh)) => {
                    let _ = ssh.input_tx.send(super::session::abort_sequence());
                }
                None => {}
            }
        }
    }

    Ok(())
}

/// Starts sending `local_paths` in response to a `zmodem-send-requested`
/// event (an `rz`-style trigger already armed the tab — see `arm_send`).
/// Spawns a dedicated thread and returns immediately; progress flows back
/// through the same `zmodem-transfer-*-{tabId}` events the receive
/// direction uses.
#[tauri::command]
pub fn zmodem_start_send(
    app: AppHandle,
    tab_id: String,
    session_nonce: u32,
    local_paths: Vec<String>,
    zmodem_state: State<'_, ZmodemMap>,
    armed_send_state: State<'_, ZmodemArmedSendMap>,
) -> Result<(), String> {
    let cancel_requested = {
        let map = zmodem_state.read().unwrap();
        match map.get(&tab_id) {
            Some(handle) if handle.session_nonce == session_nonce => {
                handle.cancel_requested.clone()
            }
            _ => return Err(format!("No armed ZMODEM send session for tab {tab_id}")),
        }
    };

    let armed = {
        let mut map = armed_send_state.lock().unwrap();
        match map.remove(&tab_id) {
            Some(armed) if armed.session_nonce == session_nonce => armed,
            _ => return Err(format!("No armed ZMODEM send session for tab {tab_id}")),
        }
    };

    let paths: Vec<PathBuf> = local_paths.into_iter().map(PathBuf::from).collect();
    let zmodem_map_for_thread = zmodem_state.inner().clone();
    let emit_tab_id = tab_id.clone();
    let emit: super::events::EmitFn = Box::new(move |kind, payload| {
        let event = format!("zmodem-{kind}-{emit_tab_id}");
        let _ = app.emit_to(tauri::EventTarget::any(), &event, payload);
    });

    std::thread::spawn(move || {
        super::send::run(super::send::ZmodemSendParams {
            emit,
            tab_id,
            zmodem_map: zmodem_map_for_thread,
            incoming: armed.incoming_rx,
            active: armed.active,
            cancel_requested,
            paths,
        });
    });

    Ok(())
}

/// Forces ZMODEM detection on for `tab_id`'s very next `rz`/`sz` invite,
/// even if `zmodem_auto_detect_enabled` is off — the fallback for a user
/// who keeps auto-detect off but wants it for one transfer, or whose
/// trigger genuinely got missed. Consumed by that next trigger (or stays
/// armed indefinitely if none comes — this is an explicit, rare,
/// user-initiated action, not passive background scanning, so there's no
/// time limit to get right).
#[tauri::command]
pub fn zmodem_arm_manual_detect(
    tab_id: String,
    session_nonce: u32,
    pty_state: State<'_, PtyMap>,
    manual_detect_state: State<'_, ZmodemManualDetectMap>,
) -> Result<(), String> {
    let sessions = pty_state.blocking_read();
    match sessions.get(&tab_id) {
        Some(session) if session.session_nonce == session_nonce => {}
        _ => return Err(format!("PTY session {tab_id} not found")),
    }

    if let Some(flag) = manual_detect_state.lock().unwrap().get(&tab_id) {
        flag.store(true, Ordering::Relaxed);
    }

    Ok(())
}
