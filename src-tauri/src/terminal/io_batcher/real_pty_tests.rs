use super::pty_fixture::PtyFixture;
use super::*;
use tauri::ipc::{Channel, InvokeResponseBody};
use tterm_core::session::recording::{RecordingSessionSink, SessionRecord};
use tterm_core::session::sink::SessionSink;
struct ReaderGuard {
    flow: Arc<OutputFlow>,
    reader: Option<std::thread::JoinHandle<()>>,
}

impl Drop for ReaderGuard {
    fn drop(&mut self) {
        self.flow.close();
        if let Some(reader) = self.reader.take() {
            reader.join().unwrap();
        }
    }
}

#[test]
fn real_pty_output_pauses_until_ack_then_drains_before_exit() {
    let total = 4 * HIGH_WATERMARK_BYTES;
    let flow = Arc::new(OutputFlow::with_stall_timeout(Duration::from_secs(60)));
    let captured = Arc::new(RecordingSessionSink::default());
    let recorded = captured.clone();
    let (delivered_tx, delivered_rx) = std::sync::mpsc::channel();
    let channel = Channel::new(move |body| {
        let InvokeResponseBody::Raw(bytes) = body else {
            panic!("expected raw bytes")
        };
        recorded.send_bytes(bytes.clone()).unwrap();
        delivered_tx.send(bytes.len()).unwrap();
        Ok(())
    });
    let output = crate::session_sink::output::with_flow(channel, flow.clone());
    let fixture = PtyFixture::spawn(&format!(
        "stty -echo -opost; python3 -c 'import sys; sys.stdout.buffer.write(bytes(range(256))*{})'",
        total / 256
    ));
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut fixture = fixture;
        let mut sender = TerminalOutputSender::spawn("real-pty", test_sink(output));
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
    let mut guard = ReaderGuard {
        flow: flow.clone(),
        reader: Some(reader),
    };
    // Wait until production reserve reaches its watermark, rather than guessing
    // when the reader and batcher will have been scheduled.
    let mut delivered = 0;
    while delivered < HIGH_WATERMARK_BYTES {
        delivered += delivered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    }
    assert!(delivered < HIGH_WATERMARK_BYTES + MAX_CHUNK_BYTES);
    assert!(done_rx.try_recv().is_err());
    assert!(
        delivered_rx
            .recv_timeout(Duration::from_millis(50))
            .is_err()
    );
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
    guard.reader.take().unwrap().join().unwrap();
    let received: Vec<u8> = captured
        .records()
        .into_iter()
        .flat_map(|record| match record {
            SessionRecord::Bytes(bytes) => bytes,
            _ => panic!("unexpected implicit acknowledgement in recording sink"),
        })
        .collect();
    assert_eq!(received.len(), total);
    assert!(
        received
            .iter()
            .enumerate()
            .all(|(index, byte)| *byte == (index % 256) as u8)
    );
}
