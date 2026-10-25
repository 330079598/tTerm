use super::*;

#[test]
fn utf8_prefix_keeps_complete_sequences() {
    assert_eq!(utf8_complete_prefix_len(b"abc"), 3);
    assert_eq!(utf8_complete_prefix_len("中文".as_bytes()), 6);
}

#[test]
fn utf8_prefix_defers_partial_trailing_sequence() {
    // "中" is E4 B8 AD; feeding only its first two bytes must hold both.
    let partial = &"中".as_bytes()[..2];
    assert_eq!(utf8_complete_prefix_len(partial), 0);
    // A complete char followed by a partial one keeps only the complete part.
    let mixed = [b'a', 0xe4, 0xb8];
    assert_eq!(utf8_complete_prefix_len(&mixed), 1);
}

#[test]
fn utf8_prefix_handles_four_byte_sequences() {
    let emoji = "😀".as_bytes(); // F0 9F 98 80
    assert_eq!(utf8_complete_prefix_len(&emoji[..3]), 0);
    assert_eq!(utf8_complete_prefix_len(emoji), 4);
}

#[test]
fn utf8_prefix_is_fine_for_plain_ascii_and_controls() {
    let data = b"hello\r\n\x1b[31mworld\x1b[0m";
    assert_eq!(utf8_complete_prefix_len(data), data.len());
}

#[test]
fn utf8_prefix_never_wedges_on_malformed_bytes() {
    // Stray continuation bytes must flush instead of buffering forever.
    assert_eq!(utf8_complete_prefix_len(&[0x80, 0x80, 0x80]), 3);
    // Truncated 4-byte lead followed by garbage also flushes.
    assert_eq!(utf8_complete_prefix_len(&[0xf0, 0x9f, 0x41]), 3);
}

struct ChunkRecorder {
    chunks: std::sync::Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
}

impl ChunkRecorder {
    /// Acknowledges every chunk as soon as it arrives, like a webview
    /// that parses instantly.
    fn channel(&self) -> TerminalOutput {
        let chunks = self.chunks.clone();
        let flow = Arc::new(OutputFlow::default());
        let ack = flow.clone();
        let channel = Channel::new(move |body: InvokeResponseBody| {
            if let InvokeResponseBody::Raw(bytes) = body {
                ack.ack(bytes.len());
                chunks.lock().unwrap().push(bytes);
            }
            Ok(())
        });
        TerminalOutput::with_flow(channel, flow)
    }
}

/// Records chunks without acknowledging them, like a webview that has
/// fallen behind.
fn unacked_output(stall_timeout: Duration) -> (TerminalOutput, Arc<Mutex<Vec<Vec<u8>>>>) {
    let chunks: Arc<Mutex<Vec<Vec<u8>>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = chunks.clone();
    let channel = Channel::new(move |body: InvokeResponseBody| {
        if let InvokeResponseBody::Raw(bytes) = body {
            sink.lock().unwrap().push(bytes);
        }
        Ok(())
    });
    let flow = Arc::new(OutputFlow::with_stall_timeout(stall_timeout));
    (TerminalOutput::with_flow(channel, flow), chunks)
}

fn delivered(chunks: &Mutex<Vec<Vec<u8>>>) -> usize {
    chunks.lock().unwrap().iter().map(Vec::len).sum()
}

fn feed(mut sender: TerminalOutputSender, total: usize) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        for _ in 0..total / 4096 {
            sender.send(vec![b'x'; 4096]);
        }
        sender.finish();
    })
}

#[test]
fn sender_pauses_at_high_watermark_until_acknowledged() {
    let (output, chunks) = unacked_output(Duration::from_secs(60));
    let flow = output.flow().clone();
    let total = HIGH_WATERMARK_BYTES * 3;
    let feeder = feed(TerminalOutputSender::spawn("flow", output), total);

    std::thread::sleep(Duration::from_millis(200));
    let paused_at = delivered(&chunks);
    assert!(
        paused_at >= HIGH_WATERMARK_BYTES,
        "paused early at {paused_at}"
    );
    assert!(
        paused_at < HIGH_WATERMARK_BYTES + MAX_CHUNK_BYTES,
        "ran past the watermark"
    );

    // Acknowledge everything delivered so far, as the webview would.
    let mut acked = 0;
    while !feeder.is_finished() {
        let now = delivered(&chunks);
        flow.ack(now - acked);
        acked = now;
        std::thread::sleep(Duration::from_millis(5));
    }
    feeder.join().unwrap();
    assert_eq!(delivered(&chunks), total);
}

#[test]
fn close_releases_a_paused_sender() {
    let (output, chunks) = unacked_output(Duration::from_secs(60));
    let flow = output.flow().clone();
    let total = HIGH_WATERMARK_BYTES * 2;
    let feeder = feed(TerminalOutputSender::spawn("flow-close", output), total);

    std::thread::sleep(Duration::from_millis(100));
    assert!(delivered(&chunks) < total);
    flow.close();
    feeder.join().unwrap();
    assert_eq!(delivered(&chunks), total);
}

