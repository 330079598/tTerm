use super::verify::KnownHostVerdict;
use super::OpenSshKnownHosts;
use base64::Engine;
use hmac::{Hmac, KeyInit, Mac};
use russh::keys::ssh_key::certificate::{Builder, CertType};
use russh::keys::ssh_key::{Algorithm, HashAlg};
use russh::keys::{Certificate, PrivateKey, PublicKey, PublicKeyOrCertificate};
use sha1::Sha1;
use std::path::Path;

const NOW: u64 = 1_800_000_000;

pub(super) fn key(algorithm: Algorithm) -> PrivateKey {
    PrivateKey::random(&mut rand::rng(), algorithm).expect("test key")
}

fn ed25519() -> PublicKey {
    key(Algorithm::Ed25519).public_key().clone()
}

pub(super) fn line(hosts: &str, key: &PublicKey) -> String {
    format!("{hosts} {}", key.to_openssh().expect("encode key"))
}

pub(super) fn known(lines: &[String]) -> OpenSshKnownHosts {
    let mut known = OpenSshKnownHosts::default();
    known.add(&lines.join("\n"), Path::new("known_hosts"));
    known
}

fn plain(key: &PublicKey) -> PublicKeyOrCertificate {
    key.clone().into()
}

struct HostCertificate<'a> {
    authority: &'a PrivateKey,
    key: &'a PublicKey,
    cert_type: CertType,
    principals: &'a [&'a str],
    valid_before: u64,
}

impl HostCertificate<'_> {
    fn sign(&self) -> PublicKeyOrCertificate {
        let mut builder =
            Builder::new_with_random_nonce(&mut rand::rng(), self.key, NOW - 60, self.valid_before)
                .expect("certificate builder");
        builder.cert_type(self.cert_type).expect("certificate type");
        builder.key_id("tterm-test").expect("key id");
        if self.principals.is_empty() {
            builder.all_principals_valid().expect("any principal");
        }
        for principal in self.principals {
            builder.valid_principal(*principal).expect("principal");
        }
        let certificate: Certificate = builder.sign(self.authority).expect("sign certificate");
        certificate.into()
    }
}

fn host_certificate<'a>(authority: &'a PrivateKey, key: &'a PublicKey) -> HostCertificate<'a> {
    HostCertificate {
        authority,
        key,
        cert_type: CertType::Host,
        principals: &["db.example.com"],
        valid_before: NOW + 3600,
    }
}

fn is_trusted(verdict: &KnownHostVerdict) -> bool {
    matches!(verdict, KnownHostVerdict::Trusted)
}

fn rejection(verdict: KnownHostVerdict) -> String {
    match verdict {
        KnownHostVerdict::Rejected(reason) => reason,
        other => panic!("expected a rejection, got {other:?}"),
    }
}

#[test]
fn a_listed_key_is_trusted_and_a_different_one_of_its_type_is_a_change() {
    let listed = ed25519();
    let known = known(&[line("db.example.com,10.0.0.5", &listed)]);

    assert!(is_trusted(&known.verify(
        "db.example.com",
        22,
        &plain(&listed),
        NOW
    )));
    assert!(is_trusted(&known.verify(
        "DB.example.com",
        22,
        &plain(&listed),
        NOW
    )));

    let KnownHostVerdict::Undecided {
        listed_fingerprint, ..
    } = known.verify("db.example.com", 22, &plain(&ed25519()), NOW)
    else {
        panic!("a different key is left to tTerm");
    };
    assert_eq!(
        listed_fingerprint,
        Some(listed.fingerprint(HashAlg::Sha256).to_string())
    );

    let other_type = key(Algorithm::Ecdsa {
        curve: russh::keys::ssh_key::EcdsaCurve::NistP256,
    });
    assert!(matches!(
        known.verify("db.example.com", 22, &plain(other_type.public_key()), NOW),
        KnownHostVerdict::Undecided {
            listed_fingerprint: None,
            ..
        }
    ));
    assert!(matches!(
        known.verify("web.example.com", 22, &plain(&listed), NOW),
        KnownHostVerdict::Undecided {
            listed_fingerprint: None,
            ..
        }
    ));
}

#[test]
fn a_host_on_another_port_is_named_with_its_port() {
    let listed = ed25519();
    let known = known(&[line("[db.example.com]:2222", &listed)]);

    assert!(is_trusted(&known.verify(
        "db.example.com",
        2222,
        &plain(&listed),
        NOW
    )));
    assert!(!is_trusted(&known.verify(
        "db.example.com",
        22,
        &plain(&listed),
        NOW
    )));
}

