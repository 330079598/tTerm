//! RSA publickey authentication signed by `ring` instead of the RustCrypto
//! `rsa` crate.
//!
//! `russh` signs RSA challenges with the `rsa` crate, whose private-key
//! operations leak timing (RUSTSEC-2023-0071, unpatched). `ring`'s RSA is
//! constant-time and blinded, and it is already linked for TLS and the SSH
//! ciphers, so an RSA key file is handed to it through the same external
//! signer hook the SSH agent uses.

use std::future::Future;

use base64::Engine;
use num_bigint::BigUint;
use ring::rand::SystemRandom;
use ring::rsa::{KeyPair, KeyPairComponents, PublicKeyComponents};
use ring::signature::{RsaEncoding, RSA_PKCS1_SHA256, RSA_PKCS1_SHA512};
use russh::keys::agent::AgentIdentity;
use russh::keys::ssh_key::private::RsaKeypair;
use russh::keys::ssh_key::public::{KeyData, RsaPublicKey};
use russh::keys::ssh_key::{Mpint, PublicKey};
use russh::keys::HashAlg;

/// Why an RSA challenge was not signed.
#[derive(Debug)]
pub(crate) enum RsaSignError {
    /// The connection went away while waiting for the server.
    Disconnected,
    Sign(String),
}

impl From<russh::SendError> for RsaSignError {
    fn from(_: russh::SendError) -> Self {
        Self::Disconnected
    }
}

impl std::fmt::Display for RsaSignError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disconnected => formatter.write_str("the connection was closed"),
            Self::Sign(reason) => formatter.write_str(reason),
        }
    }
}

/// Whether `path` is an old PEM-format RSA key that [`RingRsaSigner`] can use
/// even though `russh` refuses to load it.
pub(crate) fn pem_key_file_is_usable(path: &str) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|pem| RingRsaSigner::from_pem(&pem))
        .is_some_and(|loaded| loaded.is_ok())
}

pub(crate) struct RingRsaSigner {
    key_pair: KeyPair,
}

fn positive_bytes<'a>(value: &'a Mpint, name: &str) -> Result<&'a [u8], String> {
    value
        .as_positive_bytes()
        .ok_or_else(|| format!("RSA key component {name} is not a positive integer"))
}

impl RingRsaSigner {
    /// Loads the key into `ring`. SSH key files carry `d`, `p`, `q` and
    /// `q⁻¹ mod p`; the two CRT exponents `ring` also wants are derived here.
    /// `ring` accepts 2048 to 8192 bit keys and checks the components agree.
    pub(crate) fn new(keypair: &RsaKeypair) -> Result<Self, String> {
        let private = keypair.private();
        let d = positive_bytes(private.d(), "d")?;
        let p = positive_bytes(private.p(), "p")?;
        let q = positive_bytes(private.q(), "q")?;
        let crt_exponent = |prime: &[u8]| {
            (BigUint::from_bytes_be(d) % (BigUint::from_bytes_be(prime) - 1u32)).to_bytes_be()
        };

        let key_pair = KeyPair::from_components(&KeyPairComponents {
            public_key: PublicKeyComponents {
                n: positive_bytes(keypair.public().n(), "n")?,
                e: positive_bytes(keypair.public().e(), "e")?,
            },
            d,
            p,
            q,
            dP: crt_exponent(p).as_slice(),
            dQ: crt_exponent(q).as_slice(),
            qInv: positive_bytes(private.iqmp(), "iqmp")?,
        })
        .map_err(|rejected| format!("the RSA key cannot be used ({rejected})"))?;
        Ok(Self { key_pair })
    }

    /// Reads an RSA key in one of the PEM formats older tools wrote: PKCS#1
    /// (`BEGIN RSA PRIVATE KEY`, the `ssh-keygen` default before OpenSSH 7.8)
    /// or unencrypted PKCS#8. `russh` only parses these with its `rsa`
    /// feature, which this build leaves off.
    ///
    /// Returns `None` when `pem` is not such a key, so the caller keeps the
    /// error it already has.
    pub(crate) fn from_pem(pem: &str) -> Option<Result<(Self, PublicKey), String>> {
        const PKCS1: &str = "RSA PRIVATE KEY";
        const PKCS8: &str = "PRIVATE KEY";

        let pem = pem.trim();
        let label = pem
            .strip_prefix("-----BEGIN ")?
            .split_once("-----")
            .map(|(label, _)| label)
            .filter(|label| *label == PKCS1 || *label == PKCS8)?;
        let body = pem
            .strip_prefix(&format!("-----BEGIN {label}-----"))?
            .trim_end()
            .strip_suffix(&format!("-----END {label}-----"))?;

        // Legacy encryption is announced by RFC 1421 headers before the data.
        if body.contains("ENCRYPTED") {
            return Some(Err(
                "this passphrase-protected PEM format is not supported; convert the key \
                 with `ssh-keygen -p -f <key file>`"
                    .to_string(),
            ));
        }

        let encoded = body
            .lines()
            .filter(|line| !line.contains(':'))
            .flat_map(|line| line.trim().bytes())
            .collect::<Vec<_>>();
        let der = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .ok()?;
        let key_pair = if label == PKCS1 {
            KeyPair::from_der(&der)
                .map_err(|rejected| format!("the RSA key cannot be used ({rejected})"))
        } else {
            // PKCS#8 also carries other algorithms, which are not ours to report.
            Ok(KeyPair::from_pkcs8(&der).ok()?)
        };

        Some(key_pair.and_then(|key_pair| {
            let public = PublicKeyComponents::<Vec<u8>>::from(key_pair.public());
            let public = RsaPublicKey::new(
                Mpint::from_positive_bytes(&public.e),
                Mpint::from_positive_bytes(&public.n),
            )
            .map_err(|error| format!("the RSA public key is invalid ({error})"))?;
            Ok((Self { key_pair }, PublicKey::from(KeyData::Rsa(public))))
        }))
    }

