//! The HTTP `CONNECT` handshake (RFC 9110, section 9.3.6).

use super::connect::ProxyError;
use super::ProxyEndpoint;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use zeroize::Zeroizing;

/// More than any real response head; bounds what a broken proxy makes us read.
const MAX_RESPONSE_HEAD: usize = 16 * 1024;

/// `host:port` as a request target, with an IPv6 address in brackets.
fn authority(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

fn request(proxy: &ProxyEndpoint, host: &str, port: u16) -> Zeroizing<String> {
    let authority = authority(host, port);
    let mut request = Zeroizing::new(format!(
        "CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\n"
    ));
    if let Some(username) = &proxy.username {
        let password = proxy.password.as_deref().map_or("", String::as_str);
        let credentials = Zeroizing::new(format!("{username}:{password}"));
        let encoded = Zeroizing::new(BASE64.encode(credentials.as_bytes()));
        request.push_str("Proxy-Authorization: Basic ");
        request.push_str(&encoded);
        request.push_str("\r\n");
    }
    request.push_str("\r\n");
    request
}

/// Reads the response head one byte at a time, so that nothing after it,
/// which is already the SSH server's banner, is taken from the stream.
async fn read_head<S: AsyncRead + Unpin>(stream: &mut S) -> Result<Vec<u8>, ProxyError> {
    let mut head = Vec::with_capacity(256);
    while !head.ends_with(b"\r\n\r\n") {
        if head.len() >= MAX_RESPONSE_HEAD {
            return Err(ProxyError::Protocol(
                "the response header is too long".to_string(),
            ));
        }
        head.push(stream.read_u8().await?);
    }
    Ok(head)
}

/// The status code and reason of `HTTP/1.1 200 Connection established`.
fn parse_status(head: &[u8]) -> Result<(u16, String), ProxyError> {
    let line = head.split(|&byte| byte == b'\n').next().unwrap_or_default();
    let line = String::from_utf8_lossy(line);
    let mut parts = line.trim_end().splitn(3, ' ');
    let version = parts.next().unwrap_or_default();
    let code = parts.next().and_then(|code| code.parse::<u16>().ok());
    match code {
        Some(code) if version.starts_with("HTTP/") => {
            Ok((code, parts.next().unwrap_or_default().trim().to_string()))
        }
        _ => Err(ProxyError::Protocol(format!(
            "it did not send an HTTP response: {:?}",
            line.chars().take(40).collect::<String>()
        ))),
    }
}

fn status_error(proxy: &ProxyEndpoint, code: u16, reason: &str) -> ProxyError {
    let status = format!("{code} {reason}").trim().to_string();
    match code {
        407 if proxy.username.is_some() => ProxyError::AuthRejected,
        407 => ProxyError::AuthRequired,
        // Bad gateway, unavailable and gateway timeout say the target or the
        // proxy's way to it failed this time.
        502..=504 => ProxyError::Refused {
            reason: status,
            retryable: true,
        },
        _ => ProxyError::Refused {
            reason: status,
            retryable: false,
        },
    }
}

/// Asks the proxy on `stream` for a tunnel to `host:port`.
pub(super) async fn handshake<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    proxy: &ProxyEndpoint,
    host: &str,
    port: u16,
) -> Result<(), ProxyError> {
    if host.is_empty() || host.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(ProxyError::Invalid(format!("'{host}' is not a host name")));
    }
    stream
        .write_all(request(proxy, host, port).as_bytes())
        .await?;
    stream.flush().await?;
    let head = read_head(stream).await?;
    match parse_status(&head)? {
        (200..=299, _) => Ok(()),
        (code, reason) => Err(status_error(proxy, code, &reason)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv6_targets_are_bracketed() {
        assert_eq!(authority("example.com", 22), "example.com:22");
        assert_eq!(authority("10.0.0.1", 2222), "10.0.0.1:2222");
        assert_eq!(authority("::1", 22), "[::1]:22");
    }

    #[test]
    fn status_lines_are_parsed() {
        let (code, reason) = parse_status(b"HTTP/1.1 200 Connection established\r\n\r\n").unwrap();
        assert_eq!((code, reason.as_str()), (200, "Connection established"));
        let (code, reason) = parse_status(b"HTTP/1.0 403\r\n\r\n").unwrap();
        assert_eq!((code, reason.as_str()), (403, ""));
        assert!(matches!(
            parse_status(b"SSH-2.0-OpenSSH_9.6\r\n\r\n"),
            Err(ProxyError::Protocol(_))
        ));
    }
}
