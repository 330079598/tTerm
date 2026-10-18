//! Sending files to a serial device over XMODEM, XMODEM-1K or YMODEM, the
//! way bootloaders take firmware. While a transfer runs it has the port: the
//! input goes to it instead of the terminal, and typing is held back as it
//! is during ZMODEM, whose cancel and transfer rows it shares.

mod disk;
mod protocol;
mod receive;
#[cfg(test)]
mod receive_tests;
#[cfg(test)]
mod tests;
mod transfer;

use crate::core::blocking::run_blocking;
use crate::core::state::{PtyMap, ZmodemMap, ZmodemTabHandle};
use crate::zmodem::events::EmitFn;
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

/// Takes a serial tab's port for a transfer: its input stops reaching the
/// terminal and typing is held back, until the claim is released.
async fn claim_port(
    app: AppHandle,
    state: &PtyMap,
    zmodem_state: &ZmodemMap,
    tab_id: String,
    session_nonce: u32,
) -> Result<(EmitFn, transfer::PortClaim, transfer::TapLink), String> {
    let (writer, tap) = super::with_serial(state, &tab_id, session_nonce, |serial| {
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
    let emit: EmitFn = Box::new(move |kind, payload| {
        let event = format!("zmodem-{kind}-{emit_tab_id}");
        let _ = app.emit_to(tauri::EventTarget::any(), &event, payload);
    });
    let claim = transfer::PortClaim {
        tab_id,
        session_nonce,
        transfers: zmodem_state.clone(),
        tap,
    };
    Ok((
        emit,
        claim,
        transfer::TapLink::new(incoming, writer, cancel_requested),
    ))
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
    let (emit, claim, link) = claim_port(app, &state, &zmodem_state, tab_id, session_nonce).await?;
    let params = transfer::TransferParams {
        emit,
        claim,
        link,
        protocol,
        files,
    };
    std::thread::spawn(move || transfer::run(params));
    Ok(())
}

/// Receives files a serial tab's device sends: over XMODEM into the file
/// `destination`, over YMODEM into the folder `destination`, under the names
/// the device gives. Returns once the transfer starts.
#[tauri::command]
#[allow(
    clippy::too_many_arguments,
    reason = "a Tauri command's arguments are its IPC fields and injected state"
)]
pub async fn serial_receive_files(
    app: AppHandle,
    tab_id: String,
    session_nonce: u32,
    protocol: XmodemProtocol,
    destination: String,
    state: State<'_, PtyMap>,
    zmodem_state: State<'_, ZmodemMap>,
) -> Result<(), String> {
    let destination = run_blocking(move || disk::Destination::new(protocol, &destination)).await?;
    let (emit, claim, link) = claim_port(app, &state, &zmodem_state, tab_id, session_nonce).await?;
    std::thread::spawn(move || disk::run_receive(emit, claim, link, protocol, destination));
    Ok(())
}
