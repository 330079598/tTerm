//! Downloading, installing and retrying updates; the state and the check
//! live in the parent module.

use std::sync::Mutex;

use tokio::sync::watch;

use super::{
    DownloadFlight, FailedAction, FailedOperation, InstallFlight, UpdateBackend, UpdateChannel,
    UpdateDownloadEvent, UpdateErrorCode, UpdateStatus, Updater, watch_value,
};

impl<B: UpdateBackend> Updater<B> {
    /// Downloads the checked release so the user can install it.
    pub async fn download(&self, channel: UpdateChannel) -> bool {
        self.run_download(channel, true).await
    }

    async fn run_download(&self, channel: UpdateChannel, publish_downloaded: bool) -> bool {
        enum Entry {
            Join(watch::Receiver<Option<bool>>),
            Run,
        }

        let entry = {
            let mut shared = self.lock();
            if shared.downloaded_update_channel == Some(channel) || shared.has_installed_update {
                let installed = shared.has_installed_update;
                drop(shared);
                if installed {
                    self.publish(|state| state.status = UpdateStatus::Ready);
                } else if publish_downloaded {
                    self.publish(|state| state.status = UpdateStatus::Downloaded);
                }
                return true;
            }
            match shared.download_flight.as_ref() {
                Some(flight) => Entry::Join(flight.wait.clone()),
                None => {
                    let (done, wait) = watch::channel(None);
                    shared.download_flight = Some(DownloadFlight {
                        done,
                        wait: wait.clone(),
                    });
                    Entry::Run
                }
            }
        };

        match entry {
            Entry::Join(mut wait) => watch_value(&mut wait).await.unwrap_or(false),
            Entry::Run => self.run_download_owner(channel, publish_downloaded).await,
        }
    }

    async fn run_download_owner(&self, channel: UpdateChannel, publish_downloaded: bool) -> bool {
        let pending = {
            let shared = self.lock();
            if shared.pending_update_channel == Some(channel) {
                Some(shared.pending_update.clone())
            } else {
                None
            }
        };
        let update = match pending {
            Some(update) => update,
            None => self.check(channel, true).await,
        };
        let Some(update) = update else {
            self.finish_download(false);
            return false;
        };

        self.publish(|state| {
            state.status = UpdateStatus::Downloading;
            state.channel = channel;
            state.latest_version = Some(update.version.clone());
            state.error = None;
            state.error_code = None;
            state.downloaded_bytes = 0;
            state.total_bytes = None;
            state.can_retry = false;
        });
        self.lock().download_in_flight = true;

        let progress = Mutex::new((0_u64, None::<u64>));
        let on_event = |event: UpdateDownloadEvent| match event {
            UpdateDownloadEvent::Started { content_length } => {
                if let Ok(mut progress) = progress.lock() {
                    *progress = (0, content_length);
                }
                self.publish(|state| {
                    state.downloaded_bytes = 0;
                    state.total_bytes = content_length;
                });
            }
            UpdateDownloadEvent::Progress { chunk_length } => {
                let values = match progress.lock() {
                    Ok(mut progress) => {
                        progress.0 += chunk_length;
                        *progress
                    }
                    Err(poisoned) => *poisoned.into_inner(),
                };
                self.publish(|state| {
                    state.downloaded_bytes = values.0;
                    state.total_bytes = values.1;
                });
            }
            UpdateDownloadEvent::Finished => {
                let values = match progress.lock() {
                    Ok(progress) => *progress,
                    Err(poisoned) => *poisoned.into_inner(),
                };
                self.publish(|state| {
                    state.downloaded_bytes = values.0;
                    state.total_bytes = values.1;
                });
            }
        };

        let result = self.backend.download_update(channel, &on_event).await;
        let (downloaded_bytes, total_bytes) = match progress.lock() {
            Ok(progress) => *progress,
            Err(poisoned) => *poisoned.into_inner(),
        };

        let success = match result {
            Ok(true) => {
                let suppressed = self.lock().auto_install_requests > 0;
                {
                    let mut shared = self.lock();
                    shared.downloaded_update_channel = Some(channel);
                    shared.download_in_flight = false;
                }
                if publish_downloaded && !suppressed {
                    self.publish(|state| {
                        state.status = UpdateStatus::Downloaded;
                        state.downloaded_bytes = downloaded_bytes;
                        state.total_bytes = total_bytes;
                    });
                }
                true
            }
            Ok(false) => {
                self.lock().download_in_flight = false;
                self.publish(|state| {
                    state.status = UpdateStatus::NotAvailable;
                    state.latest_version = None;
                });
                false
            }
            Err(error) => {
                let suppressed = self.lock().auto_install_requests > 0;
                {
                    let mut shared = self.lock();
                    shared.download_in_flight = false;
                    shared.failed_operation = Some(FailedOperation {
                        action: if publish_downloaded && !suppressed {
                            FailedAction::Download
                        } else {
                            FailedAction::DownloadAndInstall
                        },
                        channel,
                    });
                }
                self.publish(|state| {
                    state.status = UpdateStatus::Error;
                    state.error = Some(error);
                    state.can_retry = true;
                });
                false
            }
        };

        self.finish_download(success);
        success
    }

