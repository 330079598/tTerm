//! Domain operations behind the IPC entry points.
use crate::backup::MAX_BACKUP_SIZE;
use crate::backup::normalized_backup_path;
use crate::backup::remote::BACKUP_PASSWORD_KEY;
use crate::backup::remote::DOWNLOAD_DIRECTORY;
use crate::backup::remote::PASSWORD_KEY;
use crate::backup::remote::RemoteBackupEntry;
use crate::backup::remote::UPLOADING;
use crate::backup::remote::WebDavBackupSettings;
use crate::backup::remote::WebDavBackupStatus;
use crate::backup::remote::WebDavDeleteResult;
use crate::backup::remote::WebDavUploadResult;
use crate::backup::remote::clear_downloads;
use crate::backup::remote::clear_settings;
use crate::backup::remote::connect;
use crate::backup::remote::files::device_name;
use crate::backup::remote::files::remote_backups;
use crate::backup::remote::files::user_agent;
use crate::backup::remote::files::validate_file_name;
use crate::backup::remote::load_settings;
use crate::backup::remote::non_empty;
use crate::backup::remote::normalized_settings;
use crate::backup::remote::run_webdav_backup_locked;
use crate::backup::remote::same_account;
use crate::backup::remote::save_keeping_history;
use crate::backup::remote::saved_secret;
use crate::backup::remote::status;
use crate::backup::webdav::{WebDavClient, split_remote_directory};
use crate::core::blocking::run_blocking;
use crate::ssh::secret_store::{
    store::put_secret, types::SecretStoreState, verification::VerificationPurpose,
};
use serde_json::Value;
use std::{fs, sync::atomic::Ordering};
use zeroize::Zeroizing;

pub async fn get_webdav_backup_status(
    secret_state: &SecretStoreState,
) -> Result<WebDavBackupStatus, String> {
    let secret_state = secret_state.clone();
    run_blocking(move || status(&secret_state)).await
}

pub async fn save_webdav_backup_settings(
    settings: WebDavBackupSettings,
    password: Option<String>,
    backup_password: Option<String>,
    secret_state: &SecretStoreState,
) -> Result<WebDavBackupStatus, String> {
    let secret_state = secret_state.clone();
    let password = non_empty(password).map(Zeroizing::new);
    let backup_password = non_empty(backup_password).map(Zeroizing::new);
    run_blocking(move || {
        let settings = normalized_settings(settings)?;
        if backup_password
            .as_deref()
            .is_some_and(|value| value.chars().count() < 8)
        {
            return Err("Backup password must contain at least 8 characters.".to_string());
        }
        let saved = secret_state.saved_keys()?;
        if password.is_none() && !saved.iter().any(|key| key == PASSWORD_KEY) {
            return Err("Enter the WebDAV password.".to_string());
        }
        if backup_password.is_none() && !saved.iter().any(|key| key == BACKUP_PASSWORD_KEY) {
            return Err("Enter a backup password to encrypt WebDAV backups.".to_string());
        }
        // Scheduled uploads and sync send saved passwords to this server,
        // encrypted with this backup password: whoever picks either can
        // read them.
        if !same_account(&settings, &load_settings()?) || backup_password.is_some() {
            secret_state.require_verification(VerificationPurpose::Sensitive)?;
        }
        let data_key = if password.is_some() || backup_password.is_some() {
            Some(secret_state.data_key()?)
        } else {
            None
        };
        // The server and its passwords are saved together or not at all.
        crate::db::write(|transaction| {
            if let Some(data_key) = data_key.as_ref() {
                if let Some(password) = password.as_deref() {
                    put_secret(transaction, data_key, PASSWORD_KEY, password)?;
                }
                if let Some(password) = backup_password.as_deref() {
                    put_secret(transaction, data_key, BACKUP_PASSWORD_KEY, password)?;
                }
            }
            save_keeping_history(transaction, settings)
        })?;
        status(&secret_state)
    })
    .await
}

pub async fn clear_webdav_backup_settings(
    secret_state: &SecretStoreState,
) -> Result<WebDavBackupStatus, String> {
    let secret_state = secret_state.clone();
    run_blocking(move || {
        crate::db::write(|transaction| clear_settings(transaction))?;
        status(&secret_state)
    })
    .await
}

