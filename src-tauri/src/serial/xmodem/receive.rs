//! The receiving side of XMODEM, XMODEM-1K and YMODEM, over any [`Link`].

use super::protocol::{
    cancelled_by_device, crc16, read, write, Check, Link, Result, TransferError, ACK,
    BLOCK_TIMEOUT, CAN, CRC_MODE, EOT, MAX_ATTEMPTS, NAK, SOH, START_TIMEOUT, STX, SUB,
};
use std::time::Duration;

/// How long to wait for the first block after asking for it, before asking
/// again. Senders wait for the request, so it is repeated until one starts.
const ASK_INTERVAL: Duration = Duration::from_secs(3);
/// Requests for CRC blocks before an XMODEM receiver falls back to asking
/// for checksums, which old senders know only.
const CRC_ASKS: u32 = 4;
/// How long a block's bytes may pause before the block counts as broken.
const BYTE_TIMEOUT: Duration = Duration::from_secs(1);

/// Where received files go.
pub(super) trait FileSink {
    /// Starts the next file: `name` and `size` as YMODEM's header gives
    /// them; XMODEM gives neither.
    fn open(&mut self, name: Option<&str>, size: Option<u64>) -> Result<()>;
    fn write(&mut self, data: &[u8]) -> Result<()>;
    /// Ends the file, all of it received.
    fn close(&mut self) -> Result<()>;
    /// How much of the open file has arrived.
    fn progress(&mut self, received: u64);
}

/// What came after a request for a block.
enum Incoming {
    Block {
        number: u8,
        data: Vec<u8>,
    },
    Eot,
    /// A block that arrived broken.
    Broken,
    /// Nothing within the wait.
    Nothing,
}

fn check_len(check: Check) -> usize {
    if check == Check::Crc {
        2
    } else {
        1
    }
}

/// Reads exactly `len` bytes of a block; `None` when the sender stalls.
fn read_exact(link: &mut impl Link, len: usize) -> Result<Option<Vec<u8>>> {
    let mut bytes = Vec::with_capacity(len);
    while bytes.len() < len {
        match read(link, BYTE_TIMEOUT)? {
            Some(byte) => bytes.push(byte),
            None => return Ok(None),
        }
    }
    Ok(Some(bytes))
}

/// Drops what is left of a broken block, so the sender's resend starts clean.
fn purge(link: &mut impl Link) -> Result<()> {
    while read(link, BYTE_TIMEOUT)?.is_some() {}
    Ok(())
}

/// Reads the next block, EOT or nothing, waiting `timeout` for it to start.
fn receive(link: &mut impl Link, check: Check, timeout: Duration) -> Result<Incoming> {
    let size = match read(link, timeout)? {
        Some(SOH) => 128,
        Some(STX) => 1024,
        Some(EOT) => return Ok(Incoming::Eot),
        Some(CAN) if cancelled_by_device(link)? => return Err(TransferError::CancelledByDevice),
        Some(_) => {
            purge(link)?;
            return Ok(Incoming::Broken);
        }
        None => return Ok(Incoming::Nothing),
    };
    let Some(frame) = read_exact(link, 2 + size + check_len(check))? else {
        return Ok(Incoming::Broken);
    };
    let (number, complement) = (frame[0], frame[1]);
    let data = &frame[2..2 + size];
    let valid = number == !complement
        && match check {
            Check::Crc => frame[2 + size..] == crc16(data).to_be_bytes(),
            Check::Sum => {
                frame[2 + size] == data.iter().fold(0u8, |sum, &byte| sum.wrapping_add(byte))
            }
        };
    if !valid {
        purge(link)?;
        return Ok(Incoming::Broken);
    }
    Ok(Incoming::Block {
        number,
        data: data.to_vec(),
    })
}

/// Asks for blocks until the sender starts, falling back from CRC to
/// checksums when `fallback` allows. Returns what the sender sent first,
/// a broken block among it, which the caller asks for again with a NAK.
fn start(link: &mut impl Link, fallback: bool) -> Result<(Incoming, Check)> {
    let deadline = link.now() + START_TIMEOUT;
    let mut asks = 0;
    while link.now() < deadline {
        let check = if fallback && asks >= CRC_ASKS {
            Check::Sum
        } else {
            Check::Crc
        };
        write(link, &[if check == Check::Crc { CRC_MODE } else { NAK }])?;
        asks += 1;
        match receive(link, check, ASK_INTERVAL)? {
            Incoming::Nothing => {}
            incoming => return Ok((incoming, check)),
        }
    }
    Err(TransferError::NoSender)
}

