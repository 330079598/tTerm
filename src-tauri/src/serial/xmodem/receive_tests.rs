use super::disk::{safe_file_name, unused_path};
use super::protocol::{
    crc16, send_xmodem, send_ymodem, Link, OutgoingFile, Received, Result, TransferError,
};
use super::receive::{receive_xmodem, receive_ymodem, FileSink};
use super::XmodemProtocol;
use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

/// One end of an in-memory line, for running a sender against a receiver.
struct Pipe {
    incoming: Receiver<u8>,
    outgoing: Sender<u8>,
}

fn pipe_pair() -> (Pipe, Pipe) {
    let (to_a, from_b) = channel();
    let (to_b, from_a) = channel();
    (
        Pipe {
            incoming: from_b,
            outgoing: to_b,
        },
        Pipe {
            incoming: from_a,
            outgoing: to_a,
        },
    )
}

impl Link for Pipe {
    fn read(&mut self, timeout: Duration) -> Received {
        match self.incoming.recv_timeout(timeout) {
            Ok(byte) => Received::Byte(byte),
            Err(RecvTimeoutError::Timeout) => Received::TimedOut,
            Err(RecvTimeoutError::Disconnected) => Received::Closed,
        }
    }

    fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        for &byte in bytes {
            let _ = self.outgoing.send(byte);
        }
        Ok(())
    }

    fn cancelled(&self) -> bool {
        false
    }
}

/// Keeps received files in memory.
#[derive(Default)]
struct MemorySink {
    files: Vec<(Option<String>, Option<u64>, Vec<u8>)>,
    closed: usize,
}

impl FileSink for MemorySink {
    fn open(&mut self, name: Option<&str>, size: Option<u64>) -> Result<()> {
        self.files
            .push((name.map(str::to_string), size, Vec::new()));
        Ok(())
    }

    fn write(&mut self, data: &[u8]) -> Result<()> {
        self.files.last_mut().unwrap().2.extend_from_slice(data);
        Ok(())
    }

    fn close(&mut self) -> Result<()> {
        self.closed += 1;
        Ok(())
    }

    fn progress(&mut self, _received: u64) {}
}

fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|index| (index * 7 % 256) as u8).collect()
}

fn outgoing(name: &str, data: Vec<u8>) -> OutgoingFile<Cursor<Vec<u8>>> {
    OutgoingFile {
        name: name.to_string(),
        size: data.len() as u64,
        modified: None,
        data: Cursor::new(data),
    }
}

/// Sends `data` with `protocol` from one thread to the receiver in this one.
fn xmodem_round_trip(protocol: XmodemProtocol, data: Vec<u8>) -> MemorySink {
    let (mut sender, mut receiver) = pipe_pair();
    let sending = std::thread::spawn(move || {
        let mut file = outgoing("fw.bin", data);
        send_xmodem(&mut sender, protocol, &mut file, &mut |_| {})
    });
    let mut sink = MemorySink::default();
    receive_xmodem(&mut receiver, &mut sink).unwrap();
    sending.join().unwrap().unwrap();
    sink
}

#[test]
fn xmodem_files_arrive_as_sent() {
    for (protocol, len) in [
        (XmodemProtocol::Xmodem, 300),
        (XmodemProtocol::Xmodem, 128),
        (XmodemProtocol::Xmodem1k, 2100),
    ] {
        // Padding is trimmed, so the data must not end in SUBs of its own.
        let data = pattern(len);
        let sink = xmodem_round_trip(protocol, data.clone());
        assert_eq!(sink.files[0].2, data, "{protocol:?} with {len} bytes");
        assert_eq!(sink.closed, 1);
    }
}

#[test]
fn ymodem_batches_arrive_with_their_names_and_exact_sizes() {
    let (mut sender, mut receiver) = pipe_pair();
    let ending_in_sub = [pattern(1030), vec![0x1a; 3]].concat();
    let expected = ending_in_sub.clone();
    let sending = std::thread::spawn(move || {
        let mut files = vec![
            outgoing("boot.bin", ending_in_sub),
            outgoing("empty.txt", Vec::new()),
            outgoing("app.bin", pattern(90)),
        ];
        send_ymodem(&mut sender, &mut files, &mut |_, _| {})
    });
    let mut sink = MemorySink::default();
    receive_ymodem(&mut receiver, &mut sink).unwrap();
    sending.join().unwrap().unwrap();
    let names: Vec<_> = sink.files.iter().map(|file| file.0.as_deref()).collect();
    assert_eq!(
        names,
        [Some("boot.bin"), Some("empty.txt"), Some("app.bin")]
    );
    // YMODEM's size keeps data that happens to end in SUBs.
    assert_eq!(sink.files[0].2, expected);
    assert!(sink.files[1].2.is_empty());
    assert_eq!(sink.files[2].2, pattern(90));
    assert_eq!(sink.closed, 3);
}

