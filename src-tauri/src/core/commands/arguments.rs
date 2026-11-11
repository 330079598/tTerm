use crate::core::commands::batch_write::PtyWriteTarget;

pub(super) struct WritePtyInput {
    pub(super) tab_id: String,
    pub(super) session_nonce: u32,
    pub(super) data: String,
}

pub(super) struct ResetPtyConsoleInput {
    pub(super) tab_id: String,
    pub(super) session_nonce: u32,
}

pub(super) struct AckPtyOutputInput {
    pub(super) tab_id: String,
    pub(super) session_nonce: u32,
    pub(super) bytes: usize,
}

pub(super) struct WritePtyBatchInput {
    pub(super) targets: Vec<PtyWriteTarget>,
    pub(super) guard_target: Option<PtyWriteTarget>,
    pub(super) data: String,
}

pub(super) struct ResizePtyInput {
    pub(super) tab_id: String,
    pub(super) session_nonce: u32,
    pub(super) rows: u16,
    pub(super) cols: u16,
}

pub(super) struct KillPtyInput {
    pub(super) tab_id: String,
    pub(super) session_nonce: u32,
}

pub(super) struct RespondSshHostKeyPromptInput {
    pub(super) request_id: String,
    pub(super) trust: bool,
}

pub(super) struct HasSavedPasswordInput {
    pub(super) profile_id: Option<String>,
    pub(super) profile_name: Option<String>,
}

pub(super) struct GetSudoPasswordSourceInput {
    pub(super) profile_id: Option<String>,
    pub(super) profile_name: Option<String>,
}

pub(super) struct WriteSavedPasswordForSudoInput {
    pub(super) tab_id: String,
    pub(super) session_nonce: u32,
    pub(super) profile_id: Option<String>,
    pub(super) profile_name: Option<String>,
    pub(super) prompt: String,
}
