use super::{SshExitSignal, finish_output};
use crate::core::session::types::SessionPlan;
use crate::ssh::jump::pool::PooledSession;
use crate::ssh::types::SshClientHandler;
use tokio::sync::mpsc;
use tterm_core::events::ssh::ShellIntegrationStatus;

/// Opens a session channel with a PTY, ready for a shell or command.
async fn open_terminal_channel(
    session: &russh::client::Handle<SshClientHandler>,
    plan: &SessionPlan,
    rows: u16,
    cols: u16,
) -> Result<russh::Channel<russh::client::Msg>, String> {
    let channel = session
        .channel_open_session()
        .await
        .map_err(|err| format!("Failed to open SSH channel: {err}"))?;

    if plan.agent_forward {
        // Best-effort: a server with agent forwarding disabled just won't
        // open a channel back later, which isn't worth failing the session over.
        if let Err(err) = channel.agent_forward(false).await {
            eprintln!("SSH agent forwarding request failed: {err}");
        }
    }

    channel
        .request_pty(false, "xterm-256color", cols as u32, rows as u32, 0, 0, &[])
        .await
        .map_err(|err| format!("Failed to request SSH PTY: {err}"))?;
    // Remote TUIs otherwise quantize to 256 colors. Servers only take it when
    // their AcceptEnv lists it (recent Debian and Ubuntu do), so a refusal is
    // ignored rather than asked for.
    if let Err(err) = channel.set_env(false, "COLORTERM", "truecolor").await {
        eprintln!("SSH COLORTERM request failed: {err}");
    }
    Ok(channel)
}

/// Returns the newest size the terminal reported while the connection was
/// being set up, or `size` when it reported none. Opening the PTY at that size
/// spares the shell a resize while it prints its first prompt: readline
/// redraws the prompt for the old width then, and on an older bash that
/// lands on the line above and leaves a broken copy behind.
pub(super) fn latest_queued_size(
    resize_rx: &mut mpsc::UnboundedReceiver<(u16, u16)>,
    size: (u16, u16),
) -> (u16, u16) {
    let mut latest = size;
    while let Ok((rows, cols)) = resize_rx.try_recv() {
        latest = (rows, cols);
    }
    latest
}

pub(super) struct PreparedTerminal {
    pub(super) channel: russh::Channel<russh::client::Msg>,
    pub(super) early_output: Option<Vec<u8>>,
    pub(super) shell_integration_status: Option<ShellIntegrationStatus>,
    pub(super) network_latency_ms: u64,
}

pub(super) async fn prepare_terminal(
    session: &PooledSession<SshClientHandler>,
    plan: &SessionPlan,
    host: &str,
    username: &str,
    size: (u16, u16),
    resize_rx: &mut mpsc::UnboundedReceiver<(u16, u16)>,
    sender: &mut Option<crate::terminal::io_batcher::TerminalOutputSender>,
) -> Result<PreparedTerminal, SshExitSignal> {
    let (rows, cols) = size;
    // The command that starts the shell with shell integration, and how
    // integration went for the connection header.
    let unavailable = |reason| Some(ShellIntegrationStatus::Unavailable { reason });
    let (shell_integration, mut shell_integration_status) = if !plan.shell_integration {
        (None, None)
    } else {
        let target = format!("{username}@{host}:{}", plan.port);
        let marks = crate::config::command_marks_enabled();
        match crate::ssh::shell_integration::prepare(&session.handle, &target, marks).await {
            Ok(prepared) => (Some(prepared), None),
            Err(reason) => (None, unavailable(reason)),
        }
    };

    let (mut rows, mut cols) = latest_queued_size(resize_rx, (rows, cols));
    let channel_open_started_at = tokio::time::Instant::now();
    let mut channel = match open_terminal_channel(&session.handle, plan, rows, cols).await {
        Ok(channel) => channel,
        Err(reason) => {
            finish_output(sender);
            return Err(SshExitSignal {
                terminated: false,
                recoverable: true,
                reason: Some(reason),
                connected_duration: None,
            });
        }
    };
    let network_latency_ms = channel_open_started_at
        .elapsed()
        .as_millis()
        .min(u64::MAX as u128) as u64;

    // With shell integration: what the shell printed before the server
    // confirmed it started. `None` means a plain shell.
    let mut early_output = None;
    if let Some(prepared) = shell_integration {
        match crate::ssh::shell_integration::start(&mut channel, &prepared.command).await {
            Ok(output) => {
                early_output = Some(output);
                shell_integration_status = Some(ShellIntegrationStatus::Active {
                    hostname: prepared.hostname,
                    shell: prepared.shell,
                });
            }
            Err(reason) => shell_integration_status = unavailable(reason),
        }
        if early_output.is_none() {
            // That channel is closed; the plain shell gets a fresh one.
            (rows, cols) = latest_queued_size(resize_rx, (rows, cols));
            channel = match open_terminal_channel(&session.handle, plan, rows, cols).await {
                Ok(channel) => channel,
                Err(reason) => {
                    finish_output(sender);
                    return Err(SshExitSignal {
                        terminated: false,
                        recoverable: true,
                        reason: Some(reason),
                        connected_duration: None,
                    });
                }
            };
        }
    }

    if early_output.is_none()
        && let Err(err) = channel.request_shell(false).await
    {
        finish_output(sender);
        return Err(SshExitSignal {
            terminated: false,
            recoverable: true,
            reason: Some(format!("Failed to request SSH shell: {err}")),
            connected_duration: None,
        });
    }

    Ok(PreparedTerminal {
        channel,
        early_output,
        shell_integration_status,
        network_latency_ms,
    })
}
