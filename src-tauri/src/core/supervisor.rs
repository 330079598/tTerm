use tterm_core::events::ssh::SshConnectionProgressPayload;

use super::session::types::SessionPlan;
use super::state::{ActiveSession, ActiveSsh, ChannelContext, SessionExitSignal, SessionKind};
use crate::ssh::client::SshChannelIo;
use crate::ssh::client::SshExitSignal;
use crate::ssh::context::ConnectionContext;
use crate::zmodem::session::ZmodemSettings;
use std::sync::Arc;
use std::sync::PoisonError;
use std::time::Duration;
use tokio::sync::{mpsc, watch};

/// Nominal delay before the first reconnect attempt.
const RECONNECT_INITIAL_DELAY_SECS: u64 = 1;
/// Upper bound for the (pre-jitter) backoff delay.
const RECONNECT_MAX_DELAY_SECS: u64 = 30;
/// A session that stayed up at least this long fully forgives the attempt
/// counter, so one flappy outage cannot exhaust the retry budget of a later
/// real drop. A reconnect that held for only part of this window still earns
/// proportional credit instead of an all-or-nothing reset — see
/// `decayed_attempt`.
const RECONNECT_STABLE_RESET: Duration = Duration::from_secs(60);

/// Map the raw SSH task outcome onto the supervisor's exit taxonomy.
pub(crate) fn map_ssh_exit_signal(result: SshExitSignal) -> SessionExitSignal {
    match (result.terminated, result.recoverable) {
        (false, true) => SessionExitSignal::Recoverable {
            reason: result.reason.unwrap_or_default(),
            connected_duration: result.connected_duration,
        },
        (false, false) => SessionExitSignal::NonRecoverable(result.reason.unwrap_or_default()),
        _ => SessionExitSignal::Terminated,
    }
}

/// What every connection attempt of one tab shares, from the first one
/// through each automatic reconnect.
#[derive(Clone)]
pub(crate) struct TabConnection {
    pub channel: ChannelContext,
    pub app: ConnectionContext,
    pub plan: SessionPlan,
    pub prompt_state: std::sync::Arc<tterm_core::prompts::PromptBroker>,
    pub stop_rx: watch::Receiver<bool>,
    pub exit_tx: mpsc::UnboundedSender<SessionExitSignal>,
    pub runtime_handle: tokio::runtime::Handle,
}

/// Spawn one SSH connection attempt, returning the live handle for `active`.
/// Used for the first attempt and again for every automatic reconnect — the
/// output `Channel` is cloned per attempt because `TerminalOutputSender` closes
/// its worker on drop.
pub(crate) fn spawn_ssh_attempt(
    tab: TabConnection,
    rows: u16,
    cols: u16,
    settings: ZmodemSettings,
) -> ActiveSsh {
    let TabConnection {
        channel,
        app,
        plan,
        prompt_state,
        stop_rx,
        exit_tx,
        runtime_handle,
    } = tab;
    let (input_tx, input_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let (resize_tx, resize_rx) = mpsc::unbounded_channel::<(u16, u16)>();
    let output_tail = Arc::new(crate::terminal::output_tail::OutputTail::default());
    let encoding = plan.encoding;
    let input = crate::terminal::types::PtyInput::over_channel(input_tx.clone());
    let sender = crate::terminal::io_batcher::TerminalOutputSender::spawn(
        &channel.tab_id,
        channel.sink.clone(),
    )
    .with_output_tail(output_tail.clone())
    .with_output_counter(input.output_counter());

    let task = runtime_handle.spawn(async move {
        let result = crate::ssh::client::run_single_ssh_connection(
            crate::ssh::client::SshAttemptContext {
                channel,
                connection: app,
            },
            settings,
            plan,
            prompt_state,
            rows,
            cols,
            SshChannelIo {
                stop_rx,
                input_rx,
                resize_rx,
                sender: Some(sender),
            },
        )
        .await;
        let _ = exit_tx.send(map_ssh_exit_signal(result));
    });

    ActiveSsh {
        input_tx,
        input,
        resize_tx,
        task,
        output_tail,
        encoding,
    }
}

/// Nominal (pre-jitter) backoff before the 1-indexed `attempt`-th reconnect:
/// 1, 2, 4, 8, 16, 30, 30, …
fn reconnect_nominal_secs(attempt: u32) -> u64 {
    let exponent = attempt.saturating_sub(1).min(31);
    (RECONNECT_INITIAL_DELAY_SECS << exponent).min(RECONNECT_MAX_DELAY_SECS)
}

/// Forgive part of the retry budget based on how long the last reconnect held
/// before dropping again: a session that stayed up for the full
/// `RECONNECT_STABLE_RESET` window is treated the same as before (fully
/// forgiven), a session that never connected keeps the whole count, and
/// anything in between is prorated. This lets a connection that keeps
/// reconnecting but only holding for, say, 30-45s at a time survive far
/// longer than a hard, immediately-failing outage — instead of both cases
/// burning the same fixed number of attempts.
fn decayed_attempt(attempt: u32, connected_duration: Duration) -> u32 {
    let stable_secs = RECONNECT_STABLE_RESET.as_secs_f64();
    let forgiveness = (connected_duration.as_secs_f64() / stable_secs).min(1.0);
    let retained = attempt as f64 * (1.0 - forgiveness);
    retained.round() as u32
}

/// Equal jitter: keep half the nominal delay fixed and randomize the rest, so
/// retries spread out without ever collapsing to a near-zero sleep.
fn equal_jitter(nominal_secs: u64) -> Duration {
    let nominal_ms = nominal_secs.saturating_mul(1000);
    let half_ms = nominal_ms / 2;
    let spread_ms = nominal_ms - half_ms;
    let extra_ms = if spread_ms > 0 {
        rand::random::<u64>() % (spread_ms + 1)
    } else {
        0
    };
    Duration::from_millis(half_ms + extra_ms)
}

/// Starts the next connection attempt of an SSH tab.
fn spawn_ssh_reconnect(tab: TabConnection, rows: u16, cols: u16) -> ActiveSession {
    // Re-read on every reconnect (not just the initial spawn) so a settings
    // change takes effect without needing a fresh tab.
    ActiveSession::Ssh(spawn_ssh_attempt(tab, rows, cols, ZmodemSettings::load()))
}

/// Clears what died with a tab's last connection attempt.
async fn clear_attempt(channel: &ChannelContext) {
    *channel.active.lock().await = None;
    // The peer's rz/sz process (if any) dies with the old channel; a ZMODEM
    // session can't meaningfully survive a reconnect.
    channel
        .zmodem
        .map
        .write()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&channel.tab_id);
    channel
        .zmodem
        .armed_send_map
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&channel.tab_id);
}

