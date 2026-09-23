//! End-to-end test of the SSH-direction wiring (M3): a real, ephemeral,
//! userspace `sshd` (own host key + a throwaway client keypair, listening
//! only on 127.0.0.1 on a high port — no system "Remote Login" setting
//! touched, no root needed) driving `ZmodemReceiveDriver` over a real
//! `russh` client channel running a real `sz` process, exactly mirroring
//! `ssh/client.rs::run_single_ssh_connection`'s `ChannelMsg::Data` handling
//! (minus the connection-setup/host-key-prompt machinery, which is
//! unrelated to ZMODEM and already covered elsewhere).
//!
//! `#[ignore]`d by default (requires `sshd` + `lrzsz` + `ssh-keygen`, all
//! standard on macOS/Linux); run explicitly with:
//! ```sh
//! cargo test --lib -- --ignored real_ssh_interop
//! ```

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use russh::keys::{load_secret_key, PrivateKeyWithHashAlg};
use russh::{client, ChannelMsg};
use tokio::io::AsyncWriteExt;

use crate::core::ZmodemMap;
use crate::zmodem::session::ZmodemReceiveDriver;

const TEST_PORT: u16 = 12299;

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

/// Resolves `name` to an absolute path via `which`. macOS's `sshd` refuses
/// to start when invoked via bare PATH lookup ("sshd requires execution
/// with an absolute path"), unlike every other binary this test file spawns.
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
    // Never read directly; keeps the config/host-key/authorized_keys files
    // sshd references alive (via `Drop`) for as long as the server runs.
    #[allow(dead_code)]
    dir: TempDir,
    client_key_path: PathBuf,
    port: u16,
}

impl TestSshd {
    /// Spawns a throwaway `sshd` on `127.0.0.1:port`, trusting only a
    /// freshly generated client key placed in a private `AuthorizedKeysFile`
    /// — never touching the real user's `~/.ssh/authorized_keys` or macOS's
    /// system-wide Remote Login setting.
    fn spawn(port: u16) -> Self {
        let sshd_path = require_absolute_path("sshd");
        require_binary("ssh-keygen");
        let dir = TempDir::new("tterm-zmodem-sshd");

        let run = |args: &[&str]| {
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
        run(&[
            "-t",
            "ed25519",
            "-N",
            "",
            "-f",
            host_key.to_str().unwrap(),
            "-q",
        ]);
        run(&[
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

        let stderr_path = dir.path.join("sshd_stderr.log");
        let child = Command::new(&sshd_path)
            .arg("-f")
            .arg(&config_path)
            .arg("-D")
            .arg("-e")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(std::fs::File::create(&stderr_path).expect("create sshd stderr log"))
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
                let stderr = std::fs::read_to_string(self.dir.path.join("sshd_stderr.log"))
                    .unwrap_or_default();
                panic!("test sshd exited early with status {status:?}\nstderr:\n{stderr}");
            }
            match tokio::net::TcpStream::connect(("127.0.0.1", self.port)).await {
                Ok(_) => return,
                Err(err) => {
                    if Instant::now() > deadline {
                        panic!(
                            "test sshd never started listening on port {} (last error: {err})",
                            self.port
                        );
                    }
                }
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

/// Accepts any server host key — appropriate only for this throwaway local
/// `sshd` whose host key was just generated moments ago by this same test.
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
#[ignore = "spawns a local sshd and requires the real `sz` binary from lrzsz"]
fn wired_driver_receives_a_file_from_real_sz_over_a_real_ssh_channel() {
    let rt = tokio::runtime::Runtime::new().expect("build tokio runtime");
    rt.block_on(async {
        let sz_path = require_absolute_path("sz");
        let mut sshd = TestSshd::spawn(TEST_PORT);
        sshd.wait_until_ready().await;

        let send_dir = TempDir::new("tterm-zmodem-ssh-send");
        let download_dir = TempDir::new("tterm-zmodem-ssh-download");
        let expected: Vec<u8> = (0..60_000u32).map(|i| ((i * 31) % 256) as u8).collect();
        std::fs::write(send_dir.path.join("payload.bin"), &expected).expect("write source file");

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
            "publickey auth against the test sshd must succeed: {auth:?}"
        );

        let mut channel = handle.channel_open_session().await.expect("open channel");
        let command = format!(
            "cd {} && {} --binary payload.bin",
            send_dir.path.display(),
            sz_path.display()
        );
        channel.exec(true, command).await.expect("exec sz over ssh");
        let mut writer = channel.make_writer();

        let zmodem_map: ZmodemMap =
            Arc::new(std::sync::RwLock::new(std::collections::HashMap::new()));
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
            "test-ssh-tab".to_string(),
            1,
            zmodem_map.clone(),
            true,
            download_dir.path.clone(),
        );

        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if Instant::now() > deadline {
                panic!("timed out waiting for the ZMODEM transfer to finish over SSH");
            }
            let event = tokio::time::timeout(Duration::from_secs(5), channel.wait())
                .await
                .expect("channel.wait() timed out");
            match event {
                Some(ChannelMsg::Data { data }) => {
                    let outcome = driver.process(data.as_ref());
                    if !outcome.outgoing.is_empty() {
                        writer
                            .write_all(&outcome.outgoing)
                            .await
                            .expect("write zmodem reply over ssh channel");
                        writer
                            .flush()
                            .await
                            .expect("flush zmodem reply over ssh channel");
                    }
                }
                Some(ChannelMsg::ExitStatus { .. }) | Some(ChannelMsg::Eof) | None => break,
                _ => {}
            }
        }

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
    });
}
