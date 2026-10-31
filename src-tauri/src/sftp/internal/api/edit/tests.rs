use super::*;
use crate::sftp::internal::transfer::test_server::{ServerOptions, TestServer};
use std::sync::{Arc, Mutex};

fn attributes(permissions: Option<u32>, uid: Option<u32>, gid: Option<u32>) -> FileAttributes {
    FileAttributes {
        permissions,
        uid,
        gid,
        ..FileAttributes::empty()
    }
}

#[test]
fn the_temporary_file_gets_only_the_read_write_and_execute_bits() {
    let setuid_script = attributes(Some(0o104_750), None, None);
    assert_eq!(
        temp_file_attributes(&setuid_script).permissions,
        Some(0o750)
    );
    assert_eq!(
        temp_file_attributes(&FileAttributes::empty()).permissions,
        None
    );
}

#[test]
fn restores_the_owner_before_the_mode_and_only_what_changed() {
    let original = attributes(Some(0o104_640), Some(0), Some(42));
    let replaced = attributes(Some(0o100_644), Some(1000), Some(1000));

    let requests = attributes_to_restore(&original, &replaced);
    assert_eq!(requests.len(), 2);
    assert_eq!((requests[0].uid, requests[0].gid), (Some(0), Some(42)));
    assert_eq!(requests[0].permissions, None);
    assert_eq!(requests[1].permissions, Some(0o4640));
    assert_eq!((requests[1].uid, requests[1].gid), (None, None));

    assert!(attributes_to_restore(&original, &original).is_empty());
}

#[test]
fn never_sends_an_owner_without_its_group() {
    let original = attributes(None, Some(0), None);
    let replaced = attributes(None, Some(1000), Some(1000));
    assert!(attributes_to_restore(&original, &replaced).is_empty());
}

/// Saves `content` over `path` the way the editor does, from a baseline
/// taken just before.
async fn save(sftp: &SftpSession, path: &str, content: &str) {
    let metadata = sftp.metadata(path).await.expect("metadata");
    let baseline = SftpEditBaseline {
        modified_at: metadata_modified_at(&metadata),
        size: metadata.size.unwrap_or(0),
    };
    save_editable_file(sftp, path, content, &baseline)
        .await
        .expect("save");
}

fn leftover_temporary_files(root: &std::path::Path) -> Vec<String> {
    std::fs::read_dir(root)
        .expect("read root")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(".tterm-save-"))
        .collect()
}

#[cfg(unix)]
fn set_mode(path: &std::path::Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("set mode");
}

#[cfg(unix)]
fn mode_of(path: &std::path::Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o7777
}

#[tokio::test]
async fn a_rename_that_overwrites_keeps_the_mode_from_start_to_end() {
    let opens = Arc::new(Mutex::new(Vec::new()));
    let server = TestServer::start(ServerOptions {
        recorded_open_modes: Some(opens.clone()),
        ..ServerOptions::default()
    })
    .await;
    let file = server.root().join("notes.txt");
    std::fs::write(&file, "old\n").expect("write file");
    #[cfg(unix)]
    set_mode(&file, 0o600);
    let sftp = server.sftp_session().await;
    let original_mode = sftp
        .metadata("/notes.txt")
        .await
        .expect("metadata")
        .permissions;

    save(&sftp, "/notes.txt", "new\n").await;

    assert_eq!(std::fs::read_to_string(&file).expect("read"), "new\n");
    assert!(leftover_temporary_files(server.root()).is_empty());
    let temp_modes = opens
        .lock()
        .expect("opens")
        .iter()
        .filter(|(path, _)| path.contains(".tterm-save-"))
        .map(|(_, mode)| *mode)
        .collect::<Vec<_>>();
    assert_eq!(temp_modes, vec![original_mode.map(|mode| mode & 0o777)]);
    // The test server creates files with the process umask, so the mode
    // that remains is the one the save set back after the rename.
    #[cfg(unix)]
    assert_eq!(mode_of(&file), 0o600);
}

#[tokio::test]
async fn a_rename_that_refuses_writes_into_the_original_file() {
    let server = TestServer::start(ServerOptions {
        rename_overwrites: false,
        ..ServerOptions::default()
    })
    .await;
    let file = server.root().join("notes.txt");
    std::fs::write(&file, "old\n").expect("write file");
    #[cfg(unix)]
    set_mode(&file, 0o600);
    #[cfg(unix)]
    let inode = std::os::unix::fs::MetadataExt::ino(&std::fs::metadata(&file).expect("stat"));
    let sftp = server.sftp_session().await;

    save(&sftp, "/notes.txt", "new\n").await;

    assert_eq!(std::fs::read_to_string(&file).expect("read"), "new\n");
    assert!(leftover_temporary_files(server.root()).is_empty());
    #[cfg(unix)]
    {
        assert_eq!(mode_of(&file), 0o600);
        let after = std::os::unix::fs::MetadataExt::ino(&std::fs::metadata(&file).expect("stat"));
        assert_eq!(after, inode, "the original file is written in place");
    }
}
