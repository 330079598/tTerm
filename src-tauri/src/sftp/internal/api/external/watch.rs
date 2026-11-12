//! Watching the local copies for saves. Each copy is looked at once a
//! second rather than through file system events: programs save in many
//! ways (in place, or by writing another file and renaming it over the
//! copy), and a look at the file itself sees every one of them the same way.

use super::ExternalEditState;
use crate::core::blocking::run_blocking;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tterm_core::events::sftp::SftpEvent;
use tterm_core::events::{AppEvent, EventSink};
use tterm_core::sftp::internal::api::external::local::{FileStamp, Fingerprint};

const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// A copy to look at, as its session last saw it.
pub(super) struct Watched {
    pub id: String,
    pub local: PathBuf,
    pub observed: Option<FileStamp>,
    pub settling: Option<FileStamp>,
}

/// What one look at a copy found.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Look {
    /// No change since the last look.
    Same,
    /// The copy changed; it is read once it looks the same a second later,
    /// as a program may still be writing it.
    Settling(FileStamp),
    /// The copy changed and held still, and this is its content now.
    Settled(Fingerprint),
}

/// What a look at a copy whose stamp is now `current` goes on to do.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Step {
    Same,
    Settling(FileStamp),
    /// Read the content: the change held still since the last look.
    Read,
}

/// A copy that is not there counts as unchanged: a program that saves by
/// replacing the file removes it for a moment.
pub(super) fn next_step(
    observed: Option<FileStamp>,
    settling: Option<FileStamp>,
    current: Option<FileStamp>,
) -> Step {
    match current {
        None => Step::Same,
        Some(current) if Some(current) == observed => Step::Same,
        Some(current) if Some(current) == settling => Step::Read,
        Some(current) => Step::Settling(current),
    }
}

pub(super) fn look_at(watched: &Watched) -> Look {
    let current = tterm_core::sftp::internal::api::external::local::stamp(&watched.local);
    match next_step(watched.observed, watched.settling, current) {
        Step::Same => Look::Same,
        Step::Settling(stamp) => Look::Settling(stamp),
        Step::Read => {
            match tterm_core::sftp::internal::api::external::local::fingerprint(&watched.local) {
                Ok(fingerprint) => Look::Settled(fingerprint),
                Err(_) => Look::Same,
            }
        }
    }
}

/// Starts the watch unless it runs. It stops by itself once no file is open.
pub(super) fn ensure_running(state: Arc<ExternalEditState>, events: Arc<dyn EventSink>) {
    if state.watching.swap(true, Ordering::SeqCst) {
        return;
    }
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(POLL_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            let Some(watched) = state.watched() else {
                break;
            };
            let looks = run_blocking(move || {
                Ok(watched
                    .into_iter()
                    .map(|watched| {
                        let look = look_at(&watched);
                        (watched.id, look)
                    })
                    .collect::<Vec<_>>())
            })
            .await;
            let Ok(looks) = looks else {
                continue;
            };
            for edit in state.apply_looks(looks) {
                let _ = events.emit(AppEvent::Sftp(SftpEvent::ExternalEditChanged(edit)));
            }
        }
    });
}
