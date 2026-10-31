//! Searching against a real OpenSSH server (see [`crate::ssh::test_sshd`]),
//! with the host's own `find`, and over SFTP alone for an account that
//! cannot run commands.

use super::find::{FindOutcome, FindSearch};
use super::matcher::NameMatcher;
use super::walk::walk;
use super::{ResultSink, MAX_RESULTS};
use crate::sftp::internal::launcher::ChannelLauncher;
use crate::ssh::test_sshd::{TempDir, TestSshd};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tauri::ipc::{Channel, InvokeResponseBody};
use tokio::sync::watch;

const SFTP: &str = "Subsystem sftp internal-sftp\n";

/// A sink whose matches land in a list of paths relative to `root`.
fn sink(root: &Path) -> (ResultSink, Arc<Mutex<Vec<String>>>) {
    let found = Arc::new(Mutex::new(Vec::new()));
    let collected = found.clone();
    let prefix = format!("{}/", root.display());
    let channel = Channel::new(move |body| {
        let InvokeResponseBody::Json(json) = body else {
            return Ok(());
        };
        let event: serde_json::Value = serde_json::from_str(&json).expect("event json");
        if event["kind"] == "matches" {
            for entry in event["entries"].as_array().expect("entries") {
                let path = entry["path"].as_str().expect("path");
                let relative = path.strip_prefix(&prefix).unwrap_or(path);
                collected.lock().unwrap().push(relative.to_string());
            }
        }
        Ok(())
    });
    (
        ResultSink {
            channel,
            found: 0,
            truncated: false,
        },
        found,
    )
}

fn write(root: &Path, relative: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, "x").unwrap();
}

/// A tree with matches at several depths, one hidden.
fn sample_tree() -> TempDir {
    let dir = TempDir::new();
    write(&dir.path, "app.log");
    write(&dir.path, "a/b/Old.LOG");
    write(&dir.path, "a/readme.txt");
    write(&dir.path, ".cache/hidden.log");
    std::fs::create_dir_all(dir.path.join("logs")).unwrap();
    dir
}

/// The root as the server names it: macOS puts temp files behind a link.
fn root_of(dir: &TempDir) -> String {
    std::fs::canonicalize(&dir.path)
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

fn sorted(found: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    let mut found = found.lock().unwrap().clone();
    found.sort();
    found
}

#[tokio::test]
#[ignore = "spawns a real sshd; run with --ignored real_sshd"]
async fn real_sshd_find_searches_subfolders() {
    let sshd = TestSshd::spawn(12341, &["-t", "ed25519"], SFTP).await;
    let (ssh, sftp) = sshd.sftp_session().await;
    let dir = sample_tree();
    let root = root_of(&dir);
    let (_cancel_sender, mut cancel) = watch::channel(false);

    for (include_hidden, expected) in [
        (false, vec!["a/b/Old.LOG", "app.log", "logs"]),
        (
            true,
            vec![".cache/hidden.log", "a/b/Old.LOG", "app.log", "logs"],
        ),
    ] {
        let matcher = NameMatcher::new("log", false).unwrap();
        let (mut results, found) = sink(Path::new(&root));
        let search = FindSearch {
            launcher: &ChannelLauncher::default(),
            ssh: &ssh,
            sftp: &sftp,
            root: &root,
            matcher: &matcher,
            include_hidden,
        };
        let outcome = search.run(&mut results, &mut cancel).await;
        assert!(matches!(
            outcome,
            FindOutcome::Finished { cancelled: false }
        ));
        assert_eq!(sorted(&found), expected);
    }

    // A regular expression is matched here rather than by `find`.
    let matcher = NameMatcher::new(r"^old\.log$", true).unwrap();
    let (mut results, found) = sink(Path::new(&root));
    let search = FindSearch {
        launcher: &ChannelLauncher::default(),
        ssh: &ssh,
        sftp: &sftp,
        root: &root,
        matcher: &matcher,
        include_hidden: false,
    };
    search.run(&mut results, &mut cancel).await;
    assert_eq!(sorted(&found), vec!["a/b/Old.LOG"]);
}

#[tokio::test]
#[ignore = "spawns a real sshd; run with --ignored real_sshd"]
async fn real_sshd_find_stops_at_the_result_limit() {
    let sshd = TestSshd::spawn(12342, &["-t", "ed25519"], SFTP).await;
    let (ssh, sftp) = sshd.sftp_session().await;
    let dir = TempDir::new();
    for index in 0..MAX_RESULTS + 20 {
        write(&dir.path, &format!("d{}/file{index}.txt", index % 7));
    }
    let root = root_of(&dir);
    let matcher = NameMatcher::new("*.txt", false).unwrap();
    let (mut results, found) = sink(Path::new(&root));
    let (_cancel_sender, mut cancel) = watch::channel(false);
    let search = FindSearch {
        launcher: &ChannelLauncher::default(),
        ssh: &ssh,
        sftp: &sftp,
        root: &root,
        matcher: &matcher,
        include_hidden: false,
    };
    search.run(&mut results, &mut cancel).await;
    assert!(results.truncated);
    assert_eq!(results.found, MAX_RESULTS);
    assert_eq!(found.lock().unwrap().len(), MAX_RESULTS);
}

/// An account forced to `internal-sftp` runs that for every command, so the
/// script's output is never `find`'s and the SFTP walk takes over.
#[tokio::test]
#[ignore = "spawns a real sshd; run with --ignored real_sshd"]
async fn real_sshd_sftp_only_account_falls_back_to_walking() {
    let sshd = TestSshd::spawn(
        12343,
        &["-t", "ed25519"],
        "Subsystem sftp internal-sftp\nForceCommand internal-sftp\n",
    )
    .await;
    let (ssh, sftp) = sshd.sftp_session().await;
    let dir = sample_tree();
    let root = root_of(&dir);
    let matcher = NameMatcher::new("log", false).unwrap();
    let (_cancel_sender, mut cancel) = watch::channel(false);

    let (mut results, found) = sink(Path::new(&root));
    let search = FindSearch {
        launcher: &ChannelLauncher::default(),
        ssh: &ssh,
        sftp: &sftp,
        root: &root,
        matcher: &matcher,
        include_hidden: false,
    };
    let outcome = search.run(&mut results, &mut cancel).await;
    assert!(matches!(outcome, FindOutcome::Unavailable));
    assert!(found.lock().unwrap().is_empty());

    let walked = walk(&sftp, &root, &matcher, false, &mut results, &mut cancel)
        .await
        .expect("walk");
    assert!(!walked.cancelled);
    assert_eq!(walked.skipped_folders, 0);
    assert_eq!(sorted(&found), vec!["a/b/Old.LOG", "app.log", "logs"]);

    let missing = walk(
        &sftp,
        &format!("{root}/missing"),
        &matcher,
        false,
        &mut results,
        &mut cancel,
    )
    .await;
    assert!(missing.is_err(), "an unreadable root is an error");
}
