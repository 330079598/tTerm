//! End-to-end tests against the real `sz`/`rz` binaries from `lrzsz`
//! (`brew install lrzsz` / `apt install lrzsz`), not just our own encoder
//! and decoder talking to each other. These are `#[ignore]`d by default so
//! `cargo test` stays green on machines without `lrzsz` installed (CI,
//! other contributors); run them explicitly with:
//!
//! ```sh
//! cargo test --lib -- --ignored real_interop
//! ```
//!
//! Per-test scope note: this exercises the protocol engine directly against
//! a real peer process over a plain pipe. It does not exercise the
//! terminal-byte-stream detection hook or the Tauri command/session layer,
//! since those don't exist yet (later milestones); it is the strongest
//! interop check available for the protocol core itself.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use crate::zmodem::protocol::{ZmodemAction, ZmodemDirection, ZmodemEngine};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let mut path = std::env::temp_dir();
        path.push(format!("{prefix}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn require_binary(name: &str) {
    let found = Command::new("which")
        .arg(name)
        .stdout(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !found {
        panic!("`{name}` not found on PATH; install lrzsz to run this test (`brew install lrzsz`)");
    }
}

/// Drives our receive-direction engine against a live `sz` child process
/// sending `expected` over a plain pipe (no pty), returning the bytes we
/// actually reconstructed.
fn receive_via_real_sz(expected: &[u8]) -> Vec<u8> {
    require_binary("sz");
    let dir = TempDir::new("tterm-zmodem-sz");
    let src = dir.path().join("payload.bin");
    std::fs::write(&src, expected).expect("write source file");

    let mut child = Command::new("sz")
        .arg("--binary")
        .arg("payload.bin")
        .current_dir(dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn sz");

    let received = pump_engine(&mut child, ZmodemDirection::Receive);
    let _ = child.wait();
    received
}

/// Drives our send-direction engine against a live `rz` child process,
/// sending `payload` to it, then returns what `rz` actually wrote to disk.
fn send_via_real_rz(payload: &[u8]) -> Vec<u8> {
    require_binary("rz");
    let dir = TempDir::new("tterm-zmodem-rz");

    let mut child = Command::new("rz")
        .arg("--binary")
        .current_dir(dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn rz");

    let mut engine = ZmodemEngine::new(ZmodemDirection::Send);
    let mut child_stdin = child.stdin.take().unwrap();
    let mut child_stdout = child.stdout.take().unwrap();

    // The first bytes read are rz's ZRINIT invite; announce our (only) file
    // right away so it's waiting by the time rz replies with ZRPOS.
    let mut sent_kickoff = false;
    let mut buf = [0u8; 8192];
    loop {
        let n = child_stdout.read(&mut buf).unwrap_or(0);
        if n == 0 {
            break;
        }
        let result = engine.feed(&buf[..n]);
        if !result.outgoing.is_empty() {
            child_stdin.write_all(&result.outgoing).ok();
            child_stdin.flush().ok();
        }
        if !sent_kickoff {
            sent_kickoff = true;
            let kickoff = engine.begin_send("out.bin", payload.len() as u64);
            child_stdin.write_all(&kickoff).ok();
            child_stdin.flush().ok();
        }
        for action in &result.actions {
            match action {
                ZmodemAction::ResendFrom(pos) => {
                    send_remaining(&mut engine, &mut child_stdin, payload, *pos);
                }
                // We only ever have one file queued, so once rz confirms it
                // (by sending ZRINIT again), end the session.
                ZmodemAction::FileComplete => {
                    let fin = engine.finish_send();
                    child_stdin.write_all(&fin).ok();
                    child_stdin.flush().ok();
                }
                _ => {}
            }
        }
        if engine.is_done() {
            break;
        }
    }
    drop(child_stdin);
    let _ = child.wait();
    std::fs::read(dir.path().join("out.bin")).unwrap_or_default()
}

fn send_remaining(engine: &mut ZmodemEngine, out: &mut impl Write, payload: &[u8], from: u64) {
    let chunk_size = 1024usize;
    let mut pos = from as usize;
    if payload.is_empty() {
        if let Some(bytes) = engine.send_chunk(0, &[], true) {
            out.write_all(&bytes).ok();
            out.flush().ok();
        }
        return;
    }
    while pos < payload.len() {
        let end = (pos + chunk_size).min(payload.len());
        let is_last = end == payload.len();
        if let Some(bytes) = engine.send_chunk(pos as u64, &payload[pos..end], is_last) {
            out.write_all(&bytes).ok();
            out.flush().ok();
        }
        pos = end;
    }
}

fn pump_engine(child: &mut Child, expected_direction: ZmodemDirection) -> Vec<u8> {
    let mut child_stdin = child.stdin.take().unwrap();
    let mut child_stdout = child.stdout.take().unwrap();
    let mut received = Vec::new();
    let mut buf = [0u8; 8192];
    // Real `sz` prints a "rz\r" hint on stdout *before* its ZRQINIT header
    // (confirmed by capture — see `frame.rs`'s doc comment), so a fresh
    // engine can't just start at byte 0 the way production code doesn't
    // either: production always routes bytes through `detect::scan` first
    // and only constructs an engine once it reports a trigger. Mirror that
    // here instead of assuming the child's very first byte is a header.
    let mut carry = Vec::new();
    let mut engine: Option<ZmodemEngine> = None;
    loop {
        let n = child_stdout.read(&mut buf).unwrap_or(0);
        if n == 0 {
            break;
        }
        let result = match engine.as_mut() {
            Some(e) => e.feed(&buf[..n]),
            None => {
                let outcome = crate::zmodem::detect::scan(&mut carry, &buf[..n]);
                match outcome.triggered {
                    Some((direction, remainder)) => {
                        assert_eq!(
                            direction, expected_direction,
                            "unexpected trigger direction"
                        );
                        let mut e = ZmodemEngine::new(direction);
                        let result = e.feed(&remainder);
                        engine = Some(e);
                        result
                    }
                    None => continue,
                }
            }
        };
        if !result.outgoing.is_empty() {
            child_stdin.write_all(&result.outgoing).ok();
            child_stdin.flush().ok();
        }
        for action in result.actions {
            match action {
                ZmodemAction::IncomingFile { .. } => {
                    let bytes = engine.as_mut().unwrap().accept_file(0);
                    child_stdin.write_all(&bytes).ok();
                    child_stdin.flush().ok();
                }
                ZmodemAction::DataChunk(chunk) => received.extend_from_slice(&chunk),
                _ => {}
            }
        }
        if engine.as_ref().map(ZmodemEngine::is_done).unwrap_or(false) {
            break;
        }
    }
    drop(child_stdin);
    received
}

#[test]
#[ignore = "requires the real `sz` binary from lrzsz"]
fn engine_receives_a_binary_file_from_real_sz() {
    let expected: Vec<u8> = (0..50_000u32).map(|i| (i % 251) as u8).collect();
    let received = receive_via_real_sz(&expected);
    assert_eq!(received, expected);
}

#[test]
#[ignore = "requires the real `sz` binary from lrzsz"]
fn engine_receives_a_small_text_file_from_real_sz() {
    let expected = b"hello from a real sz process\r\nwith a second line\n".to_vec();
    let received = receive_via_real_sz(&expected);
    assert_eq!(received, expected);
}

#[test]
#[ignore = "requires the real `rz` binary from lrzsz"]
fn engine_sends_a_binary_file_to_real_rz() {
    let payload: Vec<u8> = (0..30_000u32).map(|i| ((i * 7) % 256) as u8).collect();
    let written = send_via_real_rz(&payload);
    assert_eq!(written, payload);
}