/// A sender that plays back a script one turn at a time, each turn after
/// the receiver says something, and records what it says. Time passes only
/// when it has nothing to say.
struct ScriptedSender {
    turns: VecDeque<Vec<u8>>,
    pending: VecDeque<u8>,
    said: Vec<u8>,
    clock: Instant,
}

impl ScriptedSender {
    fn new(turns: Vec<Vec<u8>>) -> Self {
        Self {
            turns: turns.into(),
            pending: VecDeque::new(),
            said: Vec::new(),
            clock: Instant::now(),
        }
    }
}

impl Link for ScriptedSender {
    fn read(&mut self, timeout: Duration) -> Received {
        match self.pending.pop_front() {
            Some(byte) => Received::Byte(byte),
            None => {
                self.clock += timeout.max(Duration::from_millis(1));
                Received::TimedOut
            }
        }
    }

    fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.said.extend_from_slice(bytes);
        if self.pending.is_empty() {
            if let Some(turn) = self.turns.pop_front() {
                self.pending.extend(turn);
            }
        }
        Ok(())
    }

    fn cancelled(&self) -> bool {
        false
    }

    fn now(&self) -> Instant {
        self.clock
    }
}

fn block(number: u8, fill: u8) -> Vec<u8> {
    let data = vec![fill; 128];
    let mut frame = vec![0x01, number, !number];
    frame.extend_from_slice(&data);
    frame.extend_from_slice(&crc16(&data).to_be_bytes());
    frame
}

#[test]
fn a_broken_block_is_asked_for_again_and_a_repeated_one_skipped() {
    let mut broken = block(1, b'a');
    broken[10] ^= 0xff;
    let mut sender = ScriptedSender::new(vec![
        broken,
        block(1, b'a'),
        block(1, b'a'),
        block(2, b'b'),
        vec![0x04],
    ]);
    let mut sink = MemorySink::default();
    receive_xmodem(&mut sender, &mut sink).unwrap();
    assert_eq!(sink.files[0].2, [vec![b'a'; 128], vec![b'b'; 128]].concat());
    // C to start, NAK for the broken block, ACK for each of the rest and the EOT.
    assert_eq!(sender.said, [b'C', 0x15, 0x06, 0x06, 0x06, 0x06]);
}

#[test]
fn a_device_cancelling_stops_the_receive() {
    let mut sender = ScriptedSender::new(vec![block(1, b'a'), vec![0x18, 0x18]]);
    let result = receive_xmodem(&mut sender, &mut MemorySink::default());
    assert_eq!(result, Err(TransferError::CancelledByDevice));
}

#[test]
fn a_block_out_of_order_stops_the_receive() {
    let mut sender = ScriptedSender::new(vec![block(1, b'a'), block(3, b'c')]);
    let result = receive_xmodem(&mut sender, &mut MemorySink::default());
    assert_eq!(result, Err(TransferError::OutOfSequence));
}

#[test]
fn a_device_that_never_sends_is_given_up_on_falling_back_to_checksums() {
    let mut sender = ScriptedSender::new(Vec::new());
    let result = receive_xmodem(&mut sender, &mut MemorySink::default());
    assert_eq!(result, Err(TransferError::NoSender));
    assert!(sender.said.starts_with(b"CCCC\x15"));
}

#[test]
fn names_from_the_device_stay_in_the_folder() {
    assert_eq!(safe_file_name("fw.bin"), "fw.bin");
    assert_eq!(safe_file_name("../../etc/passwd"), "passwd");
    assert_eq!(safe_file_name("C:\\boot\\a:b?.bin"), "a_b_.bin");
    assert_eq!(safe_file_name(".."), "received.bin");
    assert_eq!(safe_file_name(""), "received.bin");
}

#[test]
fn a_taken_name_gets_a_number() {
    let folder = std::env::temp_dir().join(format!("tterm-xmodem-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&folder).unwrap();
    assert_eq!(unused_path(&folder, "fw.bin"), folder.join("fw.bin"));
    std::fs::write(folder.join("fw.bin"), b"").unwrap();
    std::fs::write(folder.join("fw (2).bin"), b"").unwrap();
    assert_eq!(unused_path(&folder, "fw.bin"), folder.join("fw (3).bin"));
    assert_eq!(unused_path(&folder, "README"), folder.join("README"));
    std::fs::remove_dir_all(&folder).unwrap();
}
