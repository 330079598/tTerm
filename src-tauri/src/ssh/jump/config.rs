use crate::ssh::types::SshConnectError;
use russh::{cipher, client, kex, mac, AlgorithmKind, Preferred};
use std::time::Duration;

/// Recognizes a connect failure caused by a server whose only host keys are
/// DSA, which `russh` cannot verify, and says so. Permanent: retrying cannot
/// change what the server offers.
pub(crate) fn unverifiable_host_key_error(error: &russh::Error) -> Option<SshConnectError> {
    let russh::Error::NoCommonAlgo {
        kind: AlgorithmKind::Key,
        theirs,
        ..
    } = error
    else {
        return None;
    };
    let dsa_only = !theirs.is_empty() && theirs.iter().all(|name| name.contains("dss"));
    dsa_only.then(|| {
        SshConnectError::Permanent(format!(
            "This server only identifies itself with a DSA host key ({}), which tTerm cannot \
             verify. Add an Ed25519, ECDSA or RSA host key on the server (ssh-keygen -A) to \
             connect.",
            theirs.join(", ")
        ))
    })
}

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

#[cfg(test)]
mod tests {
    use super::{compatibility_preferred_algorithms, unverifiable_host_key_error};
    use russh::keys::{Algorithm, HashAlg};
    use russh::AlgorithmKind;

    fn no_common(kind: AlgorithmKind, theirs: &[&str]) -> russh::Error {
        russh::Error::NoCommonAlgo {
            kind,
            ours: vec!["ssh-ed25519".to_string()],
            theirs: theirs.iter().map(|name| name.to_string()).collect(),
        }
    }

    /// A server with both kinds of key must keep presenting the Ed25519 or
    /// ECDSA key it was trusted with, so RSA comes after them, and SHA-1
    /// `ssh-rsa` only when the server signs with nothing better.
    #[test]
    fn rsa_host_keys_are_offered_after_the_others() {
        let offered = compatibility_preferred_algorithms().key.into_owned();
        let position = |wanted: &Algorithm| {
            offered
                .iter()
                .position(|algorithm| algorithm == wanted)
                .unwrap_or_else(|| panic!("{wanted:?} is not offered: {offered:?}"))
        };

        let rsa_sha512 = position(&Algorithm::Rsa {
            hash: Some(HashAlg::Sha512),
        });
        let rsa_sha256 = position(&Algorithm::Rsa {
            hash: Some(HashAlg::Sha256),
        });
        let rsa_sha1 = position(&Algorithm::Rsa { hash: None });
        assert!(position(&Algorithm::Ed25519) < rsa_sha512);
        assert!(rsa_sha512 < rsa_sha256 && rsa_sha256 < rsa_sha1);
        assert_eq!(rsa_sha1, offered.len() - 1);
        assert!(russh::keys::key::ALL_KEY_TYPES
            .iter()
            .any(|algorithm| matches!(algorithm, Algorithm::Rsa { .. })));
    }

    #[test]
    fn explains_servers_with_only_dsa_host_keys() {
        let error = unverifiable_host_key_error(&no_common(AlgorithmKind::Key, &["ssh-dss"]))
            .expect("DSA-only server");
        assert!(!error.is_retryable());
        assert!(error.to_string().contains("DSA host key"), "{error}");
        assert!(error.to_string().contains("ssh-dss"), "{error}");

        // Anything else keeps its original message.
        for other in [
            no_common(
                AlgorithmKind::Key,
                &["ssh-dss", "sk-ssh-ed25519@openssh.com"],
            ),
            no_common(AlgorithmKind::Key, &["rsa-sha2-512", "ssh-rsa"]),
            no_common(AlgorithmKind::Key, &[]),
            no_common(AlgorithmKind::Cipher, &["ssh-dss"]),
            russh::Error::Disconnect,
        ] {
            assert!(unverifiable_host_key_error(&other).is_none(), "{other}");
        }
    }
}
