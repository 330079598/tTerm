use tauri::ipc::Response;
use tterm_core::events::session_log::TerminalLogStatus;
use tterm_core::session_log::library;
use tterm_core::{
    session_log::ExportTerminalLogAsciicastInput, session_log::SaveTerminalContentsInput,
    session_log::SetTerminalLogRecordingInput, session_log::delete_terminal_log_service,
    session_log::delete_terminal_logs_service, session_log::export_terminal_log_asciicast_service,
    session_log::get_terminal_log_status_service, session_log::list_terminal_logs_service,
    session_log::load_terminal_log_recording_service, session_log::load_terminal_log_text_service,
    session_log::open_terminal_log_directory_service, session_log::retry_terminal_logging_service,
    session_log::reveal_terminal_log_service, session_log::save_terminal_contents_service,
    session_log::set_terminal_log_recording_service, session_log::terminal_log_ids_for_tab_service,
};

/// The logging status; the disk usage in it is remeasured in the background
/// and sent again with `terminal-log-status` if it changed.
#[tauri::command]
pub fn get_terminal_log_status(
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<TerminalLogStatus, String> {
    let app = crate::connection_context::log(&services);
    let state = services.app.session_log.clone();
    get_terminal_log_status_service(app, state)
}

/// The logs a tab has written since tTerm started, oldest first.
#[tauri::command]
pub fn terminal_log_ids_for_tab(
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
    tab_id: String,
) -> Result<Vec<String>, String> {
    let state = services.app.session_log.clone();
    terminal_log_ids_for_tab_service(state, tab_id)
}

/// Starts or stops logging one tab, whatever the settings say, until the
/// tab closes.
#[tauri::command]
pub async fn set_terminal_log_recording(
    tab_id: String,
    recording: bool,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<TerminalLogStatus, String> {
    let app = crate::connection_context::log(&services);
    set_terminal_log_recording_service(app, SetTerminalLogRecordingInput { tab_id, recording })
        .await
}

/// The saved sessions in the log directory, newest first.
#[tauri::command]
pub async fn list_terminal_logs(
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<Vec<library::LogSession>, String> {
    let app = crate::connection_context::log(&services);
    list_terminal_logs_service(app).await
}

/// A session's raw log as replay frames (see `library::load_recording`).
#[tauri::command]
pub async fn load_terminal_log_recording(
    id: String,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<Response, String> {
    let app = crate::connection_context::log(&services);
    load_terminal_log_recording_service(app, id)
        .await
        .map(Response::new)
}

/// A session's plain-text log as UTF-8 bytes.
#[tauri::command]
pub async fn load_terminal_log_text(
    id: String,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<Response, String> {
    let app = crate::connection_context::log(&services);
    load_terminal_log_text_service(app, id)
        .await
        .map(Response::new)
}

#[tauri::command]
pub async fn export_terminal_log_asciicast(
    id: String,
    path: String,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<(), String> {
    let app = crate::connection_context::log(&services);
    export_terminal_log_asciicast_service(app, ExportTerminalLogAsciicastInput { id, path }).await
}

#[tauri::command]
pub async fn delete_terminal_log(
    id: String,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<(), String> {
    let app = crate::connection_context::log(&services);
    delete_terminal_log_service(app, id).await
}

/// Deletes several sessions; one that cannot be deleted (such as one still
/// being logged) is reported and does not stop the rest.
#[tauri::command]
pub async fn delete_terminal_logs(
    ids: Vec<String>,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<library::DeleteReport, String> {
    let app = crate::connection_context::log(&services);
    delete_terminal_logs_service(app, ids).await
}

#[tauri::command]
pub async fn reveal_terminal_log(
    id: String,
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<(), String> {
    let app = crate::connection_context::log(&services);
    let open = services.shared.platform.open.clone();
    reveal_terminal_log_service(app, open, id).await
}

/// Saves what a terminal shows (its text) to a file the user picked.
#[tauri::command]
pub async fn save_terminal_contents(path: String, contents: String) -> Result<(), String> {
    save_terminal_contents_service(SaveTerminalContentsInput { path, contents }).await
}

#[tauri::command]
pub async fn open_terminal_log_directory(
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<(), String> {
    let app = crate::connection_context::log(&services);
    let open = services.shared.platform.open.clone();
    open_terminal_log_directory_service(app, open).await
}

#[tauri::command]
pub async fn retry_terminal_logging(
    services: tauri::State<'_, std::sync::Arc<tterm_core::services::AppServices>>,
) -> Result<TerminalLogStatus, String> {
    let app = crate::connection_context::log(&services);
    retry_terminal_logging_service(app).await
}
