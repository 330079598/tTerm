//! The archive scripts against a real OpenSSH server and the host's own
//! `tar`, `zip` and `gzip` (see [`crate::ssh::test_sshd`]).

use super::script::{compress_script, extract_script, ArchiveScript, MISSING_TOOL_STATUS};
use super::ArchiveFormat;
use crate::sftp::internal::exec::{run_to_end, ScriptOutcome};
use crate::sftp::internal::launcher::ChannelLauncher;
use crate::ssh::test_sshd::{AcceptAnyServerKey, TempDir, TestSshd};
use russh::client::Handle;
use std::path::Path;
use std::time::Duration;
use tokio::sync::watch;

const SFTP: &str = "Subsystem sftp internal-sftp\n";

async fn run(ssh: &Handle<AcceptAnyServerKey>, script: &ArchiveScript) -> ScriptOutcome {
    let (_sender, mut cancel) = watch::channel(false);
    run_to_end(&ChannelLauncher, ssh, &script.text, &mut cancel)
        .await
        .expect("run script")
        .expect("not cancelled")
}

fn path(dir: &Path, relative: &str) -> String {
    dir.join(relative).to_string_lossy().into_owned()
}

fn sample(dir: &Path) {
    std::fs::create_dir_all(dir.join("src/project/nested")).unwrap();
    std::fs::write(dir.join("src/project/a.txt"), "alpha").unwrap();
    std::fs::write(dir.join("src/project/nested/b.txt"), "beta").unwrap();
    std::fs::write(dir.join("src/-dash.txt"), "dash").unwrap();
}

#[tokio::test]
#[ignore = "spawns a real sshd; run with --ignored real_sshd"]
async fn real_sshd_round_trips_tar_and_zip_archives() {
    let sshd = TestSshd::spawn(12344, &["-t", "ed25519"], SFTP).await;
    let (ssh, _sftp) = sshd.sftp_session().await;
    let dir = TempDir::new();
    sample(&dir.path);
    let names = vec!["project".to_string(), "-dash.txt".to_string()];

    for (format, archive) in [
        (ArchiveFormat::TarGz, "out.tar.gz"),
        (ArchiveFormat::Zip, "out.zip"),
        (ArchiveFormat::TarXz, "out.tar.xz"),
    ] {
        let compress = compress_script(&path(&dir.path, "src"), &names, archive, format).unwrap();
        let outcome = run(&ssh, &compress).await;
        assert!(
            outcome.succeeded(),
            "{}",
            outcome.failure_message("compress")
        );

        let target = dir.path.join(format!("{archive}-out"));
        let extract = extract_script(
            &path(&dir.path, &format!("src/{archive}")),
            &target.to_string_lossy(),
            format,
            false,
        )
        .unwrap();
        let outcome = run(&ssh, &extract).await;
        assert!(
            outcome.succeeded(),
            "{}",
            outcome.failure_message("extract")
        );
        assert_eq!(
            std::fs::read_to_string(target.join("project/nested/b.txt")).unwrap(),
            "beta"
        );
        assert_eq!(
            std::fs::read_to_string(target.join("-dash.txt")).unwrap(),
            "dash"
        );
    }
}

#[tokio::test]
#[ignore = "spawns a real sshd; run with --ignored real_sshd"]
async fn real_sshd_extracting_keeps_existing_files_unless_overwriting() {
    let sshd = TestSshd::spawn(12345, &["-t", "ed25519"], SFTP).await;
    let (ssh, _sftp) = sshd.sftp_session().await;
    let dir = TempDir::new();
    sample(&dir.path);
    let compress = compress_script(
        &path(&dir.path, "src"),
        &["project".to_string()],
        "p.zip",
        ArchiveFormat::Zip,
    )
    .unwrap();
    assert!(run(&ssh, &compress).await.succeeded());

    let target = dir.path.join("target");
    std::fs::create_dir_all(target.join("project")).unwrap();
    std::fs::write(target.join("project/a.txt"), "mine").unwrap();
    for (overwrite, expected) in [(false, "mine"), (true, "alpha")] {
        let extract = extract_script(
            &path(&dir.path, "src/p.zip"),
            &target.to_string_lossy(),
            ArchiveFormat::Zip,
            overwrite,
        )
        .unwrap();
        assert!(run(&ssh, &extract).await.succeeded());
        assert_eq!(
            std::fs::read_to_string(target.join("project/a.txt")).unwrap(),
            expected
        );
    }
}

