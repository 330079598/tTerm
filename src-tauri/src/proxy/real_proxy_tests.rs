//! SSH sessions through real proxy servers to a real OpenSSH server (see
//! [`crate::ssh::test_sshd`]): OpenSSH's own SOCKS5 server (`ssh -D`),
//! tinyproxy for HTTP `CONNECT`, and proxy commands (`nc`, `ssh -W`). Run
//! them with:
//! ```sh
//! cargo test --lib -- --ignored real_proxy
//! ```

use super::command::{expand, spawn, CommandTarget};
use super::connect::connect_through_proxy;
use super::{ProxyEndpoint, ProxyKind};
use crate::ssh::auth::{authenticate, AuthPrompter, AuthTarget};
use crate::ssh::test_sshd::{current_user, AcceptAnyServerKey, TempDir, TestSshd};
use russh::ChannelMsg;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

/// A helper process killed when the test ends.
struct Helper(Child);

impl Drop for Helper {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

async fn wait_for_port(port: u16) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .is_err()
    {
        assert!(Instant::now() < deadline, "nothing listens on port {port}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn endpoint(kind: ProxyKind, port: u16, credentials: Option<(&str, &str)>) -> ProxyEndpoint {
    ProxyEndpoint {
        name: "Test".to_string(),
        kind,
        host: "127.0.0.1".to_string(),
        port,
        username: credentials.map(|(username, _)| username.to_string()),
        password: credentials.map(|(_, password)| Zeroizing::new(password.to_string())),
        command: None,
    }
}

/// Signs in to `sshd` through `proxy`, reaching it as `host`, and runs a
/// command, which proves the whole session crossed the proxy.
async fn run_through(proxy: &ProxyEndpoint, host: &str, sshd: &TestSshd) -> String {
    let stream = connect_through_proxy(proxy, host, sshd.port)
        .await
        .unwrap_or_else(|error| panic!("proxy tunnel: {error}"));
    run_session(stream, host, sshd).await
}

/// Signs in to `sshd` as `host` over `stream` and runs a command.
async fn run_session<S>(stream: S, host: &str, sshd: &TestSshd) -> String
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let mut session = russh::client::connect_stream(
        Arc::new(crate::ssh::jump::config::compatibility_client_config(15, 3)),
        stream,
        AcceptAnyServerKey,
    )
    .await
    .expect("SSH handshake through the proxy");
    let username = current_user();
    authenticate(
        &mut session,
        &AuthTarget {
            host,
            port: sshd.port,
            username: &username,
            hop: None,
        },
        sshd.key_method(),
        &AuthPrompter::Unavailable,
    )
    .await
    .expect("key authentication through the proxy");

    let mut channel = session.channel_open_session().await.expect("open session");
    channel
        .exec(true, "echo through-proxy")
        .await
        .expect("exec");
    let mut output = String::new();
    while let Some(message) = channel.wait().await {
        match message {
            ChannelMsg::Data { data } => output.push_str(&String::from_utf8_lossy(&data)),
            ChannelMsg::ExitStatus { .. } | ChannelMsg::Eof => break,
            _ => {}
        }
    }
    output.trim().to_string()
}

#[tokio::test]
#[ignore = "spawns a real sshd and ssh -D; run with --ignored real_proxy"]
async fn real_proxy_socks5_from_openssh_carries_a_session() {
    let sshd = TestSshd::spawn(12331, &["-t", "ed25519"], "").await;
    let socks_port = 12332;
    let socks = Helper(
        Command::new("ssh")
            .args(["-N", "-D", &format!("127.0.0.1:{socks_port}")])
            .args(["-o", "BatchMode=yes", "-o", "IdentitiesOnly=yes"])
            .args([
                "-o",
                "StrictHostKeyChecking=no",
                "-o",
                "UserKnownHostsFile=/dev/null",
            ])
            .args(["-o", "LogLevel=ERROR", "-F", "/dev/null"])
            .arg("-i")
            .arg(&sshd.client_key)
            .args(["-p", &sshd.port.to_string()])
            .arg(format!("{}@127.0.0.1", current_user()))
            .stdin(Stdio::null())
            .spawn()
            .expect("spawn ssh -D"),
    );
    wait_for_port(socks_port).await;

    let proxy = endpoint(ProxyKind::Socks5, socks_port, None);
    assert_eq!(
        run_through(&proxy, "127.0.0.1", &sshd).await,
        "through-proxy"
    );
    // A host name goes to the proxy unresolved; ssh -D resolves it.
    assert_eq!(
        run_through(&proxy, "localhost", &sshd).await,
        "through-proxy"
    );
    drop(socks);
}

#[tokio::test]
#[ignore = "spawns a real sshd and tinyproxy; run with --ignored real_proxy"]
async fn real_proxy_http_connect_from_tinyproxy_carries_a_session() {
    let sshd = TestSshd::spawn(12333, &["-t", "ed25519"], "").await;
    let proxy_port = 12334;
    let dir = TempDir::new();
    let config = dir.path.join("tinyproxy.conf");
    std::fs::write(
        &config,
        format!(
            "Port {proxy_port}\nListen 127.0.0.1\nTimeout 30\nMaxClients 8\n\
             Allow 127.0.0.1\nConnectPort {}\nBasicAuth me secret\nLogLevel Error\n",
            sshd.port
        ),
    )
    .expect("write tinyproxy.conf");
    let tinyproxy = Helper(
        Command::new("tinyproxy")
            .args(["-d", "-c"])
            .arg(&config)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .expect("spawn tinyproxy (brew install tinyproxy)"),
    );
    wait_for_port(proxy_port).await;

    let signed_in = endpoint(ProxyKind::Http, proxy_port, Some(("me", "secret")));
    assert_eq!(
        run_through(&signed_in, "127.0.0.1", &sshd).await,
        "through-proxy"
    );

    let error = connect_through_proxy(
        &endpoint(ProxyKind::Http, proxy_port, Some(("me", "wrong"))),
        "127.0.0.1",
        sshd.port,
    )
    .await
    .expect_err("a wrong password is refused");
    assert!(
        error.message().contains("rejected the user name"),
        "{error}"
    );

    let error = connect_through_proxy(
        &endpoint(ProxyKind::Http, proxy_port, None),
        "127.0.0.1",
        sshd.port,
    )
    .await
    .expect_err("no credentials are refused");
    assert!(error.message().contains("requires a user name"), "{error}");
    drop(tinyproxy);
}

/// Signs in to `sshd` over the proxy command `template`.
async fn run_command(template: &str, sshd: &TestSshd) -> String {
    let username = current_user();
    let command_line = expand(
        template,
        &CommandTarget {
            host: "127.0.0.1",
            port: sshd.port,
            username: &username,
        },
    )
    .expect("expand the command");
    let stream = spawn(&command_line)
        .await
        .unwrap_or_else(|error| panic!("proxy command: {error}"));
    run_session(stream, "127.0.0.1", sshd).await
}

#[tokio::test]
#[ignore = "spawns a real sshd, nc and ssh -W; run with --ignored real_proxy"]
async fn real_proxy_command_carries_a_session() {
    let sshd = TestSshd::spawn(12335, &["-t", "ed25519"], "").await;
    assert_eq!(run_command("nc %h %p", &sshd).await, "through-proxy");

    // The usual jump through another server, here the same one.
    let jump = format!(
        "ssh -F /dev/null -o BatchMode=yes -o IdentitiesOnly=yes \
         -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
         -i {} -p %p %r@%h -W %h:%p",
        sshd.client_key.display()
    );
    assert_eq!(run_command(&jump, &sshd).await, "through-proxy");

    // A command that cannot reach the host says why.
    let error = russh::client::connect_stream(
        Arc::new(crate::ssh::jump::config::compatibility_client_config(15, 3)),
        spawn("nc 127.0.0.1 1").await.expect("spawn nc"),
        AcceptAnyServerKey,
    )
    .await
    .err()
    .expect("nothing listens on port 1");
    assert!(
        error.to_string().contains("exited with status 1"),
        "{error}"
    );
}
