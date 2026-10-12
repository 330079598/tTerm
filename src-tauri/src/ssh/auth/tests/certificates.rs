//! OpenSSH certificates offered from the agent.

use std::time::{SystemTime, UNIX_EPOCH};

use russh::keys::agent::client::AgentClient;
use russh::keys::agent::AgentIdentity;
use russh::keys::signature::Signer;
use russh::keys::ssh_key::certificate::{Builder, CertType};
use russh::keys::ssh_key::Algorithm;
use russh::keys::{Certificate, PrivateKey, PublicKey};
use russh::server::Auth;
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};

use super::{Harness, Script, USER};
use crate::ssh::agent::{offer_identities, offered_before};

/// Signs a user certificate for `key` with a throwaway certificate authority.
fn certify(key: &PublicKey) -> Certificate {
    let ca = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).expect("CA key");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after 1970")
        .as_secs();
    let mut builder = Builder::new_with_random_nonce(&mut rand::rng(), key, now - 60, now + 3600)
        .expect("certificate builder");
    builder.cert_type(CertType::User).expect("certificate type");
    builder.key_id("tterm-test").expect("key id");
    builder.valid_principal(USER).expect("principal");
    builder.sign(&ca).expect("sign certificate")
}

fn put_string(buffer: &mut Vec<u8>, bytes: &[u8]) {
    buffer.extend_from_slice(&u32::try_from(bytes.len()).expect("short").to_be_bytes());
    buffer.extend_from_slice(bytes);
}

fn take_string(buffer: &[u8]) -> (&[u8], &[u8]) {
    let (length, rest) = buffer.split_at(4);
    let length = u32::from_be_bytes(length.try_into().expect("four bytes")) as usize;
    rest.split_at(length)
}

/// Answers one agent request: the certificate as the only identity, and
/// signatures made with `key`. `russh`'s own agent server cannot hold
/// certificates.
fn agent_reply(request: &[u8], key: &PrivateKey, certificate: &Certificate) -> Vec<u8> {
    const FAILURE: u8 = 5;
    const REQUEST_IDENTITIES: u8 = 11;
    const IDENTITIES_ANSWER: u8 = 12;
    const SIGN_REQUEST: u8 = 13;
    const SIGN_RESPONSE: u8 = 14;

    match request.split_first() {
        Some((&REQUEST_IDENTITIES, _)) => {
            let mut reply = vec![IDENTITIES_ANSWER];
            reply.extend_from_slice(&1u32.to_be_bytes());
            put_string(
                &mut reply,
                &certificate.to_bytes().expect("encode certificate"),
            );
            put_string(&mut reply, b"");
            reply
        }
        Some((&SIGN_REQUEST, body)) => {
            let (_identity, rest) = take_string(body);
            let (data, _flags) = take_string(rest);
            let signature = key.try_sign(data).expect("sign");
            let mut blob = Vec::new();
            put_string(&mut blob, signature.algorithm().as_str().as_bytes());
            put_string(&mut blob, signature.as_bytes());
            let mut reply = vec![SIGN_RESPONSE];
            put_string(&mut reply, &blob);
            reply
        }
        _ => vec![FAILURE],
    }
}

/// Starts an SSH agent holding `key` only as `certificate`, and returns the
/// client end of its connection.
fn spawn_certificate_agent(key: PrivateKey, certificate: Certificate) -> DuplexStream {
    let (client, mut server) = tokio::io::duplex(64 * 1024);
    tokio::spawn(async move {
        while let Ok(length) = server.read_u32().await {
            let mut request = vec![0; length as usize];
            if server.read_exact(&mut request).await.is_err() {
                return;
            }
            let mut reply = Vec::new();
            put_string(&mut reply, &agent_reply(&request, &key, &certificate));
            if server.write_all(&reply).await.is_err() {
                return;
            }
        }
    });
    client
}

#[tokio::test]
async fn the_agent_offers_a_certificate_as_a_certificate() {
    let mut harness = Harness::start(
        Script {
            certificate: || Auth::Accept,
            ..Script::default()
        },
        vec![],
    )
    .await;

    let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).expect("client key");
    let certificate = certify(key.public_key());
    let mut agent = AgentClient::connect(spawn_certificate_agent(key, certificate.clone()));
    let identities = agent.request_identities().await.expect("list identities");
    let mut offered = Vec::new();
    let verdict = offer_identities(
        &mut harness.handle,
        USER,
        &mut agent,
        identities,
        &mut offered,
    )
    .await
    .expect("a verdict");

    assert!(verdict.success());
    assert_eq!(harness.log(), ["certificate"]);
    assert!(offered_before(&offered, &AgentIdentity::from(certificate)));
}

#[test]
fn a_certificate_and_the_key_it_certifies_are_offered_apart() {
    let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).expect("client key");
    let certificate = AgentIdentity::from(certify(key.public_key()));
    let bare_key = AgentIdentity::from(key.public_key().clone());

    assert!(!offered_before(
        std::slice::from_ref(&bare_key),
        &certificate
    ));
    assert!(!offered_before(
        std::slice::from_ref(&certificate),
        &bare_key
    ));
    assert!(offered_before(
        std::slice::from_ref(&certificate),
        &certificate
    ));
    // Another certificate for the same key, such as a renewed one, is new.
    let renewed = AgentIdentity::from(certify(key.public_key()));
    assert!(!offered_before(&[certificate], &renewed));
}
