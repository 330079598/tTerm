use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub const HOST_KEY_PROMPT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
pub const HOST_KEY_REJECTED_REASON: &str = "SSH host fingerprint rejected by user";

#[derive(Clone)]
pub struct SshClientHandler {
    pub app: tauri::AppHandle,
    pub tab_id: String,
    pub profile_id: Option<String>,
    pub profile_name: String,
    pub host: String,
    pub port: u16,
    pub prompts: crate::core::state::HostPromptMap,
    pub user_rejected_host_key: Arc<AtomicBool>,
}
