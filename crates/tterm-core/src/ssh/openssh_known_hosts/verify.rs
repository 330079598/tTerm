//! Decides about the key a host presents from what `known_hosts` lists for
//! it, the way `check_host_key` in OpenSSH's `sshconnect.c` does.

use super::{KnownHostLine, OpenSshKnownHosts};
use russh::keys::PublicKeyOrCertificate;
use russh::keys::ssh_key::certificate::CertType;
use russh::keys::ssh_key::known_hosts::Marker;
use russh::keys::ssh_key::public::KeyData;
use russh::keys::ssh_key::{Algorithm, Certificate, HashAlg, PublicKey};

/// What `known_hosts` says about the key a host presents.
#[derive(Debug)]
pub(crate) enum KnownHostVerdict {
    /// A key `ssh` trusts for the host, or a certificate that an authority
    /// it trusts for the host signed and that holds for the host.
    Trusted,
    /// A revoked key, or a certificate from a trusted authority that does
    /// not hold for the host, which `ssh` refuses, with the reason why.
    Rejected(String),
    /// Nothing decides, so the key goes through tTerm's own check. The
    /// fingerprint is that of a different key of the same type that
    /// `known_hosts` lists for the host, which means the key has changed.
    Undecided {
        key: PublicKey,
        listed_fingerprint: Option<String>,
    },
}

/// RSA keys are one type whichever hash they sign with.
fn same_key_type(left: &Algorithm, right: &Algorithm) -> bool {
    match (left, right) {
        (Algorithm::Rsa { .. }, Algorithm::Rsa { .. }) => true,
        _ => left == right,
    }
}

impl OpenSshKnownHosts {
    /// Decides about what `host` at `port` presented, at `now` in seconds
    /// since 1970.
    pub(crate) fn verify(
        &self,
        host: &str,
        port: u16,
        presented: &PublicKeyOrCertificate,
        now: u64,
    ) -> KnownHostVerdict {
        match presented {
            PublicKeyOrCertificate::PublicKey { key, .. } => {
                self.verify_key(host, port, key.key_data())
            }
            PublicKeyOrCertificate::Certificate(certificate) => {
                self.verify_certificate(host, port, certificate, now)
            }
        }
    }

    fn revoked(&self, host: &str, port: u16, key: &KeyData) -> Option<&KnownHostLine> {
        self.lines_for(host, port)
            .find(|line| line.marker == Some(Marker::Revoked) && &line.key == key)
    }

    fn verify_key(&self, host: &str, port: u16, key: &KeyData) -> KnownHostVerdict {
        if let Some(line) = self.revoked(host, port, key) {
            return KnownHostVerdict::Rejected(format!(
                "The host key of {host} is marked @revoked in {}, and ssh refuses it too.",
                line.place
            ));
        }
        let mut listed_fingerprint = None;
        for line in self
            .lines_for(host, port)
            .filter(|line| line.marker.is_none())
        {
            if &line.key == key {
                return KnownHostVerdict::Trusted;
            }
            if listed_fingerprint.is_none()
                && same_key_type(&line.key.algorithm(), &key.algorithm())
            {
                listed_fingerprint = Some(line.key.fingerprint(HashAlg::Sha256).to_string());
            }
        }
        KnownHostVerdict::Undecided {
            key: PublicKey::new(key.clone(), ""),
            listed_fingerprint,
        }
    }

    fn verify_certificate(
        &self,
        host: &str,
        port: u16,
        certificate: &Certificate,
        now: u64,
    ) -> KnownHostVerdict {
        let authority_key = certificate.signature_key();
        for key in [certificate.public_key(), authority_key] {
            if let Some(line) = self.revoked(host, port, key) {
                return KnownHostVerdict::Rejected(format!(
                    "The host certificate of {host} uses a key marked @revoked in {}, and ssh refuses it too.",
                    line.place
                ));
            }
        }
        let Some(authority) = self
            .lines_for(host, port)
            .find(|line| line.marker == Some(Marker::CertAuthority) && &line.key == authority_key)
        else {
            // `ssh` checks the key inside a certificate from an authority it
            // does not trust for the host as it would check a plain key.
            return self.verify_key(host, port, certificate.public_key());
        };
        match certificate_problem(certificate, host, now) {
            Some(problem) => KnownHostVerdict::Rejected(format!(
                "The host certificate of {host} {problem}, though {} trusts the authority that signed it, and ssh refuses it too.",
                authority.place
            )),
            None => KnownHostVerdict::Trusted,
        }
    }
}

/// Why a certificate signed by a trusted authority does not hold for `host`,
/// checked the way `sshkey_cert_check_authority` checks a host certificate.
fn certificate_problem(certificate: &Certificate, host: &str, now: u64) -> Option<String> {
    if certificate.cert_type() != CertType::Host {
        return Some("is a user certificate, not a host certificate".to_string());
    }
    if now < certificate.valid_after() {
        return Some("is not valid yet".to_string());
    }
    if now >= certificate.valid_before() {
        return Some("has expired".to_string());
    }
    // `ssh` compares principals with the host name in lower case, and takes
    // a host certificate that lists none for any host.
    let host = host.to_ascii_lowercase();
    let principals = certificate.valid_principals();
    if !principals.is_empty() && !principals.contains(&host) {
        return Some(format!(
            "is for {} but not for {host}",
            principals.join(", ")
        ));
    }
    if !certificate.critical_options().is_empty() {
        return Some("has critical options, which a host certificate may not have".to_string());
    }
    let authority = certificate.signature_key().fingerprint(HashAlg::Sha256);
    if certificate.validate_at(now, [&authority]).is_err() {
        return Some("has a signature that does not verify".to_string());
    }
    None
}
