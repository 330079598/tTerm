//! A bounded mailbox of shell prompts, directories and agent reports.
//!
//! The parser thread sees every output chunk, including the prompt a shell
//! prints immediately after it starts. An `AgentResume` controller attaches
//! after the session exists, so the mailbox keeps a short backlog and replays
//! it on attach instead of losing the first prompt in that window.
//!
//! A full queue or a full backlog loses reports. The prompt signal and a loss
//! flag are updated on the parser thread instead of through the queue, so a
//! lost command-start mark can never make the controller act on an old
//! prompt.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

use tokio::sync::mpsc;
use tterm_terminal::session_reports::{SessionReport, SessionReportScanner};

/// The most reports waiting for the UI thread.
const QUEUE_CAPACITY: usize = 256;
/// The most reports kept before a controller attaches.
const BACKLOG_CAPACITY: usize = 64;

/// Shell reports for one session generation.
pub(crate) struct Reports {
    inner: Mutex<Inner>,
    at_prompt: Arc<AtomicBool>,
    lost: Arc<AtomicBool>,
}

struct Inner {
    scanner: SessionReportScanner,
    sender: Option<mpsc::Sender<SessionReport>>,
    backlog: VecDeque<SessionReport>,
}

impl Reports {
    /// Creates an empty mailbox.
    pub(crate) fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                scanner: SessionReportScanner::new(),
                sender: None,
                backlog: VecDeque::new(),
            }),
            at_prompt: Arc::new(AtomicBool::new(false)),
            lost: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Scans one applied output chunk and delivers every report it contains.
    pub(crate) fn observe(&self, bytes: &[u8]) {
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        for report in inner.scanner.scan(bytes) {
            match report {
                SessionReport::Prompt => self.at_prompt.store(true, Ordering::Release),
                SessionReport::CommandStarted => self.at_prompt.store(false, Ordering::Release),
                SessionReport::AgentReport(_) | SessionReport::Cwd(_) => {}
            }
            inner.publish(report, &self.lost);
        }
    }

    /// Takes the backlog and starts delivering live reports to the returned
    /// receiver, together with the signals the controller reads directly.
    ///
    /// Calling this again replaces the previous receiver. A backlog that
    /// overflowed before this call stays marked as lost.
    pub(crate) fn attach(
        &self,
    ) -> (
        Vec<SessionReport>,
        mpsc::Receiver<SessionReport>,
        ReportsSignal,
    ) {
        let (sender, receiver) = mpsc::channel(QUEUE_CAPACITY);
        let mut inner = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let backlog = inner.backlog.drain(..).collect();
        inner.sender = Some(sender);
        let signal = ReportsSignal {
            at_prompt: Arc::clone(&self.at_prompt),
            lost: Arc::clone(&self.lost),
        };
        (backlog, receiver, signal)
    }
}

impl Inner {
    fn publish(&mut self, report: SessionReport, lost: &AtomicBool) {
        let Some(sender) = &self.sender else {
            if self.backlog.len() == BACKLOG_CAPACITY {
                self.backlog.pop_front();
                lost.store(true, Ordering::Release);
            }
            self.backlog.push_back(report);
            return;
        };
        match sender.try_send(report) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(_)) => {
                lost.store(true, Ordering::Release);
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                self.sender = None;
            }
        }
    }
}

/// The report state a controller reads without waiting for the queue.
///
/// The parser updates both flags synchronously, so a dropped report cannot
/// leave the prompt state stale.
#[derive(Clone)]
pub(crate) struct ReportsSignal {
    at_prompt: Arc<AtomicBool>,
    lost: Arc<AtomicBool>,
}

impl ReportsSignal {
    /// Whether the newest prompt or command-start mark says the shell waits
    /// for input.
    pub(crate) fn at_prompt(&self) -> bool {
        self.at_prompt.load(Ordering::Acquire)
    }

    /// Whether this generation lost at least one report.
    pub(crate) fn lost(&self) -> bool {
        self.lost.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::{Reports, ReportsSignal};
    use tterm_terminal::session_reports::SessionReport;

    #[test]
    fn a_report_seen_before_attach_is_replayed() {
        let reports = Reports::new();
        reports.observe(b"\x1b]133;A\x07");
        let (backlog, _receiver, signal) = reports.attach();
        assert_eq!(backlog, vec![SessionReport::Prompt]);
        assert!(signal.at_prompt());
        assert!(!signal.lost());
    }

    #[test]
    fn live_reports_arrive_in_order_and_survive_chunk_splits() {
        let reports = Reports::new();
        let (backlog, mut receiver, _signal) = reports.attach();
        assert!(backlog.is_empty());
        reports.observe(b"\x1b]7;file:///work\x07\x1b]133;");
        reports.observe(b"A\x07");
        assert_eq!(
            receiver.try_recv().expect("cwd"),
            SessionReport::Cwd("/work".to_string())
        );
        assert_eq!(receiver.try_recv().expect("prompt"), SessionReport::Prompt);
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn a_dropped_command_start_still_clears_the_live_prompt_signal() {
        let reports = Reports::new();
        let (_backlog, _receiver, signal) = reports.attach();
        let mut directory = Vec::new();
        for _ in 0..super::QUEUE_CAPACITY {
            directory.extend_from_slice(b"\x1b]7;file:///work\x07");
        }
        reports.observe(&directory);
        reports.observe(b"\x1b]133;A\x07");
        reports.observe(b"\x1b]133;C\x07");
        assert!(signal.lost());
        assert!(
            !signal.at_prompt(),
            "the dropped command start must still stop a stale send"
        );
    }

    #[test]
    fn an_overfull_backlog_is_marked_lost_before_attach() {
        let reports = Reports::new();
        let mut directory = Vec::new();
        for _ in 0..=super::BACKLOG_CAPACITY {
            directory.extend_from_slice(b"\x1b]7;file:///work\x07");
        }
        reports.observe(&directory);
        let (backlog, _receiver, signal) = reports.attach();
        assert_eq!(backlog.len(), super::BACKLOG_CAPACITY);
        assert!(signal.lost());
    }

    #[test]
    fn the_signal_is_shared_with_every_attach() {
        let reports = Reports::new();
        let (_backlog, _receiver, first) = reports.attach();
        let (_backlog, _receiver, second) = reports.attach();
        assert!(same_signal(&first, &second));
        reports.observe(b"\x1b]133;A\x07");
        assert!(first.at_prompt());
        assert!(second.at_prompt());
    }

    fn same_signal(first: &ReportsSignal, second: &ReportsSignal) -> bool {
        std::sync::Arc::ptr_eq(&first.at_prompt, &second.at_prompt)
    }
}
