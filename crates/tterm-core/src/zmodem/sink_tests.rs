use super::*;
use crate::events::{AppEvent, zmodem::ZmodemEvent};
use crate::session::recording::{RecordingSessionSink, SessionRecord};

#[test]
fn arm_publishes_maps_and_trigger_before_send_requested_and_cleans_pipe() {
    let sink = Arc::new(RecordingSessionSink::default());
    let channel = crate::core::channel_test_support::channel(sink.clone());
    let tx = arm_send(&channel, vec![0, 255]);
    let armed = channel
        .zmodem
        .armed_send_map
        .lock()
        .unwrap()
        .remove(&channel.tab_id)
        .unwrap();
    assert_eq!(armed.session_nonce, 6);
    assert_eq!(armed.incoming_rx.try_recv().unwrap(), [0, 255]);
    assert!(
        channel
            .zmodem
            .map
            .read()
            .unwrap()
            .contains_key(&channel.tab_id)
    );
    assert_eq!(
        sink.records(),
        [SessionRecord::Event(AppEvent::Zmodem(
            ZmodemEvent::SendRequested {
                tab_id: channel.tab_id
            }
        ))]
    );
    let mut pipe = ZmodemSendPipe::default();
    pipe.arm(tx);
    assert!(pipe.try_forward(b"binary"));
    assert_eq!(armed.incoming_rx.try_recv().unwrap(), b"binary");
    drop(armed);
    assert!(!pipe.try_forward(b"normal"));
}
