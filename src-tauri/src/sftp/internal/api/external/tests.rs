use super::local::{self, FileStamp, Fingerprint};
use super::remote::RemoteStamp;
use super::watch::{self, Look, Step, Watched};
use super::*;
use std::time::{Duration, SystemTime};

/// A folder for one test, removed when it ends.
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("tterm-external-edit-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[test]
fn local_names_keep_the_remote_name_outside_windows() {
    assert_eq!(local::local_file_name("a:b*?.conf", false), "a:b*?.conf");
}

#[test]
fn local_names_avoid_what_windows_forbids() {
    assert_eq!(local::local_file_name("a:b*?.conf", true), "a_b__.conf");
    assert_eq!(local::local_file_name("notes.txt. ", true), "notes.txt");
    assert_eq!(local::local_file_name("...", true), "file");
    assert_eq!(local::local_file_name("con.txt", true), "_con.txt");
    assert_eq!(local::local_file_name("COM1", true), "_COM1");
    assert_eq!(local::local_file_name("com10.log", true), "com10.log");
    assert_eq!(local::local_file_name("console.log", true), "console.log");
}

#[test]
fn files_the_system_would_run_are_told_apart() {
    for name in [
        "setup.EXE",
        "x.bat",
        "deploy.sh",
        "Tool.command",
        "a.tar.desktop",
    ] {
        assert!(local::runs_when_opened(name), "{name}");
    }
    for name in ["nginx.conf", "README", "app.log", "exe", "data.json"] {
        assert!(!local::runs_when_opened(name), "{name}");
    }
}

fn stamp(size: u64) -> FileStamp {
    FileStamp {
        size,
        modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(size)),
    }
}

#[test]
fn a_change_is_read_once_it_holds_for_a_look() {
    let (old, new) = (Some(stamp(1)), Some(stamp(2)));
    assert_eq!(watch::next_step(old, None, old), Step::Same);
    assert_eq!(watch::next_step(old, None, new), Step::Settling(stamp(2)));
    assert_eq!(watch::next_step(old, new, new), Step::Read);
    // Still being written: the stamp moved again.
    assert_eq!(
        watch::next_step(old, new, Some(stamp(3))),
        Step::Settling(stamp(3))
    );
    // Gone for a moment while a program replaces it.
    assert_eq!(watch::next_step(old, new, None), Step::Same);
}

fn fingerprint(byte: u8) -> Fingerprint {
    Fingerprint {
        stamp: stamp(u64::from(byte)),
        hash: [byte; 32],
    }
}

fn state_with_session(status: EditStatus) -> ExternalEditState {
    let state = ExternalEditState::default();
    state.insert(EditSession {
        edit: ExternalEdit {
            id: "one".to_string(),
            tab_id: "tab".to_string(),
            remote_path: "/etc/app.conf".to_string(),
            file_name: "app.conf".to_string(),
            local_path: "/tmp/app.conf".to_string(),
            status,
            error: None,
        },
        dir: PathBuf::from("/tmp"),
        local: PathBuf::from("/tmp/app.conf"),
        baseline: RemoteStamp {
            size: 1,
            mtime: Some(1),
        },
        synced: fingerprint(1),
        observed: Some(fingerprint(1).stamp),
        settling: None,
    });
    state
}

fn settled(byte: u8) -> Vec<(String, Look)> {
    vec![("one".to_string(), Look::Settled(fingerprint(byte)))]
}

#[test]
fn a_save_that_differs_from_the_server_is_reported_each_time() {
    let state = state_with_session(EditStatus::Synced);
    let changed = state.apply_looks(settled(2));
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].status, EditStatus::Modified);
    // Another save with changes of its own is reported again.
    assert_eq!(state.apply_looks(settled(3)).len(), 1);
    // Undoing the changes makes the copy match the server again.
    let changed = state.apply_looks(settled(1));
    assert_eq!(changed[0].status, EditStatus::Synced);
    assert!(state.apply_looks(settled(1)).is_empty());
}

