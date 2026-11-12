//! Minimal SOCKS5 server handshake (RFC 1928): no authentication, CONNECT only.

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const VERSION: u8 = 5;
const REP_SUCCESS: u8 = 0x00;
const REP_GENERAL_FAILURE: u8 = 0x01;
const REP_NOT_ALLOWED: u8 = 0x02;
const REP_CONNECTION_REFUSED: u8 = 0x05;
const REP_TTL_EXPIRED: u8 = 0x06;
const REP_COMMAND_NOT_SUPPORTED: u8 = 0x07;
const REP_ADDRESS_NOT_SUPPORTED: u8 = 0x08;

/// Outcome of a `connect` attempt, mapped onto a SOCKS5 reply code.
#[derive(Clone, Copy)]
pub enum ConnectOutcome {
    Success,
    /// The server's policy forbids this destination.
    NotAllowed,
    /// The server tried and the destination did not accept.
    Refused,
    /// Opening the channel took too long.
    TimedOut,
    /// Anything else, e.g. the SSH session itself failing.
    Failed,
}

impl ConnectOutcome {
    fn reply_code(self) -> u8 {
        match self {
            Self::Success => REP_SUCCESS,
            Self::NotAllowed => REP_NOT_ALLOWED,
            Self::Refused => REP_CONNECTION_REFUSED,
            Self::TimedOut => REP_TTL_EXPIRED,
            Self::Failed => REP_GENERAL_FAILURE,
        }
    }
}

/// Reads the greeting and CONNECT request, returning the requested target.
pub async fn read_connect_request<S>(stream: &mut S) -> Result<(String, u16), String>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let io_err = |e: std::io::Error| format!("SOCKS5 handshake failed: {e}");

    let mut head = [0u8; 2];
    stream.read_exact(&mut head).await.map_err(io_err)?;
    if head[0] != VERSION {
        return Err("Not a SOCKS5 client".to_string());
    }
    let mut methods = vec![0u8; head[1] as usize];
    stream.read_exact(&mut methods).await.map_err(io_err)?;
    if !methods.contains(&0x00) {
        stream.write_all(&[VERSION, 0xFF]).await.map_err(io_err)?;
        return Err("SOCKS5 client requires authentication".to_string());
    }
    stream.write_all(&[VERSION, 0x00]).await.map_err(io_err)?;

    let mut request = [0u8; 4];
    stream.read_exact(&mut request).await.map_err(io_err)?;
    if request[0] != VERSION {
        return Err("Invalid SOCKS5 request".to_string());
    }
    if request[1] != 0x01 {
        write_reply(stream, REP_COMMAND_NOT_SUPPORTED).await?;
        return Err("Only SOCKS5 CONNECT is supported".to_string());
    }

    let host = match request[3] {
        0x01 => {
            let mut octets = [0u8; 4];
            stream.read_exact(&mut octets).await.map_err(io_err)?;
            std::net::Ipv4Addr::from(octets).to_string()
        }
        0x03 => {
            let mut len = [0u8; 1];
            stream.read_exact(&mut len).await.map_err(io_err)?;
            let mut name = vec![0u8; len[0] as usize];
            stream.read_exact(&mut name).await.map_err(io_err)?;
            String::from_utf8(name).map_err(|_| "Invalid SOCKS5 domain name".to_string())?
        }
        0x04 => {
            let mut octets = [0u8; 16];
            stream.read_exact(&mut octets).await.map_err(io_err)?;
            std::net::Ipv6Addr::from(octets).to_string()
        }
        _ => {
            write_reply(stream, REP_ADDRESS_NOT_SUPPORTED).await?;
            return Err("Unsupported SOCKS5 address type".to_string());
        }
    };
    let mut port = [0u8; 2];
    stream.read_exact(&mut port).await.map_err(io_err)?;

    Ok((host, u16::from_be_bytes(port)))
}

pub async fn write_outcome<S>(stream: &mut S, outcome: ConnectOutcome) -> Result<(), String>
where
    S: AsyncWrite + Unpin,
{
    write_reply(stream, outcome.reply_code()).await
}

async fn write_reply<S>(stream: &mut S, code: u8) -> Result<(), String>
where
    S: AsyncWrite + Unpin,
{
    // Bound address is unknown to the client of a tunnel, so report 0.0.0.0:0.
    stream
        .write_all(&[VERSION, code, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await
        .map_err(|e| format!("SOCKS5 reply failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn handshake(request: &[u8]) -> (Result<(String, u16), String>, Vec<u8>) {
        let (mut client, mut server) = tokio::io::duplex(256);
        client.write_all(request).await.unwrap();
        let result = read_connect_request(&mut server).await;
        drop(server);
        let mut replies = Vec::new();
        client.read_to_end(&mut replies).await.unwrap();
        (result, replies)
    }

    #[tokio::test]
    async fn parses_a_domain_connect_request() {
        let mut req = vec![5, 1, 0, 5, 1, 0, 3, 11];
        req.extend_from_slice(b"example.com");
        req.extend_from_slice(&443u16.to_be_bytes());
        let (result, replies) = handshake(&req).await;
        assert_eq!(result.unwrap(), ("example.com".to_string(), 443));
        assert_eq!(replies, vec![5, 0]);
    }

    #[tokio::test]
    async fn parses_ipv4_and_ipv6_requests() {
        let mut v4 = vec![5, 1, 0, 5, 1, 0, 1, 10, 0, 0, 7];
        v4.extend_from_slice(&80u16.to_be_bytes());
        assert_eq!(
            handshake(&v4).await.0.unwrap(),
            ("10.0.0.7".to_string(), 80)
        );

        let mut v6 = vec![5, 1, 0, 5, 1, 0, 4];
        v6.extend_from_slice(&std::net::Ipv6Addr::LOCALHOST.octets());
        v6.extend_from_slice(&8080u16.to_be_bytes());
        assert_eq!(handshake(&v6).await.0.unwrap(), ("::1".to_string(), 8080));
    }

    #[tokio::test]
    async fn rejects_unsupported_commands_with_a_reply() {
        // BIND (0x02) is not supported.
        let req = vec![5, 1, 0, 5, 2, 0, 1, 1, 1, 1, 1, 0, 80];
        let (result, replies) = handshake(&req).await;
        assert!(result.is_err());
        assert_eq!(replies[0..2], [5, 0]);
        assert_eq!(replies[2], 5);
        assert_eq!(replies[3], REP_COMMAND_NOT_SUPPORTED);
    }

    #[test]
    fn outcomes_map_to_distinct_reply_codes() {
        assert_eq!(ConnectOutcome::Success.reply_code(), 0x00);
        assert_eq!(ConnectOutcome::Failed.reply_code(), 0x01);
        assert_eq!(ConnectOutcome::NotAllowed.reply_code(), 0x02);
        assert_eq!(ConnectOutcome::Refused.reply_code(), 0x05);
        assert_eq!(ConnectOutcome::TimedOut.reply_code(), 0x06);
    }

    #[tokio::test]
    async fn refuses_clients_that_need_authentication() {
        let (result, replies) = handshake(&[5, 1, 2]).await;
        assert!(result.is_err());
        assert_eq!(replies, vec![5, 0xFF]);
    }

    #[tokio::test]
    async fn rejects_non_socks5_traffic() {
        let (result, _) = handshake(&[4, 1, 0]).await;
        assert!(result.is_err());
    }
}
