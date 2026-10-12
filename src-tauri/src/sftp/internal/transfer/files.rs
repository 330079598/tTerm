//! Whole-file helpers on either side of a transfer: small remote reads and
//! writes, remote directories, and source fingerprints.

use super::{failed, TransferError};
use russh_sftp::client::error::Error as SftpError;
use russh_sftp::client::RawSftpSession;
use russh_sftp::protocol::{FileAttributes, OpenFlags, StatusCode};
use std::path::Path;

pub(super) async fn remote_read_all(
    session: &RawSftpSession,
    path: &str,
    step: u64,
) -> Result<Option<Vec<u8>>, TransferError> {
    let attrs = match session.stat(path).await {
        Ok(attrs) => attrs,
        Err(SftpError::Status(status)) if status.status_code == StatusCode::NoSuchFile => {
            return Ok(None)
        }
        Err(err) => return Err(failed(format!("Failed to stat '{path}': {err}"))),
    };

    let size = attrs.attrs.size.unwrap_or(0);
    if size == 0 {
        return Ok(Some(Vec::new()));
    }

    let handle = session
        .open(path, OpenFlags::READ, FileAttributes::empty())
        .await
        .map_err(|err| failed(format!("Failed to open '{path}' for reading: {err}")))?
        .handle;

    let mut buffer = Vec::with_capacity(size as usize);
    let step = step.clamp(1, u32::MAX as u64);

    while (buffer.len() as u64) < size {
        let remaining = size - buffer.len() as u64;
        let request = remaining.min(step) as u32;
        match session
            .read(handle.as_str(), buffer.len() as u64, request)
            .await
        {
            Ok(data) => {
                if data.data.is_empty() {
                    break;
                }
                buffer.extend_from_slice(&data.data);
            }
            Err(err) => {
                let _ = session.close(handle.as_str()).await;
                return Err(failed(format!("Failed to read '{path}': {err}")));
            }
        }
    }

    let _ = session.close(handle.as_str()).await;
    Ok(Some(buffer))
}

pub(super) async fn remote_write_all(
    session: &RawSftpSession,
    path: &str,
    data: &[u8],
    step: u64,
) -> Result<(), TransferError> {
    let handle = session
        .open(
            path,
            OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
            FileAttributes::empty(),
        )
        .await
        .map_err(|err| failed(format!("Failed to create '{path}': {err}")))?
        .handle;

    let step = step.clamp(1, u32::MAX as u64) as usize;
    let mut offset = 0usize;
    while offset < data.len() {
        let end = (offset + step).min(data.len());
        if let Err(err) = session
            .write(handle.as_str(), offset as u64, data[offset..end].to_vec())
            .await
        {
            let _ = session.close(handle.as_str()).await;
            return Err(failed(format!("Failed to write '{path}': {err}")));
        }
        offset = end;
    }

    let _ = session.close(handle.as_str()).await;
    Ok(())
}

pub(super) async fn ensure_remote_dir_all(
    session: &RawSftpSession,
    remote_dir: &str,
) -> Result<(), TransferError> {
    let normalized = crate::sftp::internal::paths::normalize_remote_path(remote_dir);
    if normalized.is_empty() || normalized == "/" {
        return Ok(());
    }

    let mut current = if normalized.starts_with('/') {
        "/".to_string()
    } else {
        String::new()
    };

    for segment in normalized.split('/').filter(|segment| !segment.is_empty()) {
        current = crate::sftp::internal::paths::join_remote_path(&current, segment);
        // Stat before mkdir: for an already-existing tree this costs one
        // round trip per segment instead of a failing mkdir plus a
        // confirming stat.
        if session.stat(&current).await.is_ok() {
            continue;
        }
        if session
            .mkdir(&current, FileAttributes::empty())
            .await
            .is_err()
            && session.stat(&current).await.is_err()
        {
            return Err(failed(format!(
                "Failed to create remote directory '{current}'"
            )));
        }
    }

    Ok(())
}

pub(super) async fn local_fingerprint(path: &Path) -> Result<(u64, Option<i64>), TransferError> {
    let metadata = tokio::fs::metadata(path).await.map_err(|err| {
        failed(format!(
            "Failed to read local file metadata '{}': {err}",
            path.display()
        ))
    })?;
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64);
    Ok((metadata.len(), mtime))
}

pub(super) async fn remote_fingerprint(
    session: &RawSftpSession,
    path: &str,
) -> Result<Option<(u64, Option<i64>)>, TransferError> {
    match session.stat(path).await {
        Ok(attrs) => Ok(Some((
            attrs.attrs.size.unwrap_or(0),
            attrs.attrs.mtime.map(|mtime| mtime as i64),
        ))),
        Err(SftpError::Status(status)) if status.status_code == StatusCode::NoSuchFile => Ok(None),
        Err(err) => Err(failed(format!("Failed to stat remote '{path}': {err}"))),
    }
}

pub(super) fn now_unix_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}
