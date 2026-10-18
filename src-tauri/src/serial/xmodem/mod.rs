//! Sending files to a serial device over XMODEM, XMODEM-1K or YMODEM, the
//! way bootloaders take firmware. While a transfer runs it has the port: the
//! input goes to it instead of the terminal, and typing is held back as it
//! is during ZMODEM, whose cancel and transfer rows it shares.

mod protocol;
#[cfg(test)]
mod tests;
mod transfer;

use crate::core::blocking::run_blocking;
use crate::core::state::{PtyMap, ZmodemMap, ZmodemTabHandle};
use serde::Deserialize;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc, PoisonError};
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum XmodemProtocol {
    /// 128-byte blocks, one file.
    Xmodem,
    /// 1024-byte blocks, one file.
    Xmodem1k,
    /// 1024-byte blocks with each file's name and size, several files.
    Ymodem,
}

/// Sends files to a serial tab's device. Returns once the transfer starts;
/// its progress shows in the transfer list, like a ZMODEM transfer's.
#[tauri::command]
#[allow(
    clippy::too_many_arguments,
    reason = "a Tauri command's arguments are its IPC fields and injected state"
)]
pub async fn serial_send_files(
    app: AppHandle,
    tab_id: String,
    session_nonce: u32,
    protocol: XmodemProtocol,
    local_paths: Vec<String>,
    state: State<'_, PtyMap>,
    zmodem_state: State<'_, ZmodemMap>,
) -> Result<(), String> {
    if local_paths.is_empty() {
        return Ok(());
    }
    if protocol != XmodemProtocol::Ymodem && local_paths.len() > 1 {
        return Err("XMODEM sends one file at a time; use YMODEM to send several".to_string());
    }
    let files = run_blocking(move || transfer::open_files(&local_paths)).await?;
    let (writer, tap) = super::with_serial(&state, &tab_id, session_nonce, |serial| {
        (serial.writer.clone(), serial.tap.clone())
    })
    .await?;

    let cancel_requested = Arc::new(AtomicBool::new(false));
    {
        let mut transfers = zmodem_state.write().unwrap_or_else(PoisonError::into_inner);
        if transfers.contains_key(&tab_id) {
            return Err("A file transfer is already running on this port".to_string());
        }
        transfers.insert(
            tab_id.clone(),
            ZmodemTabHandle {
                session_nonce,
                transfer_id: crate::zmodem::events::new_transfer_id(),
                cancel_requested: cancel_requested.clone(),
            },
        );
    }
    let (sender, incoming) = mpsc::channel();
    *tap.lock().unwrap_or_else(PoisonError::into_inner) = Some(sender);

    let emit_tab_id = tab_id.clone();
    let emit: crate::zmodem::events::EmitFn = Box::new(move |kind, payload| {
        let event = format!("zmodem-{kind}-{emit_tab_id}");
        let _ = app.emit_to(tauri::EventTarget::any(), &event, payload);
    });
    let params = transfer::TransferParams {
        emit,
        tab_id,
        session_nonce,
        transfers: zmodem_state.inner().clone(),
        tap,
        link: transfer::TapLink::new(incoming, writer, cancel_requested),
        protocol,
        files,
    };
    std::thread::spawn(move || transfer::run(params));
    Ok(())
}
