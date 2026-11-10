use super::*;
struct Sink<'a> {
    state: &'a LaunchDirs,
}
impl EventSink for Sink<'_> {
    fn emit(&self, event: AppEvent) -> Result<(), String> {
        assert_eq!(event, AppEvent::LaunchDir(LaunchDirEvent::Queued));
        let dirs = self
            .state
            .0
            .try_lock()
            .expect("queue lock released before emit");
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].path, "/test");
        Err("closed".into())
    }
}
#[test]
fn queue_is_complete_and_unlocked_before_event_even_when_delivery_fails() {
    let state = LaunchDirs::default();
    queue_with_events(
        &state,
        vec![LaunchDir {
            path: "/test".into(),
            exists: true,
        }],
        &Sink { state: &state },
    );
    assert_eq!(state.0.lock().unwrap().len(), 1);
}
