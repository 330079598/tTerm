//! The only updater implementation that knows Tauri's Update and Error types.
use std::{future::Future, sync::Arc, time::Duration};
use tauri_plugin_updater::UpdaterExt;
use tterm_platform::{
    PlatformFuture,
    updates::{
        AvailableUpdate, DownloadEvent, DownloadedUpdate, UpdateMetadata, UpdateProgress, Updates,
    },
};
const UPDATE_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
// Resets after every chunk, so a slow but steady download is never cut off; only a stall is.
const UPDATE_READ_TIMEOUT: Duration = Duration::from_secs(30);
const UPDATE_MAX_ATTEMPTS: u32 = 3;
const UPDATE_RETRY_BASE_DELAY: Duration = Duration::from_secs(2);

fn is_retryable_update_error(err: &tauri_plugin_updater::Error) -> bool {
    matches!(
        err,
        tauri_plugin_updater::Error::Reqwest(_)
            | tauri_plugin_updater::Error::Network(_)
            // The check reports a non-2xx manifest response as ReleaseNotFound.
            | tauri_plugin_updater::Error::ReleaseNotFound
    )
}

async fn retry_update_request<T, F, Fut>(mut request: F) -> Result<T, tauri_plugin_updater::Error>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, tauri_plugin_updater::Error>>,
{
    let mut attempt = 1;
    loop {
        match request().await {
            Err(err) if attempt < UPDATE_MAX_ATTEMPTS && is_retryable_update_error(&err) => {
                eprintln!("Update request failed (attempt {attempt}/{UPDATE_MAX_ATTEMPTS}): {err}");
                tokio::time::sleep(UPDATE_RETRY_BASE_DELAY * attempt).await;
                attempt += 1;
            }
            result => return result,
        }
    }
}

pub(super) struct TauriUpdates(pub tauri::AppHandle);
struct Release(tauri_plugin_updater::Update);
struct Download {
    update: tauri_plugin_updater::Update,
    bytes: Vec<u8>,
}
impl Updates for TauriUpdates {
    fn check(&self, endpoint: String) -> PlatformFuture<'_, Option<Box<dyn AvailableUpdate>>> {
        Box::pin(async move {
            let endpoint = tauri::Url::parse(&endpoint)
                .map_err(|e| format!("Invalid update endpoint: {e}"))?;
            let updater = self
                .0
                .updater_builder()
                .endpoints(vec![endpoint])
                .map_err(|e| e.to_string())?
                .configure_client(|client| {
                    client
                        .connect_timeout(UPDATE_CONNECT_TIMEOUT)
                        .read_timeout(UPDATE_READ_TIMEOUT)
                })
                .build()
                .map_err(|e| e.to_string())?;
            retry_update_request(|| updater.check())
                .await
                .map(|update| {
                    update.map(|update| Box::new(Release(update)) as Box<dyn AvailableUpdate>)
                })
                .map_err(|e| e.to_string())
        })
    }
}
impl AvailableUpdate for Release {
    fn metadata(&self) -> UpdateMetadata {
        UpdateMetadata {
            version: self.0.version.clone(),
            current_version: self.0.current_version.clone(),
            body: self.0.body.clone(),
            date: self.0.date.map(|date| date.to_string()),
        }
    }
    fn download(
        self: Box<Self>,
        progress: Arc<dyn UpdateProgress>,
    ) -> PlatformFuture<'static, Box<dyn DownloadedUpdate>> {
        Box::pin(async move {
            let bytes = retry_update_request(|| async {
                let mut started = false;
                self.0
                    .download(
                        |chunk_length, content_length| {
                            if !started {
                                progress.report(DownloadEvent::Started { content_length });
                                started = true;
                            }
                            progress.report(DownloadEvent::Progress { chunk_length });
                        },
                        || progress.report(DownloadEvent::Finished),
                    )
                    .await
            })
            .await
            .map_err(|e| e.to_string())?;
            Ok(Box::new(Download {
                update: self.0,
                bytes,
            }) as Box<dyn DownloadedUpdate>)
        })
    }
}
impl DownloadedUpdate for Download {
    fn take_bytes(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.bytes)
    }
    fn install(self: Arc<Self>, bytes: Vec<u8>) -> PlatformFuture<'static, ()> {
        Box::pin(async move {
            crate::core::blocking::run_blocking(move || {
                self.update.install(bytes).map_err(|e| e.to_string())
            })
            .await
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test(start_paused = true)]
    async fn retries_network_errors_until_success() {
        let calls = AtomicU32::new(0);
        let result = retry_update_request(|| async {
            if calls.fetch_add(1, Ordering::SeqCst) < UPDATE_MAX_ATTEMPTS - 1 {
                Err(tauri_plugin_updater::Error::Network("reset".into()))
            } else {
                Ok(7)
            }
        })
        .await;

        assert_eq!(result.unwrap(), 7);
        assert_eq!(calls.load(Ordering::SeqCst), UPDATE_MAX_ATTEMPTS);
    }

    #[tokio::test(start_paused = true)]
    async fn stops_after_max_attempts() {
        let calls = AtomicU32::new(0);
        let result: Result<(), _> = retry_update_request(|| async {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(tauri_plugin_updater::Error::Network("reset".into()))
        })
        .await;

        assert!(result.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), UPDATE_MAX_ATTEMPTS);
    }

    #[tokio::test(start_paused = true)]
    async fn does_not_retry_non_network_errors() {
        let calls = AtomicU32::new(0);
        let result: Result<(), _> = retry_update_request(|| async {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(tauri_plugin_updater::Error::TargetNotFound(
                "windows-x86_64".into(),
            ))
        })
        .await;

        assert!(result.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
