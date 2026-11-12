#[path = "base_commands.rs"]
pub(crate) mod commands;
mod operations;
use crate::core::session::types::SessionPlan;
use crate::sftp::internal::connection::map_sftp_error;
use crate::sftp::store;
use russh_sftp::client::SftpSession;
use russh_sftp::client::fs::Metadata;
use tterm_core::sftp::entry::SftpDirectoryEntry;

fn push_unique_candidate(candidates: &mut Vec<String>, path: impl Into<String>) {
    let path = path.into();
    if !path.trim().is_empty() && !candidates.iter().any(|candidate| candidate == &path) {
        candidates.push(path);
    }
}

fn initial_directory_candidates(plan: &SessionPlan) -> Result<Vec<String>, String> {
    let host = plan.host.as_ref().ok_or("Host is required")?;
    let username = plan.username.as_ref().ok_or("Username is required")?;

    let mut candidates = Vec::new();
    if let Ok(Some(last_path)) = store::get_last_directory(host, plan.port, username) {
        push_unique_candidate(&mut candidates, last_path);
    }
    push_unique_candidate(&mut candidates, ".");
    push_unique_candidate(&mut candidates, "/");

    Ok(candidates)
}

async fn resolve_initial_directory(
    sftp: &SftpSession,
    plan: &SessionPlan,
) -> Result<String, String> {
    let mut last_error = None;

    for path in initial_directory_candidates(plan)? {
        match sftp.canonicalize(&path).await {
            Ok(canonical) => match sftp.read_dir(&canonical).await {
                Ok(_) => return Ok(canonical),
                Err(err) => last_error = Some(map_sftp_error(err)),
            },
            Err(err) => last_error = Some(map_sftp_error(err)),
        }
    }

    match last_error {
        Some(error) => Err(format!(
            "Failed to access any directory (last saved, home, or root): {error}"
        )),
        None => Err("Failed to access any directory (last saved, home, or root)".to_string()),
    }
}

/// The entry for `name` in the folder `parent`, as a listing shows it.
pub(crate) fn directory_entry(
    parent: &str,
    name: String,
    metadata: &Metadata,
) -> SftpDirectoryEntry {
    SftpDirectoryEntry {
        path: crate::sftp::internal::paths::join_remote_path(parent, &name),
        name,
        is_dir: metadata.is_dir(),
        is_symlink: metadata.is_symlink(),
        size: metadata.size,
        modified_at: metadata.mtime.map(|value| value as i64 * 1000),
        permissions: metadata
            .permissions
            .map(|_| metadata.permissions().to_string()),
        mode: metadata
            .permissions
            .map(|mode| mode & super::permissions::PERMISSION_MASK),
        owner: metadata
            .user
            .clone()
            .or_else(|| metadata.uid.map(|value| value.to_string())),
        group: metadata
            .group
            .clone()
            .or_else(|| metadata.gid.map(|value| value.to_string())),
    }
}

async fn read_directory_entries(
    sftp: &SftpSession,
    current_path: &str,
) -> Result<Vec<SftpDirectoryEntry>, String> {
    let mut entries = sftp
        .read_dir(current_path)
        .await
        .map_err(map_sftp_error)?
        .map(|entry| directory_entry(current_path, entry.file_name(), &entry.metadata()))
        .collect::<Vec<_>>();

    crate::sftp::internal::paths::sort_entries(&mut entries);
    Ok(entries)
}
