//! A server that has a host certificate presents it to tTerm's client config
//! exactly when `known_hosts` trusts an authority for the host, and the
//! certificate it presents passes the check.

use super::OpenSshKnownHosts;
use super::tests::{key, known, line};
use super::verify::KnownHostVerdict;
use crate::ssh::jump::config::compatibility_client_config;
use russh::keys::ssh_key::Algorithm;
use russh::keys::ssh_key::certificate::{Builder, CertType};
use russh::keys::{PrivateKey, PublicKeyOrCertificate};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const HOST: &str = "db.example.com";

struct Server;

impl russh::server::Handler for Server {
    type Error = russh::Error;
}

/// What the client was shown and what `known_hosts` made of it.
#[derive(Clone, Copy, Default)]
struct Seen {
    certificate: bool,
    trusted: bool,
}

struct Client {
    known: Arc<OpenSshKnownHosts>,
    seen: Arc<Mutex<Seen>>,
}

impl russh::client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        presented: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or_default();
        let trusted = matches!(
            self.known.verify(HOST, 22, presented, now),
            KnownHostVerdict::Trusted
        );
        if let Ok(mut seen) = self.seen.lock() {
            seen.certificate = presented.certificate().is_some();
            seen.trusted = trusted;
        }
        Ok(trusted)
    }
}

/// Connects tTerm's client config to a server with a host key and a
/// certificate for it, signed by `authority`.
async fn connect(known: OpenSshKnownHosts, authority: &PrivateKey) -> Seen {
    let host_key = key(Algorithm::Ed25519);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after 1970")
        .as_secs();
    let mut builder = Builder::new_with_random_nonce(
        &mut rand::rng(),
        host_key.public_key(),
        now - 60,
        now + 3600,
    )
    .expect("certificate builder");
    builder.cert_type(CertType::Host).expect("certificate type");
    builder.key_id("tterm-test").expect("key id");
    builder.valid_principal(HOST).expect("principal");
    let certificate = builder.sign(authority).expect("sign certificate");

    let server_config = Arc::new(russh::server::Config {
        keys: vec![host_key],
        certificates: vec![certificate],
        auth_rejection_time: Duration::from_millis(1),
        ..Default::default()
    });
    let (client_io, server_io) = tokio::io::duplex(1024 * 1024);
    tokio::spawn(async move { russh::server::run_stream(server_config, server_io, Server).await });

    let mut config = compatibility_client_config(15, 3);
    known.request_host_certificates(&mut config, HOST, 22);
    let seen = Arc::new(Mutex::new(Seen::default()));
    let client = Client {
        known: Arc::new(known),
        seen: seen.clone(),
    };
    let _ = russh::client::connect_stream(Arc::new(config), client_io, client).await;
    let seen = seen.lock().map(|seen| *seen);
    seen.unwrap_or_default()
}

#[tokio::test]
async fn a_host_proves_itself_with_a_certificate_from_a_trusted_authority() {
    let authority = key(Algorithm::Ed25519);
    let known = known(&[format!(
        "@cert-authority {}",
        line("*.example.com", authority.public_key())
    )]);

    let seen = connect(known, &authority).await;
    assert!(seen.certificate, "the certificate is asked for");
    assert!(seen.trusted);
}

#[tokio::test]
async fn a_host_without_a_trusted_authority_presents_its_plain_key() {
    let authority = key(Algorithm::Ed25519);

    let seen = connect(OpenSshKnownHosts::default(), &authority).await;
    assert!(
        !seen.certificate,
        "a certificate nothing could check is not asked for"
    );
}
