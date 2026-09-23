//! The push-based ZMODEM state machine. Pure and I/O-free: callers feed it
//! raw incoming bytes and get back bytes to write and high-level actions
//! (file offered, chunk received, resend requested, ...); actually reading
//! or writing the local file is the caller's job, since the two call sites
//! that will eventually drive this (the local-PTY reader thread and the
//! async SSH channel task) write outgoing bytes through completely
//! different mechanisms and neither hands this engine a `Read`/`Write`
//! stream to block on.

use super::consts::*;
use super::frame::{
    encode_bin32_header, encode_data_subpacket, encode_hex_header, scan_header, scan_subpacket,
    CrcKind, Header, HeaderOutcome, SubpacketOutcome,
};

/// The capability bitmask tTerm advertises when acting as a receiver,
/// matching real `rz`'s own `ZRINIT` exactly (`CANFDX|CANOVIO|CANFC32`,
/// captured as `0x23`).
const OUR_CAPABILITIES: u8 = CANFDX | CANOVIO | CANFC32;
/// Consecutive `CAN` bytes recognized as an abort signal, matching the
/// classic ZMODEM convention (real senders emit 5, but shells/humans often
/// send fewer via Ctrl-C-Ctrl-C; treat any run of 4+ as unambiguous).
const ABORT_RUN_LEN: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZmodemAction {
    /// The peer offered a file (receive direction). The caller should open
    /// the destination path and then call `accept_file` or `skip_file`.
    IncomingFile { name: String, size: u64 },
    /// A chunk of file data was decoded and CRC-verified (receive
    /// direction); append it to the open destination file.
    DataChunk(Vec<u8>),
    /// The file currently being transferred finished successfully.
    FileComplete,
    /// The peer asked to resume from `offset` (send direction): seek the
    /// local file reader there and resume calling `send_chunk`.
    ResendFrom(u64),
    /// The whole ZMODEM session ended normally.
    SessionEnded,
    /// The peer aborted the transfer (a run of `CAN` bytes, or `ZABORT`).
    PeerCancelled,
}

#[derive(Debug, Default)]
pub struct ZmodemFeedResult {
    pub outgoing: Vec<u8>,
    pub actions: Vec<ZmodemAction>,
    /// Bytes that arrived after the session had already ended (e.g. the
    /// shell prompt printed right after the peer's `rz`/`sz` exits) and
    /// should be forwarded to the terminal as ordinary output.
    pub passthrough: Vec<u8>,
}

