use super::{reference_dir, should_regenerate};
use std::path::Path;

#[test]
fn a_dirty_frontend_is_never_reused() {
    assert!(should_regenerate(true, false, true));
}

#[test]
fn update_reference_forces_a_capture() {
    assert!(should_regenerate(false, true, true));
}

#[test]
fn a_current_clean_cache_is_reused() {
    assert!(!should_regenerate(false, false, true));
}

#[test]
fn a_missing_or_stale_cache_is_captured() {
    assert!(should_regenerate(false, false, false));
}

#[test]
fn dirty_references_live_in_their_own_directory() {
    let root = Path::new("/tmp/tterm");
    assert_eq!(
        reference_dir(root, "abc123", false),
        root.join("target/parity/reference/abc123")
    );
    assert_eq!(
        reference_dir(root, "abc123", true),
        root.join("target/parity/reference/abc123-dirty")
    );
}