    /// Appends the signature over `data` to it, in the form `russh` sends on.
    fn sign(&self, hash_alg: Option<HashAlg>, mut data: Vec<u8>) -> Result<Vec<u8>, RsaSignError> {
        let (name, padding): (&str, &'static dyn RsaEncoding) = match hash_alg {
            Some(HashAlg::Sha256) => ("rsa-sha2-256", &RSA_PKCS1_SHA256),
            Some(HashAlg::Sha512) => ("rsa-sha2-512", &RSA_PKCS1_SHA512),
            // `ssh-rsa` proper is SHA-1, which `ring` does not sign.
            _ => {
                return Err(RsaSignError::Sign(
                    "SHA-1 RSA signatures are not supported".to_string(),
                ))
            }
        };

        let mut signature = vec![0; self.key_pair.public().modulus_len()];
        self.key_pair
            .sign(padding, &SystemRandom::new(), &data, &mut signature)
            .map_err(|_| RsaSignError::Sign("RSA signing failed".to_string()))?;

        // string signature = string algorithm || string blob
        let put_string = |buffer: &mut Vec<u8>, bytes: &[u8]| {
            buffer.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
            buffer.extend_from_slice(bytes);
        };
        data.extend_from_slice(&((name.len() + signature.len() + 8) as u32).to_be_bytes());
        put_string(&mut data, name.as_bytes());
        put_string(&mut data, &signature);
        Ok(data)
    }
}

impl russh::Signer for RingRsaSigner {
    type Error = RsaSignError;

    fn auth_sign(
        &mut self,
        _key: &AgentIdentity,
        hash_alg: Option<HashAlg>,
        to_sign: Vec<u8>,
    ) -> impl Future<Output = Result<Vec<u8>, Self::Error>> + Send {
        std::future::ready(self.sign(hash_alg, to_sign))
    }
}

#[cfg(test)]
mod tests {
    use super::RingRsaSigner;

    #[test]
    fn pem_loader_leaves_other_files_to_the_caller() {
        for not_ours in [
            "",
            "not a key",
            "-----BEGIN OPENSSH PRIVATE KEY-----\nAAAA\n-----END OPENSSH PRIVATE KEY-----\n",
            "-----BEGIN EC PRIVATE KEY-----\nAAAA\n-----END EC PRIVATE KEY-----\n",
            "-----BEGIN RSA PRIVATE KEY-----\nno footer",
            "-----BEGIN RSA PRIVATE KEY-----\n!!not base64!!\n-----END RSA PRIVATE KEY-----",
        ] {
            assert!(RingRsaSigner::from_pem(not_ours).is_none(), "{not_ours}");
        }
    }

    #[test]
    fn pem_loader_explains_what_it_cannot_read() {
        let encrypted = "-----BEGIN RSA PRIVATE KEY-----\nProc-Type: 4,ENCRYPTED\n\
                         DEK-Info: AES-128-CBC,0123456789ABCDEF0123456789ABCDEF\n\nAAAA\n\
                         -----END RSA PRIVATE KEY-----\n";
        let error = RingRsaSigner::from_pem(encrypted)
            .expect("an RSA PEM key")
            .err()
            .expect("unsupported encryption");
        assert!(error.contains("ssh-keygen -p"), "{error}");

        let garbage = "-----BEGIN RSA PRIVATE KEY-----\nAAAA\n-----END RSA PRIVATE KEY-----\n";
        let error = RingRsaSigner::from_pem(garbage)
            .expect("an RSA PEM key")
            .err()
            .expect("not a key");
        assert!(error.contains("cannot be used"), "{error}");
    }
}
