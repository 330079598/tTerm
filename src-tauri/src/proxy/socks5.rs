//! The SOCKS5 client handshake (RFC 1928), with user name and password
//! authentication (RFC 1929).

use super::connect::ProxyError;
use super::ProxyEndpoint;
use std::net::{Ipv4Addr, Ipv6Addr};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use zeroize::Zeroizing;

const VERSION: u8 = 0x05;
const METHOD_NONE: u8 = 0x00;
const METHOD_PASSWORD: u8 = 0x02;
const NO_ACCEPTABLE_METHOD: u8 = 0xFF;
const PASSWORD_AUTH_VERSION: u8 = 0x01;
const COMMAND_CONNECT: u8 = 0x01;
const ADDRESS_IPV4: u8 = 0x01;
const ADDRESS_DOMAIN: u8 = 0x03;
const ADDRESS_IPV6: u8 = 0x04;

/// What a reply code means, and whether trying again may succeed.
fn reply_error(code: u8) -> ProxyError {
    let (reason, retryable) = match code {
        0x01 => ("general SOCKS server failure", true),
        0x02 => ("the proxy's rules do not allow this connection", false),
        0x03 => ("network unreachable", true),
        0x04 => ("host unreachable", true),
        0x05 => ("connection refused", true),
        0x06 => ("timed out", true),
        0x07 => ("the proxy does not support CONNECT", false),
        0x08 => ("the proxy does not support this address type", false),
        _ => ("unknown SOCKS error", false),
    };
    ProxyError::Refused {
        reason: format!("{reason} (SOCKS reply {code:#04x})"),
        retryable,
    }
}

/// The target address as a CONNECT request carries it. A host name is sent
/// as it is, so the proxy resolves it and names only it can resolve work.
fn target_address(host: &str) -> Result<Vec<u8>, ProxyError> {
    if let Ok(address) = host.parse::<Ipv4Addr>() {
        return Ok([&[ADDRESS_IPV4][..], &address.octets()].concat());
    }
    if let Ok(address) = host.parse::<Ipv6Addr>() {
        return Ok([&[ADDRESS_IPV6][..], &address.octets()].concat());
    }
    let length = u8::try_from(host.len())
        .ok()
        .filter(|&length| length > 0)
        .ok_or_else(|| {
            ProxyError::Invalid(format!("'{host}' is not a host name SOCKS5 can send"))
        })?;
    Ok([&[ADDRESS_DOMAIN, length][..], host.as_bytes()].concat())
}

/// Sends the user name and password once the proxy chose that method.
async fn authenticate<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    proxy: &ProxyEndpoint,
) -> Result<(), ProxyError> {
    let username = proxy.username.as_deref().ok_or(ProxyError::AuthRequired)?;
    let password = proxy.password.as_deref().map_or("", String::as_str);
    let too_long = || {
        ProxyError::Invalid("the user name and password may be at most 255 bytes each".to_string())
    };
    let username_length = u8::try_from(username.len()).map_err(|_| too_long())?;
    let password_length = u8::try_from(password.len()).map_err(|_| too_long())?;
    let mut request = Zeroizing::new(Vec::with_capacity(3 + username.len() + password.len()));
    request.extend_from_slice(&[PASSWORD_AUTH_VERSION, username_length]);
    request.extend_from_slice(username.as_bytes());
    request.push(password_length);
    request.extend_from_slice(password.as_bytes());
    stream.write_all(&request).await?;
    stream.flush().await?;

    let mut reply = [0u8; 2];
    stream.read_exact(&mut reply).await?;
    if reply[1] != 0x00 {
        return Err(ProxyError::AuthRejected);
    }
    Ok(())
}

/// Reads the rest of a CONNECT reply, the address the proxy bound, which a
/// client has no use for but must take off the stream.
async fn skip_bound_address<S: AsyncRead + Unpin>(
    stream: &mut S,
    address_type: u8,
) -> Result<(), ProxyError> {
    let length = match address_type {
        ADDRESS_IPV4 => 4,
        ADDRESS_IPV6 => 16,
        ADDRESS_DOMAIN => usize::from(stream.read_u8().await?),
        other => {
            return Err(ProxyError::Protocol(format!(
                "unknown address type {other:#04x} in its reply"
            )))
        }
    };
    let mut rest = vec![0u8; length + 2];
    stream.read_exact(&mut rest).await?;
    Ok(())
}

/// Asks the proxy on `stream` for a tunnel to `host:port`.
pub(super) async fn handshake<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S,
    proxy: &ProxyEndpoint,
    host: &str,
    port: u16,
) -> Result<(), ProxyError> {
    let address = target_address(host)?;
    let greeting: &[u8] = if proxy.username.is_some() {
        &[VERSION, 2, METHOD_NONE, METHOD_PASSWORD]
    } else {
        &[VERSION, 1, METHOD_NONE]
    };
    stream.write_all(greeting).await?;
    stream.flush().await?;

    let mut choice = [0u8; 2];
    stream.read_exact(&mut choice).await?;
    if choice[0] != VERSION {
        return Err(ProxyError::Protocol(format!(
            "it did not answer as a SOCKS5 proxy (version byte {:#04x})",
            choice[0]
        )));
    }
    match choice[1] {
        METHOD_NONE => {}
        METHOD_PASSWORD => authenticate(stream, proxy).await?,
        NO_ACCEPTABLE_METHOD if proxy.username.is_some() => return Err(ProxyError::AuthRejected),
        NO_ACCEPTABLE_METHOD => return Err(ProxyError::AuthRequired),
        other => {
            return Err(ProxyError::Protocol(format!(
                "it chose authentication method {other:#04x}, which was not offered"
            )))
        }
    }

    let mut request = vec![VERSION, COMMAND_CONNECT, 0x00];
    request.extend_from_slice(&address);
    request.extend_from_slice(&port.to_be_bytes());
    stream.write_all(&request).await?;
    stream.flush().await?;

    let mut reply = [0u8; 4];
    stream.read_exact(&mut reply).await?;
    if reply[0] != VERSION {
        return Err(ProxyError::Protocol(format!(
            "version byte {:#04x} in its reply",
            reply[0]
        )));
    }
    if reply[1] != 0x00 {
        return Err(reply_error(reply[1]));
    }
    skip_bound_address(stream, reply[3]).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_encoded_by_type() {
        assert_eq!(target_address("10.0.0.1").unwrap(), [1, 10, 0, 0, 1]);
        let ipv6 = target_address("::1").unwrap();
        assert_eq!((ipv6[0], ipv6.len(), ipv6[16]), (ADDRESS_IPV6, 17, 1));
        assert_eq!(
            target_address("ab.c").unwrap(),
            [3, 4, b'a', b'b', b'.', b'c']
        );
        assert!(target_address("").is_err());
        assert!(target_address(&"a".repeat(256)).is_err());
    }

    #[test]
    fn transient_replies_are_retryable() {
        assert!(matches!(
            reply_error(0x05),
            ProxyError::Refused {
                retryable: true,
                ..
            }
        ));
        assert!(matches!(
            reply_error(0x02),
            ProxyError::Refused {
                retryable: false,
                ..
            }
        ));
    }
}
