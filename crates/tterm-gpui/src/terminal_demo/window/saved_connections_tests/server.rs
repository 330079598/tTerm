//! A loopback-only SSH server. Its shell records input but executes nothing.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use russh::keys::PrivateKey;
use russh::keys::ssh_key::{Algorithm, HashAlg};
use russh::server::{Auth, ChannelOpenHandle, Msg, Session};
use russh::{Channel, ChannelId, Pty};
use tokio::net::TcpListener;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

#[derive(Default)]
struct Activity {
    connections: AtomicUsize,
    shells: AtomicUsize,
    inputs: Mutex<Vec<u8>>,
}

pub(super) struct ShellServer {
    port: u16,
    fingerprint: String,
    activity: Arc<Activity>,
    task: JoinHandle<()>,
}

impl ShellServer {
    pub(super) fn start(runtime: &Handle) -> Self {
        let host_key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).unwrap();
        let fingerprint = host_key
            .public_key()
            .fingerprint(HashAlg::Sha256)
            .to_string();
        let config = Arc::new(russh::server::Config {
            keys: vec![host_key],
            ..Default::default()
        });
        let listener = runtime
            .block_on(TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)))
            .unwrap();
        let port = listener.local_addr().unwrap().port();
        let activity = Arc::new(Activity::default());
        let counts = activity.clone();
        let task = runtime.spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                counts.connections.fetch_add(1, Ordering::SeqCst);
                let handler = Shell {
                    activity: counts.clone(),
                    channels: Vec::new(),
                };
                let config = config.clone();
                tokio::spawn(async move {
                    if let Ok(session) = russh::server::run_stream(config, stream, handler).await {
                        let _ = session.await;
                    }
                });
            }
        });
        Self {
            port,
            fingerprint,
            activity,
            task,
        }
    }

    pub(super) fn port(&self) -> u16 {
        self.port
    }
    pub(super) fn fingerprint(&self) -> String {
        self.fingerprint.clone()
    }
    pub(super) fn algorithm(&self) -> String {
        Algorithm::Ed25519.to_string()
    }
    pub(super) fn connections(&self) -> usize {
        self.activity.connections.load(Ordering::SeqCst)
    }
    pub(super) fn shells(&self) -> usize {
        self.activity.shells.load(Ordering::SeqCst)
    }
    pub(super) fn inputs(&self) -> Vec<u8> {
        self.activity.inputs.lock().unwrap().clone()
    }
}

impl Drop for ShellServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

struct Shell {
    activity: Arc<Activity>,
    channels: Vec<Channel<Msg>>,
}

impl russh::server::Handler for Shell {
    type Error = russh::Error;

    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        Ok(
            if user == "picker-test" && password == "disposable-test-password" {
                Auth::Accept
            } else {
                Auth::reject()
            },
        )
    }

    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: ChannelOpenHandle,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        self.channels.push(channel);
        reply.accept().await;
        Ok(())
    }

    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _: &str,
        _: u32,
        _: u32,
        _: u32,
        _: u32,
        _: &[(Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)
    }

    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.activity.shells.fetch_add(1, Ordering::SeqCst);
        session.channel_success(channel)?;
        session.data(channel, b"SAVED_SSH_READY\r\n".to_vec())
    }

    async fn data(
        &mut self,
        _: ChannelId,
        data: &[u8],
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        self.activity.inputs.lock().unwrap().extend_from_slice(data);
        Ok(())
    }
}
