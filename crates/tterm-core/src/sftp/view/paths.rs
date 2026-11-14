//! The path strings the SFTP drawer builds, from
//! `src/components/SftpDrawer/sftpDrawerUtils.ts`.

/// Appends a name to the directory shown in the drawer.
///
/// The drawer keeps the text the server gave it, so a trailing slash remains
/// when the name does not start with one.
pub fn join_remote_path(base_path: &str, name: &str) -> String {
    if base_path == "/" {
        format!("/{name}")
    } else {
        format!("{base_path}/{name}")
    }
}

/// The folder holding `path`; `/` for an entry of the root.
pub fn parent_remote_path(path: &str) -> String {
    match path.rfind('/') {
        Some(slash) if slash > 0 => path[..slash].to_owned(),
        _ => "/".to_owned(),
    }
}

/// Whether the server refused a request because the account may not do it.
pub fn is_permission_denied(error: Option<&str>) -> bool {
    error.is_some_and(|error| error.to_ascii_lowercase().contains("permission denied"))
}
