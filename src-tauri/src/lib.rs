mod config;
mod core;
mod fonts;
mod profiles;
mod sftp;
mod session;
mod ssh;
mod terminal;

use core::PtyMap;
use std::sync::Arc;
use tauri::Manager;
use tauri_plugin_frame::FramePluginBuilder;
use tokio::sync::RwLock;
pub struct TokioRuntimeState {
    pub runtime: tokio::runtime::Runtime,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let pty_map: PtyMap = Arc::new(RwLock::new(std::collections::HashMap::new()));
    let host_prompt_map: core::HostPromptMap =
        Arc::new(RwLock::new(std::collections::HashMap::new()));
    let sftp_pool: sftp::SftpConnectionPool =
        Arc::new(RwLock::new(std::collections::HashMap::new()));
    let transfer_cancel_map: sftp::TransferCancelMap =
        Arc::new(RwLock::new(std::collections::HashMap::new()));
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");
    let secret_store = ssh::SecretStoreState::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            FramePluginBuilder::new()
                .titlebar_height(32)
                .button_width(46)
                .auto_titlebar(true)
                .snap_overlay_delay_ms(15)
                .close_hover_bg("rgba(196,43,28,1)")
                .button_hover_bg("rgba(255,255,255,0.1)")
                .build(),
        )
        .manage(pty_map)
        .manage(host_prompt_map)
        .manage(sftp_pool)
        .manage(transfer_cancel_map)
        .manage(TokioRuntimeState { runtime })
        .manage(secret_store)
        .invoke_handler(tauri::generate_handler![
            config::load_config,
            config::save_config,
            session::load_session,
            session::save_session,
            session::clear_session,
            core::commands::create_pty,
            core::commands::write_pty,
            core::commands::resize_pty,
            core::commands::kill_pty,
            core::commands::respond_ssh_host_key_prompt,
            fonts::list_fonts,
            profiles::list_profiles,
            profiles::save_profile,
            profiles::delete_profile,
            profiles::test_connection,
            sftp::internal::api::sftp_list_directory,
            sftp::internal::api::sftp_create_directory,
            sftp::internal::api::sftp_delete_entry,
            sftp::internal::api::sftp_rename_entry,
            sftp::internal::api::sftp_upload_file,
            sftp::internal::api::sftp_cancel_upload,
            sftp::internal::api::sftp_download_file,
            sftp::internal::api::get_file_size,
            ssh::secret_commands::get_secret_backend_status,
            ssh::secret_commands::unlock_secret_vault,
            ssh::secret_commands::lock_secret_vault,
            ssh::secret_commands::set_secret_vault_enabled,
        ])
        .setup(|app| {
            if let Err(err) = migrate_legacy_ssh_passwords(app.handle()) {
                eprintln!("Failed to migrate legacy SSH passwords: {}", err);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn migrate_legacy_ssh_passwords(app: &tauri::AppHandle) -> Result<(), String> {
    let secret_state = app.state::<ssh::SecretStoreState>();
    let store = ssh::load_legacy_password_store()?;
    if store.profiles.is_empty() {
        return Ok(());
    }

    if !secret_state.keyring_available()? {
        return Ok(());
    }

    for record in store.profiles {
        if record.password.is_empty() {
            continue;
        }
        let password = secret_state.get_password(app, &record.profile_name)?;
        if password.is_none() {
            secret_state.save_password(app, &record.profile_name, &record.password)?;
        }
    }

    ssh::remove_legacy_password_store()?;
    Ok(())
}
