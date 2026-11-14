use crate::sftp::internal::api::search::*;
use crate::sftp::internal::test_support;
use std::sync::Mutex;

#[derive(Default)]
struct Collected(Mutex<Vec<SearchEvent>>);
impl SearchResultSink for Collected {
    fn send(&self, event: SearchEvent) -> Result<(), String> {
        self.0.lock().unwrap().push(event);
        Ok(())
    }
}
#[tokio::test]
#[ignore = "requires the P1-04 disposable Docker host on port 22404"]
async fn container_recursive_find_walk_and_cancel() {
    let (ssh, sftp) = test_support::connect().await;
    let root = format!("/home/alice/p104-search-{}", uuid::Uuid::new_v4());
    let launcher = crate::sftp::internal::launcher::ChannelLauncher::default();
    launcher.start_script(&ssh,&format!("mkdir -p '{root}/a/b'; touch '{root}/a/b/中文.log'; i=0; while [ $i -lt 55 ]; do mkdir '{root}/d'$i; i=$((i+1)); done")).await.unwrap().finish().await;
    let matcher = NameMatcher::new("log", false).unwrap();
    let record = Arc::new(Collected::default());
    let mut sink = ResultSink {
        channel: record.clone(),
        found: 0,
        truncated: false,
    };
    let (sender, mut cancel) = watch::channel(false);
    let find = FindSearch {
        launcher: &launcher,
        ssh: &ssh,
        sftp: &sftp,
        root: &root,
        matcher: &matcher,
        include_hidden: false,
    };
    assert!(matches!(
        find.run(&mut sink, &mut cancel).await,
        FindOutcome::Finished { cancelled: false }
    ));
    assert_eq!(sink.found, 1);
    let mut walk_sink = ResultSink {
        channel: record.clone(),
        found: 0,
        truncated: false,
    };
    assert!(
        !walk::walk(&sftp, &root, &matcher, false, &mut walk_sink, &mut cancel)
            .await
            .unwrap()
            .cancelled
    );
    assert_eq!(walk_sink.found, 1);
    assert!(record.0.lock().unwrap().iter().any(|e| matches!(
        e,
        SearchEvent::Progress {
            scanned_folders: 25
        }
    )));
    sender.send(true).unwrap();
    assert!(matches!(
        find.run(&mut sink, &mut cancel).await,
        FindOutcome::Finished { cancelled: true }
    ));
    launcher
        .start_script(&ssh, &format!("rm -rf -- '{root}'"))
        .await
        .unwrap()
        .finish()
        .await;
}