impl ZmodemFeedResult {
    fn push_frame(&mut self, bytes: Vec<u8>) {
        self.outgoing.extend_from_slice(&bytes);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    RecvAwaitingFile,
    /// A `ZFILE` was parsed and its metadata subpacket accepted; waiting for
    /// the caller to open the destination and call `accept_file`.
    RecvAwaitingAccept {
        name: String,
        size: u64,
    },
    RecvReceivingData {
        crc_kind: CrcKind,
        position: u64,
    },
    /// Constructed, but no confirmed `ZRINIT` from the peer yet: `begin_send`
    /// must not succeed here, even though nothing else distinguishes this
    /// from `SendAwaitingKickoff` structurally — without this state, a
    /// caller that (incorrectly, or racily) calls `begin_send` before ever
    /// feeding a real `ZRINIT` would succeed anyway, since a fresh engine
    /// starts "ready" by construction. Confirmed as a real failure mode: a
    /// real-PTY test where `sz`'s human-readable status text and its actual
    /// `ZRINIT` bytes arrived as separate reads sent `ZFILE` off the first
    /// (non-protocol) read, before the real `ZRINIT` was ever seen — real
    /// `rz` responded `ZNAK` and the session hung waiting for a `ZRPOS`
    /// that was never coming.
    SendIdle,
    SendAwaitingKickoff,
    /// `ZFILE` sent, waiting for the peer's `ZRPOS` before any data may flow.
    SendAwaitingPos,
    /// A `ZRPOS` (initial or a mid-stream resend request) was just seen: the
    /// *next* `send_chunk` call must open a fresh `ZDATA` header at
    /// `position` before its subpacket, since a resumed run always needs
    /// its own header per the protocol.
    SendReadyAt {
        crc_kind: CrcKind,
        position: u64,
    },
    /// Mid-run: subsequent `send_chunk` calls need only a subpacket, no
    /// new header, until the next `ZRPOS` or the file ends.
    SendStreaming {
        crc_kind: CrcKind,
        position: u64,
    },
    SendAwaitingNext,
    SendAwaitingFinEcho,
    Done,
}

pub struct ZmodemEngine {
    direction: ZmodemDirection,
    inbuf: Vec<u8>,
    /// Once a `ZDATA`/`ZFILE` header opens a run of subpackets, incoming
    /// bytes are subpackets (not headers) until one ends with `ZCRCE`.
    expect_subpacket: Option<CrcKind>,
    state: State,
    /// The `ZFILE` announcement bytes from the most recent `begin_send`,
    /// kept so a `ZNAK` while waiting for `ZRPOS` can retransmit it. Real
    /// evidence this matters, found via a real local-PTY test against real
    /// `rz`: a structurally-valid `ZFILE` sent immediately after detecting
    /// the trigger got `ZNAK`'d because the pty hadn't been switched to raw
    /// mode yet (cooked-mode echo/flow-control/CRNL translation corrupts
    /// 8-bit binary data) — `rz` sets raw mode on its own stdin shortly
    /// after starting, but there is no guarantee it has finished before our
    /// reply arrives, especially over a zero-latency local pty. Retrying is
    /// exactly what `ZNAK` is for, and self-heals once raw mode is set.
    last_zfile: Option<Vec<u8>>,
}

impl ZmodemEngine {
    pub fn new(direction: ZmodemDirection) -> Self {
        let state = match direction {
            ZmodemDirection::Receive => State::RecvAwaitingFile,
            ZmodemDirection::Send => State::SendIdle,
        };
        Self {
            direction,
            inbuf: Vec::new(),
            expect_subpacket: None,
            state,
            last_zfile: None,
        }
    }

    #[cfg(test)]
    pub fn is_done(&self) -> bool {
        matches!(self.state, State::Done)
    }

    /// Feeds raw incoming bytes and drains as many complete headers/
    /// subpackets as the buffer currently allows.
    pub fn feed(&mut self, chunk: &[u8]) -> ZmodemFeedResult {
        self.inbuf.extend_from_slice(chunk);
        let mut result = ZmodemFeedResult::default();
        loop {
            if self.state == State::Done {
                result.passthrough.extend_from_slice(&self.inbuf);
                self.inbuf.clear();
                return result;
            }
            if let Some(consumed) = self.detect_abort() {
                self.inbuf.drain(..consumed);
                self.state = State::Done;
                result.actions.push(ZmodemAction::PeerCancelled);
                continue;
            }
            let made_progress = match self.expect_subpacket {
                Some(crc_kind) => self.step_subpacket(crc_kind, &mut result),
                None => self.step_header(&mut result),
            };
            if !made_progress {
                return result;
            }
        }
    }

    fn detect_abort(&self) -> Option<usize> {
        let run = self.inbuf.iter().take_while(|&&b| b == ZDLE).count();
        if run >= ABORT_RUN_LEN {
            Some(run)
        } else {
            None
        }
    }

    fn step_header(&mut self, result: &mut ZmodemFeedResult) -> bool {
        match scan_header(&self.inbuf) {
            HeaderOutcome::Found {
                header,
                crc_kind,
                consumed,
            } => {
                self.inbuf.drain(..consumed);
                self.handle_header(header, crc_kind, result);
                true
            }
            HeaderOutcome::Corrupt { consumed } => {
                self.inbuf.drain(..consumed);
                true
            }
            HeaderOutcome::NeedMore => false,
        }
    }

