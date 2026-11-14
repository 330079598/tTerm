use crate::core::batch_write::PtyWriteTarget;

pub struct WritePtyInput {
    pub tab_id: String,
    pub session_nonce: u32,
    pub data: String,
}

pub struct ResetPtyConsoleInput {
    pub tab_id: String,
    pub session_nonce: u32,
}

pub struct WritePtyBatchInput {
    pub targets: Vec<PtyWriteTarget>,
    pub guard_target: Option<PtyWriteTarget>,
    pub data: String,
}

pub struct ResizePtyInput {
    pub tab_id: String,
    pub session_nonce: u32,
    pub rows: u16,
    pub cols: u16,
}

pub struct KillPtyInput {
    pub tab_id: String,
    pub session_nonce: u32,
}

pub struct RespondSshHostKeyPromptInput {
    pub request_id: String,
    pub trust: bool,
}

pub struct HasSavedPasswordInput {
    pub profile_id: Option<String>,
    pub profile_name: Option<String>,
}

pub struct GetSudoPasswordSourceInput {
    pub profile_id: Option<String>,
    pub profile_name: Option<String>,
}

pub struct WriteSavedPasswordForSudoInput {
    pub tab_id: String,
    pub session_nonce: u32,
    pub profile_id: Option<String>,
    pub profile_name: Option<String>,
    pub prompt: String,
}

pub struct SudoPasswordContext<'a> {
    pub app: &'a crate::ssh::context::ConnectionContext,
    pub secret_state: &'a crate::ssh::secret_store::types::SecretStoreState,
}
