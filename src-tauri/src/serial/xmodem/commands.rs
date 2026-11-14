use tterm_core::serial::xmodem::SerialReceiveFilesInput;
use tterm_core::serial::xmodem::SerialSendFilesInput;
use tterm_core::serial::xmodem::XmodemProtocol;
use tterm_core::serial::xmodem::serial_receive_files_service;
use tterm_core::serial::xmodem::serial_send_files_service;

/// Sends files to a serial tab's device. Returns once the transfer starts;
/// its progress shows in the transfer list, like a ZMODEM transfer's.
#[tauri::command]
pub async fn serial_send_files(
    tab_id: String,
    session_nonce: u32,
    protocol: XmodemProtocol,
    local_paths: Vec<String>,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    let events = services.shared.events.clone();
    let state = services.session.ptys.clone();
    let zmodem_state = services.session.zmodem.clone();
    serial_send_files_service(
        events,
        state,
        zmodem_state,
        SerialSendFilesInput {
            tab_id,
            session_nonce,
            protocol,
            local_paths,
        },
    )
    .await
}

/// Receives files a serial tab's device sends: over XMODEM into the file
/// `destination`, over YMODEM into the folder `destination`, under the names
/// the device gives. Returns once the transfer starts.
#[tauri::command]
pub async fn serial_receive_files(
    tab_id: String,
    session_nonce: u32,
    protocol: XmodemProtocol,
    destination: String,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    let events = services.shared.events.clone();
    let state = services.session.ptys.clone();
    let zmodem_state = services.session.zmodem.clone();
    serial_receive_files_service(
        events,
        state,
        zmodem_state,
        SerialReceiveFilesInput {
            tab_id,
            session_nonce,
            protocol,
            destination,
        },
    )
    .await
}
