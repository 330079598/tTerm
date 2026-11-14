use crate::background::*;
use std::sync::Mutex;

struct Sink {
    fail: bool,
    events: Mutex<Vec<AppEvent>>,
}
impl EventSink for Sink {
    fn emit(&self, event: AppEvent) -> Result<(), String> {
        self.events.lock().unwrap().push(event);
        if self.fail {
            Err("closed".into())
        } else {
            Ok(())
        }
    }
}
#[test]
fn close_failure_rearms_prompt_and_success_keeps_it_open() {
    for fail in [false, true] {
        let state = CloseBehaviorState::default();
        let sink = Sink {
            fail,
            events: Mutex::new(Vec::new()),
        };
        assert!(state.should_ask());
        assert_eq!(emit_close_request(&state, &sink), !fail);
        assert_eq!(state.prompt_open.load(Ordering::SeqCst), !fail);
        assert_eq!(
            *sink.events.lock().unwrap(),
            vec![AppEvent::Background(BackgroundEvent::CloseRequested)]
        );
    }
}
