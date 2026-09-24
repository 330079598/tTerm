mod backup;
mod clipboard_files;
pub mod command_library;
mod config;
mod core;
mod db;
mod fonts;
mod migrate;
mod monitor;
mod profiles;
mod session;
mod session_log;
mod sftp;
mod ssh;
mod terminal;
mod tunnel;
mod updater;
mod zmodem;

use core::PtyMap;
use std::sync::Arc;
use tauri::Manager;
#[cfg(any(target_os = "windows", target_os = "macos"))]
use tauri_plugin_frame::FramePluginBuilder;
use tokio::sync::RwLock;

pub struct TokioRuntimeState {
    pub runtime: tokio::runtime::Runtime,
}

#[tauri::command]
fn toggle_devtools(app: tauri::AppHandle, enable: bool) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or("Main window not found")?;

    if enable {
        window.open_devtools();
    } else {
        window.close_devtools();
    }

    Ok(())
}

fn tokio_worker_threads_from(
    override_value: Option<&str>,
    available_parallelism: Option<usize>,
) -> usize {
    const MIN_WORKERS: usize = 2;
    const DEFAULT_MAX_WORKERS: usize = 8;
    const ENV_MAX_WORKERS: usize = 16;

    if let Some(raw) = override_value {
        if let Ok(parsed) = raw.trim().parse::<usize>() {
            return parsed.clamp(MIN_WORKERS, ENV_MAX_WORKERS);
        }
    }

    available_parallelism
        .map(|n| n.clamp(MIN_WORKERS, DEFAULT_MAX_WORKERS))
        .unwrap_or(MIN_WORKERS)
}

/// Adaptive Tokio worker count for desktop workloads (SSH/SFTP concurrency).
/// Override with `TTERM_TOKIO_WORKERS` (clamped to 2-16).
fn tokio_worker_threads() -> usize {
    let available_parallelism = std::thread::available_parallelism().ok().map(|n| n.get());
    tokio_worker_threads_from(
        std::env::var("TTERM_TOKIO_WORKERS").ok().as_deref(),
        available_parallelism,
    )
}

#[cfg(test)]
mod tokio_worker_tests {
    use super::tokio_worker_threads_from;

    #[test]
    fn worker_count_uses_bounded_available_parallelism() {
        assert_eq!(tokio_worker_threads_from(None, None), 2);
        assert_eq!(tokio_worker_threads_from(None, Some(1)), 2);
        assert_eq!(tokio_worker_threads_from(None, Some(6)), 6);
        assert_eq!(tokio_worker_threads_from(None, Some(64)), 8);
    }

    #[test]
    fn worker_count_honors_a_valid_bounded_override() {
        assert_eq!(tokio_worker_threads_from(Some("6"), Some(2)), 6);
        assert_eq!(tokio_worker_threads_from(Some("1"), Some(8)), 2);
        assert_eq!(tokio_worker_threads_from(Some("99"), Some(2)), 16);
    }

