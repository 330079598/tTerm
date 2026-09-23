//! Wire-format constants for the ZMODEM protocol, byte values verified
//! against real `lrzsz` traffic (captured by piping `sz`/`rz` against each
//! other through a pair of FIFOs and inspecting the raw bytes) rather than
//! transcribed from documentation alone.
//!
//! The full frame-type table is kept even where tTerm never emits or
//! handles a given type, so the byte values stay documented in one place.
#![allow(dead_code)]

/// Pad character that opens every header (`*`). Real senders sometimes emit
/// it more than once as a resync aid; scanning must tolerate that.
pub const ZPAD: u8 = b'*';
/// Link-escape byte. Introduces an escaped payload byte or a header-type /
/// data-subpacket-end indicator.
pub const ZDLE: u8 = 0x18;
/// XOR mask applied to a byte to escape it after `ZDLE`.
pub const ZDLE_ESCAPE_MASK: u8 = 0x40;
/// Sent after a hex header and after some binary-header terminators to wake
/// a flow-controlled peer; purely advisory, never required to be present.
pub const XON: u8 = 0x11;

/// Header encodings, sent as the byte immediately following `ZDLE` at the
/// start of a header.
pub const ZBIN: u8 = b'A'; // Binary header, 16-bit CRC.
pub const ZHEX: u8 = b'B'; // Hex-digit header, always 16-bit CRC.
pub const ZBIN32: u8 = b'C'; // Binary header, 32-bit CRC.

/// Frame types (the first byte of every header's 5-byte payload).
pub const ZRQINIT: u8 = 0;
pub const ZRINIT: u8 = 1;
pub const ZSINIT: u8 = 2;
pub const ZACK: u8 = 3;
pub const ZFILE: u8 = 4;
pub const ZSKIP: u8 = 5;
pub const ZNAK: u8 = 6;
pub const ZABORT: u8 = 7;
pub const ZFIN: u8 = 8;
pub const ZRPOS: u8 = 9;
pub const ZDATA: u8 = 10;
pub const ZEOF: u8 = 11;
pub const ZFERR: u8 = 12;
pub const ZCRC: u8 = 13;
pub const ZCHALLENGE: u8 = 14;
pub const ZCOMPL: u8 = 15;
pub const ZCAN: u8 = 16;
pub const ZFREECNT: u8 = 17;
pub const ZCOMMAND: u8 = 18;
pub const ZSTDERR: u8 = 19;

/// Data-subpacket terminators: sent as `ZDLE <one of these>` right after the
/// subpacket's payload bytes, then covered (together with the payload) by
/// the subpacket's own CRC.
pub const ZCRCE: u8 = b'h'; // End of frame, no ZACK expected (last subpacket of a ZEOF/ZFILE run).
pub const ZCRCG: u8 = b'i'; // Frame continues, no ZACK expected (streaming interior subpacket).
pub const ZCRCQ: u8 = b'j'; // Frame continues, ZACK expected.
pub const ZCRCW: u8 = b'k'; // End of frame, ZACK expected.

/// `ZRINIT` capability bits, carried in the 4th header data byte (index 3).
pub const CANFDX: u8 = 0x01;
pub const CANOVIO: u8 = 0x02;
pub const CANBRK: u8 = 0x04;
pub const CANFC32: u8 = 0x20; // Receiver accepts ZBIN32 (CRC-32) headers/subpackets.

/// Literal byte sequences that mark the start of a ZMODEM session inside an
/// otherwise-ordinary terminal byte stream: `sz` announces itself with
/// ZRQINIT, `rz` with ZRINIT. Both are `ZPAD ZPAD ZDLE ZHEX` followed by the
/// hex-encoded frame type, confirmed byte-for-byte against real `lrzsz`
/// captures (`**\x18B00...` / `**\x18B01...`).
pub const TRIGGER_ZRQINIT: &[u8] = b"**\x18B00";
pub const TRIGGER_ZRINIT: &[u8] = b"**\x18B01";

/// Bytes ZDLE-escaped unconditionally, regardless of context. Confirmed
/// against a real capture: none of these ever appear raw in `lrzsz` output
/// (0 unescaped occurrences across a 20KB random-data transfer containing
/// ~78 statistical occurrences of each).
pub const ALWAYS_ESCAPED: [u8; 7] = [ZDLE, 0x10, 0x90, 0x11, 0x91, 0x13, 0x93];

/// `@` (and its high-bit variant): a literal CR immediately following one of
/// these is escaped too, to avoid old modems' auto-answer sequences. In the
/// same real capture, 95/98 CR bytes were sent raw and only the 3 that
/// happened to follow `@` were escaped, confirming this is conditional, not
/// unconditional like the bytes above.
pub const AT_SIGN: u8 = 0x40;
pub const AT_SIGN_HIGH_BIT: u8 = 0xc0;
pub const CR: u8 = 0x0d;
pub const CR_HIGH_BIT: u8 = 0x8d;

/// Which direction tTerm is acting in for a given transfer: the peer running
/// `sz` makes tTerm a receiver (download); the peer running `rz` makes tTerm
/// a sender (upload).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZmodemDirection {
    Receive,
    Send,
}
