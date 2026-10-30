//! Moving an opened file between the server and its local copy.

use crate::sftp::internal::connection::map_sftp_error;
use russh_sftp::client::error::Error as SftpError;
use russh_sftp::client::fs::Metadata;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::{OpenFlags, StatusCode};
use std::path::Path;
use tokio::io::AsyncWriteExt;

/// The remote file as it was when last downloaded or uploaded. An upload
/// compares the file against it to tell whether someone changed it since.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RemoteStamp {
    pub size: u64,
    pub mtime: Option<u32>,
}

/// Why a folder cannot be opened like a file.
pub(super) const FOLDER_ERROR: &str = "Folders cannot be opened in another program";

impl RemoteStamp {
    pub(super) fn of(metadata: &Metadata) -> Self {
        Self {
            size: metadata.size.unwrap_or(0),
            mtime: metadata.mtime,
        }
    }
}

fn is_missing(error: &SftpError) -> bool {
    matches!(error, SftpError::Status(status) if status.status_code == StatusCode::NoSuchFile)
}

/// Opens a local file for writing a copy into; a new one is readable only by
/// this user.
async fn create_local(path: &Path) -> Result<tokio::fs::File, String> {
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    options.mode(0o600);
    options
        .open(path)
        .await
        .map_err(|error| format!("Failed to write '{}': {error}", path.display()))
}

/// Downloads the remote file to `local`, replacing what is there.
pub(super) async fn download(
    sftp: &SftpSession,
    remote: &str,
    local: &Path,
) -> Result<RemoteStamp, String> {
    let mut remote_file = sftp.open(remote).await.map_err(map_sftp_error)?;
    let metadata = remote_file.metadata().await.map_err(map_sftp_error)?;
    if metadata.is_dir() {
        return Err(FOLDER_ERROR.to_string());
    }
    let mut local_file = create_local(local).await?;
    tokio::io::copy(&mut remote_file, &mut local_file)
        .await
        .map_err(|error| format!("Failed to download '{remote}': {error}"))?;
    local_file
        .flush()
        .await
        .map_err(|error| format!("Failed to write '{}': {error}", local.display()))?;
    Ok(RemoteStamp::of(&metadata))
}

/// Uploads `local` over the remote file. Unless `overwrite` is set, nothing
/// is written and None comes back when the remote file changed, or went,
/// since `baseline`.
///
/// The file is written in place rather than replaced by a renamed copy, so
/// it keeps its owner, group and permissions, and a symlink stays a symlink
/// with its target changed. An upload cut off part way leaves the remote
/// file short; the local copy still has the changes for another upload.
pub(super) async fn upload(
    sftp: &SftpSession,
    remote: &str,
    local: &Path,
    baseline: RemoteStamp,
    overwrite: bool,
) -> Result<Option<RemoteStamp>, String> {
    if !overwrite {
        match sftp.metadata(remote).await {
            Ok(metadata) if RemoteStamp::of(&metadata) == baseline => {}
            Ok(_) => return Ok(None),
            Err(error) if is_missing(&error) => return Ok(None),
            Err(error) => return Err(map_sftp_error(error)),
        }
    }

    let mut local_file = tokio::fs::File::open(local)
        .await
        .map_err(|error| format!("Failed to read '{}': {error}", local.display()))?;
    let mut remote_file = sftp
        .open_with_flags(
            remote,
            OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
        )
        .await
        .map_err(map_sftp_error)?;
    let write_error = |error: std::io::Error| format!("Failed to upload '{remote}': {error}");
    tokio::io::copy(&mut local_file, &mut remote_file)
        .await
        .map_err(write_error)?;
    remote_file.flush().await.map_err(write_error)?;
    remote_file.sync_all().await.map_err(map_sftp_error)?;
    remote_file.shutdown().await.map_err(write_error)?;

    let metadata = sftp.metadata(remote).await.map_err(map_sftp_error)?;
    Ok(Some(RemoteStamp::of(&metadata)))
}
