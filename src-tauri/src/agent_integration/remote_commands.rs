//! The Tauri commands for agent integration on an SSH host.

use crate::{agent_integration::Agent, agent_integration::AgentIntegrationStatus};
use crate::{agent_integration::remote::remote_change, agent_integration::remote::remote_status};
use std::sync::Arc;
use tterm_core::core::session::types::PtyConnectionOptions;
use tterm_core::sftp::internal::api::SftpHost;

/// Each agent's integration on the tab's SSH host.
#[tauri::command]
pub async fn remote_agent_integration_status(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    services: tauri::State<'_, Arc<crate::services::AppServices>>,
) -> Result<Vec<AgentIntegrationStatus>, String> {
    let app = crate::connection_context::sftp(&services);
    let handles = SftpHost {
        app: &app,
        tab_id: &tab_id,
        connection,
        prompts: services.shared.prompts.clone(),
        secret_state: &services.shared.secrets,
        pool: &services.sftp.pool,
    }
    .handles()
    .await?;
    remote_status(handles).await
}

/// Installs or removes `agent`'s integration on the tab's SSH host.
#[tauri::command]
pub async fn change_remote_agent_integration(
    tab_id: String,
    connection: Option<PtyConnectionOptions>,
    agent: Agent,
    install: bool,
    services: tauri::State<'_, Arc<crate::services::AppServices>>,
) -> Result<AgentIntegrationStatus, String> {
    let app = crate::connection_context::sftp(&services);
    let handles = SftpHost {
        app: &app,
        tab_id: &tab_id,
        connection,
        prompts: services.shared.prompts.clone(),
        secret_state: &services.shared.secrets,
        pool: &services.sftp.pool,
    }
    .handles()
    .await?;
    remote_change(handles, agent, install).await
}