#[test]
fn a_save_after_a_failed_upload_clears_the_error() {
    let state = state_with_session(EditStatus::Synced);
    state.begin_upload("one").unwrap();
    state.upload_stopped("one", Some("broken pipe".to_string()));
    let changed = state.apply_looks(settled(2));
    assert_eq!(changed[0].status, EditStatus::Modified);
    assert_eq!(changed[0].error, None);
}

#[test]
fn uploading_files_are_neither_watched_nor_changed() {
    let state = state_with_session(EditStatus::Synced);
    state.begin_upload("one").unwrap();
    assert!(state.begin_upload("one").is_err());
    assert!(state.watched().unwrap().is_empty());
    assert!(state.apply_looks(settled(2)).is_empty());
    let edit = state
        .uploaded(
            "one",
            RemoteStamp {
                size: 2,
                mtime: None,
            },
            fingerprint(2),
        )
        .unwrap();
    assert_eq!(edit.status, EditStatus::Synced);
    assert!(state.apply_looks(settled(2)).is_empty());
}

#[test]
fn the_watch_ends_once_no_file_is_open() {
    let state = state_with_session(EditStatus::Synced);
    state
        .watching
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(state.watched().is_some());
    state.remove("one");
    assert!(state.watched().is_none());
    assert!(!state.watching.load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn files_with_changes_are_kept_when_their_tab_closes() {
    let state = state_with_session(EditStatus::Synced);
    state.apply_looks(settled(2));
    let sessions = state.remove_tab("tab");
    assert_eq!(sessions.len(), 1);
    assert!(sessions[0].has_changes());
    assert!(state.list("tab").is_empty());
}

#[test]
fn a_save_that_replaces_the_copy_is_seen() {
    let dir = TempDir::new();
    let path = dir.path.join("app.conf");
    std::fs::write(&path, "old").unwrap();
    let synced = local::fingerprint(&path).unwrap();

    // Save the way many editors do: write another file, rename it over.
    let saved = dir.path.join(".app.conf.swp");
    std::fs::write(&saved, "new").unwrap();
    std::fs::rename(&saved, &path).unwrap();
    // The same size: only the content and modification time tell it apart.
    let file = std::fs::File::options().write(true).open(&path).unwrap();
    file.set_modified(SystemTime::now() + Duration::from_secs(5))
        .unwrap();

    let mut watched = Watched {
        id: "one".to_string(),
        local: path.clone(),
        observed: Some(synced.stamp),
        settling: None,
    };
    let Look::Settling(stamp) = watch::look_at(&watched) else {
        panic!("the first look after a save waits for it to hold");
    };
    watched.settling = Some(stamp);
    let Look::Settled(fingerprint) = watch::look_at(&watched) else {
        panic!("the second look reads the save");
    };
    assert_ne!(fingerprint.hash, synced.hash);
}

#[cfg(unix)]
#[test]
fn session_folders_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let root = TempDir::new();
    let dir = local::create_session_dir(&root.path, "one").unwrap();
    let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o700);
    local::remove_stale(&root.path);
    assert!(dir.exists(), "a fresh folder is not stale");
    local::remove_session_dir(&dir);
    assert!(!dir.exists());
}

#[cfg(target_os = "macos")]
#[test]
fn macos_lists_the_applications_for_text_with_icons() {
    let apps = super::apps::apps_for("notes.txt");
    assert!(
        apps.iter().any(|app| app.path.ends_with("TextEdit.app")),
        "TextEdit opens text"
    );
    assert!(apps.iter().filter(|app| app.is_default).count() <= 1);
    let text_edit = apps
        .iter()
        .find(|app| app.path.ends_with("TextEdit.app"))
        .unwrap();
    let icons = super::apps::icons_for(vec![text_edit.path.clone()]);
    assert!(icons[&text_edit.path]
        .as_deref()
        .is_some_and(|icon| icon.starts_with("data:image/png;base64,")));
    // Drawn once, the icon comes with the list from then on.
    let again = super::apps::apps_for("notes.txt");
    assert!(again
        .iter()
        .any(|app| app.path == text_edit.path && app.icon.is_some()));
    // A name without an extension is taken for text.
    assert!(!super::apps::apps_for("Makefile").is_empty());
}
