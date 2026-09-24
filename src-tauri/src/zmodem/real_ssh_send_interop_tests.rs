//! End-to-end test of the SSH-direction send wiring (M5): the same
//! ephemeral local `sshd` `real_ssh_interop_tests.rs` uses, this time
//! running a real `rz` process that receives a file we push via
//! `zmodem::send::run` over a real `russh` channel — mirroring
//! `ssh/client.rs::run_single_ssh_connection`'s send-armed path. Over SSH we
//! have no way to force the remote pty into raw mode (unlike the local-PTY
//! case in `real_pty_send_interop_tests.rs`), so this also exercises the
//! engine's `ZNAK`/`ZRINIT` retry resilience (see `last_zfile`) as the sole
//! defense against any cooked-mode corruption on the remote end.
//!
//! `#[ignore]`d by default (requires `sshd` + `lrzsz` + `ssh-keygen`); run
//! explicitly with:
//! ```sh
//! cargo test --lib -- --ignored real_ssh_send_interop
//! ```

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use russh::keys::{load_secret_key, PrivateKeyWithHashAlg};
use russh::{client, ChannelMsg};
use tokio::sync::Mutex as TokioMutex;

use crate::core::state::ActiveSession;
use crate::core::ZmodemMap;
use crate::zmodem::send::{run, ZmodemSendParams};

const TEST_PORT: u16 = 12399;

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
    let found = Command::new("which")
        .arg(name)
        .stdout(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !found {
        panic!("`{name}` not found on PATH; required to run this test");
    }
}

fn require_absolute_path(name: &str) -> PathBuf {
    let output = Command::new("which")
        .arg(name)
        .output()
        .unwrap_or_else(|err| panic!("failed to run `which {name}`: {err}"));
    if !output.status.success() {
        panic!("`{name}` not found on PATH; required to run this test");
    }
    PathBuf::from(String::from_utf8_lossy(&output.stdout).trim())
}

struct TestSshd {
    child: Child,
    #[allow(dead_code)]
    dir: TempDir,
    client_key_path: PathBuf,
    port: u16,
}

impl TestSshd {
    fn spawn(port: u16) -> Self {
        let sshd_path = require_absolute_path("sshd");
        require_binary("ssh-keygen");
        let dir = TempDir::new("tterm-zmodem-send-sshd");

        let run_keygen = |args: &[&str]| {
            let status = Command::new("ssh-keygen")
                .args(args)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .expect("run ssh-keygen");
            assert!(status.success(), "ssh-keygen {args:?} failed");
        };
        let host_key = dir.path.join("host_key");
        let client_key = dir.path.join("client_key");
        run_keygen(&[
            "-t",
            "ed25519",
            "-N",
            "",
            "-f",
            host_key.to_str().unwrap(),
            "-q",
        ]);
        run_keygen(&[
            "-t",
            "ed25519",
            "-N",
            "",
            "-f",
            client_key.to_str().unwrap(),
            "-q",
        ]);
        std::fs::copy(
            client_key.with_extension("pub"),
            dir.path.join("authorized_keys"),
        )
        .expect("stage authorized_keys");

        let config_path = dir.path.join("sshd_config");
        std::fs::write(
            &config_path,
            format!(
                "Port {port}\n\
                 ListenAddress 127.0.0.1\n\
                 HostKey {host}\n\
                 AuthorizedKeysFile {authorized}\n\
                 PubkeyAuthentication yes\n\
                 PasswordAuthentication no\n\
                 KbdInteractiveAuthentication no\n\
                 UsePAM no\n\
                 StrictModes no\n\
                 LogLevel ERROR\n",
                host = host_key.display(),
                authorized = dir.path.join("authorized_keys").display(),
            ),
        )
        .expect("write sshd_config");

        let child = Command::new(&sshd_path)
            .arg("-f")
            .arg(&config_path)
            .arg("-D")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sshd");

        Self {
            child,
            dir,
            client_key_path: client_key,
            port,
        }
    }

