use crate::sftp::internal::api::external::*;
use crate::sftp::internal::test_support;

#[tokio::test]
#[ignore = "requires the P1-04 disposable Docker host on port 22404"]
async fn container_external_edit_upload_and_remote_change() {
    let (ssh, sftp) = test_support::connect().await;
    let launcher = crate::sftp::internal::launcher::ChannelLauncher::default();
    let path = format!("/home/alice/p104-edit-{}.txt", uuid::Uuid::new_v4());
    launcher
        .start_script(&ssh, &format!("printf old > '{path}'"))
        .await
        .unwrap()
        .finish()
        .await;
    let dir = crate::sftp::internal::transfer::test_server::TempDir::new("p104-edit");
    let local = dir.path().join("edit.txt");
    let baseline = remote::download(&sftp, &path, &local).await.unwrap();
    tokio::fs::write(&local, "local changed").await.unwrap();
    let uploaded = remote::upload(&sftp, &path, &local, baseline, false)
        .await
        .unwrap()
        .unwrap();
    launcher
        .start_script(&ssh, &format!("printf 'remote changed again' > '{path}'"))
        .await
        .unwrap()
        .finish()
        .await;
    assert_eq!(
        remote::upload(&sftp, &path, &local, uploaded, false)
            .await
            .unwrap(),
        None
    );
    sftp.remove_file(path).await.unwrap();
}
