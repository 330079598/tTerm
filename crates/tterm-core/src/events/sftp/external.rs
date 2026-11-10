use serde::Serialize;

/// The state of a local copy opened in an external editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EditStatus {
    /// The copy matches the server as last downloaded or uploaded.
    Synced,
    /// The copy has saved changes that are not on the server.
    Modified,
    /// Saved local changes are being sent to the server.
    Uploading,
    /// The last upload failed, so the changes are still only in the copy.
    Failed,
}

/// A remote file open in a program here, as the drawer lists it.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExternalEdit {
    /// Identifies the watched copy across successive state snapshots.
    pub id: String,
    /// The terminal tab that owns the remote connection.
    pub tab_id: String,
    pub remote_path: String,
    pub file_name: String,
    /// The local working copy opened by the external editor.
    pub local_path: String,
    pub status: EditStatus,
    /// The last upload failure, when the status is Failed.
    pub error: Option<String>,
}
