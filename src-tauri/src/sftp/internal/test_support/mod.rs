//! Explicit opt-in connection to this card's disposable Docker host.
use crate::sftp::internal::launcher::ChannelLauncher;
use russh::client::{self, Handle};
use russh_sftp::client::SftpSession;
use std::sync::Arc;
use tterm_core::sftp::internal::transfer::RemoteChannels;

pub(crate) struct Client;
impl client::Handler for Client {
    type Error = russh::Error;
    async fn check_server_key(
        &mut self,
        _: &russh::keys::PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}
pub(crate) async fn connect() -> (Handle<Client>, Arc<SftpSession>) {
    assert_eq!(
        std::env::var("TTERM_P104_CONTAINER").as_deref(),
        Ok("1"),
        "requires disposable tterm-P1-04-host on 22404"
    );
    let mut ssh = client::connect(
        Arc::new(client::Config::default()),
        ("127.0.0.1", 22404),
        Client,
    )
    .await
    .unwrap();
    assert!(
        ssh.authenticate_password("alice", "alice-pass")
            .await
            .unwrap()
            .success()
    );
    let sftp = ChannelLauncher::default()
        .open_sftp_session(&ssh)
        .await
        .unwrap();
    (ssh, sftp)
}
pub(crate) async fn channels(ssh: &Handle<Client>) -> RemoteChannels {
    let mut sessions = Vec::new();
    let mut limits = None;
    for _ in 0..2 {
        let (session, negotiated) = ChannelLauncher::default()
            .open_raw_session(ssh)
            .await
            .unwrap();
        sessions.push(session);
        limits = negotiated;
    }
    RemoteChannels::new(sessions, limits)
}
