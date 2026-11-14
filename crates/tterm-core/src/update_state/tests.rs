//! Specification tests transcribed from `src/lib/updater.test.ts`.
//!
//! The Tauri `invoke`, `Channel`, `relaunch` and timers are replaced by a
//! scripted [`UpdateBackend`]; the state transitions and their coalescing are
//! the same cases as the TypeScript tests.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::Notify;

use super::{
    AppUpdateMetadata, UpdateBackend, UpdateChannel, UpdateCheckFrequency, UpdateDownloadEvent,
    UpdateErrorCode, UpdateState, UpdateStatus, Updater, next_check_delay,
};

#[derive(Clone)]
struct Gate {
    started: Arc<Notify>,
    release: Arc<Notify>,
}

impl Gate {
    fn new() -> Gate {
        Gate {
            started: Arc::new(Notify::new()),
            release: Arc::new(Notify::new()),
        }
    }

    fn start(&self) {
        self.started.notify_one();
    }

    fn release(&self) {
        self.release.notify_one();
    }

    async fn wait_started(&self) {
        self.started.notified().await;
    }

    async fn wait(&self) {
        self.release.notified().await;
    }
}

enum CheckStep {
    Immediate(Result<Option<AppUpdateMetadata>, String>),
    Gated {
        gate: Gate,
        result: Result<Option<AppUpdateMetadata>, String>,
    },
}

enum DownloadStep {
    Immediate(Result<bool, String>),
    Gated {
        gate: Gate,
        result: Result<bool, String>,
    },
}

struct FakeBackend {
    version: Option<String>,
    checks: Mutex<VecDeque<CheckStep>>,
    downloads: Mutex<VecDeque<DownloadStep>>,
    installs: Mutex<VecDeque<Result<bool, String>>>,
    calls: Mutex<Vec<String>>,
    download_events: Mutex<Vec<UpdateDownloadEvent>>,
}

impl FakeBackend {
    fn new() -> FakeBackend {
        FakeBackend {
            version: Some("1.0.0".to_owned()),
            checks: Mutex::new(VecDeque::new()),
            downloads: Mutex::new(VecDeque::new()),
            installs: Mutex::new(VecDeque::new()),
            calls: Mutex::new(Vec::new()),
            download_events: Mutex::new(Vec::new()),
        }
    }

    fn push_check(&self, step: CheckStep) {
        if let Ok(mut checks) = self.checks.lock() {
            checks.push_back(step);
        }
    }

    fn push_download(&self, step: DownloadStep) {
        if let Ok(mut downloads) = self.downloads.lock() {
            downloads.push_back(step);
        }
    }

    fn push_install(&self, result: Result<bool, String>) {
        if let Ok(mut installs) = self.installs.lock() {
            installs.push_back(result);
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls
            .lock()
            .map(|calls| calls.clone())
            .unwrap_or_default()
    }

    fn call_count(&self, prefix: &str) -> usize {
        self.calls()
            .iter()
            .filter(|call| call.starts_with(prefix))
            .count()
    }
}

#[async_trait::async_trait]
impl UpdateBackend for FakeBackend {
    async fn get_version(&self) -> Option<String> {
        self.version.clone()
    }

    async fn check_update(
        &self,
        channel: UpdateChannel,
    ) -> Result<Option<AppUpdateMetadata>, String> {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(format!("check:{}", channel.as_str()));
        }
        let step = self
            .checks
            .lock()
            .ok()
            .and_then(|mut checks| checks.pop_front());
        match step {
            Some(CheckStep::Gated { gate, result }) => {
                gate.start();
                gate.wait().await;
                result
            }
            Some(CheckStep::Immediate(result)) => result,
            None => Ok(None),
        }
    }

    async fn download_update(
        &self,
        channel: UpdateChannel,
        progress: &(dyn Fn(UpdateDownloadEvent) + Send + Sync),
    ) -> Result<bool, String> {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(format!("download:{}", channel.as_str()));
        }
        let step = self
            .downloads
            .lock()
            .ok()
            .and_then(|mut downloads| downloads.pop_front());
        let result = match step {
            Some(DownloadStep::Gated { gate, result }) => {
                gate.start();
                gate.wait().await;
                result
            }
            Some(DownloadStep::Immediate(result)) => result,
            None => Ok(true),
        };
        if result.is_ok()
            && let Ok(mut events) = self.download_events.lock()
        {
            for event in events.drain(..) {
                progress(event);
            }
        }
        result
    }

    async fn install_downloaded_update(&self, channel: UpdateChannel) -> Result<bool, String> {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(format!("install:{}", channel.as_str()));
        }
        self.installs
            .lock()
            .ok()
            .and_then(|mut installs| installs.pop_front())
            .unwrap_or(Ok(true))
    }
}

