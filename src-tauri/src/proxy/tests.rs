use super::connect::connect_with_timeout;
use super::*;
use crate::db::Database;
use crate::ssh::types::SshConnectError;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const BANNER: &[u8] = b"SSH-2.0-test\r\n";

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

/// A one-connection proxy that runs `serve` on the accepted stream.
async fn fake_proxy<F, Fut>(serve: F) -> u16
where
    F: FnOnce(TcpStream) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        serve(stream).await;
    });
    port
}

async fn read_request_head(stream: &mut TcpStream) -> String {
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") {
        head.push(stream.read_u8().await.unwrap());
    }
    String::from_utf8(head).unwrap()
}

async fn connect(proxy: &ProxyEndpoint) -> Result<TcpStream, SshConnectError> {
    connect_with_timeout(proxy, "target.internal", 22, Duration::from_secs(5)).await
}

#[tokio::test]
async fn http_connect_keeps_the_banner_sent_with_the_response() {
    let port = fake_proxy(|mut stream| async move {
        let head = read_request_head(&mut stream).await;
        assert!(head.starts_with("CONNECT target.internal:22 HTTP/1.1\r\n"));
        // "me:pw" in Base64.
        assert!(head.contains("Proxy-Authorization: Basic bWU6cHc=\r\n"));
        // The banner arrives in the same packet as the response head.
        let mut reply = b"HTTP/1.1 200 Connection established\r\n\r\n".to_vec();
        reply.extend_from_slice(BANNER);
        stream.write_all(&reply).await.unwrap();
    })
    .await;

    let mut stream = connect(&endpoint(ProxyKind::Http, port, Some(("me", "pw"))))
        .await
        .unwrap();
    let mut banner = vec![0u8; BANNER.len()];
    stream.read_exact(&mut banner).await.unwrap();
    assert_eq!(banner, BANNER);
}

#[tokio::test]
async fn http_proxy_errors_say_what_to_fix() {
    for (status, credentials, expected, retryable) in [
        (
            "407 Proxy Authentication Required",
            None,
            "requires a user name",
            false,
        ),
        (
            "407 Proxy Authentication Required",
            Some(("me", "x")),
            "rejected the user name",
            false,
        ),
        (
            "403 Forbidden",
            None,
            "could not connect to target.internal:22: 403 Forbidden",
            false,
        ),
        ("502 Bad Gateway", None, "502 Bad Gateway", true),
    ] {
        let port = fake_proxy(move |mut stream| async move {
            read_request_head(&mut stream).await;
            let reply = format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\n\r\n");
            stream.write_all(reply.as_bytes()).await.unwrap();
        })
        .await;
        let error = connect(&endpoint(ProxyKind::Http, port, credentials))
            .await
            .unwrap_err();
        assert!(error.message().contains(expected), "{status}: {error}");
        assert_eq!(error.is_retryable(), retryable, "{status}");
    }
}

#[tokio::test]
async fn socks5_sends_the_host_name_and_signs_in() {
    let port = fake_proxy(|mut stream| async move {
        let mut greeting = [0u8; 4];
        stream.read_exact(&mut greeting).await.unwrap();
        assert_eq!(greeting, [5, 2, 0, 2]);
        stream.write_all(&[5, 2]).await.unwrap();

        let mut auth = [0u8; 7];
        stream.read_exact(&mut auth).await.unwrap();
        assert_eq!(auth, [1, 2, b'm', b'e', 2, b'p', b'w']);
        stream.write_all(&[1, 0]).await.unwrap();

        let mut request = vec![0u8; 5 + "target.internal".len() + 2];
        stream.read_exact(&mut request).await.unwrap();
        assert_eq!(&request[..5], &[5, 1, 0, 3, 15]);
        assert_eq!(&request[5..20], b"target.internal");
        assert_eq!(&request[20..], &22u16.to_be_bytes());

        // Bound to 10.0.0.9:4242, then the server's banner right behind it.
        let mut reply = vec![5, 0, 0, 1, 10, 0, 0, 9, 0x10, 0x92];
        reply.extend_from_slice(BANNER);
        stream.write_all(&reply).await.unwrap();
    })
    .await;

    let mut stream = connect(&endpoint(ProxyKind::Socks5, port, Some(("me", "pw"))))
        .await
        .unwrap();
    let mut banner = vec![0u8; BANNER.len()];
    stream.read_exact(&mut banner).await.unwrap();
    assert_eq!(banner, BANNER);
}

#[tokio::test]
async fn socks5_failures_are_classified() {
    // A refused target may come back; a refused password will not.
    let port = fake_proxy(|mut stream| async move {
        let mut greeting = [0u8; 3];
        stream.read_exact(&mut greeting).await.unwrap();
        stream.write_all(&[5, 0]).await.unwrap();
        let mut request = vec![0u8; 22];
        stream.read_exact(&mut request).await.unwrap();
        stream
            .write_all(&[5, 5, 0, 1, 0, 0, 0, 0, 0, 0])
            .await
            .unwrap();
    })
    .await;
    let error = connect(&endpoint(ProxyKind::Socks5, port, None))
        .await
        .unwrap_err();
    assert!(error.message().contains("connection refused"), "{error}");
    assert!(error.is_retryable());

    let port = fake_proxy(|mut stream| async move {
        let mut greeting = [0u8; 3];
        stream.read_exact(&mut greeting).await.unwrap();
        stream.write_all(&[5, 0xFF]).await.unwrap();
    })
    .await;
    let error = connect(&endpoint(ProxyKind::Socks5, port, None))
        .await
        .unwrap_err();
    assert!(error.message().contains("requires a user name"), "{error}");
    assert!(!error.is_retryable());
}

