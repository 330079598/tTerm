use tterm_core::serial::history::DeleteSerialSendInput;
use tterm_core::serial::history::RecordSerialSendInput;
use tterm_core::serial::history::SerialSend;
use tterm_core::serial::history::SerialSendMode;
use tterm_core::serial::history::clear_serial_sends_service;
use tterm_core::serial::history::delete_serial_send_service;
use tterm_core::serial::history::list_serial_sends_service;
use tterm_core::serial::history::record_serial_send_service;

/// The connection's sends, newest first.
#[tauri::command]
pub async fn list_serial_sends(connection_key: String) -> Result<Vec<SerialSend>, String> {
    list_serial_sends_service(connection_key).await
}

/// Records a send, moving it to the top when it was sent before; returns
/// the connection's sends.
#[tauri::command]
pub async fn record_serial_send(
    connection_key: String,
    mode: SerialSendMode,
    text: String,
) -> Result<Vec<SerialSend>, String> {
    record_serial_send_service(RecordSerialSendInput {
        connection_key,
        mode,
        text,
    })
    .await
}

/// Forgets one send; returns the connection's sends.
#[tauri::command]
pub async fn delete_serial_send(
    connection_key: String,
    id: String,
) -> Result<Vec<SerialSend>, String> {
    delete_serial_send_service(DeleteSerialSendInput { connection_key, id }).await
}

/// Forgets everything the connection's send bar sent.
#[tauri::command]
pub async fn clear_serial_sends(connection_key: String) -> Result<(), String> {
    clear_serial_sends_service(connection_key).await
}