    fn step_subpacket(&mut self, crc_kind: CrcKind, result: &mut ZmodemFeedResult) -> bool {
        match scan_subpacket(&self.inbuf, crc_kind) {
            SubpacketOutcome::Found {
                payload,
                ender,
                consumed,
            } => {
                self.inbuf.drain(..consumed);
                self.handle_subpacket(payload, ender, crc_kind, result);
                true
            }
            SubpacketOutcome::Corrupt { consumed } => {
                self.inbuf.drain(..consumed);
                self.handle_corrupt_subpacket(result);
                true
            }
            SubpacketOutcome::NeedMore => false,
        }
    }

    fn handle_header(&mut self, header: Header, crc_kind: CrcKind, result: &mut ZmodemFeedResult) {
        match header.frame_type {
            ZRQINIT if self.state == State::RecvAwaitingFile => {
                result.push_frame(encode_hex_header(Header::with_flags(
                    ZRINIT,
                    OUR_CAPABILITIES,
                )));
            }
            ZFILE if self.state == State::RecvAwaitingFile => {
                self.expect_subpacket = Some(crc_kind);
                // The name/size aren't known until the metadata subpacket
                // (handled in `handle_subpacket`) arrives right after this.
            }
            ZDATA if matches!(self.state, State::RecvReceivingData { .. }) => {
                if let State::RecvReceivingData { position, .. } = &mut self.state {
                    *position = header.position();
                }
                self.expect_subpacket = Some(crc_kind);
            }
            ZEOF if matches!(self.state, State::RecvReceivingData { .. }) => {
                result.actions.push(ZmodemAction::FileComplete);
                result.push_frame(encode_hex_header(Header::with_flags(
                    ZRINIT,
                    OUR_CAPABILITIES,
                )));
                self.state = State::RecvAwaitingFile;
            }
            ZFIN if self.direction == ZmodemDirection::Receive => {
                result.push_frame(encode_hex_header(Header::with_position(ZFIN, 0)));
                result.actions.push(ZmodemAction::SessionEnded);
                self.state = State::Done;
            }
            ZRINIT if matches!(self.state, State::SendIdle | State::SendAwaitingKickoff) => {
                // Confirmed: the peer is really there and ready. Caller now
                // calls `begin_send`; nothing to send yet ourselves, since we
                // don't know the filename/size here. Tolerates the peer
                // retrying its invite (a second ZRINIT while still in
                // SendAwaitingKickoff) as a harmless no-op.
                self.state = State::SendAwaitingKickoff;
            }
            ZRPOS
                if matches!(
                    self.state,
                    State::SendAwaitingPos
                        | State::SendReadyAt { .. }
                        | State::SendStreaming { .. }
                        | State::SendAwaitingNext
                ) =>
            {
                let crc_kind = match &self.state {
                    State::SendReadyAt { crc_kind, .. } | State::SendStreaming { crc_kind, .. } => {
                        *crc_kind
                    }
                    _ => CrcKind::Crc32,
                };
                self.state = State::SendReadyAt {
                    crc_kind,
                    position: header.position(),
                };
                result
                    .actions
                    .push(ZmodemAction::ResendFrom(header.position()));
            }
            ZNAK if self.state == State::SendAwaitingPos => {
                // The peer couldn't validate our ZFILE (see `last_zfile`'s
                // doc comment for why this reliably happens against real
                // `rz` over a fresh local pty) — resend it verbatim.
                if let Some(bytes) = &self.last_zfile {
                    result.outgoing.extend_from_slice(bytes);
                }
            }
            ZRINIT if self.state == State::SendAwaitingPos => {
                // The peer restarted its invite instead of NAK'ing (observed
                // from real `rz` after the same corrupted-ZFILE scenario
                // `ZNAK` above handles) — same recovery, resend ZFILE.
                if let Some(bytes) = &self.last_zfile {
                    result.outgoing.extend_from_slice(bytes);
                }
            }
            ZNAK if self.state == State::SendAwaitingFinEcho => {
                result
                    .outgoing
                    .extend_from_slice(&encode_hex_header(Header::with_position(ZFIN, 0)));
            }
            ZRINIT if matches!(self.state, State::SendAwaitingNext) => {
                // Peer confirmed the previous file and is ready for another
                // (or, if the caller has none left, for us to send ZFIN).
                result.actions.push(ZmodemAction::FileComplete);
                self.state = State::SendAwaitingKickoff;
            }
            ZFIN if self.direction == ZmodemDirection::Send
                && self.state == State::SendAwaitingFinEcho =>
            {
                // Peer echoed our ZFIN; "OO" is the literal (non-framed)
                // sign-off real `sz` sends, appended by the caller.
                result.outgoing.extend_from_slice(b"OO");
                result.actions.push(ZmodemAction::SessionEnded);
                self.state = State::Done;
            }
            ZABORT | ZCAN => {
                result.actions.push(ZmodemAction::PeerCancelled);
                self.state = State::Done;
            }
            _ => {
                // Unexpected frame for the current state: ignore rather than
                // fail the whole session, mirroring real implementations'
                // tolerance of stray/duplicate control frames.
            }
        }
    }

