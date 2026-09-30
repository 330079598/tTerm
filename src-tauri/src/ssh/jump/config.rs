use crate::ssh::types::SshConnectError;
use russh::keys::Algorithm;
use russh::{cipher, client, kex, mac, AlgorithmKind, Preferred};
use std::time::Duration;

/// Whether `russh` can verify RSA signatures, i.e. was built with its `rsa`
/// feature. tTerm leaves it off: it brings in the RustCrypto `rsa` crate and
/// with it RUSTSEC-2023-0071.
fn rsa_host_keys_verifiable() -> bool {
    russh::keys::key::ALL_KEY_TYPES
        .iter()
        .any(|algorithm| matches!(algorithm, Algorithm::Rsa { .. }))
}

/// The host key algorithms to offer. `russh` offers RSA even when it cannot
/// verify it, which ends in a misleading "Wrong server signature"; leaving
/// RSA out makes such a server fail the negotiation instead, where
/// [`unverifiable_host_key_error`] can explain it.
fn host_key_algorithms() -> Vec<Algorithm> {
    Preferred::DEFAULT
        .key
        .iter()
        .filter(|algorithm| {
            rsa_host_keys_verifiable() || !matches!(algorithm, Algorithm::Rsa { .. })
        })
        .cloned()
        .collect()
}

/// Recognizes a connect failure caused by a server whose only host keys are
/// RSA (or DSA), common on old network equipment, and says so. Permanent:
/// retrying cannot change what the server offers.
pub(crate) fn unverifiable_host_key_error(error: &russh::Error) -> Option<SshConnectError> {
    let russh::Error::NoCommonAlgo {
        kind: AlgorithmKind::Key,
        theirs,
        ..
    } = error
    else {
        return None;
    };
    let legacy_only = !theirs.is_empty()
        && theirs
            .iter()
            .all(|name| name.contains("rsa") || name.contains("dss"));
    legacy_only.then(|| {
        SshConnectError::Permanent(format!(
            "This server only identifies itself with an RSA host key ({}), which this \
             version of tTerm cannot verify. Add an Ed25519 or ECDSA host key on the server \
             (ssh-keygen -A) to connect.",
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
        key: std::borrow::Cow::Owned(host_key_algorithms()),
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
    use super::{host_key_algorithms, rsa_host_keys_verifiable, unverifiable_host_key_error};
    use russh::keys::Algorithm;
    use russh::AlgorithmKind;

    fn no_common(kind: AlgorithmKind, theirs: &[&str]) -> russh::Error {
        russh::Error::NoCommonAlgo {
            kind,
            ours: vec!["ssh-ed25519".to_string()],
            theirs: theirs.iter().map(|name| name.to_string()).collect(),
        }
    }

    #[test]
    fn rsa_is_offered_only_when_it_can_be_verified() {
        let offers_rsa = host_key_algorithms()
            .iter()
            .any(|algorithm| matches!(algorithm, Algorithm::Rsa { .. }));
        assert_eq!(offers_rsa, rsa_host_keys_verifiable());
        assert!(host_key_algorithms().contains(&Algorithm::Ed25519));
    }

    #[test]
    fn explains_servers_with_only_rsa_host_keys() {
        let error = unverifiable_host_key_error(&no_common(
            AlgorithmKind::Key,
            &["rsa-sha2-512", "rsa-sha2-256", "ssh-rsa"],
        ))
        .expect("RSA-only server");
        assert!(!error.is_retryable());
        assert!(error.to_string().contains("RSA host key"), "{error}");
        assert!(error.to_string().contains("rsa-sha2-512"), "{error}");

        // Anything else keeps its original message.
        for other in [
            no_common(
                AlgorithmKind::Key,
                &["ssh-rsa", "sk-ssh-ed25519@openssh.com"],
            ),
            no_common(AlgorithmKind::Key, &[]),
            no_common(AlgorithmKind::Cipher, &["ssh-rsa"]),
            russh::Error::Disconnect,
        ] {
            assert!(unverifiable_host_key_error(&other).is_none(), "{other}");
        }
    }
}
