//! The updater's state machine, from `src/lib/updater.ts`.
//!
//! Checking, downloading, installing, coalescing requests and retrying are
//! decided here; the platform's IPC, channel, relaunch and timers stay with
//! the clients behind [`UpdateBackend`].

mod transfer;

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

/// How long after startup a due check waits, so the window opens first.
pub const STARTUP_UPDATE_DELAY_MS: u64 = 6_000;

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/// Which release channel the updater follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateChannel {
    /// The stable releases.
    Stable,
    /// Beta builds.
    BetaDev,
}

impl UpdateChannel {
    /// The name the frontend and the backend use.
    pub fn as_str(self) -> &'static str {
        match self {
            UpdateChannel::Stable => "stable",
            UpdateChannel::BetaDev => "beta-dev",
        }
    }

    /// The channel a stored name names, or `None` for anything unknown.
    pub fn parse(value: &str) -> Option<UpdateChannel> {
        match value {
            "stable" => Some(UpdateChannel::Stable),
            "beta-dev" => Some(UpdateChannel::BetaDev),
            _ => None,
        }
    }
}

/// How often the updater checks in the background.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateCheckFrequency {
    /// Once a day.
    Daily,
    /// Every three days.
    Every3Days,
    /// Once a week.
    Weekly,
    /// Only when the user asks.
    Never,
}

impl UpdateCheckFrequency {
    /// The check interval, or `None` when checks are off.
    pub fn interval_ms(self) -> Option<i64> {
        match self {
            UpdateCheckFrequency::Daily => Some(DAY_MS),
            UpdateCheckFrequency::Every3Days => Some(3 * DAY_MS),
            UpdateCheckFrequency::Weekly => Some(7 * DAY_MS),
            UpdateCheckFrequency::Never => None,
        }
    }
}

/// How long to wait before the next background check.
///
/// A check that is already due waits for the window to open; a later one
/// waits out the rest of its interval. `None` means checks are off.
pub fn next_check_delay(
    now_ms: i64,
    last_checked_ms: Option<i64>,
    frequency: UpdateCheckFrequency,
) -> Option<Duration> {
    let interval = frequency.interval_ms()?;
    let elapsed = last_checked_ms
        .filter(|last| *last != 0)
        .map_or(i64::MAX, |last| (now_ms - last).max(0));
    let delay = if elapsed >= interval {
        STARTUP_UPDATE_DELAY_MS as i64
    } else {
        interval - elapsed
    };
    Some(Duration::from_millis(delay as u64))
}

/// What the updater is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateStatus {
    /// Nothing checked yet, or a silent check changed nothing.
    Idle,
    /// A check is in flight.
    Checking,
    /// A newer release exists.
    Available,
    /// This is the newest release.
    NotAvailable,
    /// The downloaded release is on its way.
    Downloading,
    /// The release is downloaded and waits for the user.
    Downloaded,
    /// The installer runs.
    Installing,
    /// The new version is installed and needs a relaunch.
    Ready,
    /// The last operation failed.
    Error,
}

/// A recoverable failure the frontend shows differently from a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateErrorCode {
    /// The installer no longer had the downloaded release; download it again.
    DownloadedUpdateUnavailable,
}

/// The state the updater shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateState {
    /// What the updater is doing.
    pub status: UpdateStatus,
    /// The channel followed.
    pub channel: UpdateChannel,
    /// The running version.
    pub current_version: String,
    /// The release a check found.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latest_version: Option<String>,
    /// The release's notes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// An error to show.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// A recoverable failure to handle specially.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<UpdateErrorCode>,
    /// How many bytes of the release are downloaded.
    pub downloaded_bytes: u64,
    /// The release's size when the backend reported it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<u64>,
    /// Whether a retry can follow the last failure.
    pub can_retry: bool,
}

/// The release a check found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateMetadata {
    /// The release's version.
    pub version: String,
    /// The version it replaces.
    pub current_version: String,
    /// The release notes.
    pub body: Option<String>,
    /// The release date.
    pub date: Option<String>,
}

/// One download progress report, as the backend sends it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateDownloadEvent {
    /// The download starts over from zero bytes.
    Started {
        /// The release's size, when known.
        content_length: Option<u64>,
    },
    /// More bytes arrived.
    Progress {
        /// The number of bytes in this chunk.
        chunk_length: u64,
    },
    /// The download finished.
    Finished,
}

