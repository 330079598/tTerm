mod agent_integration;
mod app_state;
mod background;
mod backup;
mod clipboard_files;
pub mod command_library;
mod config;
mod core;
mod db;
#[cfg(any(target_os = "windows", target_os = "macos"))]
mod drag_out;
mod fonts;
mod ime_focus;
mod main_window;
mod migrate;
mod monitor;
mod notification;
mod profiles;
mod proxy;
mod session;
mod session_log;
mod sftp;
mod ssh;
mod sync;
mod terminal;
mod themes;
mod tunnel;
mod updater;
mod window_blur;
mod zmodem;

use core::state::PtyMap;
use std::sync::Arc;
use tauri::Manager;
#[cfg(any(target_os = "windows", target_os = "macos"))]
use tauri_plugin_frame::FramePluginBuilder;
use tokio::sync::RwLock;

pub struct TokioRuntimeState {
    pub runtime: tokio::runtime::Runtime,
}

/// Runs one of tTerm's console helpers instead of the app when started as one
/// (see `terminal::console_reset` and `agent_integration::console_report`);
/// returns its exit code.
#[cfg(target_os = "windows")]
pub fn run_console_helper() -> Option<i32> {
    terminal::console_reset::run_helper_if_requested()
        .or_else(agent_integration::console_report::run_helper_if_requested)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[expect(
    clippy::too_many_lines,
    reason = "one builder chain registers every plugin and Tauri command"
)]
pub fn run() {
    let pty_map: PtyMap = Arc::new(RwLock::new(std::collections::HashMap::new()));
    let host_prompt_map: core::state::HostPromptMap =
        Arc::new(RwLock::new(std::collections::HashMap::new()));
    let sftp_pool: sftp::internal::types::SftpConnectionPool =
        Arc::new(RwLock::new(std::collections::HashMap::new()));
    let monitor_sessions: monitor::types::MonitorSessionMap =
        Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()));
    let transfer_cancel_map: sftp::internal::types::TransferCancelMap =
        Arc::new(RwLock::new(std::collections::HashMap::new()));
    let zmodem_map: core::state::ZmodemMap =
        Arc::new(std::sync::RwLock::new(std::collections::HashMap::new()));
    let zmodem_armed_send_map: core::state::ZmodemArmedSendMap =
        Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
    let zmodem_manual_detect_map: core::state::ZmodemManualDetectMap =
        Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(core::runtime::tokio_worker_threads())
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");
    let secret_store = ssh::secret_store::types::SecretStoreState::new();
    ssh::key_vault::install(secret_store.clone());
    let session_log_state = session_log::SessionLogState::default();

    let builder = tauri::Builder::default()
        // Registered first so a second launch exits before touching the
        // database, tunnels or migrations the running instance owns.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            background::show_main_window(app);
        }))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build());

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    let builder = builder.plugin(
        FramePluginBuilder::new()
            // Matches `--title-bar-height` in layout.css, so the native window
            // buttons share the app's centre line and their hover fills the bar.
            .titlebar_height(36)
            .button_width(46)
            .auto_titlebar(true)
            .close_hover_bg("rgba(196,43,28,1)")
            .button_hover_bg("rgba(255,255,255,0.1)")
            .build(),
    );

    builder
        .manage(pty_map)
        .manage(host_prompt_map)
        .manage(ssh::auth::AuthPromptMap::default())
        .manage(proxy::approval::ProxyCommandPrompts::default())
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
        .manage(tunnel::commands::TunnelManager::default())
        .manage(background::CloseBehaviorState::default())
        .manage(background::tray::TrayState::default())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if background::on_close_requested(window) {
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
            backup::remote::get_webdav_backup_status,
            backup::remote::save_webdav_backup_settings,
            backup::remote::clear_webdav_backup_settings,
            backup::remote::test_webdav_connection,
            backup::remote::run_webdav_backup,
            backup::remote::list_webdav_backups,
            backup::remote::download_webdav_backup,
            backup::remote::discard_webdav_download,
            backup::remote::delete_webdav_backup,
            backup::remote::delete_webdav_backups,
            backup::remote::save_webdav_backup,
            sync::get_sync_status,
            sync::save_sync_settings,
            sync::reset_sync,
            sync::run_sync,
            config::load_config,
            config::save_config,
            app_state::load_app_state,
            app_state::save_app_state,
            themes::load_custom_themes,
            themes::save_custom_themes,
            proxy::list_proxies,
            proxy::save_proxy,
            proxy::delete_proxy,
            proxy::approval::pending_proxy_command_prompts,
            proxy::approval::respond_proxy_command_prompt,
            config::window_background::save_window_background,
            window_blur::set_window_blur,
            window_blur::system_prefers_dark,
            session_log::get_terminal_log_status,
            session_log::set_terminal_log_recording,
            session_log::list_terminal_logs,
            session_log::terminal_log_ids_for_tab,
            session_log::load_terminal_log_recording,
            session_log::load_terminal_log_text,
            session_log::export_terminal_log_asciicast,
            session_log::delete_terminal_log,
            session_log::delete_terminal_logs,
            session_log::reveal_terminal_log,
            session_log::save_terminal_contents,
            session_log::open_terminal_log_directory,
            session_log::retry_terminal_logging,
            clipboard_files::read_clipboard_file_paths,
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            drag_out::start_drag_out,
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            drag_out::refresh_drop_folder,
            session::load_session,
            session::save_session,
            session::clear_session,
            core::commands::create_pty,
            core::commands::write_pty,
            core::commands::ack_pty_output,
            core::commands::reset_pty_console,
            core::commands::write_pty_batch,
            core::commands::resize_pty,
            core::commands::kill_pty,
            core::commands::respond_ssh_host_key_prompt,
            ssh::auth::respond_ssh_auth_prompt,
            ssh::typed_passwords::has_typed_password,
            ssh::typed_passwords::forget_typed_passwords,
            core::commands::has_saved_password,
            core::commands::has_saved_jump_host_password,
            core::commands::write_saved_password_for_sudo,
            core::commands::get_sudo_password_source,
            core::commands::has_saved_sudo_password,
            zmodem::commands::zmodem_cancel,
            zmodem::commands::zmodem_start_send,
            zmodem::commands::zmodem_arm_manual_detect,
            terminal::pty::list_available_terminal_shells,
            terminal::pty::local_terminal_shell_name,
            terminal::foreground::pty_shell_in_foreground,
            fonts::list_fonts,
            monitor::get_server_metrics_snapshot,
            monitor::release_server_monitor_session,
            profiles::commands::list_profiles,
            profiles::commands::preview_ssh_config_import,
            profiles::commands::import_ssh_config_profiles,
            profiles::commands::list_profile_groups,
            profiles::commands::save_profile_group,
            profiles::commands::rename_profile_group,
            profiles::commands::delete_profile_group,
            profiles::commands::move_profile_to_group,
            profiles::commands::save_profile,
            profiles::commands::delete_profile,
            profiles::commands::delete_profiles,
            profiles::commands::set_profile_server_monitor_visible,
            profiles::commands::test_connection,
            tunnel::commands::list_tunnels,
            tunnel::commands::list_tunnel_statuses,
            tunnel::commands::save_tunnel,
            tunnel::commands::delete_tunnel,
            tunnel::commands::start_tunnel,
            tunnel::commands::stop_tunnel,
            tunnel::commands::reset_tunnel_traffic_totals,
            tunnel::commands::auto_start_tunnels,
            tunnel::commands::confirm_quit_app,
            tunnel::commands::cancel_quit_prompt,
            background::resolve_close_request,
            command_library::commands::list_saved_commands,
            command_library::commands::list_command_tags,
            command_library::commands::create_command_tag,
            command_library::commands::rename_command_tag,
            command_library::commands::delete_command_tag,
            command_library::commands::save_saved_command,
            command_library::commands::delete_saved_command,
            command_library::commands::set_saved_command_favorite,
            command_library::commands::record_saved_command_use,
            sftp::internal::api::base::sftp_list_directory,
            sftp::internal::api::base::sftp_release_tab_connections,
            sftp::internal::api::base::sftp_create_directory,
            sftp::internal::api::delete::commands::sftp_delete_entry,
            sftp::internal::api::delete::commands::sftp_delete_entries,
            sftp::internal::api::delete::commands::sftp_preview_delete_entries,
            sftp::internal::api::base::sftp_rename_entry,
            sftp::internal::api::permissions::sftp_set_permissions,
            sftp::internal::api::edit::sftp_open_file_for_edit,
            sftp::internal::api::edit::sftp_save_edited_file,
            sftp::internal::api::upload::commands::sftp_upload_file,
            sftp::internal::api::upload::commands::sftp_upload_paths,
            sftp::internal::api::upload::commands::sftp_check_upload_conflicts,
            sftp::internal::api::upload::commands::sftp_cancel_upload,
            sftp::internal::api::download::sftp_download_file,
            sftp::internal::api::download::sftp_download_paths,
            sftp::internal::api::download::sftp_check_download_conflicts,
            sftp::internal::api::download::get_file_size,
            ssh::secret_commands::get_secret_backend_status,
            ssh::secret_commands::unlock_secret_vault,
            ssh::secret_commands::lock_secret_vault,
            ssh::secret_commands::retry_system_unlock,
            ssh::secret_commands::reset_saved_passwords,
            ssh::secret_commands::change_vault_password,
            ssh::secret_commands::set_master_password,
            ssh::secret_commands::remove_master_password,
            ssh::secret_commands::set_secret_storage_mode,
            ssh::secret_commands::list_saved_secrets,
            ssh::secret_commands::get_saved_secret,
            ssh::secret_commands::verify_user,
            ssh::secret_commands::delete_saved_secret,
            ssh::key_vault::list_vault_keys,
            ssh::key_vault::import_vault_key,
            ssh::key_vault::rename_vault_key,
            ssh::key_vault::set_vault_key_certificate,
            ssh::key_vault::delete_vault_key,
            ssh::key_vault::export_vault_key,
            ssh::known_host_commands::list_known_hosts,
            ssh::known_host_commands::delete_known_hosts,
            updater::check_app_update,
            updater::download_app_update,
            updater::install_downloaded_app_update,
            updater::download_install_app_update,
            main_window::toggle_devtools,
            ime_focus::repair_ime_focus,
            notification::show_notification,
            notification::notification_permission,
            notification::import_notification_sound,
            notification::read_notification_sound,
            notification::badge::set_unseen_notifications,
            agent_integration::agent_integration_status,
            agent_integration::install_agent_integration,
            agent_integration::uninstall_agent_integration,
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();
            config::paths::init_config_dir(&app_handle)?;

            if let Err(err) = updater::cleanup_stale_pending_update_files(&app_handle) {
                eprintln!("Failed to clean stale update cache files: {}", err);
            }

            if let Err(error) = backup::remote::clear_downloads() {
                eprintln!("Failed to clear WebDAV downloads: {error}");
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

            // Settings files and state older versions kept beside config.json.
            if let Err(error) = db::import_settings_files() {
                eprintln!("Failed to move settings files into the database: {error}");
            }

            // Moves passwords saved by older versions into the database once,
            // then unlocks them from the system credential store, in the
            // background.
            app_handle
                .state::<ssh::secret_store::types::SecretStoreState>()
                .initialize(&app_handle);

            if let Err(error) = migrate::remove_retired_files(&app_handle) {
                eprintln!("Failed to remove retired files: {error}");
            }

            let cfg = config::load_config_file().ok();
            if let Some(cfg) = &cfg {
                sftp::internal::api::apply_bandwidth_limits(cfg);
                let log_state = app_handle.state::<session_log::SessionLogState>();
                if let Err(err) = log_state.apply_config(&app_handle, cfg) {
                    eprintln!("Failed to initialize terminal logging: {}", err);
                }
            }

            sftp::internal::connection::spawn_idle_reaper(
                app_handle
                    .state::<sftp::internal::types::SftpConnectionPool>()
                    .inner()
                    .clone(),
            );
            notification::init(&app_handle);
            #[cfg(target_os = "macos")]
            terminal::pty::preload_login_shell_env();
            main_window::create_main_window(app, cfg.as_ref())?;
            background::apply_config(&app_handle, &cfg.unwrap_or_default());

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Cmd+Q and the like skip the window-close path. An exit with a
            // code is one the app requested itself and is never held back.
            match event {
                tauri::RunEvent::ExitRequested {
                    api, code: None, ..
                } => {
                    if app
                        .state::<tunnel::commands::TunnelManager>()
                        .hold_exit_for_confirmation(app)
                    {
                        api.prevent_exit();
                    }
                }
                tauri::RunEvent::Exit => {
                    // Tunnels may still be running; keep what they carried.
                    app.state::<tunnel::commands::TunnelManager>()
                        .save_traffic();
                    // Logs keep what they have not written out yet.
                    session_log::shutdown(app);
                }
                // Clicking the Dock icon brings back a window closed to the
                // background.
                #[cfg(target_os = "macos")]
                tauri::RunEvent::Reopen {
                    has_visible_windows: false,
                    ..
                } => background::show_main_window(app),
                _ => {}
            }
        });
}