#[tokio::test]
async fn a_proxy_of_another_kind_is_reported_as_such() {
    // An HTTP proxy answers a SOCKS5 greeting with an HTTP error.
    let port = fake_proxy(|mut stream| async move {
        let mut greeting = [0u8; 3];
        stream.read_exact(&mut greeting).await.unwrap();
        let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n").await;
    })
    .await;
    let error = connect(&endpoint(ProxyKind::Socks5, port, None))
        .await
        .unwrap_err();
    assert!(
        error.message().contains("Check that it is a SOCKS5 proxy"),
        "{error}"
    );
    assert!(!error.is_retryable());
}

#[tokio::test]
async fn an_unreachable_or_silent_proxy_can_be_retried() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let closed_port = listener.local_addr().unwrap().port();
    drop(listener);
    let error = connect(&endpoint(ProxyKind::Http, closed_port, None))
        .await
        .unwrap_err();
    assert!(error.message().starts_with("Could not reach"), "{error}");
    assert!(error.is_retryable());

    let silent = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = silent.local_addr().unwrap().port();
    let proxy = endpoint(ProxyKind::Http, port, None);
    let error = connect_with_timeout(&proxy, "target.internal", 22, Duration::from_millis(200))
        .await
        .unwrap_err();
    assert!(error.message().contains("did not answer"), "{error}");
    assert!(error.is_retryable());
    drop(silent);
}

fn server(id: &str, name: &str) -> ProxyServer {
    ProxyServer {
        id: id.to_string(),
        name: name.to_string(),
        kind: ProxyKind::Socks5,
        host: "127.0.0.1".to_string(),
        port: 7890,
        username: None,
        command: None,
    }
}

#[test]
fn servers_are_trimmed_and_checked() {
    let server = normalize_server(ProxyServer {
        name: " Clash ".to_string(),
        host: " [::1] ".to_string(),
        username: Some("  ".to_string()),
        ..server("p1", "")
    })
    .unwrap();
    assert_eq!(server.name, "Clash");
    assert_eq!(server.host, "::1");
    assert_eq!(server.username, None);

    assert!(normalize_server(server_with(|s| s.name.clear())).is_err());
    assert!(normalize_server(server_with(|s| s.host = "a b".to_string())).is_err());
    assert!(normalize_server(server_with(|s| s.port = 0)).is_err());
}

#[test]
fn a_command_proxy_keeps_only_its_command() {
    let command = |text: &str| {
        server_with(|s| {
            s.kind = ProxyKind::Command;
            s.username = Some("me".to_string());
            s.command = Some(text.to_string());
        })
    };
    let server = normalize_server(command("  cloudflared access ssh --hostname %h  ")).unwrap();
    assert_eq!(
        server.command.as_deref(),
        Some("cloudflared access ssh --hostname %h")
    );
    assert_eq!((server.host.as_str(), server.port), ("", 0));
    assert_eq!(server.username, None);
    // It stores without the fields it does not use.
    let stored = serde_json::to_value(&server).unwrap();
    assert_eq!(stored["kind"], "command");
    assert!(stored.get("username").is_none());

    assert!(normalize_server(command(" ")).is_err());
    assert!(normalize_server(command("nc %h\n%p")).is_err());
    assert!(normalize_server(command("nc %x")).is_err());
    // A server of another kind drops a leftover command.
    let http = normalize_server(server_with(|s| s.command = Some("nc".to_string()))).unwrap();
    assert_eq!(http.command, None);
}

fn server_with(change: impl FnOnce(&mut ProxyServer)) -> ProxyServer {
    let mut server = server("p1", "P");
    change(&mut server);
    server
}

#[test]
fn storage_keeps_order_and_updates_in_place() {
    let database = Database::open_in_memory().unwrap();
    database
        .write(|connection| {
            storage::upsert_proxy(connection, &server("a", "A"))?;
            storage::upsert_proxy(connection, &server("b", "B"))?;
            storage::upsert_proxy(connection, &server("a", "A2"))?;
            let names: Vec<_> = storage::list_proxies(connection)?
                .into_iter()
                .map(|server| server.name)
                .collect();
            assert_eq!(names, ["A2", "B"]);
            assert_eq!(storage::get_proxy(connection, "b")?, Some(server("b", "B")));

            storage::delete_proxy(connection, "a")?;
            assert_eq!(storage::get_proxy(connection, "a")?, None);

            storage::replace_proxies(connection, &[server("c", "C"), server("b", "B")])?;
            let ids: Vec<_> = storage::list_proxies(connection)?
                .into_iter()
                .map(|server| server.id)
                .collect();
            assert_eq!(ids, ["c", "b"]);
            Ok(())
        })
        .unwrap();
}

#[test]
fn a_choice_serializes_with_its_mode() {
    let server = ProxyChoice::Server {
        id: "p1".to_string(),
    };
    assert_eq!(
        serde_json::to_value(&server).unwrap(),
        serde_json::json!({"mode": "server", "id": "p1"})
    );
    let direct: ProxyChoice = serde_json::from_str(r#"{"mode":"direct"}"#).unwrap();
    assert_eq!(direct, ProxyChoice::Direct);
}
