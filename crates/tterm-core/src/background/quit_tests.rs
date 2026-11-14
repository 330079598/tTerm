use super::*;
use std::sync::Mutex;

#[derive(Default)]
struct Sink {
    events: Mutex<Vec<AppEvent>>,
    fail: bool,
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
fn save_event_is_synchronous_and_sent_only_on_the_first_enabled_quit() {
    let state = QuitState::default();
    let sink = Sink::default();
    assert!(!begin_save(&state, &sink));
    assert!(sink.events.lock().unwrap().is_empty());
    state.save_before_quit.store(true, Ordering::SeqCst);
    assert!(begin_save(&state, &sink));
    assert_eq!(
        *sink.events.lock().unwrap(),
        vec![AppEvent::Background(BackgroundEvent::WillQuit)]
    );
    assert!(!begin_save(&state, &sink));
    assert_eq!(sink.events.lock().unwrap().len(), 1);
}

#[test]
fn failed_delivery_allows_exit_and_does_not_rearm_save() {
    let state = QuitState::default();
    state.save_before_quit.store(true, Ordering::SeqCst);
    let sink = Sink {
        fail: true,
        ..Sink::default()
    };
    assert!(!begin_save(&state, &sink));
    assert!(!begin_save(&state, &sink));
    assert_eq!(sink.events.lock().unwrap().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn early_reply_permit_is_kept_and_timeout_remains_1500ms() {
    let state = QuitState::default();
    state.saved.notify_one();
    let start = tokio::time::Instant::now();
    wait_for_save(&state).await;
    assert_eq!(start.elapsed(), Duration::ZERO);
    let wait = wait_for_save(&state);
    tokio::pin!(wait);
    tokio::select! { biased; () = &mut wait => panic!("exited early"), () = tokio::time::sleep(Duration::from_millis(1499)) => {} }
    assert_eq!(start.elapsed(), Duration::from_millis(1499));
    wait.await;
    assert_eq!(start.elapsed(), SAVE_TIMEOUT);
}

#[tokio::test(start_paused = true)]
async fn reply_releases_wait_before_timeout() {
    let state = QuitState::default();
    let reply = async {
        tokio::time::sleep(Duration::from_millis(10)).await;
        state.saved.notify_one();
    };
    let start = tokio::time::Instant::now();
    tokio::join!(wait_for_save(&state), reply);
    assert_eq!(start.elapsed(), Duration::from_millis(10));
}