#[test]
fn moving_to_a_new_page_resumes_output_there() {
    // The old page stopped acknowledging; the sender waits on it.
    let (output, old_page) = unacked_output(Duration::from_secs(60));
    let (new_output, new_page) = unacked_output(Duration::from_secs(60));
    let total = HIGH_WATERMARK_BYTES * 2;
    let feeder = feed(
        TerminalOutputSender::spawn("flow-move", output.clone()),
        total,
    );
    std::thread::sleep(Duration::from_millis(100));
    let stranded = delivered(&old_page);
    assert!(stranded < total);

    output.move_to(&new_output);
    let flow = output.flow().clone();
    let mut acked = 0;
    while !feeder.is_finished() {
        let now = delivered(&new_page);
        flow.ack(now - acked);
        acked = now;
        std::thread::sleep(Duration::from_millis(5));
    }
    feeder.join().unwrap();
    assert_eq!(delivered(&old_page), stranded);
    assert_eq!(stranded + delivered(&new_page), total);
}

#[test]
fn stalled_acknowledgements_do_not_wedge_the_sender() {
    let (output, chunks) = unacked_output(Duration::from_millis(50));
    let total = HIGH_WATERMARK_BYTES * 2;
    feed(TerminalOutputSender::spawn("flow-stall", output), total)
        .join()
        .unwrap();
    assert_eq!(delivered(&chunks), total);
}

#[test]
fn flush_all_splits_large_batches_into_capped_chunks() {
    let recorder = ChunkRecorder {
        chunks: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
    };
    let channel = recorder.channel();
    let mut pending = vec![b'x'; MAX_CHUNK_BYTES + 10];
    flush_all(&mut pending, &channel);

    let chunks = recorder.chunks.lock().unwrap();
    assert!(chunks.len() >= 2);
    assert!(chunks.iter().all(|chunk| chunk.len() <= MAX_CHUNK_BYTES));
    assert!(pending.is_empty());
}

#[test]
fn flush_all_never_emits_partial_utf8_across_chunks() {
    let recorder = ChunkRecorder {
        chunks: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
    };
    let channel = recorder.channel();
    // ~600k bytes of "中": crosses the MAX_CHUNK boundary mid-character.
    let mut pending = Vec::new();
    for _ in 0..200_003 {
        pending.extend_from_slice("中".as_bytes());
    }
    flush_all(&mut pending, &channel);

    let chunks = recorder.chunks.lock().unwrap();
    assert!(chunks.len() >= 2);
    for chunk in chunks.iter() {
        assert!(std::str::from_utf8(chunk).is_ok(), "chunk split mid-UTF-8");
    }
    assert!(pending.is_empty());
}

#[test]
fn flush_all_buffers_partial_trailing_sequence() {
    let recorder = ChunkRecorder {
        chunks: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
    };
    let channel = recorder.channel();
    let mut pending = vec![b'a', b'b', 0xe4, 0xb8];
    flush_all(&mut pending, &channel);

    assert_eq!(pending, vec![0xe4, 0xb8]);
    let chunks = recorder.chunks.lock().unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], b"ab");
}

#[test]
fn finish_flushes_queued_bytes_in_order() {
    let recorder = ChunkRecorder {
        chunks: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
    };
    let mut sender = TerminalOutputSender::spawn("test-tab", recorder.channel());
    sender.send(b"hello ".to_vec());
    sender.send("世界".as_bytes().to_vec());
    sender.finish();

    let chunks = recorder.chunks.lock().unwrap();
    let joined: Vec<u8> = chunks.concat();
    assert_eq!(joined, "hello 世界".as_bytes());
}

#[test]
fn finish_waits_for_blocked_reader_sends() {
    let recorder = ChunkRecorder {
        chunks: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
    };
    let mut sender = TerminalOutputSender::spawn("test-tab", recorder.channel());
    // Far more than the queue capacity: some sends must block until the
    // worker drains them, proving finish() flushes everything in order.
    for i in 0..1000 {
        sender.send(vec![b'a' + (i % 26) as u8; 1024]);
    }
    sender.finish();

    let chunks = recorder.chunks.lock().unwrap();
    let joined: Vec<u8> = chunks.concat();
    assert_eq!(joined.len(), 1000 * 1024);
    for (index, byte) in joined.iter().enumerate() {
        let round = index / 1024;
        assert_eq!(
            *byte,
            b'a' + (round % 26) as u8,
            "byte {index} out of order"
        );
    }
}

#[test]
fn send_after_finish_is_a_noop() {
    let recorder = ChunkRecorder {
        chunks: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
    };
    let mut sender = TerminalOutputSender::spawn("test-tab", recorder.channel());
    sender.send(b"first".to_vec());
    sender.finish();
    sender.send(b"dropped".to_vec());

    let chunks = recorder.chunks.lock().unwrap();
    let joined: Vec<u8> = chunks.concat();
    assert_eq!(joined, b"first");
}