#[tokio::test]
#[ignore = "spawns a real sshd; run with --ignored real_sshd"]
async fn real_sshd_decompresses_a_single_file_and_names_missing_tools() {
    let sshd = TestSshd::spawn(12346, &["-t", "ed25519"], SFTP).await;
    let (ssh, _sftp) = sshd.sftp_session().await;
    let dir = TempDir::new();
    std::fs::write(dir.path.join("app.log"), "line\n").unwrap();
    let status = std::process::Command::new("gzip")
        .arg(dir.path.join("app.log"))
        .status()
        .unwrap();
    assert!(status.success());

    let archive = path(&dir.path, "app.log.gz");
    let destination = dir.path.to_string_lossy().into_owned();
    let extract = extract_script(&archive, &destination, ArchiveFormat::Gzip, false).unwrap();
    assert!(run(&ssh, &extract).await.succeeded());
    assert_eq!(
        std::fs::read_to_string(dir.path.join("app.log")).unwrap(),
        "line\n"
    );
    let again = run(&ssh, &extract).await;
    assert!(!again.succeeded());
    assert!(
        again.stderr.contains("app.log already exists"),
        "{}",
        again.stderr
    );

    // No 7-Zip on the test host: the script says so with its status.
    let seven = extract_script(&archive, &destination, ArchiveFormat::SevenZip, true).unwrap();
    let outcome = run(&ssh, &seven).await;
    if !["7zz", "7z", "7za"].iter().any(|tool| which(tool)) {
        assert_eq!(outcome.exit_status, Some(MISSING_TOOL_STATUS));
    }
}

fn which(tool: &str) -> bool {
    std::process::Command::new("which")
        .arg(tool)
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Cancelling sends the command SIGTERM, so it stops on the host instead of
/// writing the archive to its end.
#[tokio::test]
#[ignore = "spawns a real sshd; run with --ignored real_sshd"]
async fn real_sshd_cancelling_stops_the_command_on_the_host() {
    let sshd = TestSshd::spawn(12347, &["-t", "ed25519"], SFTP).await;
    let (ssh, _sftp) = sshd.sftp_session().await;
    let dir = TempDir::new();
    std::fs::create_dir_all(dir.path.join("src")).unwrap();
    let noise = (0..64 * 1024 * 1024u32)
        .map(|index| (index.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect::<Vec<_>>();
    std::fs::write(dir.path.join("src/noise.bin"), noise).unwrap();
    let archive_name = format!("cancel-{}.tar.xz", uuid::Uuid::new_v4());
    let compress = compress_script(
        &path(&dir.path, "src"),
        &["noise.bin".to_string()],
        &archive_name,
        ArchiveFormat::TarXz,
    )
    .unwrap();

    let (sender, mut cancel) = watch::channel(false);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let _ = sender.send(true);
    });
    let outcome = run_to_end(&ChannelLauncher, &ssh, &compress.text, &mut cancel)
        .await
        .expect("run script");
    assert!(
        outcome.is_none(),
        "the compress finished before it was cancelled"
    );

    tokio::time::sleep(Duration::from_secs(1)).await;
    let still_running = std::process::Command::new("pgrep")
        .args(["-f", &archive_name])
        .output()
        .unwrap();
    assert!(
        !still_running.status.success(),
        "tar is still running after cancel"
    );
}