    fn handle_subpacket(
        &mut self,
        payload: Vec<u8>,
        ender: u8,
        crc_kind: CrcKind,
        result: &mut ZmodemFeedResult,
    ) {
        match &self.state {
            State::RecvAwaitingFile => {
                // This is the ZFILE metadata subpacket.
                let (name, size) = parse_file_metadata(&payload);
                self.expect_subpacket = None;
                self.state = State::RecvAwaitingAccept {
                    name: name.clone(),
                    size,
                };
                result
                    .actions
                    .push(ZmodemAction::IncomingFile { name, size });
            }
            State::RecvReceivingData { position, .. } => {
                let position = *position + payload.len() as u64;
                result.actions.push(ZmodemAction::DataChunk(payload));
                self.state = State::RecvReceivingData { crc_kind, position };
                if matches!(ender, ZCRCE | ZCRCW) {
                    self.expect_subpacket = None;
                }
                if ender == ZCRCW {
                    result.push_frame(encode_hex_header(Header::with_position(ZACK, position)));
                }
            }
            _ => {
                // A subpacket arriving outside receive-data flow (e.g. after
                // a cancel raced with in-flight bytes) is simply dropped.
                self.expect_subpacket = None;
            }
        }
    }

    fn handle_corrupt_subpacket(&mut self, result: &mut ZmodemFeedResult) {
        self.expect_subpacket = None;
        if let State::RecvReceivingData { position, .. } = &self.state {
            result.push_frame(encode_hex_header(Header::with_position(ZRPOS, *position)));
        }
    }

    /// Receive direction: accept the file just offered via
    /// `ZmodemAction::IncomingFile`, telling the peer to start sending from
    /// `resume_from` (always 0 for v1 — no crash-recovery across restarts).
    pub fn accept_file(&mut self, resume_from: u64) -> Vec<u8> {
        match &self.state {
            State::RecvAwaitingAccept { .. } => {
                self.state = State::RecvReceivingData {
                    crc_kind: CrcKind::Crc32,
                    position: resume_from,
                };
                // A `ZDATA` header (handled in `handle_header`) must arrive
                // before any subpacket; stay in header-scanning mode.
                encode_hex_header(Header::with_position(ZRPOS, resume_from))
            }
            _ => Vec::new(),
        }
    }

    /// Receive direction: decline the file just offered.
    pub fn skip_file(&mut self) -> Vec<u8> {
        match &self.state {
            State::RecvAwaitingAccept { .. } => {
                self.state = State::RecvAwaitingFile;
                encode_hex_header(Header::with_flags(ZSKIP, 0))
            }
            _ => Vec::new(),
        }
    }

