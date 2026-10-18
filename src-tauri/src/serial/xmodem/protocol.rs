//! The sending side of XMODEM, XMODEM-1K and YMODEM, over any [`Link`].
//! Kept apart from the serial port so a scripted receiver can drive it.

use super::XmodemProtocol;
use std::io::Read;
use std::time::{Duration, Instant};

pub(super) const SOH: u8 = 0x01;
pub(super) const STX: u8 = 0x02;
pub(super) const EOT: u8 = 0x04;
pub(super) const ACK: u8 = 0x06;
pub(super) const NAK: u8 = 0x15;
pub(super) const CAN: u8 = 0x18;
pub(super) const SUB: u8 = 0x1a;
pub(super) const CRC_MODE: u8 = b'C';

/// How long a receiver has to ask for the first block. Bootloaders ask once
/// a second, and a person may start the transfer before the device.
pub(super) const START_TIMEOUT: Duration = Duration::from_secs(60);
/// How long a receiver has to answer one block.
pub(super) const BLOCK_TIMEOUT: Duration = Duration::from_secs(10);
/// How often a block is sent before the transfer gives up on it.
pub(super) const MAX_ATTEMPTS: u32 = 10;

/// What the receiver sent, as [`Link::read`] reports it.
pub(super) enum Received {
    Byte(u8),
    TimedOut,
    /// The port is gone.
    Closed,
}

/// The line a transfer runs over.
pub(super) trait Link {
    /// The next byte from the receiver, waiting at most `timeout`.
    fn read(&mut self, timeout: Duration) -> Received;
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<()>;
    /// Whether the user has asked to stop.
    fn cancelled(&self) -> bool;
    /// The time deadlines are measured in.
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Why a transfer stopped before it finished.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum TransferError {
    /// The user stopped it.
    Cancelled,
    /// The device sent CAN CAN.
    CancelledByDevice,
    /// The receiver never asked for the first block.
    NoReceiver,
    /// The device never sent the first block.
    NoSender,
    /// The device sent a block out of order.
    OutOfSequence,
    /// A YMODEM header named no file the way the protocol writes one.
    BadHeader,
    /// The receiver kept refusing or not answering a block.
    NoAnswer,
    Disconnected,
    Io(String),
}

impl TransferError {
    pub(super) fn message(&self) -> String {
        match self {
            Self::Cancelled => "Cancelled".to_string(),
            Self::CancelledByDevice => "The device cancelled the transfer".to_string(),
            Self::NoReceiver => {
                "The device never asked for the file. Start its receiver (XMODEM or YMODEM) and try again"
                    .to_string()
            }
            Self::NoSender => {
                "The device never started sending. Start its sender (XMODEM or YMODEM) and try again"
                    .to_string()
            }
            Self::OutOfSequence => "The device sent a block out of order".to_string(),
            Self::BadHeader => "The device sent a file header that can't be read".to_string(),
            Self::NoAnswer => "The device stopped answering".to_string(),
            Self::Disconnected => "The serial device was disconnected".to_string(),
            Self::Io(err) => err.clone(),
        }
    }
}

pub(super) type Result<T> = std::result::Result<T, TransferError>;

/// How each block is checked, as the receiver asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Check {
    Crc,
    Sum,
}

/// One file to send.
pub(super) struct OutgoingFile<R> {
    pub name: String,
    pub size: u64,
    /// Seconds since the epoch, sent in YMODEM's header.
    pub modified: Option<u64>,
    pub data: R,
}

