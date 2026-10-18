use super::session::{serial_options, spawn_serial_attempt, ActiveSerial};
use crate::core::supervisor::TabConnection;
use crate::ssh::types::{
    emit_connection_progress, ConnectionStatusOptions, SshConnectionProgressPayload,
};
use std::time::Duration;
use tokio::sync::watch;

/// How often a tab whose device is gone looks for it again. USB serial
/// adapters come back within a second or two of a board's reset or replug.
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Waits for a tab's serial device to come back and opens it again, for as
/// long as the tab stays open: a board is often unplugged for minutes while
/// it is rewired. `None` once the tab closes.
pub(crate) async fn reopen(
    tab: &TabConnection,
    reason: &str,
    stop_rx: &mut watch::Receiver<bool>,
) -> Option<ActiveSerial> {
    let app = &tab.channel.app;
    let tab_id = tab.channel.tab_id.as_str();
    let saved_port = serial_options(tab).ok()?.port.clone();
    emit_connection_progress(
        app,
        tab_id,
        ConnectionStatusOptions::VERBOSE,
        SshConnectionProgressPayload::new(
            "serial_waiting",
            format!("{saved_port} disconnected ({reason}); waiting for it to come back"),
        )
        .host(saved_port.clone(), 0)
        .reason(reason),
    );

    loop {
        tokio::select! {
            _ = tokio::time::sleep(POLL_INTERVAL) => {}
            _ = stop_rx.changed() => {}
        }
        if *stop_rx.borrow() {
            return None;
        }

        let attempt_tab = tab.clone();
        let opened = tokio::task::spawn_blocking(move || {
            let options = serial_options(&attempt_tab)?;
            let ports = super::ports::list_ports()?;
            let path = super::ports::port_to_open(options, &ports)
                .ok_or_else(|| "The device is not connected".to_string())?;
            spawn_serial_attempt(&attempt_tab, &path)
        })
        .await;
        // A device still starting up refuses to open for a moment; keep trying.
        let Ok(Ok(serial)) = opened else {
            continue;
        };
        if *stop_rx.borrow() {
            return None;
        }
        emit_connection_progress(
            app,
            tab_id,
            ConnectionStatusOptions::VERBOSE,
            SshConnectionProgressPayload::new("ready", format!("Reconnected to {}", serial.path))
                .host(serial.path.clone(), 0),
        );
        return Some(serial);
    }
}
