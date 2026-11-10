use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SftpDirectoryEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: Option<u64>,
    pub modified_at: Option<i64>,
    pub permissions: Option<String>,
    /// Permission bits without the file type, for editing; `permissions` is
    /// the same value rendered like `ls -l`.
    pub mode: Option<u32>,
    pub owner: Option<String>,
    pub group: Option<String>,
}