/// CRC-16/XMODEM.
pub(super) fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0;
    for &byte in data {
        crc ^= u16::from(byte) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

pub(super) fn write(link: &mut impl Link, bytes: &[u8]) -> Result<()> {
    link.write(bytes)
        .map_err(|err| TransferError::Io(format!("Failed to write to the serial port: {err}")))
}

/// Tells the receiver the transfer is over, the way a sender cancels.
pub(super) fn send_cancel(link: &mut impl Link) {
    let _ = link.write(&[CAN; 3]);
}

/// The next byte, or why there is none within `timeout`.
pub(super) fn read(link: &mut impl Link, timeout: Duration) -> Result<Option<u8>> {
    if link.cancelled() {
        return Err(TransferError::Cancelled);
    }
    match link.read(timeout) {
        Received::Byte(byte) => Ok(Some(byte)),
        Received::TimedOut => Ok(None),
        Received::Closed => Err(TransferError::Disconnected),
    }
}

/// Whether a CAN is the device cancelling: a second one follows at once.
pub(super) fn cancelled_by_device(link: &mut impl Link) -> Result<bool> {
    Ok(read(link, Duration::from_secs(1))? == Some(CAN))
}

/// Waits for the receiver to ask for blocks: `C` for CRC, NAK for checksums.
fn wait_for_start(link: &mut impl Link) -> Result<Check> {
    let deadline = link.now() + START_TIMEOUT;
    while link.now() < deadline {
        match read(link, Duration::from_secs(1))? {
            Some(CRC_MODE) => return Ok(Check::Crc),
            Some(NAK) => return Ok(Check::Sum),
            Some(CAN) if cancelled_by_device(link)? => {
                return Err(TransferError::CancelledByDevice)
            }
            _ => {}
        }
    }
    Err(TransferError::NoReceiver)
}

/// Waits for the receiver's answer to a block: `true` for ACK, `false` for
/// NAK or none. Anything else, such as a `C` left from the start, is skipped.
fn wait_for_answer(link: &mut impl Link) -> Result<bool> {
    let deadline = link.now() + BLOCK_TIMEOUT;
    while link.now() < deadline {
        match read(link, deadline.saturating_duration_since(link.now()))? {
            Some(ACK) => return Ok(true),
            Some(NAK) => return Ok(false),
            Some(CAN) if cancelled_by_device(link)? => {
                return Err(TransferError::CancelledByDevice)
            }
            _ => {}
        }
    }
    Ok(false)
}

/// One block as it goes on the line: header, number, its complement, the
/// data and its check.
fn frame(number: u8, data: &[u8], check: Check) -> Vec<u8> {
    let mut frame = Vec::with_capacity(data.len() + 5);
    frame.push(if data.len() == 1024 { STX } else { SOH });
    frame.push(number);
    frame.push(!number);
    frame.extend_from_slice(data);
    match check {
        Check::Crc => frame.extend_from_slice(&crc16(data).to_be_bytes()),
        Check::Sum => frame.push(data.iter().fold(0u8, |sum, &byte| sum.wrapping_add(byte))),
    }
    frame
}

/// Sends one block until the receiver takes it.
fn send_block(link: &mut impl Link, number: u8, data: &[u8], check: Check) -> Result<()> {
    let frame = frame(number, data, check);
    for _ in 0..MAX_ATTEMPTS {
        write(link, &frame)?;
        if wait_for_answer(link)? {
            return Ok(());
        }
    }
    Err(TransferError::NoAnswer)
}

/// Ends a file. A YMODEM receiver refuses the first EOT, so it is sent again.
fn send_eot(link: &mut impl Link) -> Result<()> {
    for _ in 0..MAX_ATTEMPTS {
        write(link, &[EOT])?;
        if wait_for_answer(link)? {
            return Ok(());
        }
    }
    Err(TransferError::NoAnswer)
}

/// Reads up to `len` bytes of the file; fewer only at its end.
fn read_chunk(data: &mut impl Read, len: usize) -> Result<Vec<u8>> {
    let mut chunk = vec![0u8; len];
    let mut filled = 0;
    while filled < len {
        match data.read(&mut chunk[filled..]) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
            Err(err) => return Err(TransferError::Io(format!("Failed to read the file: {err}"))),
        }
    }
    chunk.truncate(filled);
    Ok(chunk)
}

/// Sends a file's data blocks, numbered from 1, and its EOT. `progress`
/// hears the bytes sent so far after each block.
fn send_data(
    link: &mut impl Link,
    file: &mut OutgoingFile<impl Read>,
    block_size: usize,
    check: Check,
    progress: &mut dyn FnMut(u64),
) -> Result<()> {
    let mut number: u8 = 1;
    let mut sent: u64 = 0;
    loop {
        let mut chunk = read_chunk(&mut file.data, block_size)?;
        if chunk.is_empty() {
            break;
        }
        let len = chunk.len();
        // A short last block goes in 128 bytes where 1K would be padding.
        let size = if len <= 128 { 128 } else { block_size };
        chunk.resize(size, SUB);
        send_block(link, number, &chunk, check)?;
        number = number.wrapping_add(1);
        sent += len as u64;
        progress(sent);
        if len < block_size {
            break;
        }
    }
    send_eot(link)
}

/// Sends one file over XMODEM or XMODEM-1K.
pub(super) fn send_xmodem(
    link: &mut impl Link,
    protocol: XmodemProtocol,
    file: &mut OutgoingFile<impl Read>,
    progress: &mut dyn FnMut(u64),
) -> Result<()> {
    let check = wait_for_start(link)?;
    let block_size = if protocol == XmodemProtocol::Xmodem1k {
        1024
    } else {
        128
    };
    send_data(link, file, block_size, check, progress)
}

/// YMODEM's block 0: the file's name and size, or all zeros to end a batch.
fn ymodem_header(file: Option<&OutgoingFile<impl Read>>) -> Vec<u8> {
    let mut header = Vec::new();
    if let Some(file) = file {
        header.extend_from_slice(file.name.as_bytes());
        header.push(0);
        header.extend_from_slice(file.size.to_string().as_bytes());
        if let Some(modified) = file.modified {
            header.extend_from_slice(format!(" {modified:o}").as_bytes());
        }
    }
    let size = if header.len() < 128 { 128 } else { 1024 };
    header.resize(size, 0);
    header
}

/// Sends files over YMODEM, one after the other, then the empty header that
/// ends the batch. `progress` hears which file and how much of it is sent.
pub(super) fn send_ymodem<R: Read>(
    link: &mut impl Link,
    files: &mut [OutgoingFile<R>],
    progress: &mut dyn FnMut(usize, u64),
) -> Result<()> {
    for (index, file) in files.iter_mut().enumerate() {
        let check = wait_for_start(link)?;
        send_block(link, 0, &ymodem_header(Some(&*file)), check)?;
        let check = wait_for_start(link)?;
        send_data(link, file, 1024, check, &mut |sent| progress(index, sent))?;
    }
    let check = wait_for_start(link)?;
    send_block(link, 0, &ymodem_header(None::<&OutgoingFile<R>>), check)
}
