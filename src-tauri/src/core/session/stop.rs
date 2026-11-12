use crate::core::state::{
    ActiveSession, PtySession, ZmodemArmedSendMap, ZmodemManualDetectMap, ZmodemMap,
};
use std::sync::PoisonError;

/// The prompt broker and tab that identify the session being stopped.
pub(crate) struct StopPtyContext<'a> {
    /// Broker used to cancel prompts owned by the session.
    pub(crate) prompts: &'a tterm_core::prompts::PromptBroker,
    /// Tab that owns the session.
    pub(crate) tab_id: &'a str,
}

/// Stops a PTY session, cancels its prompts, closes its output, and removes
/// its ZMODEM and supervisor state.
pub(crate) fn stop_pty_session(
    context: StopPtyContext<'_>,
    session: PtySession,
    zmodem_state: &ZmodemMap,
    zmodem_armed_send_state: &ZmodemArmedSendMap,
    zmodem_manual_detect_state: &ZmodemManualDetectMap,
) {
    let StopPtyContext { prompts, tab_id } = context;
    prompts.cancel_session(&tterm_core::prompts::SessionKey {
        tab_id: tab_id.to_string(),
        session_nonce: session.session_nonce,
    });
    let _ = session.stop_tx.send(true);
    // Nobody will acknowledge this channel's output again.
    session.sink.close_output();

    if let Some(active) = session.active.blocking_lock().take() {
        match active {
            ActiveSession::Local(mut local) => {
                let _ = local.child.kill();
            }
            ActiveSession::Ssh(ssh) => {
                ssh.task.abort();
            }
            // Dropping it closes the port.
            ActiveSession::Serial(_) => {}
        }
    }

    // The peer's rz/sz process (if any) dies with this channel; a ZMODEM
    // session can't meaningfully survive it.
    zmodem_state
        .write()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(tab_id);
    zmodem_armed_send_state
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(tab_id);
    zmodem_manual_detect_state
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(tab_id);

    session.supervisor.abort();
}
