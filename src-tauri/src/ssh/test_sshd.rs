//! An ephemeral, userspace OpenSSH server for tests that need the real thing:
//! `sshd` on 127.0.0.1 with its own host key and a throwaway client key. No
//! system setting is touched and no root is needed. Tests using it are
//! `#[ignore]`d (they need `sshd` and `ssh-keygen`); run them with:
//! ```sh
//! cargo test --lib -- --ignored real_sshd
//! ```

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use russh::client;

use crate::ssh::auth::AuthMethod;

pub(crate) struct TempDir {
    pub(crate) path: PathBuf,
}

impl TempDir {
    pub(crate) fn new() -> Self {
        let path = std::env::temp_dir().join(format!("tterm-auth-sshd-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// macOS's `sshd` refuses to start unless it is invoked by absolute path.
fn absolute_path(name: &str) -> PathBuf {
    let output = Command::new("which")
        .arg(name)
        .output()
        .unwrap_or_else(|err| panic!("failed to run `which {name}`: {err}"));
    assert!(
        output.status.success(),
        "`{name}` not found on PATH; required to run this test"
    );
    PathBuf::from(String::from_utf8_lossy(&output.stdout).trim())
}

pub(crate) struct TestSshd {
    child: Child,
    dir: TempDir,
    pub(crate) client_key: PathBuf,
    pub(crate) port: u16,
}

impl TestSshd {
    /// Starts `sshd` trusting one freshly generated client key of `key_type`
    /// (`ssh-keygen -t` arguments). `extra_config` goes first, because sshd
    /// keeps the first value it sees for a keyword.
    pub(crate) async fn spawn(port: u16, key_type: &[&str], extra_config: &str) -> Self {
        let sshd_path = absolute_path("sshd");
        let keygen = absolute_path("ssh-keygen");
        let dir = TempDir::new();
        let host_key = dir.path.join("host_key");
        let client_key = dir.path.join("client_key");

        for (key_type, path) in [(&["-t", "ed25519"][..], &host_key), (key_type, &client_key)] {
            let status = Command::new(&keygen)
                .args(key_type)
                .args(["-N", "", "-q", "-f"])
                .arg(path)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .expect("run ssh-keygen");
            assert!(status.success(), "ssh-keygen {key_type:?} failed");
        }
        std::fs::copy(
            client_key.with_extension("pub"),
            dir.path.join("authorized_keys"),
        )
        .expect("stage authorized_keys");

        let config_path = dir.path.join("sshd_config");
        std::fs::write(
            &config_path,
            format!(
                "{extra_config}\
                 Port {port}\n\
                 ListenAddress 127.0.0.1\n\
                 HostKey {host}\n\
                 AuthorizedKeysFile {authorized}\n\
                 PubkeyAuthentication yes\n\
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
            .args(["-D", "-e"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(std::fs::File::create(dir.path.join("sshd_stderr.log")).expect("stderr log"))
            .spawn()
            .expect("spawn sshd");

        let mut sshd = Self {
            child,
            dir,
            client_key,
            port,
        };
        sshd.wait_until_ready().await;
        sshd
    }

    async fn wait_until_ready(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(Some(status)) = self.child.try_wait() {
                let stderr = std::fs::read_to_string(self.dir.path.join("sshd_stderr.log"))
                    .unwrap_or_default();
                panic!("test sshd exited early with status {status:?}\nstderr:\n{stderr}");
            }
            if tokio::net::TcpStream::connect(("127.0.0.1", self.port))
                .await
                .is_ok()
            {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "test sshd never started listening on port {}",
                self.port
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    pub(crate) async fn connect(&self) -> client::Handle<AcceptAnyServerKey> {
        client::connect(
            Arc::new(crate::ssh::jump::compatibility_client_config(15, 3)),
            ("127.0.0.1", self.port),
            AcceptAnyServerKey,
        )
        .await
        .expect("connect to test sshd")
    }

    pub(crate) fn key_method(&self) -> AuthMethod<'_> {
        AuthMethod::Key {
            path: self.client_key.to_str().expect("utf-8 key path"),
            passphrase: None,
        }
    }
}

impl Drop for TestSshd {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Accepts any host key: the server's was generated by this test moments ago.
pub(crate) struct AcceptAnyServerKey;

impl client::Handler for AcceptAnyServerKey {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

pub(crate) fn current_user() -> String {
    std::env::var("USER").expect("USER is set")
}
