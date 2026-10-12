use super::*;

fn item(name: &str, size: u64, mtime: i64) -> DirectoryDownloadItem {
    DirectoryDownloadItem {
        remote_path: format!("/remote/{name}"),
        relative_path: PathBuf::from(name),
        file_name: name.to_string(),
        size,
        mtime: Some(mtime),
    }
}

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tterm-conflict-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn rename_takes_the_first_free_copy_name() {
    let dir = temp_dir();
    std::fs::write(dir.join("app.log"), b"old").unwrap();
    std::fs::write(dir.join("app (1).log"), b"older").unwrap();

    let resolved = resolve_download_target(
        &dir.join("app.log"),
        &item("app.log", 10, 0),
        ConflictPolicy::Rename,
        false,
    )
    .await
    .unwrap();
    assert_eq!(resolved, Some(dir.join("app (2).log")));

    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn rename_retry_skips_the_copy_an_earlier_attempt_finished() {
    let dir = temp_dir();
    std::fs::write(dir.join("app.log"), b"someone else's file").unwrap();
    // The earlier attempt already wrote the whole file to the copy name;
    // its fresh mtime is newer than the remote's.
    std::fs::write(dir.join("app (1).log"), b"0123456789").unwrap();

    let resolved = resolve_download_target(
        &dir.join("app.log"),
        &item("app.log", 10, 0),
        ConflictPolicy::Rename,
        true,
    )
    .await
    .unwrap();
    assert_eq!(resolved, None);

    std::fs::remove_dir_all(dir).unwrap();
}

fn plan_paths(plan: &DirectoryDownloadPlan) -> (Vec<PathBuf>, Vec<(PathBuf, u64)>) {
    let mut directories = plan.directories.clone();
    directories.sort();
    let mut files: Vec<_> = plan
        .files
        .iter()
        .map(|item| (item.relative_path.clone(), item.size))
        .collect();
    files.sort();
    (directories, files)
}

/// Lays out `report.txt` and `logs/{a.log,nested/b.log}` under `root`.
fn write_mixed_selection(root: &Path) {
    std::fs::write(root.join("report.txt"), b"12345").unwrap();
    std::fs::create_dir_all(root.join("logs/nested")).unwrap();
    std::fs::write(root.join("logs/a.log"), b"abc").unwrap();
    std::fs::write(root.join("logs/nested/b.log"), b"de").unwrap();
}

fn assert_mixed_selection_plan(plan: &DirectoryDownloadPlan) {
    assert_eq!(plan.total_size, 10);
    assert_eq!(
        plan_paths(plan),
        (
            vec![PathBuf::from("logs"), PathBuf::from("logs/nested")],
            vec![
                (PathBuf::from("logs/a.log"), 3),
                (PathBuf::from("logs/nested/b.log"), 2),
                (PathBuf::from("report.txt"), 5),
            ],
        )
    );
}

#[tokio::test]
async fn plans_files_and_folders_of_a_selection_side_by_side() {
    use crate::sftp::internal::transfer::test_server::{ServerOptions, TestServer};

    let server = TestServer::start(ServerOptions::default()).await;
    write_mixed_selection(server.root());
    let sftp = server.sftp_session().await;

    let plan = collect_download_plan(&sftp, &["/report.txt".into(), "/logs".into()])
        .await
        .unwrap();
    assert_mixed_selection_plan(&plan);
}

/// The same plan against OpenSSH's own `sftp-server`.
#[cfg(unix)]
#[tokio::test]
#[ignore = "spawns a real sshd; run with --ignored real_sshd"]
async fn real_sshd_plans_files_and_folders_of_a_selection() {
    use crate::ssh::auth::{authenticate, AuthPrompter, AuthTarget};
    use crate::ssh::test_sshd::{current_user, TempDir, TestSshd};

    let sshd = TestSshd::spawn(12322, &["-t", "ed25519"], "Subsystem sftp internal-sftp\n").await;
    let username = current_user();
    let mut session = sshd.connect().await;
    authenticate(
        &mut session,
        &AuthTarget {
            host: "127.0.0.1",
            port: sshd.port,
            username: &username,
            hop: None,
        },
        sshd.key_method(),
        &AuthPrompter::Unavailable,
    )
    .await
    .expect("authenticate");
    let channel = session.channel_open_session().await.expect("open channel");
    channel
        .request_subsystem(true, "sftp")
        .await
        .expect("sftp subsystem");
    let sftp = SftpSession::new(channel.into_stream())
        .await
        .expect("sftp session");

    let dir = TempDir::new();
    write_mixed_selection(&dir.path);
    let root = dir.path.to_string_lossy().into_owned();
    let plan = collect_download_plan(
        &sftp,
        &[format!("{root}/report.txt"), format!("{root}/logs")],
    )
    .await
    .unwrap();
    assert_mixed_selection_plan(&plan);
}

#[tokio::test]
async fn skip_and_overwrite_if_newer_keep_a_newer_local_file() {
    let dir = temp_dir();
    std::fs::write(dir.join("app.log"), b"edited locally").unwrap();
    let local = dir.join("app.log");

    for policy in [ConflictPolicy::Skip, ConflictPolicy::OverwriteIfNewer] {
        let resolved = resolve_download_target(&local, &item("app.log", 10, 0), policy, false)
            .await
            .unwrap();
        assert_eq!(resolved, None, "{policy:?}");
    }

    // A remote copy stamped in the far future is newer than the local one.
    let resolved = resolve_download_target(
        &local,
        &item("app.log", 10, i64::MAX / 2),
        ConflictPolicy::OverwriteIfNewer,
        false,
    )
    .await
    .unwrap();
    assert_eq!(resolved, Some(local.clone()));

    std::fs::remove_dir_all(dir).unwrap();
}
