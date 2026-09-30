//! [`authenticate`] against a real OpenSSH server (see
//! [`crate::ssh::test_sshd`]).
//!
//! The in-process `russh` server used by the other tests cannot signal
//! partial success for a public key, so that, and PAM's keyboard-interactive
//! prompts, are checked here.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use super::{authenticate, AuthMethod, AuthPrompter, AuthTarget, ScriptedPrompts};
use crate::ssh::test_sshd::{current_user, TestSshd};

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
