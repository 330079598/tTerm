use super::*;
use crate::sftp::internal::test_support;
use tterm_core::events::recording::RecordingEventSink;
use tterm_core::events::sftp::SftpEvent;

#[tokio::test]
#[ignore = "requires the P1-04 disposable Docker host on port 22404"]
async fn container_upload_download_and_cancel_record_events() {
    let (ssh, sftp) = test_support::connect().await;
    let root = format!("/home/alice/p104-{}", uuid::Uuid::new_v4());
    sftp.create_dir(&root).await.unwrap();
    let local = tterm_core::sftp::internal::transfer::test_server::TempDir::new("p104-upload");
    let input = local.path().join("中文 file.txt");
    let bytes = vec![42u8; 1024 * 1024];
    tokio::fs::write(&input, &bytes).await.unwrap();
    let record = RecordingEventSink::default();
    let events: Arc<dyn EventSink> = Arc::new(record.clone());
    let cancels = TransferCancelMap::default();
    let plan = UploadFilePlanItem {
        file_name: "中文 file.txt".into(),
        file_size: bytes.len() as u64,
        local_path: input.display().to_string(),
        remote_dir: root.clone(),
        remote_path: format!("{root}/中文 file.txt"),
    };
    upload_single_file_with_progress(
        &events,
        "p104",
        test_support::channels(&ssh).await,
        &cancels,
        None,
        &plan,
        "upload",
    )
    .await
    .unwrap();
    assert!(cancels.read().await.is_empty());
    assert!(record.events().iter().any(|event|matches!(event,AppEvent::Sftp(SftpEvent::UploadProgress {payload,..}) if payload.progress==100)));
    let output = local.path().join("download.txt");
    crate::sftp::internal::api::download::download_file_with_progress(
        &events,
        "p104",
        test_support::channels(&ssh).await,
        &cancels,
        "download",
        &plan.remote_path,
        &output.display().to_string(),
    )
    .await
    .unwrap();
    assert_eq!(tokio::fs::read(&output).await.unwrap(), bytes);
    assert!(cancels.read().await.is_empty());
    let (_, batch_cancel) = watch::channel(true);
    let error = upload_single_file_with_progress(
        &events,
        "p104",
        test_support::channels(&ssh).await,
        &cancels,
        Some(batch_cancel),
        &UploadFilePlanItem {
            remote_path: format!("{root}/cancelled.txt"),
            ..plan.clone()
        },
        "cancel",
    )
    .await
    .unwrap_err();
    assert!(error.contains("cancelled"));
    assert!(cancels.read().await.is_empty());
    crate::sftp::internal::launcher::ChannelLauncher::default()
        .start_script(&ssh, &format!("rm -rf -- '{root}'"))
        .await
        .unwrap()
        .finish()
        .await;
}
