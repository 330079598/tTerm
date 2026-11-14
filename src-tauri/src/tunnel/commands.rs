//! The Tauri commands for port-forwarding rules.

use std::sync::Arc;
use tterm_core::events::tunnel::TunnelStatus;
use tterm_core::tunnel::credentials::StartOutcome;
use tterm_core::tunnel::credentials::TunnelCredentials;
use tterm_core::tunnel::manager::auto_start_tunnels_service;
use tterm_core::tunnel::manager::delete_tunnel_service;
use tterm_core::tunnel::manager::list_tunnel_statuses_service;
use tterm_core::tunnel::manager::list_tunnels_service;
use tterm_core::tunnel::manager::reset_tunnel_traffic_totals_service;
use tterm_core::tunnel::manager::save_tunnel_service;
use tterm_core::tunnel::manager::start_rule;
use tterm_core::tunnel::types::TunnelRule;

#[tauri::command]
pub async fn list_tunnels() -> Result<Vec<TunnelRule>, String> {
    list_tunnels_service().await
}

#[tauri::command]
pub async fn list_tunnel_statuses(
    services: tauri::State<'_, Arc<crate::services::AppServices>>,
) -> Result<Vec<TunnelStatus>, String> {
    list_tunnel_statuses_service(&services.app.tunnels).await
}

#[tauri::command]
pub async fn reset_tunnel_traffic_totals(
    id: String,
    services: tauri::State<'_, Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    reset_tunnel_traffic_totals_service(id, &services.app.tunnels).await
}

#[tauri::command]
pub async fn save_tunnel(
    mut tunnel: TunnelRule,
    services: tauri::State<'_, Arc<crate::services::AppServices>>,
) -> Result<TunnelRule, String> {
    save_tunnel_service(&crate::app_context::tunnel(&services), &mut tunnel).await
}

#[tauri::command]
pub async fn delete_tunnel(
    id: String,
    services: tauri::State<'_, Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    delete_tunnel_service(
        &crate::app_context::tunnel(&services),
        id,
        &services.app.tunnels,
    )
    .await
}

#[tauri::command]
pub async fn start_tunnel(
    id: String,
    credentials: Option<TunnelCredentials>,
    services: tauri::State<'_, Arc<crate::services::AppServices>>,
) -> Result<StartOutcome, String> {
    start_rule(
        &crate::app_context::tunnel(&services),
        &services.app.tunnels,
        &id,
        &credentials.unwrap_or_default(),
        false,
    )
    .await
}

#[tauri::command]
pub async fn auto_start_tunnels(
    services: tauri::State<'_, Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    auto_start_tunnels_service(
        &crate::app_context::tunnel(&services),
        &services.app.tunnels,
    )
    .await
}

#[tauri::command]
pub async fn stop_tunnel(
    id: String,
    services: tauri::State<'_, Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    services.app.tunnels.stop(&id).await;
    Ok(())
}
