//! A loopback-only SSH server. Its shell records input but executes nothing.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

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

/// The shell packets a scripted loopback server sends.
///
/// `initial` is sent once the shell request succeeds. Each entry in
/// `after_input` is sent after the client sends its unique gate, in the same
/// order. Query replies are ordinary client data too, so a gate match keeps
/// them from advancing the script. Packets inside one entry are separated by
/// [`PACKET_GAP`] so a split escape sequence really crosses channel-data
/// boundaries.
pub(super) struct ShellScript {
    initial: Vec<Vec<u8>>,
    after_input: VecDeque<ShellStep>,
}

struct ShellStep {
    gate: Vec<u8>,
    packets: Vec<Vec<u8>>,
}

impl ShellScript {
    /// Creates a script that sends `initial` when the shell opens.
    pub(super) fn new(initial: Vec<Vec<u8>>) -> Self {
        Self {
            initial,
            after_input: VecDeque::new(),
        }
    }

    /// Adds one group of packets to send after the client sends `gate`.
    pub(super) fn after_input(mut self, gate: &[u8], packets: Vec<Vec<u8>>) -> Self {
        self.after_input.push_back(ShellStep {
            gate: gate.to_vec(),
            packets,
        });
        self
    }
}

const PACKET_GAP: Duration = Duration::from_millis(10);

pub(super) struct ShellServer {
    port: u16,
    fingerprint: String,
    activity: Arc<Activity>,
    script: Arc<Mutex<VecDeque<ShellStep>>>,
    task: JoinHandle<()>,
}

impl ShellServer {
    pub(super) fn start(runtime: &Handle) -> Self {
        Self::start_scripted(
            runtime,
            ShellScript::new(vec![b"SAVED_SSH_READY\r\n".to_vec()]),
        )
    }

    pub(super) fn start_scripted(runtime: &Handle, script: ShellScript) -> Self {
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
        let initial = script.initial;
        let after_input = Arc::new(Mutex::new(script.after_input));
        let scheduled = Arc::clone(&after_input);
        let task = runtime.spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                counts.connections.fetch_add(1, Ordering::SeqCst);
                let handler = Shell {
                    activity: counts.clone(),
                    channels: Vec::new(),
                    shell_channel: None,
                    gate_buffer: Vec::new(),
                    initial: initial.clone(),
                    after_input: Arc::clone(&scheduled),
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
            script: after_input,
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
    pub(super) fn script_remaining(&self) -> usize {
        self.script.lock().unwrap().len()
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
    shell_channel: Option<ChannelId>,
    gate_buffer: Vec<u8>,
    initial: Vec<Vec<u8>>,
    after_input: Arc<Mutex<VecDeque<ShellStep>>>,
}

impl Shell {
    /// Takes the front script step when the accumulated shell input contains
    /// its gate. Query replies are also client data, so the buffer keeps only
    /// a suffix that can still be the start of the expected gate.
    fn take_script_step(&mut self, data: &[u8]) -> Option<ShellStep> {
        self.gate_buffer.extend_from_slice(data);
        let mut queue = self.after_input.lock().unwrap();
        let gate = &queue.front()?.gate;
        if let Some(position) = self
            .gate_buffer
            .windows(gate.len())
            .position(|window| window == gate.as_slice())
        {
            self.gate_buffer.drain(..position + gate.len());
            return queue.pop_front();
        }
        let keep = (0..=self.gate_buffer.len().min(gate.len()))
            .rev()
            .find(|&length| self.gate_buffer[self.gate_buffer.len() - length..] == gate[..length])
            .unwrap_or(0);
        self.gate_buffer.drain(..self.gate_buffer.len() - keep);
        None
    }
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
        self.shell_channel = Some(channel);
        session.channel_success(channel)?;
        let packets = std::mem::take(&mut self.initial);
        send_packets(session, channel, packets).await
    }

    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.shell_channel != Some(channel) {
            // The remote-OS probe opens its own channel. Its bytes are not part
            // of the interactive shell's input transcript.
            return Ok(());
        }
        self.activity.inputs.lock().unwrap().extend_from_slice(data);
        let next = self.take_script_step(data);
        match next {
            Some(step) => send_packets(session, channel, step.packets).await,
            None => Ok(()),
        }
    }
}

async fn send_packets(
    session: &mut Session,
    channel: ChannelId,
    packets: Vec<Vec<u8>>,
) -> Result<(), russh::Error> {
    for (index, packet) in packets.into_iter().enumerate() {
        if index > 0 {
            tokio::time::sleep(PACKET_GAP).await;
        }
        session.data(channel, packet)?;
    }
    Ok(())
}
