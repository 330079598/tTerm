mod config;
mod core;
mod fonts;
mod profiles;
mod session;
mod ssh;
mod terminal;

use core::PtyMap;
use std::sync::Arc;
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
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            FramePluginBuilder::new()
                // Titlebar height in pixels
                .titlebar_height(32)
                // Button width in pixels
                .button_width(46)
                // Automatically apply titlebar to all windows
                .auto_titlebar(true)
                // Delay before pressing Alt to hide snap overlay numbers (ms)
                .snap_overlay_delay_ms(15)
                // Close button hover background color
                .close_hover_bg("rgba(196,43,28,1)")
                // Other buttons hover background color
                .button_hover_bg("rgba(255,255,255,0.1)")
                .build(),
        )
        .manage(pty_map)
        .manage(host_prompt_map)
        .manage(TokioRuntimeState { runtime })
        .invoke_handler(tauri::generate_handler![
            // Config
            config::load_config,
            config::save_config,
            // Session
            session::load_session,
            session::save_session,
            session::clear_session,
            // PTY/SSH
            core::commands::create_pty,
            core::commands::write_pty,
            core::commands::resize_pty,
            core::commands::kill_pty,
            core::commands::respond_ssh_host_key_prompt,
            // Fonts
            fonts::list_fonts,
            // Profiles
            profiles::list_profiles,
            profiles::save_profile,
            profiles::delete_profile,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