pub async fn test_webdav_connection(
    app_version: String,
    settings: WebDavBackupSettings,
    password: Option<String>,
    secret_state: &SecretStoreState,
) -> Result<(), String> {
    let secret_state = secret_state.clone();
    let password = non_empty(password).map(Zeroizing::new);
    let (settings, password) = run_blocking(move || {
        let settings = normalized_settings(settings)?;
        let password = match password {
            Some(password) => password,
            // The saved password only goes to the server it was saved for.
            None if !same_account(&settings, &load_settings()?) => {
                return Err("Enter the WebDAV password.".to_string());
            }
            None => saved_secret(&secret_state, PASSWORD_KEY)?
                .ok_or_else(|| "Enter the WebDAV password.".to_string())?,
        };
        Ok((settings, password))
    })
    .await?;
    let client = WebDavClient::new(
        &settings.url,
        &settings.username,
        password,
        &user_agent(&app_version),
    )?;
    client.check().await?;
    client
        .ensure_directory(&split_remote_directory(&settings.remote_directory)?)
        .await
}

pub async fn run_webdav_backup(
    app_version: String,
    frontend_state: Option<Value>,
    force: bool,
    secret_state: &SecretStoreState,
) -> Result<Option<WebDavUploadResult>, String> {
    struct Uploading;
    impl Drop for Uploading {
        fn drop(&mut self) {
            UPLOADING.store(false, Ordering::Release);
        }
    }

    if UPLOADING.swap(true, Ordering::AcqRel) {
        return if force {
            Err("A WebDAV backup is already being uploaded.".to_string())
        } else {
            Ok(None)
        };
    }
    let _uploading = Uploading;
    run_webdav_backup_locked(app_version, frontend_state, force, secret_state).await
}

pub async fn list_webdav_backups(
    app_version: String,
    secret_state: &SecretStoreState,
) -> Result<Vec<RemoteBackupEntry>, String> {
    let (client, directory) = connect(&app_version, secret_state).await?;
    let entries = client.list(&directory).await?;
    Ok(remote_backups(&entries, &device_name()))
}

pub async fn download_webdav_backup(
    app_version: String,
    file_name: String,
    secret_state: &SecretStoreState,
) -> Result<String, String> {
    validate_file_name(&file_name)?;
    let (client, directory) = connect(&app_version, secret_state).await?;
    let data = client.get(&directory, &file_name, MAX_BACKUP_SIZE).await?;
    run_blocking(move || {
        // Only the backup being restored is kept.
        clear_downloads()?;
        let downloads = crate::config::paths::ensure_config_dir()?.join(DOWNLOAD_DIRECTORY);
        fs::create_dir_all(&downloads)
            .map_err(|error| format!("Failed to create the WebDAV download folder: {error}"))?;
        let path = downloads.join(&file_name);
        crate::config::atomic::atomic_write_private(&path, data)?;
        Ok(path.to_string_lossy().into_owned())
    })
    .await
}

pub async fn discard_webdav_download() -> Result<(), String> {
    run_blocking(clear_downloads).await
}

pub async fn delete_webdav_backup(
    app_version: String,
    file_name: String,
    secret_state: &SecretStoreState,
) -> Result<(), String> {
    validate_file_name(&file_name)?;
    let (client, directory) = connect(&app_version, secret_state).await?;
    client.delete(&directory, &file_name).await
}

pub async fn delete_webdav_backups(
    app_version: String,
    file_names: Vec<String>,
    secret_state: &SecretStoreState,
) -> Result<WebDavDeleteResult, String> {
    for file_name in &file_names {
        validate_file_name(file_name)?;
    }
    let (client, directory) = connect(&app_version, secret_state).await?;
    let mut deleted = Vec::new();
    let mut error = None;
    for file_name in file_names {
        match client.delete(&directory, &file_name).await {
            Ok(()) => deleted.push(file_name),
            Err(failure) => {
                error.get_or_insert(failure);
            }
        }
    }
    Ok(WebDavDeleteResult { deleted, error })
}

pub async fn save_webdav_backup(
    app_version: String,
    file_name: String,
    output_path: String,
    secret_state: &SecretStoreState,
) -> Result<String, String> {
    validate_file_name(&file_name)?;
    let output = normalized_backup_path(&output_path)?;
    let (client, directory) = connect(&app_version, secret_state).await?;
    let data = client.get(&directory, &file_name, MAX_BACKUP_SIZE).await?;
    run_blocking(move || {
        crate::config::atomic::atomic_write_private(&output, data)?;
        Ok(output.to_string_lossy().into_owned())
    })
    .await
}
