use crate::sftp::internal::api::delete::*;
use crate::sftp::internal::test_support;
use tterm_core::events::recording::RecordingEventSink;
use tterm_core::events::sftp::SftpEvent;

#[tokio::test]
#[ignore = "requires the P1-04 disposable Docker host on port 22404"]
async fn container_recursive_delete_preserves_sampled_events() {
    let (ssh, sftp) = test_support::connect().await;
    let launcher = tterm_core::sftp::internal::launcher::ChannelLauncher::default();
    let root = format!("/home/alice/p104-delete-{}", uuid::Uuid::new_v4());
    launcher.start_script(&ssh,&format!("mkdir '{root}'; i=0; while [ $i -lt 80 ]; do touch '{root}/file'$i; i=$((i+1)); done")).await.unwrap().finish().await;
    let record = RecordingEventSink::default();
    let events: Arc<dyn EventSink> = Arc::new(record.clone());
    let entries = vec![DeleteEntryRequest {
        path: root.clone(),
        name: "root".into(),
        is_dir: true,
    }];
    let stats = DeleteStats {
        directories: 1,
        files: 80,
        entries: 81,
        truncated: false,
    };
    let progress =
        batch::delete_entries_with_sftp(&events, "p104", &sftp, "delete", &entries, &stats).await;
    assert_eq!(
        (
            progress.deleted_files,
            progress.deleted_directories,
            progress.failed
        ),
        (80, 1, 0)
    );
    assert!(sftp.metadata(&root).await.is_err());
    let samples = record
        .events()
        .into_iter()
        .map(|event| match event {
            AppEvent::Sftp(SftpEvent::DeleteProgress { payload, .. }) => (
                payload.deleted_files + payload.deleted_directories,
                payload.current_path,
            ),
            _ => panic!("unexpected event"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        samples.iter().map(|(count, _)| *count).collect::<Vec<_>>(),
        vec![25, 50, 75, 81]
    );
    assert!(samples.last().unwrap().1.is_empty());
}

#[test]
fn delete_sampling_stays_at_twenty_five_items() {
    for (count, expected) in [(0, true), (24, false), (25, true), (26, false), (50, true)] {
        let progress = DeleteProgress {
            deleted_files: count,
            ..Default::default()
        };
        assert_eq!(should_emit_delete_progress(&progress), expected);
    }
}
