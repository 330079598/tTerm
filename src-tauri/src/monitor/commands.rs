use super::{
    GetServerMetricsSnapshotInput, ReleaseServerMonitorSessionInput,
    get_server_metrics_snapshot_service, release_server_monitor_session_service,
};
use crate::core::session::types::PtyConnectionOptions;
use crate::monitor::types::ServerMetricsSnapshot;

#[tauri::command]
pub async fn get_server_metrics_snapshot(
    tab_id: String,
    session_nonce: u32,
    connection: Option<PtyConnectionOptions>,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<ServerMetricsSnapshot, String> {
    let app = crate::connection_context::connection(&services);
    let prompt_state = services.shared.prompts.clone();
    let secret_state = services.shared.secrets.clone();
    let monitor_sessions = services.session.monitors.clone();
    let registry = services.session.monitor_prompts.clone();
    get_server_metrics_snapshot_service(
        app,
        prompt_state,
        secret_state,
        monitor_sessions,
        registry,
        GetServerMetricsSnapshotInput {
            tab_id,
            session_nonce,
            connection,
        },
    )
    .await
}

#[tauri::command]
pub async fn release_server_monitor_session(
    tab_id: String,
    session_nonce: Option<u32>,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    let app = crate::connection_context::connection(&services);
    let monitor_sessions = services.session.monitors.clone();
    let registry = services.session.monitor_prompts.clone();
    release_server_monitor_session_service(
        app,
        monitor_sessions,
        registry,
        ReleaseServerMonitorSessionInput {
            tab_id,
            session_nonce,
        },
    )
    .await
}