    async fn wait_until_ready(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(Some(status)) = self.child.try_wait() {
                panic!("test sshd exited early with status {status:?}");
            }
            if tokio::net::TcpStream::connect(("127.0.0.1", self.port))
                .await
                .is_ok()
            {
                return;
            }
            if Instant::now() > deadline {
                panic!("test sshd never started listening on port {}", self.port);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

impl Drop for TestSshd {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct AcceptAnyServerKey;

impl client::Handler for AcceptAnyServerKey {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

#[test]
#[ignore = "spawns a local sshd and requires the real `rz` binary from lrzsz"]
fn wired_send_uploads_a_file_to_real_rz_over_a_real_ssh_channel() {
    let rt = tokio::runtime::Runtime::new().expect("build tokio runtime");
    rt.block_on(async {
        let rz_path = require_absolute_path("rz");
        let mut sshd = TestSshd::spawn(TEST_PORT);
        sshd.wait_until_ready().await;

        let recv_dir = TempDir::new("tterm-zmodem-send-ssh-recv");
        let source_dir = TempDir::new("tterm-zmodem-send-ssh-source");
        let payload: Vec<u8> = (0..40_000u32).map(|i| ((i * 11) % 256) as u8).collect();
        let source_path = source_dir.path.join("upload.bin");
        std::fs::write(&source_path, &payload).expect("write source file");

        let config = Arc::new(client::Config::default());
        let mut handle = client::connect(config, ("127.0.0.1", sshd.port), AcceptAnyServerKey)
            .await
            .expect("connect to test sshd");

        let key = load_secret_key(&sshd.client_key_path, None).expect("load client test key");
        let username = std::env::var("USER").expect("USER env var (current unix user)");
        let auth = handle
            .authenticate_publickey(&username, PrivateKeyWithHashAlg::new(Arc::new(key), None))
            .await
            .expect("publickey auth request");
        assert!(
            matches!(auth, client::AuthResult::Success),
            "publickey auth must succeed"
        );

        let mut channel = handle.channel_open_session().await.expect("open channel");
        let command = format!(
            "cd {} && {} --binary",
            recv_dir.path.display(),
            rz_path.display()
        );
        channel.exec(true, command).await.expect("exec rz over ssh");

        // `send::run`'s write-back goes through `ActiveSession::Ssh`'s
        // `input_tx`, exactly like a real connection; a small task drains
        // that channel into the SSH channel's writer.
        let (input_tx, mut input_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();
        let (resize_tx, _resize_rx) = tokio::sync::mpsc::unbounded_channel::<(u16, u16)>();
        let mut writer = channel.make_writer();
        let write_task = tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            while let Some(data) = input_rx.recv().await {
                if writer.write_all(&data).await.is_err() {
                    break;
                }
                let _ = writer.flush().await;
            }
        });
        let active: Arc<TokioMutex<Option<ActiveSession>>> = Arc::new(TokioMutex::new(Some(
            ActiveSession::Ssh(crate::core::state::ActiveSsh {
                input_tx,
                resize_tx,
                task: tokio::spawn(async {}),
                output_tail: Default::default(),
            }),
        )));

        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        let zmodem_map: ZmodemMap =
            Arc::new(std::sync::RwLock::new(std::collections::HashMap::new()));
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
                tab_id: "test-send-ssh-tab".to_string(),
                zmodem_map,
                incoming: rx,
                active,
                cancel_requested,
                paths: vec![source_path.clone()],
            });
        });

        // Reader loop: forwards every byte from the SSH channel into the
        // send session, exactly like `ZmodemSendPipe::try_forward` does in
        // production once armed.
        let deadline = Instant::now() + Duration::from_secs(25);
        loop {
            if Instant::now() > deadline {
                panic!("timed out waiting for the ZMODEM upload to finish over SSH");
            }
            let event = tokio::time::timeout(Duration::from_secs(5), channel.wait())
                .await
                .expect("channel.wait() timed out");
            match event {
                Some(ChannelMsg::Data { data }) => {
                    if tx.send(data.to_vec()).is_err() {
                        break; // send::run finished and dropped its Receiver.
                    }
                }
                Some(ChannelMsg::ExitStatus { .. }) | Some(ChannelMsg::Eof) | None => break,
                _ => {}
            }
        }
        drop(tx);

        send_thread.join().expect("send thread panicked");
        write_task.abort();
        let _ = tokio::time::timeout(Duration::from_secs(1), write_task).await;

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
    });
}
