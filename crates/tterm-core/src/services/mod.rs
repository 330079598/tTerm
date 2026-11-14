//! The services both clients build once and hand to their commands.

pub mod app;
pub mod session;
pub mod sftp;
pub mod shared;
#[cfg(test)]
pub(crate) mod test_support;

/// Every service the commands and background tasks use. Built once in
/// `setup()` and handed out as `Arc<AppServices>`; it holds no client types so
/// P1-09 can move it into `tterm_core` unchanged.
pub struct AppServices {
    pub shared: crate::services::shared::SharedServices,
    pub session: crate::services::session::SessionServices,
    pub sftp: crate::services::sftp::SftpServices,
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

    /// Creates or reattaches a session using this client's shared services.
    pub fn create_pty_blocking(
        &self,
        identity: crate::core::create::PtyIdentity,
        connection: Option<crate::core::session::types::PtyConnectionOptions>,
        output: crate::terminal::output::TerminalOutput,
        sink: std::sync::Arc<dyn crate::session::sink::SessionSink>,
    ) -> Result<u32, String> {
        crate::core::create::create_pty_blocking(
            crate::core::create::CreateContext {
                connection: crate::connection_context::connection(self),
                ptys: self.session.ptys.clone(),
                secrets: self.shared.secrets.clone(),
                runtime: self.shared.runtime.clone(),
                zmodem: self.session.zmodem.clone(),
                armed: self.session.zmodem_armed_send.clone(),
                manual: self.session.zmodem_manual_detect.clone(),
            },
            identity,
            connection,
            output,
            sink,
        )
    }

    /// Sends raw bytes to a session owned by this client.
    pub fn write_pty(&self, key: &crate::prompts::SessionKey, data: &[u8]) -> Result<(), String> {
        crate::core::session::operations::write_pty(
            &crate::connection_context::connection(self),
            &self.session.ptys,
            &self.session.zmodem,
            key,
            data,
        )
    }

    /// Resizes a session owned by this client.
    pub fn resize_pty(
        &self,
        key: &crate::prompts::SessionKey,
        rows: u16,
        cols: u16,
    ) -> Result<(), String> {
        crate::core::session::operations::resize_pty(
            &crate::connection_context::connection(self),
            &self.session.ptys,
            key,
            rows,
            cols,
        )
    }

    /// Stops a session owned by this client and releases its prompts and output.
    pub fn kill_pty(&self, key: &crate::prompts::SessionKey) -> Result<(), String> {
        crate::core::session::operations::kill_pty(
            &crate::connection_context::connection(self),
            &self.session.ptys,
            &self.session.zmodem,
            &self.session.zmodem_armed_send,
            &self.session.zmodem_manual_detect,
            key,
        )
    }
}

#[cfg(test)]
mod tests;
