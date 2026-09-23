//! Header and data-subpacket encode/decode, including `ZDLE` escaping.
//!
//! Every byte-order and escaping decision here was cross-checked against a
//! real `sz`<->`rz` capture (see `crc.rs`'s doc comment for how it was
//! taken) rather than transcribed from the written spec alone.

use super::consts::*;
use super::crc::{crc16, crc32};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrcKind {
    Crc16,
    Crc32,
}

impl CrcKind {
    fn len(self) -> usize {
        match self {
            CrcKind::Crc16 => 2,
            CrcKind::Crc32 => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub frame_type: u8,
    pub data: [u8; 4],
}

impl Header {
    /// The 4 data bytes read as a little-endian file offset (used by
    /// `ZRPOS`/`ZDATA`/`ZEOF`), confirmed against a real capture: `ZEOF`'s
    /// data bytes `20 4e 00 00` decode to `0x4e20` = 20000, the exact size
    /// of the file that was transferred.
    pub fn position(&self) -> u64 {
        u32::from_le_bytes(self.data) as u64
    }

    pub fn with_position(frame_type: u8, position: u64) -> Self {
        let mut data = [0u8; 4];
        data.copy_from_slice(&(position as u32).to_le_bytes());
        Header { frame_type, data }
    }

    pub fn with_flags(frame_type: u8, flags: u8) -> Self {
        Header {
            frame_type,
            data: [0, 0, 0, flags],
        }
    }
}

/// Appends `byte` to `out`, `ZDLE`-escaping it if required. `prev` is the
/// previous logical (already-decoded) byte written to this same stream,
/// needed only to decide whether a CR must be escaped (real `lrzsz` escapes
/// CR conditionally, only right after `@` — see `consts.rs`).
fn push_escaped(out: &mut Vec<u8>, byte: u8, prev: u8) {
    let is_at_cr = matches!(byte, CR | CR_HIGH_BIT) && matches!(prev, AT_SIGN | AT_SIGN_HIGH_BIT);
    if ALWAYS_ESCAPED.contains(&byte) || is_at_cr {
        out.push(ZDLE);
        out.push(byte ^ ZDLE_ESCAPE_MASK);
    } else {
        out.push(byte);
    }
}

fn push_escaped_seq(out: &mut Vec<u8>, bytes: &[u8]) {
    let mut prev = 0u8;
    for &b in bytes {
        push_escaped(out, b, prev);
        prev = b;
    }
}

fn hex_nibble(n: u8) -> u8 {
    match n {
        0..=9 => b'0' + n,
        _ => b'a' + (n - 10),
    }
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Encodes a header the way real `rz` sends its control frames: hex digits,
/// always CRC16, double `ZPAD`, terminated `CR LF(high-bit) XON`.
pub fn encode_hex_header(header: Header) -> Vec<u8> {
    let mut fields = [0u8; 5];
    fields[0] = header.frame_type;
    fields[1..5].copy_from_slice(&header.data);
    let crc = crc16(&fields).to_be_bytes();

    let mut all = [0u8; 7];
    all[..5].copy_from_slice(&fields);
    all[5..7].copy_from_slice(&crc);

    let mut out = Vec::with_capacity(4 + 14 + 3);
    out.extend_from_slice(&[ZPAD, ZPAD, ZDLE, ZHEX]);
    for &b in &all {
        out.push(hex_nibble(b >> 4));
        out.push(hex_nibble(b & 0x0f));
    }
    out.extend_from_slice(&[b'\r', b'\n' | 0x80, XON]);
    out
}

/// Encodes a header the way real `sz` sends its bulk-data frames: binary,
/// CRC32, single `ZPAD`, `ZDLE`-escaped.
pub fn encode_bin32_header(header: Header) -> Vec<u8> {
    let mut fields = [0u8; 5];
    fields[0] = header.frame_type;
    fields[1..5].copy_from_slice(&header.data);
    let crc = crc32(&fields).to_le_bytes();

    let mut out = Vec::with_capacity(3 + 18);
    out.extend_from_slice(&[ZPAD, ZDLE, ZBIN32]);
    push_escaped_seq(&mut out, &fields);
    push_escaped_seq(&mut out, &crc);
    out
}

/// Encodes one data subpacket: escaped payload, `ZDLE <ender>`, then the CRC
/// (16- or 32-bit depending on `crc_kind`) covering payload+ender together —
/// confirmed against a real capture (the CRC32 of the metadata subpacket's
/// bytes plus its `ZCRCW` ender byte matches the wire bytes exactly).
pub fn encode_data_subpacket(payload: &[u8], ender: u8, crc_kind: CrcKind) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + payload.len() / 8 + 8);
    push_escaped_seq(&mut out, payload);
    out.push(ZDLE);
    out.push(ender);

    let mut crc_input = Vec::with_capacity(payload.len() + 1);
    crc_input.extend_from_slice(payload);
    crc_input.push(ender);
    match crc_kind {
        CrcKind::Crc16 => push_escaped_seq(&mut out, &crc16(&crc_input).to_be_bytes()),
        CrcKind::Crc32 => push_escaped_seq(&mut out, &crc32(&crc_input).to_le_bytes()),
    }
    out
}

#[derive(Debug)]
pub enum HeaderOutcome {
    Found {
        header: Header,
        crc_kind: CrcKind,
        consumed: usize,
    },
    Corrupt {
        consumed: usize,
    },
    NeedMore,
}

/// Scans for a header at the front of `buf`. Tolerates any number of
/// leading `ZPAD` bytes (real senders sometimes emit more than one), and
/// discards leading bytes that aren't part of a header at all: confirmed
/// against a real local-PTY capture that real `sz`/`rz` interleave
/// human-readable status text (sent to what becomes the same tty fd as the
/// protocol bytes once there's no separate stderr stream — e.g. a lone `\r`
/// observed right before a real ZFIN header) between frames, not just
/// before the very first one.
pub fn scan_header(buf: &[u8]) -> HeaderOutcome {
    let Some(first_pad) = buf.iter().position(|&b| b == ZPAD) else {
        // No ZPAD anywhere yet: a bare `ZPAD` byte can't itself be split
        // across reads (it's one byte), so anything present now that isn't
        // one is definitely garbage, not a partial header.
        return if buf.is_empty() {
            HeaderOutcome::NeedMore
        } else {
            HeaderOutcome::Corrupt {
                consumed: buf.len(),
            }
        };
    };
    if first_pad > 0 {
        return HeaderOutcome::Corrupt {
            consumed: first_pad,
        };
    }

    let mut pos = 0;
    while buf.get(pos) == Some(&ZPAD) {
        pos += 1;
    }
    match buf.get(pos) {
        Some(&ZDLE) => {}
        Some(_) => return HeaderOutcome::Corrupt { consumed: pos + 1 },
        None => return HeaderOutcome::NeedMore,
    }
    pos += 1;
    let encoding = match buf.get(pos) {
        Some(&b) => b,
        None => return HeaderOutcome::NeedMore,
    };
    pos += 1;
    match encoding {
        ZHEX => scan_hex_header(buf, pos),
        ZBIN => scan_bin_header(buf, pos, CrcKind::Crc16),
        ZBIN32 => scan_bin_header(buf, pos, CrcKind::Crc32),
        _ => HeaderOutcome::Corrupt { consumed: pos },
    }
}

fn scan_hex_header(buf: &[u8], start: usize) -> HeaderOutcome {
    const HEX_CHARS: usize = 14; // 7 bytes (type + 4 data + 2 CRC) as ASCII hex.
    if buf.len() < start + HEX_CHARS {
        return HeaderOutcome::NeedMore;
    }
    let hex = &buf[start..start + HEX_CHARS];
    let mut bytes = [0u8; 7];
    for i in 0..7 {
        match (hex_digit(hex[i * 2]), hex_digit(hex[i * 2 + 1])) {
            (Some(hi), Some(lo)) => bytes[i] = (hi << 4) | lo,
            _ => {
                return HeaderOutcome::Corrupt {
                    consumed: start + HEX_CHARS,
                }
            }
        }
    }
    let mut pos = start + HEX_CHARS;
    // CR LF(high-bit) XON terminate a hex header; tolerate their absence so
    // a truncated-but-CRC-valid header still parses.
    if buf.get(pos) == Some(&b'\r') {
        pos += 1;
    }
    if matches!(buf.get(pos), Some(&b) if b == b'\n' || b == (b'\n' | 0x80)) {
        pos += 1;
    }
    if buf.get(pos) == Some(&XON) {
        pos += 1;
    }

    let frame_type = bytes[0];
    let data = [bytes[1], bytes[2], bytes[3], bytes[4]];
    let received_crc = u16::from_be_bytes([bytes[5], bytes[6]]);
    if crc16(&bytes[..5]) != received_crc {
        return HeaderOutcome::Corrupt { consumed: pos };
    }
    HeaderOutcome::Found {
        header: Header { frame_type, data },
        crc_kind: CrcKind::Crc16,
        consumed: pos,
    }
}

fn scan_bin_header(buf: &[u8], start: usize, crc_kind: CrcKind) -> HeaderOutcome {
    let logical_len = 5 + crc_kind.len();
    let mut decoded = Vec::with_capacity(logical_len);
    let mut pos = start;
    while decoded.len() < logical_len {
        match take_escaped_byte(buf, pos) {
            TakeOutcome::Byte(b, next) => {
                decoded.push(b);
                pos = next;
            }
            TakeOutcome::End(_, _) => {
                // A subpacket-end code where a plain data byte was expected
                // can't happen in a well-formed header; treat as corrupt.
                return HeaderOutcome::Corrupt { consumed: pos + 2 };
            }
            TakeOutcome::NeedMore => return HeaderOutcome::NeedMore,
        }
    }

    let frame_type = decoded[0];
    let data = [decoded[1], decoded[2], decoded[3], decoded[4]];
    let crc_ok = match crc_kind {
        CrcKind::Crc16 => crc16(&decoded[..5]) == u16::from_be_bytes([decoded[5], decoded[6]]),
        CrcKind::Crc32 => {
            crc32(&decoded[..5])
                == u32::from_le_bytes([decoded[5], decoded[6], decoded[7], decoded[8]])
        }
    };
    if !crc_ok {
        return HeaderOutcome::Corrupt { consumed: pos };
    }
    HeaderOutcome::Found {
        header: Header { frame_type, data },
        crc_kind,
        consumed: pos,
    }
}

enum TakeOutcome {
    Byte(u8, usize),
    End(u8, usize),
    NeedMore,
}

/// Reads one logical position from a `ZDLE`-escaped stream at `buf[pos..]`:
/// either a plain byte, an escaped data byte, or (only meaningful inside a
/// data subpacket) a subpacket-end code.
fn take_escaped_byte(buf: &[u8], pos: usize) -> TakeOutcome {
    match buf.get(pos) {
        None => TakeOutcome::NeedMore,
        Some(&ZDLE) => match buf.get(pos + 1) {
            None => TakeOutcome::NeedMore,
            Some(&code) if matches!(code, ZCRCE | ZCRCG | ZCRCQ | ZCRCW) => {
                TakeOutcome::End(code, pos + 2)
            }
            Some(&escaped) => TakeOutcome::Byte(escaped ^ ZDLE_ESCAPE_MASK, pos + 2),
        },
        Some(&b) => TakeOutcome::Byte(b, pos + 1),
    }
}

pub enum SubpacketOutcome {
    Found {
        payload: Vec<u8>,
        ender: u8,
        consumed: usize,
    },
    Corrupt {
        consumed: usize,
    },
    NeedMore,
}

/// Scans one data subpacket at the front of `buf`: escaped payload bytes up
/// to `ZDLE <ender>`, then the CRC covering payload+ender.
pub fn scan_subpacket(buf: &[u8], crc_kind: CrcKind) -> SubpacketOutcome {
    let mut payload = Vec::new();
    let mut pos = 0;
    let ender;
    loop {
        match take_escaped_byte(buf, pos) {
            TakeOutcome::Byte(b, next) => {
                payload.push(b);
                pos = next;
            }
            TakeOutcome::End(code, next) => {
                ender = code;
                pos = next;
                break;
            }
            TakeOutcome::NeedMore => return SubpacketOutcome::NeedMore,
        }
    }

    let mut crc_bytes = Vec::with_capacity(crc_kind.len());
    while crc_bytes.len() < crc_kind.len() {
        match take_escaped_byte(buf, pos) {
            TakeOutcome::Byte(b, next) => {
                crc_bytes.push(b);
                pos = next;
            }
            TakeOutcome::End(_, _) => return SubpacketOutcome::Corrupt { consumed: pos + 2 },
            TakeOutcome::NeedMore => return SubpacketOutcome::NeedMore,
        }
    }
    // An advisory XON sometimes follows, matching the hex-header convention.
    if buf.get(pos) == Some(&XON) {
        pos += 1;
    }

    let mut crc_input = payload.clone();
    crc_input.push(ender);
    let crc_ok = match crc_kind {
        CrcKind::Crc16 => crc16(&crc_input) == u16::from_be_bytes([crc_bytes[0], crc_bytes[1]]),
        CrcKind::Crc32 => {
            crc32(&crc_input)
                == u32::from_le_bytes([crc_bytes[0], crc_bytes[1], crc_bytes[2], crc_bytes[3]])
        }
    };
    if !crc_ok {
        return SubpacketOutcome::Corrupt { consumed: pos };
    }
    SubpacketOutcome::Found {
        payload,
        ender,
        consumed: pos,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bytes captured verbatim from a real `sz` process: the ZFILE header
    /// (ZBIN32) for `payload.bin`, immediately followed by its metadata
    /// subpacket (payload = "payload.bin\0<size> <mtime> <mode> ...",
    /// ended ZCRCW).
    const REAL_ZFILE_AND_METADATA: &[u8] = &[
        0x2a, 0x18, 0x43, 0x04, 0x00, 0x00, 0x00, 0x01, 0x4b, 0x61, 0xa5,
        0x44, // ZFILE header
        b'p', b'a', b'y', b'l', b'o', b'a', b'd', b'.', b'b', b'i', b'n',
        0x00, // "payload.bin\0"
        b'2', b'0', b'0', b'0', b'0', b' ', b'1', b'5', b'2', b'5', b'4', b'4', b'4', b'0', b'1',
        b'6', b'0', b' ', b'1', b'0', b'0', b'6', b'4', b'4', b' ', b'0', b' ', b'1', b' ', b'2',
        b'0', b'0', b'0', b'0', 0x00, // metadata string
        0x18, 0x6b, // ZDLE ZCRCW
        0x7c, 0x18, 0x50, 0x3f, 0xf7, // CRC32 (0x10 escaped as 18 50)
        0x11, // trailing XON
    ];

    #[test]
    fn decodes_real_zfile_header() {
        match scan_header(REAL_ZFILE_AND_METADATA) {
            HeaderOutcome::Found {
                header, crc_kind, ..
            } => {
                assert_eq!(header.frame_type, ZFILE);
                assert_eq!(header.data, [0x00, 0x00, 0x00, 0x01]);
                assert_eq!(crc_kind, CrcKind::Crc32);
            }
            _ => panic!("expected a decoded header"),
        }
    }

    #[test]
    fn decodes_real_metadata_subpacket() {
        let header_len = match scan_header(REAL_ZFILE_AND_METADATA) {
            HeaderOutcome::Found { consumed, .. } => consumed,
            _ => panic!("expected a decoded header"),
        };
        match scan_subpacket(&REAL_ZFILE_AND_METADATA[header_len..], CrcKind::Crc32) {
            SubpacketOutcome::Found {
                payload,
                ender,
                consumed,
            } => {
                assert_eq!(ender, ZCRCW);
                assert!(payload.starts_with(b"payload.bin\x0020000 "));
                assert_eq!(header_len + consumed, REAL_ZFILE_AND_METADATA.len());
            }
            _ => panic!("expected a decoded subpacket"),
        }
    }

    #[test]
    fn header_scan_reports_need_more_on_truncation() {
        for len in 1..REAL_ZFILE_AND_METADATA.len().min(11) {
            assert!(matches!(
                scan_header(&REAL_ZFILE_AND_METADATA[..len]),
                HeaderOutcome::NeedMore
            ));
        }
    }

    #[test]
    fn encode_hex_header_round_trips_through_scan() {
        let header = Header::with_flags(ZRINIT, CANFDX | CANOVIO | CANFC32);
        let encoded = encode_hex_header(header);
        // Matches the real rz capture exactly: "**\x18B0100000023be50\r\n\x8a\x11"-ish.
        assert_eq!(&encoded[..7], b"**\x18B010");
        match scan_header(&encoded) {
            HeaderOutcome::Found {
                header: decoded,
                crc_kind,
                consumed,
            } => {
                assert_eq!(decoded, header);
                assert_eq!(crc_kind, CrcKind::Crc16);
                assert_eq!(consumed, encoded.len());
            }
            _ => panic!("expected the freshly encoded header to decode"),
        }
    }

    #[test]
    fn encode_bin32_header_round_trips_through_scan() {
        let header = Header::with_position(ZEOF, 20000);
        let encoded = encode_bin32_header(header);
        assert_eq!(&encoded[..3], &[ZPAD, ZDLE, ZBIN32]);
        match scan_header(&encoded) {
            HeaderOutcome::Found {
                header: decoded,
                crc_kind,
                consumed,
            } => {
                assert_eq!(decoded, header);
                assert_eq!(decoded.position(), 20000);
                assert_eq!(crc_kind, CrcKind::Crc32);
                assert_eq!(consumed, encoded.len());
            }
            _ => panic!("expected the freshly encoded header to decode"),
        }
    }

    #[test]
    fn data_subpacket_round_trips_with_zdle_at_every_offset() {
        for crc_kind in [CrcKind::Crc16, CrcKind::Crc32] {
            for offset in 0..32 {
                let mut payload = vec![0x41u8; 32];
                payload[offset] = ZDLE; // Force an escape at every possible position.
                let encoded = encode_data_subpacket(&payload, ZCRCW, crc_kind);
                match scan_subpacket(&encoded, crc_kind) {
                    SubpacketOutcome::Found {
                        payload: decoded,
                        ender,
                        consumed,
                    } => {
                        assert_eq!(decoded, payload);
                        assert_eq!(ender, ZCRCW);
                        assert_eq!(consumed, encoded.len());
                    }
                    _ => panic!("expected subpacket to round-trip (offset {offset}, {crc_kind:?})"),
                }
            }
        }
    }

    #[test]
    fn data_subpacket_escapes_cr_only_right_after_at_sign() {
        // Matches the real capture's observed behavior: CR is sent raw
        // almost always, only escaped when immediately preceded by '@'.
        let payload = vec![b'@', CR, b'x', CR];
        let encoded = encode_data_subpacket(&payload, ZCRCE, CrcKind::Crc32);
        // "@" (raw) + ZDLE-escaped CR + "x" (raw) + raw CR (no preceding '@').
        assert_eq!(
            &encoded[..5],
            &[b'@', ZDLE, CR ^ ZDLE_ESCAPE_MASK, b'x', CR]
        );
    }

    #[test]
    fn corrupted_subpacket_is_reported_as_corrupt_not_need_more() {
        let payload = vec![1, 2, 3];
        let mut encoded = encode_data_subpacket(&payload, ZCRCE, CrcKind::Crc32);
        let last = encoded.len() - 1;
        encoded[last] ^= 0xff; // flip a CRC bit
        assert!(matches!(
            scan_subpacket(&encoded, CrcKind::Crc32),
            SubpacketOutcome::Corrupt { .. }
        ));
    }

    /// A real local-PTY capture showed a stray `\r` land between two real
    /// headers (sz's human-readable status text shares the tty fd with the
    /// protocol bytes once there's no separate stderr, unlike a plain pipe)
    /// — `scan_header` must skip it and find the header that follows rather
    /// than getting stuck waiting for it to somehow become a `ZPAD`.
    #[test]
    fn header_scan_skips_stray_bytes_before_a_real_header() {
        let header = Header::with_position(ZFIN, 0);
        let encoded = encode_hex_header(header);

        let mut garbled = vec![b'\r'];
        garbled.extend_from_slice(&encoded);

        match scan_header(&garbled) {
            HeaderOutcome::Corrupt { consumed } => assert_eq!(consumed, 1),
            other => panic!("expected the leading byte to be reported as corrupt, got a header/need-more instead: {other:?}"),
        }
        match scan_header(&garbled[1..]) {
            HeaderOutcome::Found {
                header: decoded, ..
            } => assert_eq!(decoded, header),
            _ => panic!("expected the real header to decode once the stray byte is dropped"),
        }
    }

    #[test]
    fn header_scan_discards_a_chunk_with_no_pad_at_all() {
        assert!(matches!(
            scan_header(b"Transfer complete\r\n"),
            HeaderOutcome::Corrupt { consumed: 19 }
        ));
    }
}