fn update(version: &str) -> AppUpdateMetadata {
    AppUpdateMetadata {
        version: version.to_owned(),
        current_version: "1.0.0".to_owned(),
        body: Some("Release notes".to_owned()),
        date: None,
    }
}

async fn wait_until(mut condition: impl FnMut() -> bool) {
    let reached = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if condition() {
                return true;
            }
            tokio::task::yield_now().await;
        }
    })
    .await;
    assert!(reached.unwrap_or(false), "the condition was not reached");
}

/// Keeps the subscription behind the returned handle so it lives as long as
/// the test that collects the statuses.
fn subscribed_statuses(
    updater: &Updater<FakeBackend>,
) -> (Arc<Mutex<Vec<UpdateStatus>>>, super::UpdateSubscription) {
    let statuses = Arc::new(Mutex::new(Vec::new()));
    let sink = statuses.clone();
    let subscription = updater.subscribe(Arc::new(move |state: &UpdateState| {
        if let Ok(mut statuses) = sink.lock() {
            statuses.push(state.status);
        }
    }));
    (statuses, subscription)
}

fn collected(statuses: &Arc<Mutex<Vec<UpdateStatus>>>) -> Vec<UpdateStatus> {
    statuses
        .lock()
        .map(|statuses| statuses.clone())
        .unwrap_or_default()
}

#[test]
fn computes_the_next_check_delay() {
    let day = 24 * 60 * 60 * 1000;
    assert_eq!(
        next_check_delay(10_000, Some(9_000), UpdateCheckFrequency::Never),
        None
    );
    assert_eq!(
        next_check_delay(10_000, None, UpdateCheckFrequency::Daily),
        Some(Duration::from_millis(super::STARTUP_UPDATE_DELAY_MS))
    );
    assert_eq!(
        next_check_delay(10_000, Some(10_000 - 1_000), UpdateCheckFrequency::Daily),
        Some(Duration::from_millis(day as u64 - 1_000))
    );
    assert_eq!(
        next_check_delay(10_000, Some(0), UpdateCheckFrequency::Weekly),
        Some(Duration::from_millis(super::STARTUP_UPDATE_DELAY_MS))
    );
}

#[tokio::test]
async fn shows_a_status_when_a_manual_request_joins_a_silent_check() {
    for (result, expected, can_retry) in [
        (Ok(None), UpdateStatus::NotAvailable, false),
        (
            Err("Network unavailable".to_owned()),
            UpdateStatus::Error,
            true,
        ),
    ] {
        let gate = Gate::new();
        let backend = FakeBackend::new();
        backend.push_check(CheckStep::Gated {
            gate: gate.clone(),
            result,
        });
        let updater = Arc::new(Updater::new(backend, "1.0.0"));

        let background = {
            let updater = updater.clone();
            tokio::spawn(async move { updater.check(UpdateChannel::Stable, true).await })
        };
        gate.wait_started().await;
        let manual = {
            let updater = updater.clone();
            tokio::spawn(async move { updater.check(UpdateChannel::Stable, false).await })
        };
        wait_until(|| {
            updater
                .lock()
                .check_flight
                .as_ref()
                .is_some_and(|flight| !flight.silent)
        })
        .await;
        gate.release();

        let (background, manual) = tokio::join!(background, manual);
        assert!(background.is_ok());
        assert!(manual.is_ok());
        assert_eq!(updater.state().status, expected);
        assert_eq!(updater.state().can_retry, can_retry);
        assert_eq!(updater.backend.call_count("check:"), 1);
    }
}

#[tokio::test]
async fn checks_the_requested_channel_after_a_check_of_another_channel_ends() {
    let gate = Gate::new();
    let backend = FakeBackend::new();
    backend.push_check(CheckStep::Gated {
        gate: gate.clone(),
        result: Ok(None),
    });
    backend.push_check(CheckStep::Immediate(Ok(Some(update("1.2.3")))));
    let updater = Arc::new(Updater::new(backend, "1.0.0"));

    let background = {
        let updater = updater.clone();
        tokio::spawn(async move { updater.check(UpdateChannel::Stable, true).await })
    };
    gate.wait_started().await;
    let manual = {
        let updater = updater.clone();
        tokio::spawn(async move { updater.check(UpdateChannel::BetaDev, false).await })
    };
    wait_until(|| {
        updater
            .lock()
            .check_flight
            .as_ref()
            .is_some_and(|flight| flight.channel == UpdateChannel::Stable)
    })
    .await;
    gate.release();

    let (background, manual) = tokio::join!(background, manual);
    assert!(background.is_ok());
    let manual = manual.expect("the manual check ran");
    assert_eq!(manual, Some(update("1.2.3")));
    assert_eq!(
        updater.backend.calls(),
        vec!["check:stable".to_owned(), "check:beta-dev".to_owned()]
    );
    let state = updater.state();
    assert_eq!(state.status, UpdateStatus::Available);
    assert_eq!(state.channel, UpdateChannel::BetaDev);
}