    #[test]
    fn invalid_override_falls_back_to_available_parallelism() {
        assert_eq!(tokio_worker_threads_from(Some("invalid"), Some(4)), 4);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let pty_map: PtyMap = Arc::new(RwLock::new(std::collections::HashMap::new()));
    let host_prompt_map: core::HostPromptMap =
        Arc::new(RwLock::new(std::collections::HashMap::new()));
    let sftp_pool: sftp::SftpConnectionPool =
        Arc::new(RwLock::new(std::collections::HashMap::new()));
    let monitor_sessions: monitor::MonitorSessionMap =
        Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()));
    let transfer_cancel_map: sftp::TransferCancelMap =
        Arc::new(RwLock::new(std::collections::HashMap::new()));
    let zmodem_map: core::ZmodemMap =
        Arc::new(std::sync::RwLock::new(std::collections::HashMap::new()));
    let zmodem_armed_send_map: core::ZmodemArmedSendMap =
        Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
    let zmodem_manual_detect_map: core::ZmodemManualDetectMap =
        Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(tokio_worker_threads())
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");
    let secret_store = ssh::SecretStoreState::new();
    let session_log_state = session_log::SessionLogState::default();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build());

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    let builder = builder.plugin(
        FramePluginBuilder::new()
            .titlebar_height(32)
            .button_width(46)
            .auto_titlebar(true)
            .snap_overlay_delay_ms(15)
            .close_hover_bg("rgba(196,43,28,1)")
            .button_hover_bg("rgba(255,255,255,0.1)")
            .build(),
    );

    builder
        .manage(pty_map)
        .manage(host_prompt_map)
        .manage(sftp_pool)
        .manage(monitor_sessions)
        .manage(transfer_cancel_map)
        .manage(zmodem_map)
        .manage(zmodem_armed_send_map)
        .manage(zmodem_manual_detect_map)
        .manage(TokioRuntimeState { runtime })
        .manage(updater::PendingUpdateDownloads::default())
        .manage(secret_store)
        .manage(session_log_state)
        .manage(tunnel::TunnelManager::default())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                if app
                    .state::<tunnel::TunnelManager>()
                    .hold_exit_for_confirmation(app)
                {
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            backup::export_backup,
            backup::inspect_backup,
            backup::import_backup,
            backup::get_automatic_backup_settings,
            backup::save_automatic_backup_settings,
            backup::run_due_automatic_backup,
            backup::list_backup_history,
            backup::delete_backup_history_entry,
            config::load_config,
            config::save_config,
            session_log::get_terminal_log_status,
            session_log::open_terminal_log_directory,
            session_log::retry_terminal_logging,
            clipboard_files::read_clipboard_file_paths,
            session::load_session,
            session::save_session,
            session::clear_session,
            core::commands::create_pty,
            core::commands::write_pty,
            core::commands::write_pty_batch,
            core::commands::resize_pty,
            core::commands::kill_pty,
            core::commands::respond_ssh_host_key_prompt,
            core::commands::has_saved_password,
            core::commands::has_saved_jump_host_password,
            core::commands::write_saved_password_for_sudo,
            zmodem::commands::zmodem_cancel,
            zmodem::commands::zmodem_start_send,
            zmodem::commands::zmodem_arm_manual_detect,
            terminal::list_available_terminal_shells,
            fonts::list_fonts,
            monitor::get_server_metrics_snapshot,
            monitor::release_server_monitor_session,
            profiles::list_profiles,
            profiles::preview_ssh_config_import,
            profiles::import_ssh_config_profiles,
            profiles::list_profile_groups,
            profiles::save_profile_group,
            profiles::rename_profile_group,
            profiles::delete_profile_group,
            profiles::move_profile_to_group,
            profiles::save_profile,
            profiles::delete_profile,
            profiles::delete_profiles,
            profiles::set_profile_server_monitor_visible,
            profiles::test_connection,
            tunnel::list_tunnels,
            tunnel::list_tunnel_statuses,
            tunnel::save_tunnel,
            tunnel::delete_tunnel,
            tunnel::start_tunnel,
            tunnel::stop_tunnel,
            tunnel::auto_start_tunnels,
            tunnel::confirm_quit_app,
            tunnel::cancel_quit_prompt,
            command_library::list_saved_commands,
            command_library::list_command_tags,
            command_library::create_command_tag,
            command_library::rename_command_tag,
            command_library::delete_command_tag,
            command_library::save_saved_command,
            command_library::delete_saved_command,
            command_library::set_saved_command_favorite,
            command_library::record_saved_command_use,
            sftp::internal::api::base::sftp_list_directory,
            sftp::internal::api::base::sftp_create_directory,
            sftp::internal::api::delete::commands::sftp_delete_entry,
            sftp::internal::api::delete::commands::sftp_delete_entries,
            sftp::internal::api::delete::commands::sftp_preview_delete_entries,
            sftp::internal::api::base::sftp_rename_entry,
            sftp::internal::api::edit::sftp_open_file_for_edit,
            sftp::internal::api::edit::sftp_save_edited_file,
            sftp::internal::api::upload::commands::sftp_upload_file,
            sftp::internal::api::upload::commands::sftp_upload_paths,
            sftp::internal::api::upload::commands::sftp_cancel_upload,
            sftp::internal::api::download::sftp_download_file,
            sftp::internal::api::download::sftp_download_directory,
            sftp::internal::api::download::get_file_size,
            ssh::secret_commands::get_secret_backend_status,
            ssh::secret_commands::unlock_secret_vault,
            ssh::secret_commands::lock_secret_vault,
            ssh::secret_commands::change_vault_password,
            ssh::secret_commands::set_master_password,
            ssh::secret_commands::remove_master_password,
            ssh::secret_commands::set_secret_storage_mode,
            ssh::secret_commands::list_saved_secrets,
            ssh::secret_commands::get_saved_secret,
            ssh::secret_commands::delete_saved_secret,
            updater::check_app_update,
            updater::download_app_update,
            updater::install_downloaded_app_update,
            updater::download_install_app_update,
            toggle_devtools,
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();
            config::init_config_dir(&app_handle)?;

            if let Err(err) = updater::cleanup_stale_pending_update_files(&app_handle) {
                eprintln!("Failed to clean stale update cache files: {}", err);
            }

            if let Err(error) = db::init() {
                eprintln!("Failed to open database: {error}");
            }

            if let Err(err) = migrate::migrate_legacy_config_files(&app_handle) {
                eprintln!("Failed to migrate legacy config files: {}", err);
            }

            // After the legacy config merge so data from the old config
            // directory is imported along with everything else.
            if let Err(error) = db::import_legacy_json_files() {
                eprintln!("Failed to import JSON data into the database: {error}");
            }

            // Moves passwords saved by older versions into the database once,
            // then unlocks them from the system credential store.
            app_handle
                .state::<ssh::SecretStoreState>()
                .initialize(&app_handle);

            if let Ok(cfg) = config::load_config_file() {
                let log_state = app_handle.state::<session_log::SessionLogState>();
                if let Err(err) = log_state.apply_config(&app_handle, &cfg) {
                    eprintln!("Failed to initialize terminal logging: {}", err);
                }
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Cmd+Q and the like skip the window-close path. An exit with a
            // code is one the app requested itself and is never held back.
            if let tauri::RunEvent::ExitRequested {
                api, code: None, ..
            } = event
            {
                if app
                    .state::<tunnel::TunnelManager>()
                    .hold_exit_for_confirmation(app)
                {
                    api.prevent_exit();
                }
            }
        });
}
