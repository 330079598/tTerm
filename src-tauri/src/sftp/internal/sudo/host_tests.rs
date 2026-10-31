//! The sudo handshake against a real host with a real sudo, such as the
//! container `scripts/sudo-test-host/Containerfile` builds, whose accounts
//! cover the cases sudo can be in. Each account's password is its name
//! followed by `-pass`.
//!
//! ```sh
//! TTERM_SUDO_TEST_HOST=127.0.0.1:2222 cargo test --lib -- --ignored real_sudo
//! ```
//!
//! `TTERM_SUDO_TEST_REQUIRETTY=1` adds the account whose sudoers entry says
//! `requiretty`, which sudo-rs does not support.

use super::handshake::authenticate;
use super::SudoAuth;
use crate::sftp::internal::launcher::ChannelLauncher;
use russh::client::{self, Handle};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use zeroize::Zeroizing;

/// Accepts any host key: the host is a throwaway test container.
struct AcceptAnyKey;

impl client::Handler for AcceptAnyKey {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

async fn login(user: &str) -> Handle<AcceptAnyKey> {
    let target = std::env::var("TTERM_SUDO_TEST_HOST").expect("TTERM_SUDO_TEST_HOST is set");
    let (host, port) = target.rsplit_once(':').expect("host:port");
    let mut ssh = client::connect(
        Arc::new(crate::ssh::jump::config::compatibility_client_config(15, 3)),
        (host, port.parse::<u16>().expect("port")),
        AcceptAnyKey,
    )
    .await
    .expect("connect");
    let result = ssh
        .authenticate_password(user, format!("{user}-pass"))
        .await
        .expect("authenticate");
    assert!(result.success(), "{user} logs in");
    ssh
}

fn saved(
    password: &str,
) -> impl FnOnce() -> std::future::Ready<Result<Option<Zeroizing<String>>, String>> {
    let password = password.to_string();
    move || std::future::ready(Ok(Some(Zeroizing::new(password))))
}

/// What a script printed on standard output, and whether it succeeded.
async fn run_script(
    launcher: &ChannelLauncher,
    ssh: &Handle<AcceptAnyKey>,
    script: &str,
) -> String {
    let mut running = launcher.start_script(ssh, script).await.expect("start");
    let mut stdout = Vec::new();
    while let Some(chunk) = running.next_output().await {
        stdout.extend_from_slice(&chunk);
    }
    let outcome = running.finish().await;
    assert!(outcome.succeeded(), "{}", outcome.stderr);
    String::from_utf8(stdout).expect("utf-8")
}

#[tokio::test]
#[ignore = "needs a host with sudo; see the module docs"]
async fn real_sudo_password_account_works_as_root() {
    let ssh = login("alice").await;
    let auth = authenticate(&ssh, saved("alice-pass"))
        .await
        .expect("sudo takes the password");
    assert!(matches!(auth, SudoAuth::Password(_)));
    let launcher = ChannelLauncher::sudo(auth);
    let sftp = launcher
        .open_sftp_session(&ssh)
        .await
        .expect("sftp as root");

    let names = sftp
        .read_dir("/root")
        .await
        .expect("list /root")
        .map(|entry| entry.file_name())
        .collect::<Vec<_>>();
    assert!(names.contains(&"secret.txt".to_string()), "{names:?}");
    let mut secret = String::new();
    sftp.open("/root/secret.txt")
        .await
        .expect("open a 0600 root file")
        .read_to_string(&mut secret)
        .await
        .expect("read");
    assert_eq!(secret, "secret\n");

    let path = format!("/srv/protected/tterm-{}", uuid::Uuid::new_v4());
    let mut file = sftp.create(&path).await.expect("create in a root folder");
    file.write_all(b"made as root\n").await.expect("write");
    file.shutdown().await.expect("close");
    assert_eq!(sftp.metadata(&path).await.expect("stat").uid, Some(0));
    sftp.remove_file(&path).await.expect("remove");

    assert_eq!(run_script(&launcher, &ssh, "id -un").await, "root\n");

    // Transfer lanes open side by side, each through sudo on its own.
    let (a, b, c, d) = tokio::join!(
        launcher.open_raw_session(&ssh),
        launcher.open_raw_session(&ssh),
        launcher.open_raw_session(&ssh),
        launcher.open_raw_session(&ssh),
    );
    for lane in [a, b, c, d] {
        lane.expect("transfer lane as root");
    }
}

#[tokio::test]
#[ignore = "needs a host with sudo; see the module docs"]
async fn real_sudo_passwordless_account_with_fish_needs_no_password() {
    let ssh = login("bob").await;
    let auth = authenticate(&ssh, || async {
        panic!("a NOPASSWD account is never asked for a password")
    })
    .await
    .expect("sudo lets bob in");
    assert!(matches!(auth, SudoAuth::NoPassword));
    let launcher = ChannelLauncher::sudo(auth);
    let sftp = launcher
        .open_sftp_session(&ssh)
        .await
        .expect("sftp as root");
    sftp.read_dir("/root").await.expect("list /root");
    assert_eq!(run_script(&launcher, &ssh, "id -un").await, "root\n");
}

#[tokio::test]
#[ignore = "needs a host with sudo; see the module docs"]
async fn real_sudo_wrong_password_fails_at_once() {
    let ssh = login("alice").await;
    let started = Instant::now();
    let error = authenticate(&ssh, saved("wrong"))
        .await
        .err()
        .expect("sudo refuses");
    assert!(error.contains("did not accept"), "{error}");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "no waiting for another try"
    );
}

#[tokio::test]
#[ignore = "needs a host with sudo; see the module docs"]
async fn real_sudo_account_limited_to_some_commands_cannot_start_sftp() {
    let ssh = login("carol").await;
    let auth = authenticate(&ssh, saved("carol-pass"))
        .await
        .expect("carol may use sudo for systemctl");
    let error = ChannelLauncher::sudo(auth)
        .open_sftp_session(&ssh)
        .await
        .err()
        .expect("sftp-server is not allowed");
    assert!(error.contains("only some commands"), "{error}");
}

#[tokio::test]
#[ignore = "needs a host with sudo; see the module docs"]
async fn real_sudo_account_outside_sudoers_is_refused() {
    let ssh = login("dave").await;
    let error = authenticate(&ssh, saved("dave-pass"))
        .await
        .err()
        .expect("dave may not use sudo");
    assert!(error.contains("would not let this account"), "{error}");
}

#[tokio::test]
#[ignore = "needs a host with sudo; see the module docs"]
async fn real_sudo_requiretty_is_named() {
    if std::env::var_os("TTERM_SUDO_TEST_REQUIRETTY").is_none() {
        return;
    }
    let ssh = login("erin").await;
    let error = authenticate(&ssh, saved("erin-pass"))
        .await
        .err()
        .expect("requiretty refuses a channel without a tty");
    assert!(error.contains("requiretty"), "{error}");
}
