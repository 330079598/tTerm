//! Tauri entry adapter for assembling concrete session dependencies.
use crate::{services::AppServices, session_log::LogContext, ssh::context::ConnectionContext};

pub(crate) fn connection(services: &AppServices) -> ConnectionContext {
    ConnectionContext {
        events: services.shared.events.clone(),
        prompts: services.shared.prompts.clone(),
        window: services.shared.platform.main_window.clone(),
        ptys: std::sync::Arc::downgrade(&services.session.ptys),
        log: log(services),
    }
}
pub(crate) fn log(services: &AppServices) -> LogContext {
    LogContext {
        state: services.app.session_log.clone(),
        events: services.shared.events.clone(),
    }
}

/// Assembles the concrete dependencies for an SFTP side connection.
pub(crate) fn sftp(services: &AppServices) -> crate::sftp::internal::connection::SftpContext {
    crate::sftp::internal::connection::SftpContext {
        ssh: connection(services),
        secrets: services.shared.secrets.clone(),
    }
}