/// Reports a session that ended for good, without a reconnect.
fn report_exit(app: &ConnectionContext, channel: &ChannelContext, signal: &SessionExitSignal) {
    let exit_reason = match signal {
        SessionExitSignal::Recoverable { reason, .. }
        | SessionExitSignal::NonRecoverable(reason) => {
            let text = crate::terminal::legacy_events::status_line("31", reason);
            crate::session_log::record_output(&app.log, &channel.tab_id, text.as_bytes());
            let _ = channel.sink.emit(tterm_core::events::AppEvent::Terminal(
                tterm_core::events::terminal::TerminalEvent::Output {
                    tab_id: channel.tab_id.clone(),
                    text,
                },
            ));
            Some(reason.as_str())
        }
        SessionExitSignal::Terminated => None,
    };
    let _ = channel.sink.exited(exit_reason.map(str::to_string));
    crate::session_log::end_session(&app.log, &channel.tab_id);
}

/// The retry budget of one SSH tab's automatic reconnects.
struct SshBackoff {
    attempt: u32,
    max_attempts: u32,
}

impl SshBackoff {
    /// Waits out the backoff before the next attempt. Returns `false` when the
    /// budget is spent, after reporting the disconnect, or when the tab stops.
    async fn wait(
        &mut self,
        app: &ConnectionContext,
        channel: &ChannelContext,
        reason: String,
        connected_duration: Duration,
        stop_rx: &mut watch::Receiver<bool>,
    ) -> bool {
        let max_attempts = self.max_attempts;
        // Forgive part of the backoff attempt counter in proportion to how
        // long this reconnect held before dropping again, instead of only
        // resetting it on a full RECONNECT_STABLE_RESET-long connection.
        self.attempt = decayed_attempt(self.attempt, connected_duration) + 1;
        let attempt = self.attempt;

        // Retry budget exhausted: report the disconnect. The localized
        // terminal line comes from the frontend via the progress event, so
        // no status line is written here.
        if attempt > max_attempts {
            let _ = channel.sink.state_changed(
                SshConnectionProgressPayload::new(
                    "retry_exhausted",
                    format!("Reconnect attempts exhausted ({reason})"),
                )
                .reason(reason.clone())
                .retry_max(max_attempts),
            );
            let _ = channel.sink.exited(Some(reason));
            crate::session_log::end_session(&app.log, &channel.tab_id);
            return false;
        }

        let delay = equal_jitter(reconnect_nominal_secs(attempt));
        let delay_secs = delay.as_secs_f64();
        // Structured retry status: the frontend renders the localized
        // message from these fields instead of the English `message`.
        let _ = channel.sink.state_changed(
            SshConnectionProgressPayload::new(
                "retrying",
                format!("Reconnecting in {delay_secs:.0}s (attempt {attempt})"),
            )
            .reason(reason)
            .retry_attempt(attempt, delay_secs)
            .retry_max(max_attempts),
        );

        tokio::select! {
            _ = tokio::time::sleep(delay) => {}
            _ = stop_rx.changed() => {}
        }
        !*stop_rx.borrow()
    }
}

