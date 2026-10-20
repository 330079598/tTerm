//! [`authenticate`] against a real OpenSSH server (see
//! [`crate::ssh::test_sshd`]).
//!
//! The in-process `russh` server used by the other tests cannot signal
//! partial success for a public key and says nothing about which RSA
//! signature algorithms a real server accepts, so those behaviours, and PAM's
//! keyboard-interactive prompts, are checked here.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use russh::keys::{load_secret_key, PrivateKeyWithHashAlg};

use super::{authenticate, AuthMethod, AuthPrompter, AuthTarget, ScriptedPrompts};
use crate::ssh::test_sshd::{current_user, TempDir, TestSshd};

fn target<'a>(username: &'a str, port: u16) -> AuthTarget<'a> {
    AuthTarget {
        host: "127.0.0.1",
        port,
        username,
        hop: None,
    }
}

#[tokio::test]
#[ignore = "spawns a real sshd; run with --ignored real_sshd"]
async fn real_sshd_accepts_an_rsa_key_file() {
    let sshd = TestSshd::spawn(12311, &["-t", "rsa", "-b", "3072"], "").await;
    let username = current_user();

    // The legacy SHA-1 signature is what key-file auth used to send, and what
    // a current OpenSSH turns down.
    let mut legacy = sshd.connect().await;
    let key = load_secret_key(&sshd.client_key, None).expect("load client key");
    let legacy_result = legacy
        .authenticate_publickey(
            username.as_str(),
            PrivateKeyWithHashAlg::new(Arc::new(key), None),
        )
        .await
        .expect("legacy auth exchange");
    assert!(
        !legacy_result.success(),
        "this sshd still accepts ssh-rsa, so the test proves nothing"
    );

    // tTerm signs with rsa-sha2 through `ring`, whether or not `russh` was
    // built with its own RSA support.
    let mut session = sshd.connect().await;
    authenticate(
        &mut session,
        &target(&username, sshd.port),
        sshd.key_method(),
        &AuthPrompter::Unavailable,
    )
    .await
    .expect("RSA key authentication");
}

#[tokio::test]
#[ignore = "spawns a real sshd; run with --ignored real_sshd"]
async fn real_sshd_partial_success_moves_on_to_the_next_method() {
    let sshd = TestSshd::spawn(
        12312,
        &["-t", "ed25519"],
        "PasswordAuthentication yes\n\
         KbdInteractiveAuthentication no\n\
         AuthenticationMethods publickey,password\n",
    )
    .await;
    let username = current_user();
    let prompts = Arc::new(Mutex::new(ScriptedPrompts {
        answers: VecDeque::from([Some(vec!["not-the-password".to_string()])]),
        asked: Vec::new(),
    }));

    let mut session = sshd.connect().await;
    let error = authenticate(
        &mut session,
        &target(&username, sshd.port),
        sshd.key_method(),
        &AuthPrompter::Scripted(prompts.clone()),
    )
    .await
    .expect_err("the password is wrong, and an unprivileged sshd could not check it anyway");

    // The key was accepted as the first of two steps, so the user was asked
    // for the second one instead of the connection failing outright.
    assert_eq!(error.to_string(), "SSH authentication failed");
    let asked = prompts.lock().unwrap().asked.clone();
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert_eq!(asked[0].kind, "password");
    assert_eq!(asked[0].username, username);
}

#[tokio::test]
#[ignore = "spawns a real sshd; run with --ignored real_sshd"]
async fn real_sshd_key_alone_is_not_enough_when_two_methods_are_required() {
    let sshd = TestSshd::spawn(
        12313,
        &["-t", "ed25519"],
        "PasswordAuthentication yes\n\
         KbdInteractiveAuthentication no\n\
         AuthenticationMethods publickey,password\n",
    )
    .await;
    let username = current_user();

    let mut session = sshd.connect().await;
    let error = authenticate(
        &mut session,
        &target(&username, sshd.port),
        sshd.key_method(),
        &AuthPrompter::Unavailable,
    )
    .await
    .expect_err("no way to ask for the password");
    assert!(
        error
            .to_string()
            .contains("needs input from you (Password)"),
        "{error}"
    );
}

