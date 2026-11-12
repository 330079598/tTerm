use super::auth::{RespondSshAuthPromptInput, respond_ssh_auth_prompt_service};

/// Delivers the user's answers to a waiting prompt; `responses: None` cancels.
#[tauri::command]
pub fn respond_ssh_auth_prompt(
    request_id: String,
    responses: Option<Vec<String>>,
    services: tauri::State<'_, std::sync::Arc<crate::services::AppServices>>,
) -> Result<(), String> {
    let pending = services.shared.prompts.clone();
    respond_ssh_auth_prompt_service(
        pending,
        RespondSshAuthPromptInput {
            request_id,
            responses,
        },
    )
}