/// Watches one tab's connection attempts and starts a new one, with backoff,
/// after each recoverable drop.
pub fn spawn_supervisor(
    tab: TabConnection,
    mut exit_rx: mpsc::UnboundedReceiver<SessionExitSignal>,
    size: Arc<super::state::AtomicTerminalSize>,
) -> tokio::task::JoinHandle<()> {
    tab.runtime_handle.clone().spawn(async move {
        let channel = tab.channel.clone();
        let app = &tab.app;
        let plan = &tab.plan;
        let mut stop_rx = tab.stop_rx.clone();
        let reconnect_enabled = plan.reconnect_enabled;
        let mut backoff = SshBackoff {
            attempt: 0,
            max_attempts: plan.reconnect_max_attempts,
        };

        while let Some(signal) = exit_rx.recv().await {
            if *stop_rx.borrow() {
                break;
            }
            clear_attempt(&channel).await;

            let retry = match &signal {
                SessionExitSignal::Recoverable {
                    reason,
                    connected_duration,
                } if reconnect_enabled => Some((reason.clone(), *connected_duration)),
                _ => None,
            };
            let Some((reason, connected_duration)) = retry else {
                report_exit(app, &channel, &signal);
                break;
            };

            let attempt_tab = TabConnection {
                stop_rx: stop_rx.clone(),
                ..tab.clone()
            };
            let next = match plan.kind {
                SessionKind::Serial => {
                    crate::serial::reconnect::reopen(&attempt_tab, &reason, &mut stop_rx)
                        .await
                        .map(ActiveSession::Serial)
                }
                // A local terminal never reconnects; its plan turns reconnects off.
                SessionKind::Ssh | SessionKind::Terminal => {
                    let resume = backoff
                        .wait(
                            app,
                            &channel,
                            reason,
                            connected_duration.unwrap_or_default(),
                            &mut stop_rx,
                        )
                        .await;
                    resume.then(|| {
                        let (rows, cols) = size.load();
                        spawn_ssh_reconnect(attempt_tab, rows, cols)
                    })
                }
            };
            let Some(next) = next else {
                break;
            };
            *channel.active.lock().await = Some(next);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{decayed_attempt, equal_jitter, reconnect_nominal_secs, RECONNECT_STABLE_RESET};
    use std::time::Duration;

    #[test]
    fn decayed_attempt_is_untouched_after_an_immediate_drop() {
        assert_eq!(decayed_attempt(5, Duration::ZERO), 5);
    }

    #[test]
    fn decayed_attempt_is_fully_forgiven_after_a_full_stable_window() {
        assert_eq!(decayed_attempt(5, RECONNECT_STABLE_RESET), 0);
        assert_eq!(decayed_attempt(5, RECONNECT_STABLE_RESET * 2), 0);
    }

    #[test]
    fn decayed_attempt_prorates_a_partial_stable_window() {
        // Half the stable window forgives roughly half the attempts.
        assert_eq!(decayed_attempt(4, RECONNECT_STABLE_RESET / 2), 2);
    }

    #[test]
    fn nominal_backoff_doubles_then_caps() {
        let schedule: Vec<u64> = (1..=8).map(reconnect_nominal_secs).collect();
        assert_eq!(schedule, vec![1, 2, 4, 8, 16, 30, 30, 30]);
    }

    #[test]
    fn nominal_backoff_never_exceeds_cap_for_large_attempts() {
        assert_eq!(reconnect_nominal_secs(1_000), 30);
        assert_eq!(reconnect_nominal_secs(u32::MAX), 30);
    }

    #[test]
    fn equal_jitter_stays_within_the_nominal_window() {
        for nominal in 1..=30u64 {
            for _ in 0..200 {
                let millis = equal_jitter(nominal).as_millis() as u64;
                assert!(
                    millis >= nominal * 500 && millis <= nominal * 1000,
                    "delay {millis}ms outside [{}, {}] for nominal {nominal}s",
                    nominal * 500,
                    nominal * 1000
                );
            }
        }
    }
}
