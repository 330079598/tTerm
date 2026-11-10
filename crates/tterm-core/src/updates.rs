//! Update routing and cache rules shared by both clients.
use std::path::{Path, PathBuf};
/// Resolves the existing channel route; unknown channels use stable.
pub fn update_endpoint(channel: &str) -> &'static str {
    match channel {
        "beta-dev" => "https://330079598.github.io/tTerm/update/beta-dev/latest.json",
        _ => "https://330079598.github.io/tTerm/update/stable/latest.json",
    }
}

fn sanitize_update_channel(channel: &str) -> String {
    channel
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

/// Returns the per-application installer cache directory.
pub fn pending_update_download_dir(paths: &crate::paths::AppPaths) -> Result<PathBuf, String> {
    Ok(paths.cache_dir.join("updates"))
}

/// Creates a unique cache destination using the existing channel sanitization.
pub fn pending_update_download_path(
    paths: &crate::paths::AppPaths,
    channel: &str,
) -> Result<PathBuf, String> {
    let update_dir = pending_update_download_dir(paths)?;

    std::fs::create_dir_all(&update_dir).map_err(|e| {
        format!(
            "Failed to create update cache dir '{}': {}",
            update_dir.display(),
            e
        )
    })?;

    Ok(update_dir.join(format!(
        "{}-{}.bin",
        sanitize_update_channel(channel),
        uuid::Uuid::new_v4()
    )))
}

/// Publishes downloaded bytes atomically, cleaning a failed temporary write.
pub async fn write_pending_update_download(
    channel: &str,
    update_path: &Path,
    bytes: &[u8],
) -> Result<(), String> {
    let update_dir = update_path
        .parent()
        .ok_or_else(|| format!("Invalid update cache path '{}'", update_path.display()))?;
    let temp_path = update_dir.join(format!(
        "{}-{}.tmp",
        sanitize_update_channel(channel),
        uuid::Uuid::new_v4()
    ));

    let write_result = async {
        tokio::fs::write(&temp_path, bytes).await.map_err(|e| {
            format!(
                "Failed to cache downloaded update '{}': {}",
                temp_path.display(),
                e
            )
        })?;
        tokio::fs::rename(&temp_path, update_path)
            .await
            .map_err(|e| {
                format!(
                    "Failed to finalize downloaded update '{}' from '{}': {}",
                    update_path.display(),
                    temp_path.display(),
                    e
                )
            })
    }
    .await;

    if write_result.is_err() {
        remove_pending_update_file(&temp_path).await;
    }

    write_result
}

/// Removes an application's cached installer; a missing file is already clean.
pub async fn remove_pending_update_file(path: &Path) {
    if let Err(err) = tokio::fs::remove_file(path).await
        && err.kind() != std::io::ErrorKind::NotFound
    {
        eprintln!(
            "Failed to remove cached update '{}': {}",
            path.display(),
            err
        );
    }
}

/// Removes stale installer files without touching other cache contents.
pub fn cleanup_stale_pending_update_files(paths: &crate::paths::AppPaths) -> Result<(), String> {
    let update_dir = pending_update_download_dir(paths)?;
    let Ok(entries) = std::fs::read_dir(&update_dir) else {
        return Ok(());
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let is_update_cache_file = matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("bin" | "tmp")
        );
        if is_update_cache_file
            && let Err(err) = std::fs::remove_file(&path)
            && err.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!(
                "Failed to remove stale update '{}': {}",
                path.display(),
                err
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn cache_preserves_verified_bytes_and_only_removes_installer_files() {
        let root = std::env::temp_dir().join(format!("p1-08-update-{}", uuid::Uuid::new_v4()));
        let mut paths = crate::paths::AppPaths::native("com.stone.tTerm.p1-08-unit").unwrap();
        paths.cache_dir = root.clone();
        let path = super::pending_update_download_path(&paths, "../beta:dev").unwrap();
        assert_eq!(path.parent().unwrap(), root.join("updates"));
        assert!(
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("___beta_dev-")
        );
        super::write_pending_update_download("beta-dev", &path, b"verified release A")
            .await
            .unwrap();
        assert_eq!(tokio::fs::read(&path).await.unwrap(), b"verified release A");
        let unrelated = root.join("updates/keep.txt");
        tokio::fs::write(&unrelated, b"keep").await.unwrap();
        super::cleanup_stale_pending_update_files(&paths).unwrap();
        assert!(!path.exists());
        assert_eq!(tokio::fs::read(&unrelated).await.unwrap(), b"keep");
        super::remove_pending_update_file(&path).await;
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn channels_preserve_existing_stable_fallback() {
        assert!(super::update_endpoint("beta-dev").contains("/beta-dev/"));
        assert!(super::update_endpoint("unknown").contains("/stable/"));
    }
}