/// What the updater needs from the client's platform layer.
#[async_trait]
pub trait UpdateBackend: Send + Sync {
    /// The running app's version, or `None` when the platform cannot tell.
    async fn get_version(&self) -> Option<String>;
    /// Checks `channel` for a newer release.
    async fn check_update(
        &self,
        channel: UpdateChannel,
    ) -> Result<Option<AppUpdateMetadata>, String>;
    /// Downloads the checked release, reporting progress.
    async fn download_update(
        &self,
        channel: UpdateChannel,
        progress: &(dyn Fn(UpdateDownloadEvent) + Send + Sync),
    ) -> Result<bool, String>;
    /// Installs the downloaded release; `false` means it is no longer cached.
    async fn install_downloaded_update(&self, channel: UpdateChannel) -> Result<bool, String>;
}

/// The operation a retry repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailedAction {
    Check,
    Download,
    DownloadAndInstall,
    Install,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FailedOperation {
    action: FailedAction,
    channel: UpdateChannel,
}

struct CheckFlight {
    channel: UpdateChannel,
    silent: bool,
    done: watch::Sender<Option<Option<AppUpdateMetadata>>>,
    wait: watch::Receiver<Option<Option<AppUpdateMetadata>>>,
}

struct DownloadFlight {
    done: watch::Sender<Option<bool>>,
    wait: watch::Receiver<Option<bool>>,
}

struct InstallFlight {
    done: watch::Sender<Option<bool>>,
    wait: watch::Receiver<Option<bool>>,
}

struct Shared {
    state: UpdateState,
    pending_update: Option<AppUpdateMetadata>,
    pending_update_channel: Option<UpdateChannel>,
    downloaded_update_channel: Option<UpdateChannel>,
    has_installed_update: bool,
    auto_install_requests: usize,
    failed_operation: Option<FailedOperation>,
    check_flight: Option<CheckFlight>,
    download_flight: Option<DownloadFlight>,
    install_flight: Option<InstallFlight>,
    download_in_flight: bool,
    install_in_flight: bool,
}

type StateListener = Arc<dyn Fn(&UpdateState) + Send + Sync>;

#[derive(Default)]
struct Listeners {
    next_id: u64,
    entries: Vec<(u64, StateListener)>,
}

impl Listeners {
    fn notify(&self, state: &UpdateState) {
        for (_, listener) in &self.entries {
            listener(state);
        }
    }
}

/// A listener registration; dropping it stops the notifications.
pub struct UpdateSubscription {
    listeners: Arc<Mutex<Listeners>>,
    id: u64,
}

impl Drop for UpdateSubscription {
    fn drop(&mut self) {
        let mut listeners = self
            .listeners
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        listeners.entries.retain(|(id, _)| *id != self.id);
    }
}

/// Drives one updater state for a client.
pub struct Updater<B: UpdateBackend> {
    backend: B,
    shared: Arc<Mutex<Shared>>,
    listeners: Arc<Mutex<Listeners>>,
}