#[test]
fn hashed_and_negated_host_names_are_matched() {
    let listed = ed25519();
    let salt = b"0123456789abcdefghij";
    let mut mac = Hmac::<Sha1>::new_from_slice(salt).expect("hmac key");
    mac.update(b"hidden.example.com");
    let engine = base64::engine::general_purpose::STANDARD;
    let hashed = format!(
        "|1|{}|{}",
        engine.encode(salt),
        engine.encode(mac.finalize().into_bytes())
    );
    let known = known(&[
        line(&hashed, &listed),
        line("*.corp.example.com,!secret.corp.example.com", &listed),
    ]);

    assert!(is_trusted(&known.verify(
        "hidden.example.com",
        22,
        &plain(&listed),
        NOW
    )));
    assert!(is_trusted(&known.verify(
        "db.corp.example.com",
        22,
        &plain(&listed),
        NOW
    )));
    assert!(!is_trusted(&known.verify(
        "secret.corp.example.com",
        22,
        &plain(&listed),
        NOW
    )));
}

#[test]
fn a_revoked_key_is_refused_even_where_it_is_listed() {
    let listed = ed25519();
    let known = known(&[
        line("db.example.com", &listed),
        format!("@revoked {}", line("*", &listed)),
    ]);

    let reason = rejection(known.verify("db.example.com", 22, &plain(&listed), NOW));
    assert!(reason.contains("@revoked in known_hosts:2"), "{reason}");
}

#[test]
fn a_certificate_from_a_trusted_authority_is_trusted_for_its_principals() {
    let authority = key(Algorithm::Ed25519);
    let host_key = ed25519();
    let known = known(&[format!(
        "@cert-authority {}",
        line("*.example.com", authority.public_key())
    )]);

    assert!(known.has_authority_for("db.example.com", 22));
    assert!(!known.has_authority_for("db.other.org", 22));

    let certificate = host_certificate(&authority, &host_key).sign();
    assert!(is_trusted(&known.verify(
        "db.example.com",
        22,
        &certificate,
        NOW
    )));

    let reason = rejection(known.verify("web.example.com", 22, &certificate, NOW));
    assert!(
        reason.contains("is for db.example.com but not for web.example.com"),
        "{reason}"
    );

    let any_host = HostCertificate {
        principals: &[],
        ..host_certificate(&authority, &host_key)
    };
    assert!(
        is_trusted(&known.verify("web.example.com", 22, &any_host.sign(), NOW)),
        "ssh takes a host certificate without principals for any host"
    );
}

#[test]
fn a_certificate_that_does_not_hold_is_refused() {
    let authority = key(Algorithm::Ed25519);
    let host_key = ed25519();
    let known = known(&[format!(
        "@cert-authority {}",
        line("db.example.com", authority.public_key())
    )]);

    let expired = HostCertificate {
        valid_before: NOW - 1,
        ..host_certificate(&authority, &host_key)
    };
    let reason = rejection(known.verify("db.example.com", 22, &expired.sign(), NOW));
    assert!(reason.contains("has expired"), "{reason}");

    let user = HostCertificate {
        cert_type: CertType::User,
        ..host_certificate(&authority, &host_key)
    };
    let reason = rejection(known.verify("db.example.com", 22, &user.sign(), NOW));
    assert!(reason.contains("not a host certificate"), "{reason}");

    let revoked = known_with_revoked(&authority);
    let certificate = host_certificate(&authority, &host_key).sign();
    let reason = rejection(revoked.verify("db.example.com", 22, &certificate, NOW));
    assert!(reason.contains("@revoked"), "{reason}");
}

fn known_with_revoked(authority: &PrivateKey) -> OpenSshKnownHosts {
    known(&[
        format!(
            "@cert-authority {}",
            line("db.example.com", authority.public_key())
        ),
        format!("@revoked {}", line("*", authority.public_key())),
    ])
}

#[test]
fn a_certificate_from_an_unknown_authority_is_checked_as_its_plain_key() {
    let trusted = key(Algorithm::Ed25519);
    let stranger = key(Algorithm::Ed25519);
    let host_key = ed25519();
    let known = known(&[
        format!("@cert-authority {}", line("*", trusted.public_key())),
        line("db.example.com", &host_key),
    ]);

    let certificate = host_certificate(&stranger, &host_key).sign();
    assert!(
        is_trusted(&known.verify("db.example.com", 22, &certificate, NOW)),
        "the key inside is listed for the host"
    );
    assert!(matches!(
        known.verify("web.example.com", 22, &certificate, NOW),
        KnownHostVerdict::Undecided { .. }
    ));
}

#[test]
fn lines_that_cannot_be_read_are_passed_over() {
    let listed = ed25519();
    let known = known(&[
        "# a comment".to_string(),
        "garbage".to_string(),
        format!(
            "db.example.com\t{}",
            listed.to_openssh().expect("encode key")
        ),
    ]);

    assert!(is_trusted(&known.verify(
        "db.example.com",
        22,
        &plain(&listed),
        NOW
    )));
}
