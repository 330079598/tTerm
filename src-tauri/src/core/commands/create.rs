use crate::core::session::stop::{StopPtyContext, stop_pty_session};
use crate::core::session::{normalize_connection, resolve_ssh_password};
use crate::core::state::{
    ActiveSession, AtomicTerminalSize, ChannelContext, PtyMap, PtySession, SessionExitSignal,
    TabZmodem, ZmodemArmedSendMap, ZmodemManualDetectMap, ZmodemMap,
};
use crate::core::supervisor::{TabConnection, spawn_ssh_attempt, spawn_supervisor};
use crate::ssh::context::ConnectionContext;
use crate::{terminal, zmodem::session::ZmodemSettings};
use std::sync::{Arc, PoisonError, atomic::AtomicBool};
use tokio::sync::{Mutex as TokioMutex, mpsc, watch};

pub(super) struct CreateContext {
    pub(super) connection: ConnectionContext,
    pub(super) ptys: PtyMap,
    pub(super) secrets: crate::ssh::secret_store::types::SecretStoreState,
    pub(super) runtime: tokio::runtime::Handle,
    pub(super) zmodem: ZmodemMap,
    pub(super) armed: ZmodemArmedSendMap,
    pub(super) manual: ZmodemManualDetectMap,
}

pub(super) struct PtyIdentity {
    pub(super) tab_id: String,
    pub(super) session_nonce: u32,
    pub(super) rows: u16,
    pub(super) cols: u16,
}

pub(super) fn create_pty_blocking(
    context: CreateContext,
    identity: PtyIdentity,
    connection: Option<crate::core::session::types::PtyConnectionOptions>,
    output_channel: crate::terminal::output::TerminalOutput,
    sink: Arc<dyn tterm_core::session::sink::SessionSink>,
) -> Result<u32, String> {
    let PtyIdentity {
        tab_id,
        session_nonce,
        rows,
        cols,
    } = identity;
    let app = context.connection;
    let state = context.ptys;
    let prompt_state = app.prompts.clone();
    let secret_state = context.secrets;
    let zmodem_state = context.zmodem;
    let zmodem_armed_send_state = context.armed;
    let zmodem_manual_detect_state = context.manual;
    let runtime_handle = context.runtime;
    let mut plan = normalize_connection(connection)?;
    resolve_ssh_password(&secret_state, &mut plan)?;

    let mut sessions = state.blocking_write();
    let reattached = sessions
        .get(&tab_id)
        .and_then(|session| session.reattach(session_nonce, rows, cols, &output_channel));
    if let Some(pid) = reattached {
        return Ok(pid);
    }

    if let Some(session) = sessions.remove(&tab_id) {
        stop_pty_session(
            StopPtyContext {
                prompts: &prompt_state,
                tab_id: &tab_id,
            },
            session,
            &zmodem_state,
            &zmodem_armed_send_state,
            &zmodem_manual_detect_state,
        );
    }

    prompt_state.open_session(&tterm_core::prompts::SessionKey {
        tab_id: tab_id.clone(),
        session_nonce,
    });
    crate::session_log::start_session(&app.log, &tab_id, session_nonce, &plan, rows, cols);

    let (exit_tx, exit_rx) = mpsc::unbounded_channel::<SessionExitSignal>();
    let active = Arc::new(TokioMutex::new(None));
    let (stop_tx, stop_rx) = watch::channel(false);
    let size = Arc::new(AtomicTerminalSize::new(rows, cols));
    // Lives for the tab's whole lifetime (unlike `ZmodemMap`, only present
    // while a transfer is in flight) so the manual "Send/Receive Files via
    // ZMODEM" keymap actions can force detection on later, even across a
    // reconnect.
    let zmodem_manual_override = Arc::new(AtomicBool::new(false));
    zmodem_manual_detect_state
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(tab_id.clone(), zmodem_manual_override.clone());

    let tab = TabConnection {
        app: app.clone(),
        channel: ChannelContext {
            sink: sink.clone(),
            tab_id: tab_id.clone(),
            session_nonce,
            active: active.clone(),
            zmodem: TabZmodem {
                map: zmodem_state.clone(),
                armed_send_map: zmodem_armed_send_state.clone(),
                manual_override: zmodem_manual_override,
            },
        },
        plan: plan.clone(),
        prompt_state,
        stop_rx: stop_rx.clone(),
        exit_tx,
        runtime_handle: runtime_handle.clone(),
    };

    let (pid, initial_active) = spawn_initial_session(&tab, rows, cols).inspect_err(|_| {
        tab.prompt_state
            .cancel_session(&tterm_core::prompts::SessionKey {
                tab_id: tab_id.clone(),
                session_nonce,
            });
    })?;

    {
        let mut guard = runtime_handle.block_on(active.lock());
        *guard = Some(initial_active);
    }

    let supervisor = spawn_supervisor(tab, exit_rx, size.clone());

    let session = PtySession {
        pid,
        session_nonce,
        active,
        stop_tx,
        supervisor,
        size,
        output: output_channel,
        sink,
        serial_view: plan.serial.as_ref().map(|serial| serial.view.clone()),
    };

    sessions.insert(tab_id, session);

    Ok(pid)
}

/// Starts a tab's first connection attempt, returning the pid of its local
/// shell (0 when there is none) and the live session.
fn spawn_initial_session(
    tab: &TabConnection,
    rows: u16,
    cols: u16,
) -> Result<(u32, ActiveSession), String> {
    let channel = &tab.channel;
    match tab.plan.kind {
        crate::core::state::SessionKind::Terminal => {
            let (pid, pty) =
                terminal::pty::spawn_local_pty(rows, cols, tab.plan.terminal_shell.clone())?;

            let reader = pty.input.track_output(
                pty.master
                    .try_clone_reader()
                    .map_err(|e| format!("Failed to clone reader: {}", e))?,
            );

            let sender = crate::terminal::io_batcher::TerminalOutputSender::spawn(
                &channel.tab_id,
                channel.sink.clone(),
            );
            terminal::reader::spawn_reader_thread(
                reader,
                channel.clone(),
                tab.exit_tx.clone(),
                Some(sender),
                ZmodemSettings::load(),
                None,
                crate::terminal::reader::reader_log(&tab.app.log, &channel.tab_id),
            );

            #[cfg(unix)]
            terminal::cwd_watch::spawn_cwd_watcher(
                tab.app.events.clone(),
                channel.tab_id.clone(),
                channel.session_nonce,
                pid,
                tab.stop_rx.clone(),
            );

            Ok((pid, ActiveSession::Local(pty)))
        }
        crate::core::state::SessionKind::Ssh => {
            let ssh = spawn_ssh_attempt(tab.clone(), rows, cols, ZmodemSettings::load());

            Ok((0, ActiveSession::Ssh(ssh)))
        }
        crate::core::state::SessionKind::Serial => {
            let serial = crate::serial::session::spawn_first_serial_attempt(tab)?;

            Ok((0, ActiveSession::Serial(serial)))
        }
    }
}