#[tokio::test]
#[ignore = "spawns a real sshd; run with --ignored real_sshd"]
async fn real_sshd_pam_prompts_reach_the_user() {
    let sshd = TestSshd::spawn(
        12314,
        &["-t", "ed25519"],
        "UsePAM yes\n\
         PasswordAuthentication no\n\
         KbdInteractiveAuthentication yes\n\
         AuthenticationMethods keyboard-interactive\n",
    )
    .await;
    let username = current_user();
    let prompts = Arc::new(Mutex::new(ScriptedPrompts {
        answers: VecDeque::from([Some(vec!["not-the-password".to_string()])]),
        asked: Vec::new(),
    }));

    let mut session = sshd.connect().await;
    let error = authenticate(
        &mut session,
        &target(&username, sshd.port),
        AuthMethod::KeyboardInteractive,
        &AuthPrompter::Scripted(prompts.clone()),
    )
    .await
    .expect_err("the answers are wrong");

    assert_eq!(error.to_string(), "SSH authentication failed");
    let asked = prompts.lock().unwrap().asked.clone();
    assert!(!asked.is_empty(), "PAM never prompted");
    assert_eq!(asked[0].kind, "keyboard-interactive");
    assert_eq!(asked[0].prompts.len(), 1);
    assert!(!asked[0].prompts[0].echo);

    // A password profile on the same server: `password` is not offered, so
    // the stored password answers PAM's prompt and the user is never asked.
    let unused = Arc::new(Mutex::new(ScriptedPrompts::default()));
    let mut session = sshd.connect().await;
    let error = authenticate(
        &mut session,
        &target(&username, sshd.port),
        AuthMethod::Password(Some("not-the-password")),
        &AuthPrompter::Scripted(unused.clone()),
    )
    .await
    .expect_err("the stored password is wrong");
    assert_eq!(error.to_string(), "SSH authentication failed");
    assert!(unused.lock().unwrap().asked.is_empty());
}

/// Keys made before OpenSSH 7.8 are PKCS#1 PEM (`BEGIN RSA PRIVATE KEY`).
#[tokio::test]
#[ignore = "spawns a real sshd; run with --ignored real_sshd"]
async fn real_sshd_accepts_a_pem_rsa_key_file() {
    let sshd = TestSshd::spawn(12315, &["-t", "rsa", "-b", "2048", "-m", "PEM"], "").await;
    let username = current_user();
    let pem = std::fs::read_to_string(&sshd.client_key).expect("read key");
    assert!(pem.starts_with("-----BEGIN RSA PRIVATE KEY-----"), "{pem}");

    let mut session = sshd.connect().await;
    authenticate(
        &mut session,
        &target(&username, sshd.port),
        sshd.key_method(),
        &AuthPrompter::Unavailable,
    )
    .await
    .expect("PEM RSA key authentication");
}

/// A server with nothing but an RSA host key, as on old Linux releases and
/// network equipment, is verified whether it signs with rsa-sha2 or, like
/// OpenSSH before 7.2, only with SHA-1 `ssh-rsa`.
#[tokio::test]
#[ignore = "spawns a real sshd; run with --ignored real_sshd"]
async fn real_sshd_with_only_an_rsa_host_key_is_verified() {
    for (port, host_key_algorithms) in [(12316, ""), (12319, "HostKeyAlgorithms ssh-rsa\n")] {
        let sshd = TestSshd::spawn_with_host_key(
            port,
            &["-t", "rsa", "-b", "3072"],
            &["-t", "ed25519"],
            host_key_algorithms,
        )
        .await;

        let mut session = sshd.connect().await;
        authenticate(
            &mut session,
            &target(&current_user(), sshd.port),
            sshd.key_method(),
            &AuthPrompter::Unavailable,
        )
        .await
        .unwrap_or_else(|error| panic!("{host_key_algorithms:?}: {error}"));
    }
}

/// Starts `sshd` with no `authorized_keys`, trusting a fresh certificate
/// authority instead, and certifies the client key with it as `ssh-keygen -s`
/// does, which leaves the certificate beside the key. The authority's
/// directory is returned too: sshd reads the key from it at every login.
async fn spawn_trusting_a_certificate_authority(
    port: u16,
    key_type: &[&str],
) -> (TestSshd, TempDir) {
    let ca_dir = TempDir::new();
    let ca_key = ca_dir.path.join("ca");
    let ca_key = ca_key.to_str().expect("utf-8 path");
    let ssh_keygen = |args: &[&str]| {
        let status = std::process::Command::new("ssh-keygen")
            .args(args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .expect("run ssh-keygen");
        assert!(status.success(), "ssh-keygen {args:?} failed");
    };
    ssh_keygen(&["-t", "ed25519", "-N", "", "-q", "-f", ca_key]);

    let sshd = TestSshd::spawn(
        port,
        key_type,
        &format!("AuthorizedKeysFile none\nTrustedUserCAKeys {ca_key}.pub\n"),
    )
    .await;
    let client_public_key = format!("{}.pub", sshd.client_key.display());
    let username = current_user();
    ssh_keygen(&[
        "-s",
        ca_key,
        "-I",
        "tterm-test",
        "-n",
        &username,
        "-q",
        &client_public_key,
    ]);
    (sshd, ca_dir)
}

#[tokio::test]
#[ignore = "spawns a real sshd; run with --ignored real_sshd"]
async fn real_sshd_accepts_the_certificate_beside_a_key_file() {
    for (port, key_type) in [
        (12317, &["-t", "ed25519"][..]),
        (12318, &["-t", "rsa", "-b", "3072"][..]),
    ] {
        let (sshd, _ca_dir) = spawn_trusting_a_certificate_authority(port, key_type).await;
        let username = current_user();

        let mut session = sshd.connect().await;
        authenticate(
            &mut session,
            &target(&username, sshd.port),
            sshd.key_method(),
            &AuthPrompter::Unavailable,
        )
        .await
        .unwrap_or_else(|error| panic!("{key_type:?} certificate authentication: {error}"));
    }
}
