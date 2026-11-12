mod agent_integration;
mod app_context;
mod app_identity;
mod app_paths;
mod app_state;
mod background;
mod backup;
mod clipboard_files;
pub mod command_library;
mod config;
mod connection_context;
mod core;
mod db;
#[cfg(any(target_os = "windows", target_os = "macos"))]
mod drag_out;
mod event_sink;
mod fonts;
mod ime_focus;
mod launch_dir;
mod main_window;
mod migrate;
mod monitor;
mod notification;
mod platform;
mod profiles;
mod prompt_scope;
mod proxy;
mod serial;
mod services;
mod session;
mod session_log;
mod session_sink;
mod sftp;
mod ssh;
mod sync;
mod system_integration;
mod terminal;
mod themes;
mod tunnel;
mod updater;
mod window_blur;
mod window_state;
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

/// Runs one of tTerm's helpers instead of the app when started as one (see
/// `terminal::console_reset`, `agent_integration::console_report` and
/// `system_integration`); returns its exit code.
#[cfg(target_os = "windows")]
pub fn run_console_helper() -> Option<i32> {
    terminal::console_reset::run_helper_if_requested()
        .or_else(agent_integration::console_report::run_helper_if_requested)
        .or_else(system_integration::run_installer_request)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[expect(
    clippy::too_many_lines,
    reason = "one builder chain registers every plugin and Tauri command"
)]
pub fn run() {
    tterm_core::identity::install_app_version(env!("CARGO_PKG_VERSION"));
    let pty_map: PtyMap = Arc::new(RwLock::new(std::collections::HashMap::new()));
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
    let runtime_handle = runtime.handle().clone();
    let external_edit_state = Arc::new(sftp::internal::api::external::ExternalEditState::default());
    let monitor_prompts = Arc::new(monitor::prompts::PendingMonitorPrompts::default());
    let pending_updates = Arc::new(updater::PendingUpdateDownloads::default());
    let secret_store = ssh::secret_store::types::SecretStoreState::new();
    ssh::key_vault::install(secret_store.clone());
    let session_log_state = Arc::new(session_log::SessionLogState::default());
    let tunnel_manager = Arc::new(tunnel::commands::TunnelManager::default());
    let close_behavior = Arc::new(background::CloseBehaviorState::default());
    let quit_state = Arc::new(background::quit::QuitState::default());
    let exit_guard = Arc::new(background::exit_guard::ExitGuardState::default());
    let launch_dirs = Arc::new(launch_dir::LaunchDirs::default());
    let tray_state = Arc::new(background::tray::TrayState::default());

    let builder = tauri::Builder::default()
        // Registered first so a second launch exits before touching the
        // database, tunnels or migrations the running instance owns.
        .plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            tterm_core::launch::receive(
                tterm_platform::launch::LaunchRequest {
                    args: argv,
                    cwd: cwd.into(),
                },
                |args, cwd| {
                    if let Some(services) = app.try_state::<Arc<services::AppServices>>() {
                        launch_dir::open_from_second_launch(
                            &services.app.launch_dirs,
                            services.shared.events.as_ref(),
                            args,
                            cwd,
                        );
                    }
                },
            );
            if let Some(services) = app.try_state::<Arc<services::AppServices>>() {
                background::show_main_window(
                    services.shared.platform.main_window.as_ref(),
                    services.shared.platform.badge.as_ref(),
                );
            }
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
        .manage(TokioRuntimeState { runtime })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event
                && window
                    .app_handle()
                    .try_state::<Arc<services::AppServices>>()
                    .is_some_and(|services| {
                        background::on_close_requested(&app_context::background(
                            &services,
                            sftp::internal::api::external::unuploaded_files(
                                &services.sftp.external_edit,
                            ),
                        ))
                    })
            {
                api.prevent_close();
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
            proxy::commands::list_proxies,
            proxy::commands::save_proxy,
            proxy::commands::delete_proxy,
            proxy::approval_commands::pending_proxy_command_prompts,
            proxy::approval_commands::respond_proxy_command_prompt,
            config::window_background::save_window_background,
            window_blur::set_window_blur,
            window_blur::system_prefers_dark,
            session_log::commands::get_terminal_log_status,
            session_log::commands::set_terminal_log_recording,
            session_log::commands::list_terminal_logs,
            session_log::commands::terminal_log_ids_for_tab,
            session_log::commands::load_terminal_log_recording,
            session_log::commands::load_terminal_log_text,
            session_log::commands::export_terminal_log_asciicast,
            session_log::commands::delete_terminal_log,
            session_log::commands::delete_terminal_logs,
            session_log::commands::reveal_terminal_log,
            session_log::commands::save_terminal_contents,
            session_log::commands::open_terminal_log_directory,
            session_log::commands::retry_terminal_logging,
            clipboard_files::read_clipboard_file_paths,
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            drag_out::start_drag_out,
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            drag_out::refresh_drop_folder,
            session::load_session,
            session::save_session,
            session::clear_session,
            session::scrollback::save_scrollback_snapshots,
            session::scrollback::load_scrollback_snapshots,
            session::scrollback::clear_scrollback_snapshots,
            core::commands::create_pty,
            core::commands::write_pty,
            core::commands::ack_pty_output,
            core::commands::reset_pty_console,
            core::commands::write_pty_batch,
            core::commands::resize_pty,
            core::commands::kill_pty,
            core::commands::respond_ssh_host_key_prompt,
            ssh::auth_commands::respond_ssh_auth_prompt,
            ssh::typed_passwords_commands::has_typed_password,
            ssh::typed_passwords_commands::forget_typed_passwords,
            core::commands::has_saved_password,
            core::commands::has_saved_jump_host_password,
            core::commands::write_saved_password_for_sudo,
            core::commands::get_sudo_password_source,
            core::commands::has_saved_sudo_password,
            zmodem::commands::zmodem_cancel,
            serial::commands::list_serial_ports,
            serial::commands::set_serial_signals,
            serial::commands::pulse_serial_line,
            serial::commands::send_serial_break,
            serial::commands::set_serial_view,
            serial::commands::write_serial_bytes,
            serial::xmodem::commands::serial_send_files,
            serial::xmodem::commands::serial_receive_files,
            serial::history::commands::list_serial_sends,
            serial::history::commands::record_serial_send,
            serial::history::commands::delete_serial_send,
            serial::history::commands::clear_serial_sends,
            zmodem::commands::zmodem_start_send,
            zmodem::commands::zmodem_arm_manual_detect,
            terminal::pty_commands::list_available_terminal_shells,
            terminal::pty_commands::local_terminal_shell_name,
            terminal::pty_commands::check_local_shell,
            terminal::foreground_commands::pty_shell_in_foreground,
            fonts::list_fonts,
            monitor::commands::get_server_metrics_snapshot,
            monitor::commands::release_server_monitor_session,
            profiles::commands::list_profiles,
            profiles::commands::preview_ssh_config_import,
            profiles::commands::resolve_ssh_config_host,
            profiles::commands::import_ssh_config_profiles,
            profiles::commands::groups::list_profile_groups,
            profiles::commands::groups::create_profile_group,
            profiles::commands::groups::rename_profile_group,
            profiles::commands::groups::move_profile_group,
            profiles::commands::groups::delete_profile_group,
            profiles::commands::groups::move_profiles_to_group,
            profiles::commands::save_profile,
            profiles::commands::delete_profile,
            profiles::commands::delete_profiles,
            profiles::commands::set_profile_server_monitor_visible,
            profiles::commands::set_profile_distro,
            profiles::commands::set_profile_tab_icon,
            profiles::commands::test_connection,
            tunnel::commands::list_tunnels,
            tunnel::commands::list_tunnel_statuses,
            tunnel::commands::save_tunnel,
            tunnel::commands::delete_tunnel,
            tunnel::commands::start_tunnel,
            tunnel::commands::stop_tunnel,
            tunnel::commands::reset_tunnel_traffic_totals,
            tunnel::commands::auto_start_tunnels,
            background::exit_guard::confirm_quit_app,
            background::exit_guard::cancel_quit_prompt,
            background::resolve_close_request,
            background::quit::set_save_before_quit,
            background::quit::ready_to_quit,
            background::tray::take_tray_update_check,
            command_library::commands::list_saved_commands,
            command_library::commands::list_command_tags,
            command_library::commands::create_command_tag,
            command_library::commands::rename_command_tag,
            command_library::commands::delete_command_tag,
            command_library::commands::save_saved_command,
            command_library::commands::delete_saved_command,
            command_library::commands::set_saved_command_favorite,
            command_library::commands::record_saved_command_use,
            sftp::internal::api::base_commands::sftp_list_directory,
            sftp::internal::api::base_commands::sftp_release_tab_connections,
            sftp::internal::api::base_commands::sftp_create_directory,
            sftp::internal::api::delete::commands::sftp_delete_entry,
            sftp::internal::api::delete::commands::sftp_delete_entries,
            sftp::internal::api::delete::commands::sftp_preview_delete_entries,
            sftp::internal::api::base_commands::sftp_rename_entry,
            sftp::internal::api::archive_commands::sftp_compress_entries,
            sftp::internal::api::archive_commands::sftp_extract_archive,
            sftp::internal::api::archive_commands::sftp_cancel_remote_operation,
            sftp::internal::api::search_commands::sftp_search_entries,
            sftp::internal::api::permissions_commands::sftp_set_permissions,
            sftp::internal::api::edit_commands::sftp_open_file_for_edit,
            sftp::internal::api::edit_commands::sftp_save_edited_file,
            sftp::internal::api::external::commands::sftp_external_edit_open,
            sftp::internal::api::external::commands::sftp_external_open_with_choices,
            sftp::internal::api::external::commands::sftp_external_app_icons,
            sftp::internal::api::external::commands::sftp_external_edit_upload,
            sftp::internal::api::external::commands::sftp_external_edit_list,
            sftp::internal::api::external::commands::sftp_external_edit_stop,
            sftp::internal::api::external::commands::sftp_external_edit_release_tab,
            sftp::internal::api::external::commands::sftp_external_edit_reveal,
            sftp::internal::api::upload::commands::sftp_upload_file,
            sftp::internal::api::upload::commands::sftp_upload_paths,
            sftp::internal::api::upload::commands::sftp_check_upload_conflicts,
            sftp::internal::api::upload::commands::sftp_cancel_upload,
            sftp::internal::api::download_commands::sftp_download_file,
            sftp::internal::api::download_commands::sftp_download_paths,
            sftp::internal::api::download_commands::sftp_check_download_conflicts,
            sftp::internal::api::download_commands::get_file_size,
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
            ssh::key_vault::commands::list_vault_keys,
            ssh::key_vault::commands::import_vault_key,
            ssh::key_vault::commands::rename_vault_key,
            ssh::key_vault::commands::set_vault_key_certificate,
            ssh::key_vault::commands::delete_vault_key,
            ssh::key_vault::commands::export_vault_key,
            ssh::known_host_commands::list_known_hosts,
            ssh::known_host_commands::delete_known_hosts,
            updater::check_app_update,
            updater::download_app_update,
            updater::install_downloaded_app_update,
            updater::download_install_app_update,
            main_window::toggle_devtools,
            ime_focus::repair_ime_focus,
            launch_dir::take_launch_dirs,
            system_integration::shell_command_status,
            system_integration::set_shell_command,
            system_integration::folder_menu_status,
            system_integration::set_folder_menu,
            notification::show_notification,
            notification::notification_permission,
            notification::import_notification_sound,
            notification::read_notification_sound,
            notification::badge::set_unseen_notifications,
            agent_integration::agent_integration_status,
            agent_integration::install_agent_integration,
            agent_integration::uninstall_agent_integration,
            core::agent_resume_commands::build_agent_resume_command,
            agent_integration::remote::remote_agent_integration_status,
            agent_integration::remote::change_remote_agent_integration,
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let paths = app_paths::initialize(&app_handle)?;
            let identity = app_identity::initialize(&app_handle);
            let events: Arc<dyn tterm_core::events::EventSink> =
                Arc::new(event_sink::TauriEventSink::new(app_handle.clone()));
            let mut prompts = None;
            let platform_services = platform::initialize(&app_handle, |main_window, badge| {
                let notifications = notification::create(
                    identity.identifier.clone(),
                    main_window,
                    badge,
                    events.clone(),
                );
                prompts = Some(Arc::new(tterm_core::prompts::PromptBroker::new(
                    events.clone(),
                )));
                let integration = system_integration::initialize((|| {
                    Ok(tterm_platform::native::integration::IntegrationContext {
                        local_data_dir: paths.local_data_dir.clone(),
                        identifier: identity.identifier.clone(),
                        home_dir: app_handle
                            .path()
                            .home_dir()
                            .map_err(|e| format!("Failed to find your home folder: {e}"))?,
                    })
                })());
                (notifications, integration)
            });
            let services = Arc::new(services::AppServices {
                shared: services::shared::SharedServices {
                    paths,
                    identity,
                    events,
                    prompts: prompts.ok_or("Prompt broker was not initialized")?,
                    secrets: secret_store.clone(),
                    runtime: runtime_handle.clone(),
                    platform: platform_services,
                },
                session: services::session::SessionServices {
                    ptys: pty_map.clone(),
                    zmodem: zmodem_map.clone(),
                    zmodem_armed_send: zmodem_armed_send_map.clone(),
                    zmodem_manual_detect: zmodem_manual_detect_map.clone(),
                    monitors: monitor_sessions.clone(),
                    monitor_prompts: monitor_prompts.clone(),
                },
                sftp: services::sftp::SftpServices {
                    pool: sftp_pool.clone(),
                    transfer_cancel: transfer_cancel_map.clone(),
                    external_edit: external_edit_state.clone(),
                },
                app: services::app::AppLevelServices {
                    tunnels: tunnel_manager.clone(),
                    session_log: session_log_state.clone(),
                    close_behavior: close_behavior.clone(),
                    quit: quit_state.clone(),
                    exit_guard: exit_guard.clone(),
                    launch_dirs: launch_dirs.clone(),
                    tray: tray_state.clone(),
                    pending_updates: pending_updates.clone(),
                },
            });
            services.validate_wiring();
            app.manage(services.clone());
            launch_dir::open_from_startup(
                &services.app.launch_dirs,
                services.shared.events.as_ref(),
                services.shared.paths.as_ref(),
            );

            if let Err(err) =
                updater::cleanup_stale_pending_update_files(services.shared.paths.as_ref())
            {
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
            services.shared.secrets.initialize(
                services.shared.identity.as_ref(),
                &services.shared.paths.data_dir,
            );

            if let Err(error) = migrate::remove_retired_files(&app_handle) {
                eprintln!("Failed to remove retired files: {error}");
            }

            let cfg = config::load_config_file().ok();
            if let Some(cfg) = &cfg {
                sftp::internal::api::apply_bandwidth_limits(cfg);
                let log_state = &services.app.session_log;
                if let Err(err) =
                    log_state.apply_config(&crate::connection_context::log(&services), cfg)
                {
                    eprintln!("Failed to initialize terminal logging: {}", err);
                }
            }

            if let Ok(paths) = crate::app_paths::get(&app_handle) {
                sftp::internal::api::external::clean_up_on_start(
                    paths.as_ref(),
                    &services.shared.runtime,
                );
            }
            sftp::internal::connection::spawn_idle_reaper(
                &services.shared.runtime,
                services.sftp.pool.clone(),
            );
            notification::init(services.shared.platform.notifications.as_ref());
            #[cfg(target_os = "macos")]
            terminal::pty::preload_login_shell_env();
            main_window::create_main_window(app, cfg.as_ref())?;
            let cfg = cfg.unwrap_or_default();
            background::apply_config(
                &services.app.close_behavior,
                &app_context::tray(&services),
                &cfg,
            );
            runtime_handle.block_on(system_integration::apply_config(
                &services.shared.platform.integration,
                &cfg,
            ));

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
                        .try_state::<Arc<services::AppServices>>()
                        .is_some_and(|services| {
                            background::on_exit_requested(&app_context::background(
                                &services,
                                sftp::internal::api::external::unuploaded_files(
                                    &services.sftp.external_edit,
                                ),
                            ))
                        })
                    {
                        api.prevent_exit();
                    }
                }
                tauri::RunEvent::Exit => {
                    let Some(services) = app.try_state::<Arc<services::AppServices>>() else {
                        return;
                    };
                    services.shared.prompts.shutdown();
                    window_state::save(app);
                    if let Ok(paths) = app_paths::get(app) {
                        launch_dir::forget_startup_dir(&paths);
                    }
                    // Tunnels may still be running; keep what they carried.
                    services.app.tunnels.save_traffic();
                    // Logs keep what they have not written out yet.
                    session_log::shutdown(&services.app.session_log);
                    sftp::internal::api::external::shutdown(&services.sftp.external_edit);
                }
                // Clicking the Dock icon brings back a window closed to the
                // background.
                #[cfg(target_os = "macos")]
                tauri::RunEvent::Reopen {
                    has_visible_windows: false,
                    ..
                } => {
                    if let Some(services) = app.try_state::<Arc<services::AppServices>>() {
                        background::show_main_window(
                            services.shared.platform.main_window.as_ref(),
                            services.shared.platform.badge.as_ref(),
                        );
                    }
                }
                _ => {}
            }
        });
}

#[cfg(test)]
mod prompt_contract_tests;
