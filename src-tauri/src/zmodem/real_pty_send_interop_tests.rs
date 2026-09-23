//! End-to-end test of the M4 wiring — the send (upload) direction — driven
//! against a REAL local PTY (via `portable_pty`) running a REAL `rz`
//! process. Mirrors production exactly: the "reader" loop (this test's main
//! thread, standing in for `terminal::pty::spawn_reader_thread`'s closure
//! once `ZmodemSendPipe` is armed) forwards raw bytes into a channel, while
//! `zmodem::send::run` — the same function `zmodem_start_send` spawns onto
//! its own thread in production — drives the whole upload from the other
//! end, writing back through a real `ActivePty` exactly like `write_pty`
//! does.
//!
//! `#[ignore]`d by default (requires `lrzsz`); run explicitly with:
//! ```sh
//! cargo test --lib -- --ignored real_pty_send_interop
//! ```

use std::io::Read;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize};
use tokio::sync::Mutex as TokioMutex;

use crate::core::state::ActiveSession;
use crate::core::ZmodemMap;
use crate::terminal::ActivePty;
use crate::zmodem::send::{run, ZmodemSendParams};

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
#[ignore = "requires the real `rz` binary from lrzsz, and spawns a real local PTY"]
fn wired_send_uploads_a_file_to_real_rz_over_a_real_pty() {
    require_binary("rz");

    let recv_dir = TempDir::new("tterm-zmodem-send-pty-recv");
    let send_source_dir = TempDir::new("tterm-zmodem-send-pty-source");
    let payload: Vec<u8> = (0..50_000u32).map(|i| ((i * 7) % 256) as u8).collect();
    let source_path = send_source_dir.path.join("upload.bin");
    std::fs::write(&source_path, &payload).expect("write source file");

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
        cmd.args(["-c", "rz --binary"]);
    }
    cmd.cwd(&recv_dir.path);

    let child = pair
        .slave
        .spawn_command(cmd)
        .expect("spawn shell running rz");
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().expect("clone pty reader");
    let writer = pair.master.take_writer().expect("take pty writer");

    let active_pty = ActivePty {
        writer,
        master: pair.master,
        child,
    };
    let active: Arc<TokioMutex<Option<ActiveSession>>> =
        Arc::new(TokioMutex::new(Some(ActiveSession::Local(active_pty))));

    let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
    let zmodem_map: ZmodemMap = Arc::new(std::sync::RwLock::new(std::collections::HashMap::new()));
    let cancel_requested = Arc::new(AtomicBool::new(false));
    let recorded_events: Arc<Mutex<Vec<(String, serde_json::Value)>>> =
        Arc::new(Mutex::new(Vec::new()));
    let recorded_for_emit = recorded_events.clone();

    let send_thread = std::thread::spawn(move || {
        run(ZmodemSendParams {
            emit: Box::new(move |kind, payload| {
                recorded_for_emit
                    .lock()
                    .unwrap()
                    .push((kind.to_string(), payload));
            }),
            tab_id: "test-send-tab".to_string(),
            zmodem_map,
            incoming: rx,
            active,
            cancel_requested,
            paths: vec![source_path.clone()],
        });
    });

    // Reader loop: stands in for the PTY reader thread once armed for send
    // (see `ZmodemSendPipe`) — forward every byte from rz straight into the
    // channel `send::run` reads from.
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut buf = [0u8; 8192];
    loop {
        if Instant::now() > deadline {
            panic!("timed out waiting for the ZMODEM upload to finish");
        }
        let n = match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };
        if tx.send(buf[..n].to_vec()).is_err() {
            break; // send::run finished and dropped its Receiver.
        }
    }
    drop(tx);

    send_thread.join().expect("send thread panicked");
    // `child` (the shell running `rz`) was moved into `ActivePty`/`active`
    // above; it's reaped when that `Arc` drops along with the send thread.

    let received = std::fs::read(recv_dir.path.join("upload.bin"))
        .expect("rz should have written the uploaded file");
    assert_eq!(
        received, payload,
        "uploaded bytes must match the source file exactly"
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
