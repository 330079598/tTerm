use super::*;
use std::sync::Mutex;
use tterm_platform::{
    PlatformFuture,
    updates::{AvailableUpdate, UpdateMetadata},
    window::MainWindow,
};

type Trace = Arc<Mutex<Vec<&'static str>>>;
struct Window(Trace);
impl MainWindow for Window {
    fn save_before_update(&self) -> PlatformFuture<'_, ()> {
        Box::pin(async {
            self.0.lock().unwrap().push("save");
            Ok(())
        })
    }
    fn send_to_background(&self, _: bool) -> Result<(), String> {
        panic!("unexpected background")
    }
    fn show_and_focus(&self) -> Result<(), String> {
        panic!("unexpected show")
    }
    fn reveal_for_prompt(&self) -> Result<(), String> {
        panic!("unexpected prompt")
    }
    fn set_attention(&self, _: bool) -> Result<(), String> {
        panic!("unexpected attention")
    }
}
struct Download {
    trace: Trace,
    fail: bool,
}
impl DownloadedUpdate for Download {
    fn take_bytes(&mut self) -> Vec<u8> {
        vec![1, 2, 3]
    }
    fn install(self: Arc<Self>, bytes: Vec<u8>) -> PlatformFuture<'static, ()> {
        Box::pin(async move {
            assert_eq!(bytes, [1, 2, 3]);
            self.trace.lock().unwrap().push("install");
            if self.fail {
                Err("installer failed".into())
            } else {
                Ok(())
            }
        })
    }
}
struct Release(Trace);
impl AvailableUpdate for Release {
    fn metadata(&self) -> UpdateMetadata {
        UpdateMetadata {
            version: "2".into(),
            current_version: "1".into(),
            body: None,
            date: None,
        }
    }
    fn download(
        self: Box<Self>,
        progress: Arc<dyn UpdateProgress>,
    ) -> PlatformFuture<'static, Box<dyn DownloadedUpdate>> {
        Box::pin(async move {
            self.0.lock().unwrap().push("download");
            progress.report(DownloadEvent::Started {
                content_length: Some(3),
            });
            progress.report(DownloadEvent::Progress { chunk_length: 3 });
            progress.report(DownloadEvent::Finished);
            Ok(Box::new(Download {
                trace: self.0,
                fail: false,
            }) as Box<dyn DownloadedUpdate>)
        })
    }
}
struct Provider {
    trace: Trace,
    available: bool,
}
impl Updates for Provider {
    fn check(&self, endpoint: String) -> PlatformFuture<'_, Option<Box<dyn AvailableUpdate>>> {
        Box::pin(async move {
            assert_eq!(endpoint, update_endpoint("stable"));
            self.trace.lock().unwrap().push("check");
            Ok(self
                .available
                .then(|| Box::new(Release(self.trace.clone())) as Box<dyn AvailableUpdate>))
        })
    }
}

#[tokio::test]
async fn immediate_update_saves_after_download_before_install_and_preserves_progress() {
    let trace = Trace::default();
    let provider = Provider {
        trace: trace.clone(),
        available: true,
    };
    let window = Window(trace.clone());
    let progress = Arc::new(Mutex::new(Vec::new()));
    let recorded = progress.clone();
    let result = download_install_app_update_service(
        &provider,
        &window,
        &PendingUpdateDownloads::default(),
        "stable".into(),
        Arc::new(Progress(Arc::new(move |event| {
            recorded
                .lock()
                .unwrap()
                .push(serde_json::to_value(event).unwrap());
        }))),
    )
    .await;
    assert!(result.unwrap());
    assert_eq!(
        *trace.lock().unwrap(),
        ["check", "download", "save", "install"]
    );
    assert_eq!(
        *progress.lock().unwrap(),
        [
            serde_json::json!({"event":"Started","data":{"contentLength":3}}),
            serde_json::json!({"event":"Progress","data":{"chunkLength":3}}),
            serde_json::json!({"event":"Finished"}),
        ]
    );
}

#[tokio::test]
async fn missing_release_or_pending_download_does_not_save_or_install() {
    let trace = Trace::default();
    let provider = Provider {
        trace: trace.clone(),
        available: false,
    };
    let window = Window(trace.clone());
    let downloads = PendingUpdateDownloads::default();
    assert!(
        !download_install_app_update_service(
            &provider,
            &window,
            &downloads,
            "stable".into(),
            Arc::new(Progress(Arc::new(|_| panic!("no download"))))
        )
        .await
        .unwrap()
    );
    assert!(
        !install_downloaded_app_update_service(&window, &downloads, "stable".into())
            .await
            .unwrap()
    );
    assert_eq!(*trace.lock().unwrap(), ["check"]);
}

#[tokio::test]
async fn cached_install_keeps_retry_on_failure_and_removes_file_after_success() {
    for fail in [true, false] {
        let trace = Trace::default();
        let path = std::env::temp_dir().join(format!("p107d-update-{}", uuid::Uuid::new_v4()));
        tokio::fs::write(&path, [1, 2, 3]).await.unwrap();
        let downloads = PendingUpdateDownloads::default();
        downloads.0.write().await.insert(
            "stable".into(),
            Arc::new(PendingUpdateDownload {
                path: path.clone(),
                update: Arc::new(Download {
                    trace: trace.clone(),
                    fail,
                }),
            }),
        );
        let result = install_downloaded_app_update_service(
            &Window(trace.clone()),
            &downloads,
            "stable".into(),
        )
        .await;
        assert_eq!(
            result,
            if fail {
                Err("installer failed".into())
            } else {
                Ok(true)
            }
        );
        assert_eq!(*trace.lock().unwrap(), ["save", "install"]);
        assert_eq!(downloads.0.read().await.contains_key("stable"), fail);
        assert_eq!(path.exists(), fail);
        if fail {
            tokio::fs::remove_file(path).await.unwrap();
        }
    }
}
