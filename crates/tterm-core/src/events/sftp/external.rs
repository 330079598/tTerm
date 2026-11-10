use serde::Serialize;

/// The state of a local copy opened in an external editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EditStatus {
    /// The copy matches the server as last downloaded or uploaded.
    Synced,
    /// The copy has saved changes that are not on the server.
    Modified,
    Uploading,
    /// The last upload failed, so the changes are still only in the copy.
    Failed,
}

/// A remote file open in a program here, as the drawer lists it.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExternalEdit {
    pub id: String,
    pub tab_id: String,
    pub remote_path: String,
    pub file_name: String,
    pub local_path: String,
    pub status: EditStatus,
    pub error: Option<String>,
}
