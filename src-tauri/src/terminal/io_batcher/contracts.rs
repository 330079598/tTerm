//! Raw-channel contracts captured before changing the output endpoint.
use super::*;

#[test]
fn binary_channel_keeps_nul_non_utf8_and_capped_messages() {
    let received = Arc::new(Mutex::new(Vec::new()));
    let captured = received.clone();
    let flow = Arc::new(OutputFlow::default());
    let ack = flow.clone();
    let output = TerminalOutput::with_flow(
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
    let mut sender = TerminalOutputSender::spawn("contract", output);
    let mut expected = vec![b'x'; MAX_CHUNK_BYTES + 1];
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

#[test]
fn flow_waits_at_existing_watermark_then_allows_one_chunk_overshoot() {
    let flow = Arc::new(OutputFlow::with_stall_timeout(Duration::from_secs(60)));
    flow.reserve(HIGH_WATERMARK_BYTES - 1);
    flow.reserve(MAX_CHUNK_BYTES);
    assert_eq!(
        flow.state.lock().unwrap().unacked,
        HIGH_WATERMARK_BYTES - 1 + MAX_CHUNK_BYTES
    );
    flow.ack(usize::MAX);
    assert_eq!(flow.state.lock().unwrap().unacked, 0);
    assert_eq!(ACK_STALL_TIMEOUT, Duration::from_secs(5));
    assert_eq!(BATCH_INTERVAL, Duration::from_millis(8));
    assert_eq!(QUEUE_CAPACITY_MESSAGES, 16);
}
