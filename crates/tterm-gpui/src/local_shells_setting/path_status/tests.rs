//! Unit tests for the path-check generations and cached answers.

use super::*;

fn paths(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| value.to_string()).collect()
}

#[test]
fn a_visit_checks_each_saved_path_once_per_change() {
    let mut status = PathStatus::new();
    status.open();
    let saved = paths(&[" /bin/nu ", "/bin/nu", "", "/bin/zsh"]);
    assert_eq!(
        status.request_saved(&saved),
        paths(&["/bin/nu", "/bin/zsh"]),
        "the same trimmed path is checked once, and blank ones not at all"
    );
    assert!(
        status.request_saved(&saved).is_empty(),
        "an unrelated config change must not check again"
    );
    let saved = paths(&["/bin/nu", "/bin/zsh", "/bin/fish"]);
    assert_eq!(
        status.request_saved(&saved),
        paths(&["/bin/fish", "/bin/nu", "/bin/zsh"]),
        "a changed list asks again for every path it holds"
    );
}

#[test]
fn a_reply_from_an_older_saved_batch_is_ignored() {
    let mut status = PathStatus::new();
    status.open();
    let first = status.request_saved(&paths(&["/bin/nu"]));
    assert_eq!(first, paths(&["/bin/nu"]));
    let first_generation = status.saved_generation();

    let second = status.request_saved(&paths(&["/bin/nu", "/bin/zsh"]));
    assert_eq!(second, paths(&["/bin/nu", "/bin/zsh"]));
    assert!(!status.apply_saved(first_generation, "/bin/nu", true));
    assert!(!status.saved_missing("/bin/nu"));

    let current = status.saved_generation();
    assert!(status.apply_saved(current, "/bin/nu", false));
    assert!(status.saved_missing("/bin/nu"));
}

#[test]
fn a_missing_saved_path_becomes_found_when_a_later_answer_finds_it() {
    let mut status = PathStatus::new();
    status.open();
    status.request_saved(&paths(&["/bin/nu"]));
    let missing = status.saved_generation();
    assert!(status.apply_saved(missing, "/bin/nu", false));
    assert!(status.saved_missing("/bin/nu"));

    status.request_saved(&paths(&["/bin/nu", "/bin/zsh"]));
    let found = status.saved_generation();
    assert!(status.apply_saved(found, "/bin/nu", true));
    assert!(!status.saved_missing("/bin/nu"));
}

#[test]
fn the_editor_checks_a_nonblank_path_and_ignores_a_stale_answer() {
    let mut status = PathStatus::new();
    status.open();
    let (generation, path) = status.edit(" /opt/homebrew/bin/fish ").expect("a check");
    assert_eq!(path, "/opt/homebrew/bin/fish");
    assert_eq!(status.editor_status(), &EditorStatus::Checking);

    let (newer, newer_path) = status.edit("/bin/zsh").expect("a new check");
    assert_eq!(newer_path, "/bin/zsh");
    assert!(
        !status.apply_editor(generation, &path, Ok("/opt/homebrew/bin/fish".to_string())),
        "the answer for the old text must not replace the new one"
    );
    assert_eq!(status.editor_status(), &EditorStatus::Checking);

    assert!(status.apply_editor(newer, &newer_path, Ok("/bin/zsh".to_string())));
    assert_eq!(
        status.editor_status(),
        &EditorStatus::Found("/bin/zsh".to_string())
    );
}

#[test]
fn the_editor_reports_a_missing_path_and_rechecks_it_when_it_is_found_later() {
    let mut status = PathStatus::new();
    status.open();
    let (generation, path) = status.edit("/bin/nu").expect("a check");
    assert!(status.apply_editor(generation, &path, Err("not found".to_string())));
    assert_eq!(status.editor_status(), &EditorStatus::NotFound);

    let (generation, path) = status.edit("/bin/nu").expect("a recheck");
    assert!(status.apply_editor(generation, &path, Ok("/bin/nu".to_string())));
    assert_eq!(
        status.editor_status(),
        &EditorStatus::Found("/bin/nu".to_string())
    );
}

#[test]
fn blank_text_stops_the_editor_check() {
    let mut status = PathStatus::new();
    status.open();
    let (generation, path) = status.edit("/bin/nu").expect("a check");
    assert!(status.edit("   ").is_none());
    assert_eq!(status.editor_status(), &EditorStatus::Idle);
    assert!(!status.apply_editor(generation, &path, Ok("/bin/nu".to_string())));
}

#[test]
fn closing_the_page_drops_every_pending_answer() {
    let mut status = PathStatus::new();
    status.open();
    status.request_saved(&paths(&["/bin/nu"]));
    let saved_generation = status.saved_generation();
    let (editor_generation, path) = status.edit("/bin/zsh").expect("a check");

    status.close();
    assert!(!status.apply_saved(saved_generation, "/bin/nu", false));
    assert!(!status.saved_missing("/bin/nu"));
    assert!(!status.apply_editor(editor_generation, &path, Ok("/bin/zsh".to_string())));
    assert_eq!(status.editor_status(), &EditorStatus::Idle);
    assert!(status.request_saved(&paths(&["/bin/nu"])).is_empty());
}
