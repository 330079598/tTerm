use russh::{cipher, client, kex, mac, Preferred};
use std::time::Duration;

fn compatibility_preferred_algorithms() -> Preferred {
    Preferred {
        kex: std::borrow::Cow::Owned(vec![
            kex::MLKEM768X25519_SHA256,
            kex::CURVE25519,
            kex::CURVE25519_PRE_RFC_8731,
            kex::ECDH_SHA2_NISTP256,
            kex::ECDH_SHA2_NISTP384,
            kex::ECDH_SHA2_NISTP521,
            kex::DH_GEX_SHA256,
            kex::DH_G14_SHA256,
            kex::DH_G14_SHA1,
            kex::DH_GEX_SHA1,
            kex::DH_G1_SHA1,
            kex::EXTENSION_SUPPORT_AS_CLIENT,
            kex::EXTENSION_OPENSSH_STRICT_KEX_AS_CLIENT,
        ]),
        cipher: std::borrow::Cow::Owned(vec![
            cipher::CHACHA20_POLY1305,
            cipher::AES_256_GCM,
            cipher::AES_128_GCM,
            cipher::AES_256_CTR,
            cipher::AES_192_CTR,
            cipher::AES_128_CTR,
            cipher::AES_256_CBC,
            cipher::AES_192_CBC,
            cipher::AES_128_CBC,
        ]),
        mac: std::borrow::Cow::Owned(vec![
            mac::HMAC_SHA512_ETM,
            mac::HMAC_SHA256_ETM,
            mac::HMAC_SHA1_ETM,
            mac::HMAC_SHA512,
            mac::HMAC_SHA256,
            mac::HMAC_SHA1,
        ]),
        ..Preferred::default()
    }
}

pub fn compatibility_client_config(
    keepalive_interval_secs: u64,
    keepalive_max: usize,
) -> client::Config {
    client::Config {
        client_id: russh::SshId::Standard(std::borrow::Cow::Borrowed("SSH-2.0-OpenSSH_9.6")),
        keepalive_interval: Some(Duration::from_secs(keepalive_interval_secs)),
        keepalive_max,
        preferred: compatibility_preferred_algorithms(),
        nodelay: true,
        ..Default::default()
    }
}
