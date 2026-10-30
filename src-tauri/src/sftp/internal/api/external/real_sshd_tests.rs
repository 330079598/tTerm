//! Downloading and uploading opened files against a real OpenSSH server
//! (see [`crate::ssh::test_sshd`]).

use super::remote::{self, RemoteStamp};
use crate::ssh::test_sshd::{TempDir, TestSshd};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

const SFTP: &str = "Subsystem sftp internal-sftp\n";

/// The path as the server names it: macOS puts temp files behind a link.
fn remote_path(path: &Path) -> String {
    let parent = std::fs::canonicalize(path.parent().unwrap()).unwrap();
    parent
        .join(path.file_name().unwrap())
        .to_string_lossy()
        .into_owned()
}

#[tokio::test]
#[ignore = "spawns a real sshd; run with --ignored real_sshd"]
async fn real_sshd_external_edit_round_trip_keeps_the_file_as_it_was() {
    let sshd = TestSshd::spawn(12362, &["-t", "ed25519"], SFTP).await;
    let (_ssh, sftp) = sshd.sftp_session().await;
    let server = TempDir::new();
    let copies = TempDir::new();

    let target = server.path.join("app.conf");
    std::fs::write(&target, "port = 80\n").unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o640)).unwrap();
    let link = server.path.join("current.conf");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    // Opened through the symlink, as a config file often is.
    let local = copies.path.join("current.conf");
    let baseline = remote::download(&sftp, &remote_path(&link), &local)
        .await
        .unwrap();
    assert_eq!(std::fs::read_to_string(&local).unwrap(), "port = 80\n");
    assert_eq!(
        std::fs::metadata(&local).unwrap().permissions().mode() & 0o777,
        0o600
    );

    std::fs::write(&local, "port = 8080\n").unwrap();
    let stamp = remote::upload(&sftp, &remote_path(&link), &local, baseline, false)
        .await
        .unwrap()
        .expect("the remote file is unchanged, so the upload goes ahead");
    assert_eq!(stamp.size, 12);
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "port = 8080\n");
    assert!(std::fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o640
    );
}

#[tokio::test]
#[ignore = "spawns a real sshd; run with --ignored real_sshd"]
async fn real_sshd_external_edit_upload_stops_at_a_remote_change() {
    let sshd = TestSshd::spawn(12363, &["-t", "ed25519"], SFTP).await;
    let (_ssh, sftp) = sshd.sftp_session().await;
    let server = TempDir::new();
    let copies = TempDir::new();
    let target = server.path.join("notes.txt");
    std::fs::write(&target, "one").unwrap();
    let remote = remote_path(&target);
    let local = copies.path.join("notes.txt");
    let baseline = remote::download(&sftp, &remote, &local).await.unwrap();

    std::fs::write(&local, "mine").unwrap();
    std::fs::write(&target, "theirs, longer").unwrap();
    let outcome = remote::upload(&sftp, &remote, &local, baseline, false)
        .await
        .unwrap();
    assert_eq!(outcome, None);
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "theirs, longer");

    // Overwriting writes the whole copy, leaving nothing of the longer file.
    remote::upload(&sftp, &remote, &local, baseline, true)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "mine");

    // A file removed meanwhile is a change too, and overwriting puts it back.
    std::fs::remove_file(&target).unwrap();
    let stale = RemoteStamp {
        size: 4,
        mtime: None,
    };
    assert_eq!(
        remote::upload(&sftp, &remote, &local, stale, false)
            .await
            .unwrap(),
        None
    );
    remote::upload(&sftp, &remote, &local, stale, true)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "mine");
}

#[tokio::test]
#[ignore = "spawns a real sshd; run with --ignored real_sshd"]
async fn real_sshd_external_edit_refuses_folders() {
    let sshd = TestSshd::spawn(12364, &["-t", "ed25519"], SFTP).await;
    let (_ssh, sftp) = sshd.sftp_session().await;
    let server = TempDir::new();
    let copies = TempDir::new();
    let folder = std::fs::canonicalize(&server.path).unwrap();
    let error = remote::download(&sftp, &folder.to_string_lossy(), &copies.path.join("x"))
        .await
        .unwrap_err();
    assert!(!error.is_empty());
}
