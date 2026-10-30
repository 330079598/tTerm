//! Sharing a jump session against a real OpenSSH server (see
//! [`crate::ssh::test_sshd`]): two connections tunnel through one logged-in
//! hop, which stays open until the last of them lets go.

use std::sync::Arc;

use russh::{client, ChannelMsg};

use super::pool::{self, PooledSession, Relay, RouteKey};
use crate::ssh::auth::{authenticate, AuthPrompter, AuthTarget};
use crate::ssh::test_sshd::{current_user, AcceptAnyServerKey, TestSshd};

async fn log_in(sshd: &TestSshd, session: &mut client::Handle<AcceptAnyServerKey>) {
    let username = current_user();
    authenticate(
        session,
        &AuthTarget {
            host: "127.0.0.1",
            port: sshd.port,
            username: &username,
            hop: None,
        },
        sshd.key_method(),
        &AuthPrompter::Unavailable,
    )
    .await
    .expect("log in");
}

/// Logs in to the server again through `relay`, as a target behind it would be.
async fn connect_through(
    sshd: &TestSshd,
    relay: &Arc<dyn Relay>,
) -> client::Handle<AcceptAnyServerKey> {
    let channel = relay
        .open_direct_tcpip("127.0.0.1", sshd.port)
        .await
        .expect("open tunnel through the shared hop");
    let mut session = client::connect_stream(
        Arc::new(crate::ssh::jump::config::compatibility_client_config(15, 3)),
        channel.into_stream(),
        AcceptAnyServerKey,
    )
    .await
    .expect("SSH over the tunnel");
    log_in(sshd, &mut session).await;
    session
}

async fn echo(session: &client::Handle<AcceptAnyServerKey>, text: &str) -> String {
    let mut channel = session.channel_open_session().await.expect("open channel");
    channel
        .exec(true, format!("echo {text}"))
        .await
        .expect("exec");
    let mut output = Vec::new();
    while let Some(message) = channel.wait().await {
        match message {
            ChannelMsg::Data { data } => output.extend_from_slice(&data),
            ChannelMsg::ExitStatus { .. } | ChannelMsg::Eof | ChannelMsg::Close => break,
            _ => {}
        }
    }
    String::from_utf8_lossy(&output).trim().to_string()
}

#[tokio::test]
#[ignore = "spawns a real sshd; run with --ignored real_sshd"]
async fn real_sshd_carries_two_targets_over_one_shared_hop() {
    let sshd = TestSshd::spawn(12361, &["-t", "ed25519"], "").await;
    let key = RouteKey::first(None, &current_user(), "127.0.0.1", sshd.port);

    let mut hop = sshd.connect().await;
    log_in(&sshd, &mut hop).await;
    let hop: Arc<PooledSession<AcceptAnyServerKey>> = Arc::new(PooledSession::new(hop, None));
    pool::offer(key.clone(), &hop);

    let first_relay = pool::find(&key).expect("the hop is offered");
    let first = connect_through(&sshd, &first_relay).await;
    let second_relay = pool::find(&key).expect("the hop is still offered");
    let second = connect_through(&sshd, &second_relay).await;
    assert_eq!(echo(&first, "first").await, "first");
    assert_eq!(echo(&second, "second").await, "second");

    // The owner closing its side leaves the hop to the connections on it.
    assert!(!pool::withdraw_if_unshared(&key, &hop));
    drop(hop);
    drop(first);
    drop(first_relay);
    assert_eq!(echo(&second, "still").await, "still");
    let third_relay = pool::find(&key).expect("a borrower keeps the hop open");
    drop(third_relay);

    drop(second);
    drop(second_relay);
    assert!(pool::find(&key).is_none(), "the last user closed the hop");
}
