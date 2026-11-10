use super::pty_fixture::PtyFixture;
use super::*;

#[test]
fn real_pty_output_pauses_until_ack_then_drains_before_exit() {
    let total = 4 * HIGH_WATERMARK_BYTES;
    let flow = Arc::new(OutputFlow::with_stall_timeout(Duration::from_secs(60)));
    let captured = Arc::new(Mutex::new(Vec::new()));
    let recorded = captured.clone();
    let (delivered_tx, delivered_rx) = std::sync::mpsc::channel();
    let channel = Channel::new(move |body| {
        let InvokeResponseBody::Raw(bytes) = body else {
            panic!("expected raw bytes")
        };
        recorded.lock().unwrap().extend_from_slice(&bytes);
        delivered_tx.send(bytes.len()).unwrap();
        Ok(())
    });
    let output = TerminalOutput::with_flow(channel, flow.clone());
    let fixture = PtyFixture::spawn(&format!(
        "stty -echo -opost; python3 -c 'import sys; sys.stdout.buffer.write(bytes(range(256))*{})'",
        total / 256
    ));
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut fixture = fixture;
        let mut sender = TerminalOutputSender::spawn("real-pty", output);
        let outcome = crate::terminal::reader::run_pty_reader(
            fixture.reader(),
            |_| {},
            |bytes| sender.send(bytes.to_vec()),
            || {},
        );
        sender.finish();
        assert!(fixture.child.wait().unwrap().success());
        done_tx.send(outcome).unwrap();
    });
    // Wait until production reserve reaches its watermark, rather than guessing
    // when the reader and batcher will have been scheduled.
    let mut delivered = 0;
    while delivered < HIGH_WATERMARK_BYTES {
        delivered += delivered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    }
    assert!(delivered < HIGH_WATERMARK_BYTES + MAX_CHUNK_BYTES);
    assert!(done_rx.try_recv().is_err());
    assert!(delivered_rx
        .recv_timeout(Duration::from_millis(50))
        .is_err());
    flow.ack(delivered);
    let mut remaining = total - delivered;
    while remaining > 0 {
        let bytes = delivered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        remaining -= bytes;
        flow.ack(bytes);
    }
    assert_eq!(
        done_rx.recv_timeout(Duration::from_secs(10)).unwrap(),
        crate::terminal::reader::ReaderOutcome::Terminated
    );
    reader.join().unwrap();
    let received = captured.lock().unwrap();
    assert_eq!(received.len(), total);
    assert!(received
        .iter()
        .enumerate()
        .all(|(index, byte)| *byte == (index % 256) as u8));
}