impl<B: UpdateBackend> Updater<B> {
    /// Creates the updater for an app that currently runs `current_version`.
    pub fn new(backend: B, current_version: &str) -> Updater<B> {
        Updater {
            backend,
            shared: Arc::new(Mutex::new(Shared {
                state: UpdateState {
                    status: UpdateStatus::Idle,
                    channel: UpdateChannel::Stable,
                    current_version: current_version.to_owned(),
                    latest_version: None,
                    notes: None,
                    error: None,
                    error_code: None,
                    downloaded_bytes: 0,
                    total_bytes: None,
                    can_retry: false,
                },
                pending_update: None,
                pending_update_channel: None,
                downloaded_update_channel: None,
                has_installed_update: false,
                auto_install_requests: 0,
                failed_operation: None,
                check_flight: None,
                download_flight: None,
                install_flight: None,
                download_in_flight: false,
                install_in_flight: false,
            })),
            listeners: Arc::new(Mutex::new(Listeners::default())),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The current state, for a client's first render.
    pub fn state(&self) -> UpdateState {
        self.lock().state.clone()
    }

    /// Calls `listener` on every change and once with the current state.
    pub fn subscribe(&self, listener: StateListener) -> UpdateSubscription {
        let (id, state) = {
            let mut listeners = self
                .listeners
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            listeners.next_id += 1;
            let id = listeners.next_id;
            listeners.entries.push((id, listener.clone()));
            (id, self.state())
        };
        listener(&state);
        UpdateSubscription {
            listeners: self.listeners.clone(),
            id,
        }
    }

    fn publish<F: FnOnce(&mut UpdateState)>(&self, change: F) {
        let state = {
            let mut shared = self.lock();
            change(&mut shared.state);
            shared.state.clone()
        };
        self.listeners
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .notify(&state);
    }

    /// Checks `channel` for an update.
    ///
    /// A manual request that joins a silent check upgrades it, and a check of
    /// another channel waits for the one in flight and then runs.
    pub async fn check(&self, channel: UpdateChannel, silent: bool) -> Option<AppUpdateMetadata> {
        loop {
            enum Entry {
                Join(watch::Receiver<Option<Option<AppUpdateMetadata>>>),
                WaitThenRetry(watch::Receiver<Option<Option<AppUpdateMetadata>>>),
                Run,
            }

            let entry = {
                let mut shared = self.lock();
                if shared.has_installed_update
                    || shared.download_in_flight
                    || shared.install_in_flight
                {
                    return shared.pending_update.clone();
                }
                match shared.check_flight.as_mut() {
                    Some(flight) if flight.channel == channel => {
                        // A manual request must see a result even when it
                        // joins a background check.
                        flight.silent = flight.silent && silent;
                        Entry::Join(flight.wait.clone())
                    }
                    Some(flight) => Entry::WaitThenRetry(flight.wait.clone()),
                    None => {
                        let (done, wait) = watch::channel(None);
                        shared.check_flight = Some(CheckFlight {
                            channel,
                            silent,
                            done,
                            wait: wait.clone(),
                        });
                        Entry::Run
                    }
                }
            };

            match entry {
                Entry::Join(mut wait) => {
                    return watch_value(&mut wait).await.flatten();
                }
                Entry::WaitThenRetry(mut wait) => {
                    let _ = watch_value(&mut wait).await;
                }
                Entry::Run => return self.run_check(channel).await,
            }
        }
    }

    async fn run_check(&self, channel: UpdateChannel) -> Option<AppUpdateMetadata> {
        let version = self.backend.get_version().await;
        self.publish(|state| {
            if let Some(version) = version {
                state.current_version = version;
            }
            state.status = UpdateStatus::Checking;
            state.channel = channel;
            state.error = None;
            state.error_code = None;
            state.downloaded_bytes = 0;
            state.total_bytes = None;
            state.can_retry = false;
        });

        let result = self.backend.check_update(channel).await;
        let silent = self
            .lock()
            .check_flight
            .as_ref()
            .is_none_or(|flight| flight.silent);
        let update = match &result {
            Ok(update) => update.clone(),
            Err(_) => None,
        };

        {
            let mut shared = self.lock();
            shared.pending_update = update.clone();
            shared.pending_update_channel = update.as_ref().map(|_| channel);
            shared.failed_operation = match &result {
                Ok(_) => None,
                Err(_) if silent => None,
                Err(_) => Some(FailedOperation {
                    action: FailedAction::Check,
                    channel,
                }),
            };
        }

        match result {
            Ok(Some(found)) => self.publish(|state| {
                state.status = UpdateStatus::Available;
                state.latest_version = Some(found.version.clone());
                state.notes = found.body.clone();
                state.downloaded_bytes = 0;
                state.total_bytes = None;
            }),
            Ok(None) => self.publish(|state| {
                state.status = if silent {
                    UpdateStatus::Idle
                } else {
                    UpdateStatus::NotAvailable
                };
                state.latest_version = None;
                state.notes = None;
            }),
            Err(error) => self.publish(|state| {
                state.status = if silent {
                    UpdateStatus::Idle
                } else {
                    UpdateStatus::Error
                };
                state.error = Some(error);
                state.can_retry = !silent;
            }),
        }

        self.finish_check(update.clone());
        update
    }

    fn finish_check(&self, update: Option<AppUpdateMetadata>) {
        let flight = self.lock().check_flight.take();
        if let Some(flight) = flight {
            let _ = flight.done.send(Some(update));
        }
    }
}
/// Waits for a flight's result, or `None` when its sender went away.
async fn watch_value<T: Clone>(receiver: &mut watch::Receiver<Option<T>>) -> Option<T> {
    loop {
        let current = receiver.borrow_and_update().clone();
        if current.is_some() {
            return current;
        }
        if receiver.changed().await.is_err() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests;
