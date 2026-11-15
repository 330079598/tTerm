//! Stream-order guard for stale mouse reporting.
//!
//! A prompt mark closes reporting at the exact byte where the mark ends. The
//! parser applies the output in pieces split at those marks, asks the real
//! model whether reporting is on at each piece, and writes the exact OFF
//! sequence before continuing. A program that turns reporting on again later
//! in the same chunk therefore keeps it on.
//!
//! The batch gate and the foreground answer share one mutex. The parser
//! advances the output epoch at the start of every batch; a foreground answer
//! is applied only while the epoch, session generation and live flag still
//! match. Neither path holds the gate across an await.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use tokio::runtime::Handle;
use tterm_core::core::state::PtyMap;
use tterm_core::terminal::foreground::{
    PtyShellInForegroundInput, pty_shell_in_foreground_service,
};
use tterm_terminal::mouse_reporting::{MOUSE_REPORTING_OFF, MousePromptScanner};

use super::SessionState;

/// The frontend's `SETTLE_DELAY_MS`: output this quiet means a program may
/// have exited and left the shell's prompt on screen.
pub(super) const SETTLE_DELAY: Duration = Duration::from_millis(200);

/// Where the foreground check gets real process data.
#[derive(Clone)]
pub(crate) enum ForegroundProbe {
    /// A local PTY; the core service compares the foreground process group
    /// with the shell and checks that the leader still runs a shell.
    Local {
        ptys: PtyMap,
        tab_id: String,
        session_nonce: u32,
    },
}

impl ForegroundProbe {
    async fn in_foreground(&self) -> bool {
        match self {
            Self::Local {
                ptys,
                tab_id,
                session_nonce,
            } => pty_shell_in_foreground_service(
                Arc::clone(ptys),
                PtyShellInForegroundInput {
                    tab_id: tab_id.clone(),
                    session_nonce: *session_nonce,
                },
            )
            .await
            .unwrap_or(false),
        }
    }
}

/// An output epoch captured when a settled batch asked for a foreground check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CheckToken {
    epoch: u64,
}

struct Gate {
    scanner: MousePromptScanner,
    epoch: u64,
    deadline: Option<Instant>,
}

/// Per-session mouse-reporting guard.
pub(crate) struct MouseGuard {
    state: SessionState,
    session_nonce: u32,
    runtime: Handle,
    foreground: Option<ForegroundProbe>,
    gate: Mutex<Gate>,
}

impl MouseGuard {
    /// Creates a guard for one session generation.
    ///
    /// `foreground` is `None` for SSH, serial and other sessions whose
    /// foreground process cannot be inspected. Those still close reporting on
    /// a prompt mark, but never schedule a host process check.
    pub(crate) fn new(
        state: SessionState,
        runtime: Handle,
        foreground: Option<ForegroundProbe>,
    ) -> Self {
        let session_nonce = state.session_nonce;
        Self {
            state,
            session_nonce,
            runtime,
            foreground,
            gate: Mutex::new(Gate {
                scanner: MousePromptScanner::default(),
                epoch: 0,
                deadline: None,
            }),
        }
    }

    /// Applies one parser batch under the gate at the current time.
    pub(crate) fn apply_batch(&self, bytes: &[u8]) -> bool {
        self.apply_batch_at(bytes, Instant::now())
    }

    /// The time the parser should wait before asking for a foreground check.
    pub(crate) fn timeout_at(&self, now: Instant) -> Option<Duration> {
        let gate = self.lock();
        gate.deadline
            .map(|deadline| deadline.saturating_duration_since(now))
    }

    /// Starts the foreground check for a settled output stream.
    pub(crate) fn settle(self: &Arc<Self>) {
        let Some(token) = self.begin_check_at(Instant::now()) else {
            return;
        };
        let Some(probe) = self.foreground.clone() else {
            return;
        };
        let guard = Arc::clone(self);
        self.runtime.spawn(async move {
            let in_foreground = probe.in_foreground().await;
            guard.resolve(token, in_foreground);
        });
    }

    fn apply_batch_at(&self, bytes: &[u8], now: Instant) -> bool {
        let mut gate = self.lock();
        if !self.is_live() {
            return false;
        }
        gate.epoch = gate.epoch.saturating_add(1);
        let offsets = gate.scanner.scan(bytes);
        let mut start = 0;
        for end in offsets {
            if end < start || end > bytes.len() {
                continue;
            }
            // An offset at the start of the batch is a prompt completed by an
            // ESC from the previous batch; the OFF write still belongs before
            // these bytes.
            self.state.terminal.write(&bytes[start..end]);
            if self.state.terminal.is_mouse_reporting() {
                self.state.terminal.write(MOUSE_REPORTING_OFF);
            }
            start = end;
        }
        if start < bytes.len() {
            self.state.terminal.write(&bytes[start..]);
        }
        gate.deadline = if self.foreground.is_some() && self.state.terminal.is_mouse_reporting() {
            Some(now + SETTLE_DELAY)
        } else {
            None
        };
        true
    }

    fn begin_check_at(&self, now: Instant) -> Option<CheckToken> {
        let mut gate = self.lock();
        let deadline = gate.deadline?;
        if now < deadline {
            return None;
        }
        gate.deadline = None;
        if !self.is_live() || !self.state.terminal.is_mouse_reporting() {
            return None;
        }
        Some(CheckToken { epoch: gate.epoch })
    }

    /// Applies a foreground answer only while the gate still describes this
    /// output and this session generation.
    fn resolve(&self, token: CheckToken, in_foreground: bool) {
        if !in_foreground {
            return;
        }
        let gate = self.lock();
        if gate.epoch != token.epoch || !self.is_live() {
            return;
        }
        if self.state.terminal.clear_mouse_reporting() {
            self.state.publish();
        }
    }

    fn is_live(&self) -> bool {
        self.state.session_nonce == self.session_nonce && self.state.alive.load(Ordering::Acquire)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Gate> {
        self.gate.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests;