#[test]
fn rapid_messages_coalesce_into_fewer_chunks() {
    let recorder = ChunkRecorder {
        chunks: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
    };
    let mut sender = TerminalOutputSender::spawn("coalesce", recorder.channel());
    // The first goes out at once; the other nine land inside the window it
    // opens, so they must reach the webview as fewer IPC messages than reads.
    let mut expected = Vec::new();
    for i in 0..10 {
        let message = format!("message-{i:02} ").into_bytes();
        expected.extend_from_slice(&message);
        sender.send(message);
    }
    sender.finish();

    let chunks = recorder.chunks.lock().unwrap();
    assert!(
        chunks.len() < 10,
        "expected coalescing, got {} chunks",
        chunks.len()
    );
    assert_eq!(chunks.concat(), expected);
}

#[test]
fn output_after_a_quiet_spell_skips_the_batching_window() {
    let (delivered_tx, delivered_rx) = std::sync::mpsc::channel::<Instant>();
    let delivered_tx = Mutex::new(delivered_tx);
    let channel = Channel::new(move |_body: InvokeResponseBody| {
        let _ = delivered_tx.lock().unwrap().send(Instant::now());
        Ok(())
    });
    let mut sender = TerminalOutputSender::spawn("echo", TerminalOutput::new(channel));
    // Each echo follows a pause longer than the window, like keystrokes, so
    // each must reach the webview well before a window could have elapsed.
    for _ in 0..3 {
        std::thread::sleep(BATCH_INTERVAL * 2);
        let sent_at = Instant::now();
        sender.send(b"x".to_vec());
        let delivered_at = delivered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("echo was never delivered");
        assert!(
            delivered_at - sent_at < BATCH_INTERVAL,
            "echo waited {:?} for the batching window",
            delivered_at - sent_at
        );
    }
    sender.finish();
}

#[test]
fn steady_trickle_flushes_without_waiting_for_disconnect() {
    let recorder = ChunkRecorder {
        chunks: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
    };
    let mut sender = TerminalOutputSender::spawn("trickle", recorder.channel());
    // Gaps below BATCH_INTERVAL keep a self-resetting recv_timeout from
    // ever firing; the window deadline must flush on its own anyway.
    let mut expected = Vec::new();
    for i in 0..10 {
        let message = format!("tick-{i} ").into_bytes();
        expected.extend_from_slice(&message);
        sender.send(message);
        std::thread::sleep(Duration::from_millis(2));
    }
    std::thread::sleep(BATCH_INTERVAL * 4);

    let delivered_before_finish = recorder.chunks.lock().unwrap().len();
    sender.finish();
    assert!(
        delivered_before_finish > 0,
        "trickle starved the webview until finish()"
    );
    assert_eq!(recorder.chunks.lock().unwrap().concat(), expected);
}

/// End-to-end: a fake PTY reader whose reads split multi-byte characters
/// at arbitrary boundaries must surface as one ordered, valid UTF-8 stream
/// through the batching sender, and the reader must only report
/// termination after every byte has been delivered.
#[test]
fn reader_thread_delivers_batched_utf8_stream_in_order() {
    use std::io::Read;

    struct FragmentedReader {
        data: Vec<u8>,
        // Read sizes chosen to slice through the middle of sequences.
        sizes: Vec<usize>,
        cursor: usize,
        size_cursor: usize,
    }

    impl Read for FragmentedReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.cursor >= self.data.len() {
                return Ok(0);
            }
            let want = self.sizes[self.size_cursor % self.sizes.len()];
            self.size_cursor += 1;
            let take = want.min(buf.len()).min(self.data.len() - self.cursor);
            buf[..take].copy_from_slice(&self.data[self.cursor..self.cursor + take]);
            self.cursor += take;
            std::thread::sleep(Duration::from_millis(1));
            Ok(take)
        }
    }

    let mut expected = String::new();
    for round in 0..400 {
        expected.push_str(&format!("line-{round}: 中文测试 😀 tail\r\n"));
    }

    let received: std::sync::Arc<std::sync::Mutex<Vec<u8>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink_received = received.clone();
    let channel = TerminalOutput::new(Channel::new(move |body| {
        if let InvokeResponseBody::Raw(bytes) = body {
            sink_received.lock().unwrap().extend_from_slice(&bytes);
        }
        Ok(())
    }));

    let reader = FragmentedReader {
        data: expected.as_bytes().to_vec(),
        // 7 deliberately lands inside multi-byte sequences.
        sizes: vec![3, 7, 13, 1, 64],
        cursor: 0,
        size_cursor: 0,
    };

    let sender = std::sync::Mutex::new(Some(TerminalOutputSender::spawn("e2e", channel)));
    let outcome = super::super::reader::run_pty_reader(
        reader,
        |_| {},
        |chunk: &[u8]| {
            if let Some(sender) = sender.lock().unwrap().as_mut() {
                sender.send(chunk.to_vec());
            }
        },
        || {
            if let Some(sender) = sender.lock().unwrap().as_mut() {
                sender.finish();
            }
        },
    );

    assert_eq!(outcome, super::super::reader::ReaderOutcome::Terminated);

    let received = received.lock().unwrap().clone();
    let text = String::from_utf8(received).expect("delivered stream is valid UTF-8");
    assert_eq!(text, expected);
}
