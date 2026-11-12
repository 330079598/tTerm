use super::approval::{
    RespondProxyCommandPromptInput, pending_proxy_command_prompts_service,
    respond_proxy_command_prompt_service,
};
use tterm_core::events::proxy::ProxyCommandPrompt;

/// The questions still waiting, for a window that opens after they were asked.
#[tauri::command]
pub fn pending_proxy_command_prompts(
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Vec<ProxyCommandPrompt> {
    let prompts = services.shared.prompts.clone();
    pending_proxy_command_prompts_service(prompts)
}

/// Answers a question; `allow` also remembers the command on this device.
#[tauri::command]
pub async fn respond_proxy_command_prompt(
    request_id: String,
    allow: bool,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    let prompts = services.shared.prompts.clone();
    respond_proxy_command_prompt_service(
        prompts,
        RespondProxyCommandPromptInput { request_id, allow },
    )
    .await
}