    fn finish_download(&self, downloaded: bool) {
        let flight = self.lock().download_flight.take();
        if let Some(flight) = flight {
            let _ = flight.done.send(Some(downloaded));
        }
    }

    /// Installs the downloaded release.
    pub async fn install(&self, channel: UpdateChannel) -> bool {
        enum Entry {
            AlreadyInstalled,
            Join(watch::Receiver<Option<bool>>),
            Run,
        }

        let entry = {
            let mut shared = self.lock();
            if shared.has_installed_update {
                Entry::AlreadyInstalled
            } else if let Some(flight) = shared.install_flight.as_ref() {
                Entry::Join(flight.wait.clone())
            } else {
                let (done, wait) = watch::channel(None);
                shared.install_flight = Some(InstallFlight {
                    done,
                    wait: wait.clone(),
                });
                Entry::Run
            }
        };

        match entry {
            Entry::AlreadyInstalled => {
                self.publish(|state| state.status = UpdateStatus::Ready);
                true
            }
            Entry::Join(mut wait) => watch_value(&mut wait).await.unwrap_or(false),
            Entry::Run => self.run_install(channel).await,
        }
    }

    async fn run_install(&self, channel: UpdateChannel) -> bool {
        self.publish(|state| {
            state.status = UpdateStatus::Installing;
            state.channel = channel;
            state.error = None;
            state.error_code = None;
            state.can_retry = false;
        });
        {
            let mut shared = self.lock();
            shared.failed_operation = None;
            shared.install_in_flight = true;
        }

        let result = self.backend.install_downloaded_update(channel).await;
        let installed = match result {
            Ok(true) => {
                {
                    let mut shared = self.lock();
                    shared.downloaded_update_channel = None;
                    shared.has_installed_update = true;
                    shared.pending_update = None;
                    shared.pending_update_channel = None;
                    shared.install_in_flight = false;
                }
                self.publish(|state| {
                    state.status = UpdateStatus::Ready;
                    state.can_retry = false;
                });
                true
            }
            Ok(false) => {
                {
                    let mut shared = self.lock();
                    shared.downloaded_update_channel = None;
                    shared.install_in_flight = false;
                    shared.failed_operation = Some(FailedOperation {
                        action: FailedAction::Check,
                        channel,
                    });
                }
                self.publish(|state| {
                    state.status = UpdateStatus::Error;
                    state.error = None;
                    state.error_code = Some(UpdateErrorCode::DownloadedUpdateUnavailable);
                    state.can_retry = true;
                });
                false
            }
            Err(error) => {
                {
                    let mut shared = self.lock();
                    shared.install_in_flight = false;
                    shared.failed_operation = Some(FailedOperation {
                        action: FailedAction::Install,
                        channel,
                    });
                }
                self.publish(|state| {
                    state.status = UpdateStatus::Error;
                    state.error = Some(error);
                    state.can_retry = true;
                });
                false
            }
        };

        self.finish_install(installed);
        installed
    }

    fn finish_install(&self, installed: bool) {
        let flight = self.lock().install_flight.take();
        if let Some(flight) = flight {
            let _ = flight.done.send(Some(installed));
        }
    }

    /// Downloads and then installs the release, without the downloaded pause.
    pub async fn download_and_install(&self, channel: UpdateChannel) -> bool {
        self.lock().auto_install_requests += 1;
        let installed = async {
            if self.run_download(channel, false).await {
                self.install(channel).await
            } else {
                false
            }
        }
        .await;
        let mut shared = self.lock();
        shared.auto_install_requests = shared.auto_install_requests.saturating_sub(1);
        installed
    }

    /// Repeats the operation that failed last, if any.
    pub async fn retry(&self) -> bool {
        let Some(operation) = self.lock().failed_operation else {
            return false;
        };
        match operation.action {
            FailedAction::Check => self.check(operation.channel, false).await.is_some(),
            FailedAction::Download => self.download(operation.channel).await,
            FailedAction::DownloadAndInstall => self.download_and_install(operation.channel).await,
            FailedAction::Install => self.install(operation.channel).await,
        }
    }
}
