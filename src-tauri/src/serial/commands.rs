use tterm_core::{
    serial::PulseSerialLineInput, serial::SendSerialBreakInput, serial::SerialLine,
    serial::SerialPortEntry, serial::SetSerialSignalsInput, serial::SetSerialViewInput,
    serial::WriteSerialBytesInput, serial::list_serial_ports_service,
    serial::pulse_serial_line_service, serial::send_serial_break_service,
    serial::set_serial_signals_service, serial::set_serial_view_service,
    serial::write_serial_bytes_service,
};

#[tauri::command]
pub async fn list_serial_ports() -> Result<Vec<SerialPortEntry>, String> {
    list_serial_ports_service().await
}

/// Sets a serial tab's DTR and RTS lines; a line left out keeps its level.
#[tauri::command]
pub async fn set_serial_signals(
    tab_id: String,
    session_nonce: u32,
    dtr: Option<bool>,
    rts: Option<bool>,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<(), String> {
    let state = services.session.ptys.clone();
    set_serial_signals_service(
        state,
        SetSerialSignalsInput {
            tab_id,
            session_nonce,
            dtr,
            rts,
        },
    )
    .await
}

/// Inverts one line for a moment and restores it, the way boards such as the
/// ESP32 or Arduino are reset from their USB serial adapter.
#[tauri::command]
pub async fn pulse_serial_line(
    tab_id: String,
    session_nonce: u32,
    line: SerialLine,
    duration_ms: Option<u64>,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<(), String> {
    let state = services.session.ptys.clone();
    pulse_serial_line_service(
        state,
        PulseSerialLineInput {
            tab_id,
            session_nonce,
            line,
            duration_ms,
        },
    )
    .await
}

/// Sends bytes to a serial tab's port as they are, without the line ending or
/// charset typed text gets: what its send bar's hex mode sends.
#[tauri::command]
pub async fn write_serial_bytes(
    tab_id: String,
    session_nonce: u32,
    data: Vec<u8>,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<(), String> {
    let log = crate::connection_context::log(&services);
    let state = services.session.ptys.clone();
    let zmodem_state = services.session.zmodem.clone();
    write_serial_bytes_service(
        log,
        state,
        zmodem_state,
        WriteSerialBytesInput {
            tab_id,
            session_nonce,
            data,
        },
    )
    .await
}

/// Shows what a serial tab receives in hex, or as text again, from now on
/// or from when its device comes back.
#[tauri::command]
pub async fn set_serial_view(
    tab_id: String,
    session_nonce: u32,
    hex: bool,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<(), String> {
    let state = services.session.ptys.clone();
    set_serial_view_service(
        state,
        SetSerialViewInput {
            tab_id,
            session_nonce,
            hex,
        },
    )
    .await
}

#[tauri::command]
pub async fn send_serial_break(
    tab_id: String,
    session_nonce: u32,
    duration_ms: Option<u64>,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<(), String> {
    let state = services.session.ptys.clone();
    send_serial_break_service(
        state,
        SendSerialBreakInput {
            tab_id,
            session_nonce,
            duration_ms,
        },
    )
    .await
}