    /// Send direction: announce a file once the peer's `ZRINIT` has been
    /// seen (`ZmodemDirection::Send` transitions to `SendAwaitingKickoff`
    /// automatically on detection).
    pub fn begin_send(&mut self, name: &str, size: u64) -> Vec<u8> {
        if self.state != State::SendAwaitingKickoff {
            return Vec::new();
        }
        self.state = State::SendAwaitingPos;
        let mut out = encode_bin32_header(Header::with_flags(ZFILE, 0));
        let mut payload = Vec::with_capacity(name.len() + 24);
        payload.extend_from_slice(name.as_bytes());
        payload.push(0);
        payload.extend_from_slice(format!("{size} 0 0 0 0 0").as_bytes());
        out.extend_from_slice(&encode_data_subpacket(&payload, ZCRCW, CrcKind::Crc32));
        self.last_zfile = Some(out.clone());
        out
    }

    /// Send direction: send one chunk of file data. `is_last` marks the
    /// final chunk of the current file (ended `ZCRCE` instead of `ZCRCG`,
    /// matching real `sz`'s streaming style — see `frame.rs`'s doc comment).
    /// Returns `None` if called outside `SendStreaming`/`SendAwaitingPos`
    /// (e.g. a resend request hasn't been applied yet).
    pub fn send_chunk(&mut self, offset: u64, data: &[u8], is_last: bool) -> Option<Vec<u8>> {
        let (crc_kind, needs_header) = match &self.state {
            State::SendReadyAt { crc_kind, .. } => (*crc_kind, true),
            State::SendStreaming { crc_kind, .. } => (*crc_kind, false),
            _ => return None,
        };
        let mut out = Vec::new();
        if needs_header {
            out.extend_from_slice(&encode_bin32_header(Header::with_position(ZDATA, offset)));
        }
        let ender = if is_last { ZCRCE } else { ZCRCG };
        out.extend_from_slice(&encode_data_subpacket(data, ender, crc_kind));
        let new_position = offset + data.len() as u64;
        self.state = State::SendStreaming {
            crc_kind,
            position: new_position,
        };
        if is_last {
            out.extend_from_slice(&encode_bin32_header(Header::with_position(
                ZEOF,
                new_position,
            )));
            self.state = State::SendAwaitingNext;
        }
        Some(out)
    }

