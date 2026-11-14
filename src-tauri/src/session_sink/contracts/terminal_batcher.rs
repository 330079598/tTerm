//! The raw binary channel contract captured before the output endpoint moved.
use std::sync::{Arc, Mutex};
use tauri::ipc::{Channel, InvokeResponseBody};
use tterm_core::terminal::io_batcher::{OutputFlow, TerminalOutputSender, test_sink};

#[test]
fn binary_channel_keeps_nul_non_utf8_and_capped_messages() {
    let received = Arc::new(Mutex::new(Vec::new()));
    let captured = received.clone();
    let flow = Arc::new(OutputFlow::default());
    let ack = flow.clone();
    let output = crate::session_sink::output::with_flow(
        Channel::new(move |body| {
            let InvokeResponseBody::Raw(bytes) = body else {
                panic!("terminal output changed from Raw to JSON");
            };
            ack.ack(bytes.len());
            captured.lock().unwrap().push(bytes);
            Ok(())
        }),
        flow,
    );
    let mut sender = TerminalOutputSender::spawn("contract", test_sink(output));
    let mut expected = vec![b'x'; 512 * 1024 + 1];
    expected.extend_from_slice(b"\0\xff\x1b[31m\r\n");
    expected.extend_from_slice("中文😀".as_bytes());
    sender.send(Vec::new());
    sender.send(expected.clone());
    sender.finish();
    let chunks = received.lock().unwrap();
    assert!(chunks.len() >= 2);
    assert!(chunks.iter().all(|bytes| bytes.len() <= 512 * 1024));
    assert_eq!(chunks.concat(), expected);
}