/// Receives one file's data blocks, numbered from 1, up to its EOT, which
/// is acknowledged. `first` is what the request for data brought. `size`,
/// when known, cuts off the padding; without it, trailing SUBs of the last
/// block are taken for padding. `refuse_first_eot` answers the first EOT
/// with NAK, as YMODEM receivers do to be sure it is one.
fn receive_data(
    link: &mut impl Link,
    sink: &mut impl FileSink,
    check: Check,
    mut incoming: Incoming,
    size: Option<u64>,
    refuse_first_eot: bool,
) -> Result<()> {
    let mut expected: u8 = 1;
    let mut received: u64 = 0;
    // The last block waits for the next, or the EOT, before it is written,
    // so its padding can be trimmed.
    let mut held: Option<Vec<u8>> = None;
    let mut failures = 0;
    let mut eots = 0;
    loop {
        match incoming {
            Incoming::Block { number, mut data } if number == expected => {
                failures = 0;
                if let Some(size) = size {
                    let left = size.saturating_sub(received);
                    data.truncate(left.min(data.len() as u64) as usize);
                }
                received += data.len() as u64;
                if let Some(previous) = held.replace(data) {
                    sink.write(&previous)?;
                }
                expected = expected.wrapping_add(1);
                write(link, &[ACK])?;
                sink.progress(received);
            }
            // The sender missed our ACK and sent the block again.
            Incoming::Block { number, .. } if number == expected.wrapping_sub(1) => {
                write(link, &[ACK])?;
            }
            Incoming::Block { .. } => return Err(TransferError::OutOfSequence),
            Incoming::Eot if refuse_first_eot && eots == 0 => {
                eots += 1;
                write(link, &[NAK])?;
            }
            Incoming::Eot => {
                if let Some(mut last) = held.take() {
                    if size.is_none() {
                        while last.last() == Some(&SUB) {
                            last.pop();
                        }
                    }
                    sink.write(&last)?;
                }
                write(link, &[ACK])?;
                return sink.close();
            }
            Incoming::Nothing | Incoming::Broken => {
                failures += 1;
                if failures > MAX_ATTEMPTS {
                    return Err(TransferError::NoAnswer);
                }
                write(link, &[NAK])?;
            }
        }
        incoming = receive(link, check, BLOCK_TIMEOUT)?;
    }
}

/// Receives one file over XMODEM or XMODEM-1K; the sender picks the block
/// size.
pub(super) fn receive_xmodem(link: &mut impl Link, sink: &mut impl FileSink) -> Result<()> {
    let (first, check) = start(link, true)?;
    sink.open(None, None)?;
    receive_data(link, sink, check, first, None, false)
}

/// A YMODEM header: the file's name and size, or `None` for the empty one
/// that ends a batch.
fn parse_header(data: &[u8]) -> Result<Option<(String, Option<u64>)>> {
    let name_end = data
        .iter()
        .position(|&byte| byte == 0)
        .ok_or(TransferError::BadHeader)?;
    if name_end == 0 {
        return Ok(None);
    }
    let name = String::from_utf8_lossy(&data[..name_end]).into_owned();
    let rest = &data[name_end + 1..];
    let size_end = rest
        .iter()
        .position(|&byte| byte == b' ' || byte == 0)
        .unwrap_or(rest.len());
    let size = std::str::from_utf8(&rest[..size_end])
        .ok()
        .and_then(|size| size.parse().ok());
    Ok(Some((name, size)))
}

/// Receives the files of a YMODEM batch until its empty header.
pub(super) fn receive_ymodem(link: &mut impl Link, sink: &mut impl FileSink) -> Result<()> {
    loop {
        let (header, check) = start(link, false)?;
        let Incoming::Block { number: 0, data } = header else {
            return Err(TransferError::OutOfSequence);
        };
        write(link, &[ACK])?;
        let Some((name, size)) = parse_header(&data)? else {
            return Ok(());
        };
        sink.open(Some(&name), size)?;
        let (first, check) = match start_data(link, check)? {
            Some(first) => (first, check),
            None => return Err(TransferError::NoAnswer),
        };
        receive_data(link, sink, check, first, size, true)?;
    }
}

/// Asks for a file's data once its header is in. A sender that missed the
/// ACK sends the header again, which is acknowledged and skipped.
fn start_data(link: &mut impl Link, check: Check) -> Result<Option<Incoming>> {
    for _ in 0..MAX_ATTEMPTS {
        write(link, &[CRC_MODE])?;
        match receive(link, check, ASK_INTERVAL)? {
            Incoming::Block { number: 0, .. } => write(link, &[ACK])?,
            Incoming::Nothing => {}
            incoming => return Ok(Some(incoming)),
        }
    }
    Ok(None)
}
