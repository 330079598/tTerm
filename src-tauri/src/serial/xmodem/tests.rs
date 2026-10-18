use super::protocol::{
    crc16, send_xmodem, send_ymodem, Link, OutgoingFile, Received, TransferError,
};
use super::XmodemProtocol;
use std::collections::VecDeque;
use std::io::Cursor;
use std::time::{Duration, Instant};

const SOH: u8 = 0x01;
const STX: u8 = 0x02;
const EOT: u8 = 0x04;
const ACK: u8 = 0x06;
const NAK: u8 = 0x15;
const CAN: u8 = 0x18;

/// A receiver that answers what the sender writes the way a bootloader
/// does, and keeps what it took. Time passes only when it has nothing to say.
struct Receiver {
    ymodem: bool,
    crc: bool,
    replies: VecDeque<u8>,
    clock: Instant,
    /// Blocks to refuse once, by how many blocks came before.
    refuse: Vec<usize>,
    blocks_seen: usize,
    eots_seen: usize,
    /// Files taken: name and data, the data cut to the header's size.
    files: Vec<(String, Vec<u8>)>,
    expected_size: Option<usize>,
    cancel_after_blocks: Option<usize>,
    user_cancelled: bool,
}

impl Receiver {
    fn new(ymodem: bool, crc: bool) -> Self {
        Self {
            ymodem,
            crc,
            replies: VecDeque::from([if crc { b'C' } else { NAK }]),
            clock: Instant::now(),
            refuse: Vec::new(),
            blocks_seen: 0,
            eots_seen: 0,
            files: Vec::new(),
            expected_size: None,
            cancel_after_blocks: None,
            user_cancelled: false,
        }
    }

    fn silent() -> Self {
        let mut receiver = Self::new(false, true);
        receiver.replies.clear();
        receiver
    }

    fn take_block(&mut self, frame: &[u8]) {
        let size = if frame[0] == STX { 1024 } else { 128 };
        assert_eq!(frame.len(), 3 + size + if self.crc { 2 } else { 1 });
        assert_eq!(frame[1], !frame[2], "block number and its complement");
        let data = &frame[3..3 + size];
        if self.crc {
            let crc = u16::from_be_bytes([frame[3 + size], frame[4 + size]]);
            assert_eq!(crc, crc16(data));
        } else {
            let sum = data.iter().fold(0u8, |sum, &byte| sum.wrapping_add(byte));
            assert_eq!(frame[3 + size], sum);
        }

        let index = self.blocks_seen;
        self.blocks_seen += 1;
        if self.cancel_after_blocks == Some(index) {
            self.replies.extend([CAN, CAN]);
            return;
        }
        if let Some(position) = self.refuse.iter().position(|&refused| refused == index) {
            self.refuse.remove(position);
            self.blocks_seen -= 1;
            self.replies.push_back(NAK);
            return;
        }

        if self.ymodem && frame[1] == 0 {
            let name_end = data.iter().position(|&byte| byte == 0).unwrap();
            self.replies.push_back(ACK);
            if name_end == 0 {
                return;
            }
            let name = String::from_utf8(data[..name_end].to_vec()).unwrap();
            let rest = &data[name_end + 1..];
            let size_end = rest
                .iter()
                .position(|&byte| byte == b' ' || byte == 0)
                .unwrap();
            let size = std::str::from_utf8(&rest[..size_end])
                .unwrap()
                .parse()
                .unwrap();
            self.expected_size = Some(size);
            self.files.push((name, Vec::new()));
            self.replies.push_back(b'C');
            return;
        }
        if self.files.is_empty() {
            self.files.push(("xmodem".to_string(), Vec::new()));
        }
        self.files.last_mut().unwrap().1.extend_from_slice(data);
        self.replies.push_back(ACK);
    }

    fn take_eot(&mut self) {
        self.eots_seen += 1;
        // A YMODEM receiver refuses the first EOT of each file.
        if self.ymodem && self.eots_seen % 2 == 1 {
            self.replies.push_back(NAK);
            return;
        }
        let file = self.files.last_mut().unwrap();
        match self.expected_size.take() {
            Some(size) => file.1.truncate(size),
            None => {
                while file.1.last() == Some(&0x1a) {
                    file.1.pop();
                }
            }
        }
        self.replies.push_back(ACK);
        if self.ymodem {
            self.replies.push_back(b'C');
        }
    }
}

impl Link for Receiver {
    fn read(&mut self, timeout: Duration) -> Received {
        match self.replies.pop_front() {
            Some(byte) => Received::Byte(byte),
            None => {
                self.clock += timeout.max(Duration::from_millis(1));
                Received::TimedOut
            }
        }
    }

    fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        match bytes[0] {
            SOH | STX => self.take_block(bytes),
            EOT => self.take_eot(),
            _ => {}
        }
        Ok(())
    }

    fn cancelled(&self) -> bool {
        self.user_cancelled
    }

    fn now(&self) -> Instant {
        self.clock
    }
}

fn file(name: &str, data: Vec<u8>) -> OutgoingFile<Cursor<Vec<u8>>> {
    OutgoingFile {
        name: name.to_string(),
        size: data.len() as u64,
        modified: Some(0o14_000_000_000),
        data: Cursor::new(data),
    }
}

fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|index| (index % 251) as u8 + 1).collect()
}

#[test]
fn crc16_matches_the_xmodem_check_value() {
    assert_eq!(crc16(b"123456789"), 0x31c3);
}

#[test]
fn xmodem_sends_a_file_in_128_byte_blocks() {
    let mut receiver = Receiver::new(false, true);
    let mut progress = Vec::new();
    let mut outgoing = file("fw.bin", pattern(300));
    send_xmodem(
        &mut receiver,
        XmodemProtocol::Xmodem,
        &mut outgoing,
        &mut |sent| progress.push(sent),
    )
    .unwrap();
    assert_eq!(receiver.files[0].1, pattern(300));
    assert_eq!(receiver.blocks_seen, 3);
    assert_eq!(progress, [128, 256, 300]);
}

#[test]
fn xmodem_uses_checksums_for_a_receiver_that_asks_with_nak() {
    let mut receiver = Receiver::new(false, false);
    let mut outgoing = file("fw.bin", pattern(200));
    send_xmodem(
        &mut receiver,
        XmodemProtocol::Xmodem,
        &mut outgoing,
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(receiver.files[0].1, pattern(200));
}

#[test]
fn xmodem_1k_sends_1024_byte_blocks() {
    let mut receiver = Receiver::new(false, true);
    let mut outgoing = file("fw.bin", pattern(2100));
    send_xmodem(
        &mut receiver,
        XmodemProtocol::Xmodem1k,
        &mut outgoing,
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(receiver.files[0].1, pattern(2100));
    // 1024, 1024, and the last 52 bytes in a 128-byte block.
    assert_eq!(receiver.blocks_seen, 3);
}

#[test]
fn a_refused_block_is_sent_again() {
    let mut receiver = Receiver::new(false, true);
    receiver.refuse = vec![1, 1];
    let mut outgoing = file("fw.bin", pattern(400));
    send_xmodem(
        &mut receiver,
        XmodemProtocol::Xmodem,
        &mut outgoing,
        &mut |_| {},
    )
    .unwrap();
    assert_eq!(receiver.files[0].1, pattern(400));
}

#[test]
fn ymodem_sends_several_files_with_their_names_and_sizes() {
    let mut receiver = Receiver::new(true, true);
    let mut files = vec![
        file("boot.bin", pattern(1500)),
        file("empty.txt", Vec::new()),
        file("app.bin", pattern(100)),
    ];
    let mut reported = Vec::new();
    send_ymodem(&mut receiver, &mut files, &mut |index, sent| {
        reported.push((index, sent))
    })
    .unwrap();
    let names: Vec<&str> = receiver
        .files
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(names, ["boot.bin", "empty.txt", "app.bin"]);
    assert_eq!(receiver.files[0].1, pattern(1500));
    assert!(receiver.files[1].1.is_empty());
    assert_eq!(receiver.files[2].1, pattern(100));
    assert_eq!(reported, [(0, 1024), (0, 1500), (2, 100)]);
}

#[test]
fn a_receiver_cancelling_stops_the_transfer() {
    let mut receiver = Receiver::new(false, true);
    receiver.cancel_after_blocks = Some(1);
    let mut outgoing = file("fw.bin", pattern(600));
    let result = send_xmodem(
        &mut receiver,
        XmodemProtocol::Xmodem,
        &mut outgoing,
        &mut |_| {},
    );
    assert_eq!(result, Err(TransferError::CancelledByReceiver));
}

#[test]
fn a_device_that_never_asks_is_given_up_on() {
    let mut receiver = Receiver::silent();
    let mut outgoing = file("fw.bin", pattern(10));
    let result = send_xmodem(
        &mut receiver,
        XmodemProtocol::Xmodem,
        &mut outgoing,
        &mut |_| {},
    );
    assert_eq!(result, Err(TransferError::NoReceiver));
}

#[test]
fn the_user_can_stop_a_transfer() {
    let mut receiver = Receiver::new(false, true);
    receiver.user_cancelled = true;
    let mut outgoing = file("fw.bin", pattern(10));
    let result = send_xmodem(
        &mut receiver,
        XmodemProtocol::Xmodem,
        &mut outgoing,
        &mut |_| {},
    );
    assert_eq!(result, Err(TransferError::Cancelled));
}
