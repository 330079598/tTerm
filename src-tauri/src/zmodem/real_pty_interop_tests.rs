//! End-to-end test of the actual M2 wiring — detection, the protocol
//! engine, destination-file I/O, and write-back — driven against a REAL
//! local PTY (via `portable_pty`, exactly like `terminal::pty::
//! spawn_local_pty`) running a REAL `sz` process. This is the strongest
//! available test of the wiring itself, not just the protocol engine (which
//! `real_interop_tests.rs` already covers via plain subprocess pipes).
//!
//! `#[ignore]`d by default (requires `lrzsz`); run explicitly with:
//! ```sh
//! cargo test --lib -- --ignored real_pty_interop
//! ```

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize};

use crate::core::ZmodemMap;
use crate::zmodem::session::ZmodemReceiveDriver;

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
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn require_binary(name: &str) {
    let found = std::process::Command::new("which")
        .arg(name)
        .stdout(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !found {
        panic!("`{name}` not found on PATH; install lrzsz to run this test (`brew install lrzsz`)");
    }
}

#[test]
#[ignore = "requires the real `sz` binary from lrzsz, and spawns a real local PTY"]
fn wired_driver_receives_a_file_from_real_sz_over_a_real_pty() {
    receive_from_real_sz_over_pty(true);
}

/// The M6 fallback path: auto-detect is switched off in settings, and the
/// user armed a one-shot manual trigger (menu/shortcut) before running `sz`.
#[test]
#[ignore = "requires the real `sz` binary from lrzsz, and spawns a real local PTY"]
fn manual_trigger_receives_from_real_sz_with_auto_detect_disabled() {
    receive_from_real_sz_over_pty(false);
}

fn receive_from_real_sz_over_pty(auto_detect_enabled: bool) {
    require_binary("sz");

    let send_dir = TempDir::new("tterm-zmodem-pty-send");
    let download_dir = TempDir::new("tterm-zmodem-pty-download");
    let expected: Vec<u8> = (0..40_000u32).map(|i| ((i * 13) % 256) as u8).collect();
    std::fs::write(send_dir.path.join("payload.bin"), &expected).expect("write source file");

    // Open a real PTY exactly like `terminal::pty::spawn_local_pty` does.
    let pty_system = portable_pty::native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open pty");

    let mut cmd = CommandBuilder::new(if cfg!(target_os = "windows") {
        "cmd.exe"
    } else {
        "/bin/sh"
    });
    if !cfg!(target_os = "windows") {
        cmd.args(["-c", "sz --binary payload.bin"]);
    }
    cmd.cwd(&send_dir.path);

    let mut child = pair
        .slave
        .spawn_command(cmd)
        .expect("spawn shell running sz");
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().expect("clone pty reader");
    let mut writer = pair.master.take_writer().expect("take pty writer");

    let zmodem_map: ZmodemMap = Arc::new(std::sync::RwLock::new(std::collections::HashMap::new()));
    let recorded_events: Arc<Mutex<Vec<(String, serde_json::Value)>>> =
        Arc::new(Mutex::new(Vec::new()));
    let recorded_for_emit = recorded_events.clone();

    let mut driver = ZmodemReceiveDriver::with_test_emit(
        move |kind, payload| {
            recorded_for_emit
                .lock()
                .unwrap()
                .push((kind.to_string(), payload));
        },
        "test-tab".to_string(),
        1,
        zmodem_map.clone(),
        auto_detect_enabled,
        download_dir.path.clone(),
    );
    if !auto_detect_enabled {
        driver.force_manual_override();
    }

    // Real `sz` completes a 40KB local transfer in well under a second;
    // 20s gives ample margin without letting a genuine hang wedge `cargo
    // test` for long.
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut buf = [0u8; 8192];
    loop {
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("timed out waiting for the ZMODEM transfer to finish");
        }
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };
        let outcome = driver.process(&buf[..n]);
        if !outcome.outgoing.is_empty() {
            let _ = writer.write_all(&outcome.outgoing);
            let _ = writer.flush();
        }
    }

    let _ = child.wait();

    assert!(
        zmodem_map.read().unwrap().is_empty(),
        "the tab's ZmodemTabHandle must be cleared once the session ends"
    );

    let received = std::fs::read(download_dir.path.join("payload.bin"))
        .expect("received file should exist in the download directory");
    assert_eq!(
        received, expected,
        "received bytes must match the source file exactly"
    );

    let events = recorded_events.lock().unwrap();
    assert!(
        events.iter().any(|(kind, _)| kind == "transfer-start"),
        "expected a transfer-start event, got: {events:?}"
    );
    assert!(
        events.iter().any(|(kind, _)| kind == "transfer-complete"),
        "expected a transfer-complete event, got: {events:?}"
    );
}