#[tokio::test]
async fn does_not_publish_the_downloaded_confirmation_state_for_download_and_install() {
    let updater = Updater::new(FakeBackend::new(), "1.0.0");
    updater
        .backend
        .push_check(CheckStep::Immediate(Ok(Some(update("1.2.3")))));
    updater.check(UpdateChannel::Stable, false).await;
    let (statuses, subscription) = subscribed_statuses(&updater);

    assert!(updater.download_and_install(UpdateChannel::Stable).await);

    assert_eq!(
        collected(&statuses),
        vec![
            UpdateStatus::Available,
            UpdateStatus::Downloading,
            UpdateStatus::Installing,
            UpdateStatus::Ready,
        ]
    );
    drop(subscription);
}

#[tokio::test]
async fn keeps_background_downloads_in_the_downloaded_state_until_the_user_installs() {
    let updater = Updater::new(FakeBackend::new(), "1.0.0");
    updater
        .backend
        .push_check(CheckStep::Immediate(Ok(Some(update("1.2.3")))));
    updater.check(UpdateChannel::Stable, false).await;
    let (statuses, subscription) = subscribed_statuses(&updater);

    assert!(updater.download(UpdateChannel::Stable).await);

    assert_eq!(
        collected(&statuses),
        vec![
            UpdateStatus::Available,
            UpdateStatus::Downloading,
            UpdateStatus::Downloaded,
        ]
    );
    assert_eq!(updater.backend.call_count("install:"), 0);
    drop(subscription);
}

#[tokio::test]
async fn restarts_the_progress_count_when_the_backend_retries_a_download() {
    let updater = Updater::new(FakeBackend::new(), "1.0.0");
    updater
        .backend
        .push_check(CheckStep::Immediate(Ok(Some(update("1.2.3")))));
    updater.check(UpdateChannel::Stable, false).await;
    if let Ok(mut events) = updater.backend.download_events.lock() {
        *events = vec![
            UpdateDownloadEvent::Started {
                content_length: Some(100),
            },
            UpdateDownloadEvent::Progress { chunk_length: 60 },
            UpdateDownloadEvent::Started {
                content_length: Some(100),
            },
            UpdateDownloadEvent::Progress { chunk_length: 100 },
            UpdateDownloadEvent::Finished,
        ];
    }

    assert!(updater.download(UpdateChannel::Stable).await);

    let state = updater.state();
    assert_eq!(state.status, UpdateStatus::Downloaded);
    assert_eq!(state.downloaded_bytes, 100);
    assert_eq!(state.total_bytes, Some(100));
}

#[tokio::test]
async fn coalesces_repeated_download_and_install_requests() {
    let updater = Updater::new(FakeBackend::new(), "1.0.0");
    updater
        .backend
        .push_check(CheckStep::Immediate(Ok(Some(update("1.2.3")))));
    updater.check(UpdateChannel::Stable, false).await;

    let (first, second) = tokio::join!(
        updater.download_and_install(UpdateChannel::Stable),
        updater.download_and_install(UpdateChannel::Stable)
    );

    assert!(first);
    assert!(second);
    assert_eq!(updater.backend.call_count("download:"), 1);
    assert_eq!(updater.backend.call_count("install:"), 1);
}

#[tokio::test]
async fn suppresses_the_downloaded_state_when_a_user_takes_over_a_background_download() {
    let gate = Gate::new();
    let backend = FakeBackend::new();
    backend.push_check(CheckStep::Immediate(Ok(Some(update("1.2.3")))));
    backend.push_download(DownloadStep::Gated {
        gate: gate.clone(),
        result: Ok(true),
    });
    let updater = Arc::new(Updater::new(backend, "1.0.0"));
    updater.check(UpdateChannel::Stable, false).await;
    let (statuses, subscription) = subscribed_statuses(&updater);

    let background = {
        let updater = updater.clone();
        tokio::spawn(async move { updater.download(UpdateChannel::Stable).await })
    };
    gate.wait_started().await;
    let user_install = {
        let updater = updater.clone();
        tokio::spawn(async move { updater.download_and_install(UpdateChannel::Stable).await })
    };
    wait_until(|| updater.lock().auto_install_requests > 0).await;
    gate.release();

    let (background, user_install) = tokio::join!(background, user_install);
    assert!(background.unwrap_or(false));
    assert!(user_install.unwrap_or(false));
    assert_eq!(
        collected(&statuses),
        vec![
            UpdateStatus::Available,
            UpdateStatus::Downloading,
            UpdateStatus::Installing,
            UpdateStatus::Ready,
        ]
    );
    drop(subscription);
}

