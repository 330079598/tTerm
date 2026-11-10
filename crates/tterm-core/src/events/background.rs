use serde::Serialize;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuitBlockers {
    /// Port-forwarding tunnels running.
    pub tunnels: usize,
    /// Files opened from SFTP with changes not uploaded; their copies stay
    /// on disk, but nothing uploads them any more.
    pub unuploaded_files: Vec<String>,
}

impl QuitBlockers {
    pub fn is_empty(&self) -> bool {
        self.tunnels == 0 && self.unuploaded_files.is_empty()
    }
}
