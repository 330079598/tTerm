//! Run explicitly in release mode after checking that the machine is idle.
use super::pty_fixture::PtyFixture;
use super::*;
use std::path::PathBuf;
use tauri::ipc::{Channel, InvokeResponseBody};

const TOTAL_BYTES: usize = 50_000_000;
const ACK_BATCH: usize = 64 * 1024;
const ACK_DELAY: Duration = Duration::from_millis(20);

#[derive(Default)]
struct Measurement {
    bytes: usize,
    first: Option<Instant>,
    last: Option<Instant>,
    digest: Option<sha2::Sha256>,
}

struct Acknowledger {
    tx: Option<std::sync::mpsc::Sender<usize>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Acknowledger {
    fn spawn(flow: Arc<OutputFlow>) -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<usize>();
        let worker = std::thread::spawn(move || {
            let mut pending = 0;
            let mut deadline: Option<Instant> = None;
            loop {
                let message = match deadline {
                    Some(at) => rx.recv_timeout(at.saturating_duration_since(Instant::now())),
                    None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
                };
                match message {
                    Ok(bytes) => {
                        pending += bytes;
                        deadline.get_or_insert_with(|| Instant::now() + ACK_DELAY);
                        if pending >= ACK_BATCH {
                            flow.ack(pending);
                            pending = 0;
                            deadline = None;
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {
                        flow.ack(pending);
                        pending = 0;
                        deadline = None;
                    }
                    Err(RecvTimeoutError::Disconnected) => {
                        flow.ack(pending);
                        return;
                    }
                }
            }
        });
        Self {
            tx: Some(tx),
            worker: Some(worker),
        }
    }
}

impl Drop for Acknowledger {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}

fn prepare_workload() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let directory = root.join("target/p1-06/perf/workload");
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("text-50MB.txt");
    if std::fs::metadata(&path).map(|metadata| metadata.len()).ok() != Some(TOTAL_BYTES as u64) {
        let mut line =
            "tTerm P0-01 deterministic ASCII workload ".to_string() + &"0123456789".repeat(6);
        line.truncate(99);
        line.push('\n');
        std::fs::write(&path, line.repeat(500_000)).unwrap();
    }
    directory
}

#[test]
#[ignore = "release throughput benchmark; run only when the 1-minute load is <= 2"]
fn p106_real_pty_cat_throughput() {
    use sha2::Digest;
    if cfg!(debug_assertions) {
        panic!("benchmark must run with --release");
    }
    let load = std::process::Command::new("uptime").output().unwrap();
    let uptime = String::from_utf8(load.stdout).unwrap();
    println!("{uptime}");
    let average = uptime
        .split("load average")
        .nth(1)
        .unwrap()
        .split(':')
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .trim_end_matches(',')
        .parse::<f64>()
        .unwrap();
    assert!(
        average <= 2.0,
        "machine busy; no performance sample collected"
    );
    let directory = prepare_workload();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let flow = Arc::new(OutputFlow::default());
    let mut acker = Acknowledger::spawn(flow.clone());
    let ack_tx = acker.tx.as_ref().unwrap().clone();
    let measured = Arc::new(Mutex::new(Measurement {
        digest: Some(sha2::Sha256::new()),
        ..Measurement::default()
    }));
    let captured = measured.clone();
    let channel = Channel::new(move |body| {
        let InvokeResponseBody::Raw(bytes) = body else {
            panic!("expected Raw output")
        };
        let at = Instant::now();
        let mut sample = captured.lock().unwrap();
        sample.first.get_or_insert(at);
        sample.last = Some(at);
        sample.bytes += bytes.len();
        sample.digest.as_mut().unwrap().update(&bytes);
        drop(sample);
        ack_tx.send(bytes.len()).unwrap();
        Ok(())
    });
    let output = crate::session_sink::output::with_flow(channel, flow);
    let mut sender = TerminalOutputSender::spawn("p106-benchmark", test_sink(output));
    let command = format!(
        "stty -echo -opost; exec /bin/bash '{}' cat '{}'",
        root.join("scripts/perf/workload.sh").display(),
        directory.display()
    );
    let mut fixture = PtyFixture::spawn(&command);
    let outcome = crate::terminal::reader::run_pty_reader(
        fixture.reader(),
        |_| {},
        |bytes| sender.send(bytes.to_vec()),
        || {},
    );
    sender.finish();
    drop(sender);
    acker.tx.take();
    assert!(fixture.child.wait().unwrap().success());
    assert_eq!(outcome, crate::terminal::reader::ReaderOutcome::Terminated);
    let mut sample = measured.lock().unwrap();
    assert_eq!(sample.bytes, TOTAL_BYTES);
    let expected = sha2::Sha256::digest(std::fs::read(directory.join("text-50MB.txt")).unwrap());
    assert_eq!(sample.digest.take().unwrap().finalize(), expected);
    let seconds = sample
        .last
        .unwrap()
        .duration_since(sample.first.unwrap())
        .as_secs_f64();
    let sha256 = expected
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    println!(
        "P106_CAT bytes={} seconds={seconds:.9} mib_per_s={:.6} sha256={sha256}",
        sample.bytes,
        sample.bytes as f64 / seconds / 1_048_576.0
    );
}