#[tokio::test]
async fn retries_installation_intent_after_a_taken_over_background_download_fails() {
    let gate = Gate::new();
    let backend = FakeBackend::new();
    backend.push_check(CheckStep::Immediate(Ok(Some(update("1.2.3")))));
    backend.push_download(DownloadStep::Gated {
        gate: gate.clone(),
        result: Err("network unavailable".to_owned()),
    });
    let updater = Arc::new(Updater::new(backend, "1.0.0"));
    updater.check(UpdateChannel::Stable, false).await;

    let background = {
        let updater = updater.clone();
        tokio::spawn(async move { updater.download(UpdateChannel::Stable).await })
    };
    gate.wait_started().await;
    let user_install = {
        let updater = updater.clone();
        tokio::spawn(async move { updater.download_and_install(UpdateChannel::Stable).await })
    };
    wait_until(|| updater.lock().auto_install_requests > 0).await;
    gate.release();

    let (background, user_install) = tokio::join!(background, user_install);
    assert!(!background.unwrap_or(true));
    assert!(!user_install.unwrap_or(true));
    assert_eq!(updater.state().status, UpdateStatus::Error);

    updater
        .backend
        .push_download(DownloadStep::Immediate(Ok(true)));
    updater.backend.push_install(Ok(true));
    assert!(updater.retry().await);
    assert_eq!(updater.state().status, UpdateStatus::Ready);
}

#[tokio::test]
async fn exposes_a_retry_action_after_an_install_failure() {
    let updater = Updater::new(FakeBackend::new(), "1.0.0");
    updater
        .backend
        .push_check(CheckStep::Immediate(Ok(Some(update("1.2.3")))));
    updater.check(UpdateChannel::Stable, false).await;
    updater.download(UpdateChannel::Stable).await;
    updater
        .backend
        .push_install(Err("installer unavailable".to_owned()));

    assert!(!updater.install(UpdateChannel::Stable).await);
    let state = updater.state();
    assert_eq!(state.status, UpdateStatus::Error);
    assert_eq!(state.error.as_deref(), Some("installer unavailable"));
    assert!(state.can_retry);
}

#[tokio::test]
async fn downloads_again_after_the_backend_discards_an_unavailable_cached_update() {
    let updater = Updater::new(FakeBackend::new(), "1.0.0");
    updater
        .backend
        .push_check(CheckStep::Immediate(Ok(Some(update("1.2.3")))));
    updater.check(UpdateChannel::Stable, false).await;
    updater.download(UpdateChannel::Stable).await;
    updater.backend.push_install(Ok(false));

    assert!(!updater.install(UpdateChannel::Stable).await);
    let state = updater.state();
    assert_eq!(state.status, UpdateStatus::Error);
    assert_eq!(
        state.error_code,
        Some(UpdateErrorCode::DownloadedUpdateUnavailable)
    );
    assert!(state.can_retry);

    updater
        .backend
        .push_check(CheckStep::Immediate(Ok(Some(update("1.2.3")))));
    updater
        .backend
        .push_download(DownloadStep::Immediate(Ok(true)));
    assert!(updater.retry().await);
    assert!(updater.download(UpdateChannel::Stable).await);
    assert_eq!(updater.backend.call_count("download:"), 2);
}

#[tokio::test]
async fn does_not_reuse_update_metadata_from_a_different_channel() {
    let backend = FakeBackend::new();
    backend.push_check(CheckStep::Immediate(Ok(Some(update("1.2.3")))));
    backend.push_check(CheckStep::Immediate(Ok(Some(update("2.0.0-beta.1")))));
    let updater = Updater::new(backend, "1.0.0");
    updater.check(UpdateChannel::Stable, false).await;

    assert!(updater.download(UpdateChannel::BetaDev).await);

    assert_eq!(
        updater.backend.calls(),
        vec![
            "check:stable".to_owned(),
            "check:beta-dev".to_owned(),
            "download:beta-dev".to_owned(),
        ]
    );
    let state = updater.state();
    assert_eq!(state.channel, UpdateChannel::BetaDev);
    assert_eq!(state.latest_version.as_deref(), Some("2.0.0-beta.1"));
    assert_eq!(state.status, UpdateStatus::Downloaded);
}
