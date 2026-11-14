//! The Tauri commands for agent integration on this computer.

use tterm_core::agent_integration::AGENTS;
use tterm_core::agent_integration::Agent;
use tterm_core::agent_integration::AgentIntegrationStatus;

/// Off the async runtime: the first lookup may start the user's shell.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn agent_integration_status() -> Result<Vec<AgentIntegrationStatus>, String> {
    blocking(|| {
        AGENTS
            .into_iter()
            .map(tterm_core::agent_integration::status)
            .collect()
    })
    .await
}

#[tauri::command]
pub async fn install_agent_integration(agent: Agent) -> Result<AgentIntegrationStatus, String> {
    blocking(move || tterm_core::agent_integration::change(agent, true)).await?
}

#[tauri::command]
pub async fn uninstall_agent_integration(agent: Agent) -> Result<AgentIntegrationStatus, String> {
    blocking(move || tterm_core::agent_integration::change(agent, false)).await?
}
