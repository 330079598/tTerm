use crate::core::state::{ChannelContext, TabZmodem};
use crate::session::recording::{RecordingSessionSink, SessionRecord};
use crate::terminal::io_batcher::TerminalOutputSender;
use crate::terminal::reader::{FilteredOutput, OutputFilter, spawn_reader_thread};
use crate::zmodem::session::ZmodemSettings;
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct HexFilter;
impl OutputFilter for HexFilter {
    fn filter(&mut self, output: Vec<u8>) -> FilteredOutput {
        FilteredOutput {
            display: b"00 FF".to_vec(),
            log: Some(output),
        }
    }
}

fn context(sink: Arc<RecordingSessionSink>) -> ChannelContext {
    ChannelContext {
        sink,
        tab_id: "reader-test".into(),
        session_nonce: 7,
        active: Default::default(),
        zmodem: TabZmodem {
            map: Default::default(),
            armed_send_map: Default::default(),
            manual_override: Default::default(),
        },
        tasks: Default::default(),
    }
}

#[test]
fn serial_filter_logs_original_bytes_and_exits_after_batched_display() {
    let sink = Arc::new(RecordingSessionSink::default());
    let log = Arc::new(Mutex::new(Vec::new()));
    let recorded = log.clone();
    let (exit_tx, mut exit_rx) = tokio::sync::mpsc::unbounded_channel();
    let sender = TerminalOutputSender::spawn("serial-filter", sink.clone());
    spawn_reader_thread(
        Box::new(std::io::Cursor::new(vec![0, 255])),
        context(sink.clone()),
        exit_tx,
        Some(sender),
        ZmodemSettings {
            auto_detect_enabled: false,
            download_dir: std::env::temp_dir(),
        },
        Some(Box::new(HexFilter)),
        Box::new(move |bytes| recorded.lock().unwrap().extend_from_slice(bytes)),
    );
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(5), exit_rx.recv())
            .await
            .unwrap()
            .unwrap();
    });
    assert_eq!(*log.lock().unwrap(), [0, 255]);
    assert_eq!(sink.records(), [SessionRecord::Bytes(b"00 FF".to_vec())]);
}

#[test]
fn reader_without_sender_preserves_lossy_string_fallback() {
    let sink = Arc::new(RecordingSessionSink::default());
    let (exit_tx, mut exit_rx) = tokio::sync::mpsc::unbounded_channel();
    spawn_reader_thread(
        Box::new(std::io::Cursor::new(b"bad:\xff")),
        context(sink.clone()),
        exit_tx,
        None,
        ZmodemSettings {
            auto_detect_enabled: false,
            download_dir: std::env::temp_dir(),
        },
        None,
        Box::new(|_| {}),
    );
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(5), exit_rx.recv())
            .await
            .unwrap()
            .unwrap();
    });
    assert_eq!(
        sink.records(),
        [SessionRecord::Event(crate::events::AppEvent::Terminal(
            crate::events::terminal::TerminalEvent::Output {
                tab_id: "reader-test".into(),
                text: "bad:�".into()
            }
        ))]
    );
}
