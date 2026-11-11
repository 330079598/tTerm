use super::*;

#[test]
fn adapter_close_releases_a_blocked_sender_and_drains_queued_bytes() {
    let captured = Arc::new(Mutex::new(0usize));
    let received = captured.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let output = TerminalOutput::new(Channel::new(move |body| {
        let InvokeResponseBody::Raw(bytes) = body else {
            panic!("expected raw bytes")
        };
        *received.lock().unwrap() += bytes.len();
        tx.send(bytes.len()).unwrap();
        Ok(())
    }));
    let sink = test_sink(output);
    let sender_sink = sink.clone();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let mut sender = TerminalOutputSender::spawn("close-sink", sender_sink);
        for _ in 0..64 {
            sender.send(vec![b'x'; 65536]);
        }
        sender.finish();
        done_tx.send(()).unwrap();
    });
    let mut bytes = 0;
    while bytes < HIGH_WATERMARK_BYTES {
        bytes += rx.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    assert!(done_rx.try_recv().is_err());
    sink.close_output();
    done_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    worker.join().unwrap();
    assert_eq!(*captured.lock().unwrap(), 4 * HIGH_WATERMARK_BYTES);
}
