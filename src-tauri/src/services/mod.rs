pub mod app;
pub mod session;
pub mod sftp;
pub mod shared;

/// Every service the commands and background tasks use. Built once in
/// `setup()` and handed out as `Arc<AppServices>`; it holds no client types so
/// P1-09 can move it into `tterm_core` unchanged.
pub struct AppServices {
    pub shared: shared::SharedServices,
    pub session: session::SessionServices,
    pub sftp: sftp::SftpServices,
    pub app: app::AppLevelServices,
}

impl AppServices {
    /// Touches every slot while the transition keeps the legacy managed states alive.
    pub fn validate_wiring(&self) {
        let _ = (
            &self.shared.events,
            &self.shared.prompts,
            &self.shared.secrets,
            &self.shared.runtime,
            &self.shared.platform,
            &self.session.ptys,
            &self.session.zmodem,
            &self.session.zmodem_armed_send,
            &self.session.zmodem_manual_detect,
            &self.session.monitors,
            &self.session.monitor_prompts,
            &self.sftp.pool,
            &self.sftp.transfer_cancel,
            &self.sftp.external_edit,
            &self.app.tunnels,
            &self.app.session_log,
            &self.app.close_behavior,
            &self.app.quit,
            &self.app.exit_guard,
            &self.app.launch_dirs,
            &self.app.tray,
            &self.app.pending_updates,
        );
    }
}