    /// Send direction: no more files to offer; ends the session.
    pub fn finish_send(&mut self) -> Vec<u8> {
        if self.state != State::SendAwaitingKickoff {
            return Vec::new();
        }
        self.state = State::SendAwaitingFinEcho;
        encode_hex_header(Header::with_position(ZFIN, 0))
    }
}

/// Parses a `ZFILE` metadata subpacket's payload: a NUL-terminated filename
/// followed by an optional space-separated `<size> <mtime> <mode> ...`
/// string (also NUL-terminated in the general case, though real `sz` was
/// observed ending the subpacket right after the last digit with no
/// trailing NUL). Unknown/missing fields default to 0, matching how a
/// receiver must tolerate senders that omit them.
fn parse_file_metadata(payload: &[u8]) -> (String, u64) {
    let mut parts = payload.splitn(2, |&b| b == 0);
    let name = parts
        .next()
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
        .unwrap_or_default();
    let size = parts
        .next()
        .and_then(|rest| {
            let rest = rest.split(|&b| b == 0).next().unwrap_or(rest);
            let text = String::from_utf8_lossy(rest);
            text.split_whitespace().next().map(|s| s.to_string())
        })
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    (name, size)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recv_engine() -> ZmodemEngine {
        ZmodemEngine::new(ZmodemDirection::Receive)
    }

    fn send_engine() -> ZmodemEngine {
        ZmodemEngine::new(ZmodemDirection::Send)
    }

    /// Full receive walkthrough: peer sends ZRQINIT, ZFILE+metadata, ZDATA +
    /// two subpackets (one CRCG, one CRCE), ZEOF, ZFIN — mirroring exactly
    /// what a real `sz` sends for a small file.
    #[test]
    fn receive_walkthrough_matches_real_sz_sequence() {
        let mut engine = recv_engine();

        let r = engine.feed(&encode_hex_header(Header::with_flags(ZRQINIT, 0)));
        assert!(matches!(engine_last_frame_type(&r.outgoing), Some(ZRINIT)));

        let mut zfile = encode_bin32_header(Header::with_flags(ZFILE, 0));
        let metadata = b"hello.bin\x0011 0 0 0 0 0";
        zfile.extend_from_slice(&encode_data_subpacket(metadata, ZCRCW, CrcKind::Crc32));
        let r = engine.feed(&zfile);
        assert_eq!(
            r.actions,
            vec![ZmodemAction::IncomingFile {
                name: "hello.bin".into(),
                size: 11
            }]
        );
        assert!(
            r.outgoing.is_empty(),
            "must wait for accept_file before replying"
        );

        let accept = engine.accept_file(0);
        assert!(matches!(engine_last_frame_type(&accept), Some(ZRPOS)));

        let mut data = encode_bin32_header(Header::with_position(ZDATA, 0));
        data.extend_from_slice(&encode_data_subpacket(b"hello ", ZCRCG, CrcKind::Crc32));
        data.extend_from_slice(&encode_data_subpacket(b"world", ZCRCE, CrcKind::Crc32));
        let r = engine.feed(&data);
        assert_eq!(
            r.actions,
            vec![
                ZmodemAction::DataChunk(b"hello ".to_vec()),
                ZmodemAction::DataChunk(b"world".to_vec()),
            ]
        );

        let r = engine.feed(&encode_bin32_header(Header::with_position(ZEOF, 11)));
        assert_eq!(r.actions, vec![ZmodemAction::FileComplete]);
        assert!(matches!(engine_last_frame_type(&r.outgoing), Some(ZRINIT)));

        let r = engine.feed(&encode_hex_header(Header::with_position(ZFIN, 0)));
        assert_eq!(r.actions, vec![ZmodemAction::SessionEnded]);
        assert!(matches!(engine_last_frame_type(&r.outgoing), Some(ZFIN)));
        assert!(engine.is_done());

        // Trailing "OO" (or anything else) after the session ends must pass
        // through untouched rather than being parsed as protocol bytes.
        let r = engine.feed(b"OO$ ");
        assert_eq!(r.passthrough, b"OO$ ");
        assert!(r.outgoing.is_empty() && r.actions.is_empty());
    }

    #[test]
    fn receive_handles_corrupt_subpacket_with_zrpos_resend() {
        let mut engine = recv_engine();
        engine.feed(&encode_hex_header(Header::with_flags(ZRQINIT, 0)));

        let mut zfile = encode_bin32_header(Header::with_flags(ZFILE, 0));
        zfile.extend_from_slice(&encode_data_subpacket(
            b"f\x005 0 0 0 0 0",
            ZCRCW,
            CrcKind::Crc32,
        ));
        engine.feed(&zfile);
        engine.accept_file(0);

        let mut data = encode_bin32_header(Header::with_position(ZDATA, 0));
        let mut good = encode_data_subpacket(b"abcde", ZCRCE, CrcKind::Crc32);
        *good.last_mut().unwrap() ^= 0xff; // corrupt the CRC
        data.extend_from_slice(&good);
        let r = engine.feed(&data);
        assert!(
            r.actions.is_empty(),
            "a corrupt subpacket must not surface as a DataChunk"
        );
        assert!(matches!(engine_last_frame_type(&r.outgoing), Some(ZRPOS)));
    }

    /// Full send walkthrough: peer's ZRINIT triggers `begin_send`, then the
    /// caller drives `send_chunk` until the file is done and finally calls
    /// `finish_send`.
    #[test]
    fn send_walkthrough_produces_a_valid_frame_sequence() {
        let mut engine = send_engine();
        engine.feed(&encode_hex_header(Header::with_flags(
            ZRINIT,
            CANFDX | CANFC32,
        )));

        let file_frames = engine.begin_send("out.bin", 10);
        assert!(!file_frames.is_empty());

        let r = engine.feed(&encode_hex_header(Header::with_position(ZRPOS, 0)));
        assert_eq!(r.actions, vec![ZmodemAction::ResendFrom(0)]);

        let chunk1 = engine.send_chunk(0, b"12345", false).unwrap();
        assert!(!chunk1.is_empty());
        let chunk2 = engine.send_chunk(5, b"67890", true).unwrap();
        assert!(matches!(engine_last_frame_type(&chunk2), Some(ZEOF)));

        let r = engine.feed(&encode_hex_header(Header::with_flags(
            ZRINIT,
            CANFDX | CANFC32,
        )));
        assert_eq!(r.actions, vec![ZmodemAction::FileComplete]);

        let fin = engine.finish_send();
        assert!(matches!(engine_last_frame_type(&fin), Some(ZFIN)));

        let r = engine.feed(&encode_hex_header(Header::with_position(ZFIN, 0)));
        assert_eq!(r.actions, vec![ZmodemAction::SessionEnded]);
        assert!(r.outgoing.ends_with(b"OO"));
        assert!(engine.is_done());
    }

    #[test]
    fn send_mid_stream_zrpos_reroutes_the_caller() {
        let mut engine = send_engine();
        engine.feed(&encode_hex_header(Header::with_flags(ZRINIT, 0)));
        engine.begin_send("out.bin", 10);
        engine.feed(&encode_hex_header(Header::with_position(ZRPOS, 0)));
        engine.send_chunk(0, b"12345", false).unwrap();

        // Peer detected a problem and wants a resend from byte 2.
        let r = engine.feed(&encode_hex_header(Header::with_position(ZRPOS, 2)));
        assert_eq!(r.actions, vec![ZmodemAction::ResendFrom(2)]);
        let resent = engine.send_chunk(2, b"345", false).unwrap();
        assert!(!resent.is_empty());
    }

    #[test]
    fn abort_run_of_can_bytes_ends_the_session() {
        let mut engine = recv_engine();
        engine.feed(&encode_hex_header(Header::with_flags(ZRQINIT, 0)));
        let r = engine.feed(&[ZDLE; 5]);
        assert_eq!(r.actions, vec![ZmodemAction::PeerCancelled]);
        assert!(engine.is_done());
    }

    #[test]
    fn bytes_split_across_many_tiny_feeds_still_decode() {
        let mut engine = recv_engine();
        let full = encode_hex_header(Header::with_flags(ZRQINIT, 0));
        let mut last = ZmodemFeedResult::default();
        for byte in &full {
            let r = engine.feed(std::slice::from_ref(byte));
            if !r.outgoing.is_empty() {
                last = r;
            }
        }
        assert!(matches!(
            engine_last_frame_type(&last.outgoing),
            Some(ZRINIT)
        ));
    }

    /// Finds the frame type of the header that ends exactly at the end of
    /// `bytes`, regardless of what precedes it (a data subpacket, in the
    /// case of a `send_chunk` result that appends `ZEOF` after the data).
    fn engine_last_frame_type(bytes: &[u8]) -> Option<u8> {
        for start in 0..bytes.len() {
            if let HeaderOutcome::Found {
                header, consumed, ..
            } = scan_header(&bytes[start..])
            {
                if start + consumed == bytes.len() {
                    return Some(header.frame_type);
                }
            }
        }
        None
    }
}
