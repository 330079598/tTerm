//! Getting through sudo: once per connection to learn how the account gets
//! in, then on every channel that works as root.

use russh::client::{Handle, Handler, Msg};
use russh::{Channel, ChannelMsg};
use std::future::Future;
use zeroize::Zeroizing;

use super::command::{PROBE, READY_MARKER, VALIDATE};
use super::errors;
use super::SudoAuth;
use crate::sftp::internal::exec::{RemoteScript, ScriptOutcome};

/// Enough of sudo's standard error to say why it refused.
const STDERR_LIMIT: usize = 8 * 1024;

/// How much a channel may print before its ready line: the command prints
/// nothing else, so more means something other than tTerm's script is
/// talking, such as a login banner from a shell startup file.
const READY_LIMIT: usize = 4 * 1024;

async fn run<H: Handler>(
    ssh: &Handle<H>,
    command: &str,
    input: &[u8],
) -> Result<ScriptOutcome, String> {
    let channel = ssh
        .channel_open_session()
        .await
        .map_err(|err| format!("Failed to open SSH channel: {err}"))?;
    Ok(RemoteScript::run_command(channel, command, input)
        .await?
        .finish()
        .await)
}

/// The password followed by the newline sudo reads up to.
fn password_line(password: &str) -> Zeroizing<Vec<u8>> {
    let mut line = Zeroizing::new(Vec::with_capacity(password.len() + 1));
    line.extend_from_slice(password.as_bytes());
    line.push(b'\n');
    line
}

/// Finds out how sudo lets the account in. Without sudoers' NOPASSWD the
/// password from `load_password` is checked once here, on a channel whose
/// standard input then ends, so that a wrong one fails at once instead of
/// leaving sudo waiting for another try on a channel the SFTP session needs.
pub async fn authenticate<H, F, Fut>(ssh: &Handle<H>, load_password: F) -> Result<SudoAuth, String>
where
    H: Handler,
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<Option<Zeroizing<String>>, String>>,
{
    let probe = run(ssh, PROBE, b"").await?;
    if probe.succeeded() {
        return Ok(SudoAuth::NoPassword);
    }
    if let Some(error) = errors::probe_failed(probe.exit_status, &probe.stderr) {
        return Err(error);
    }

    let password = load_password()
        .await?
        .ok_or_else(errors::no_saved_password)?;
    let check = run(ssh, VALIDATE, &password_line(&password)).await?;
    if check.succeeded() {
        Ok(SudoAuth::Password(password))
    } else {
        Err(errors::validation_failed(check.exit_status, &check.stderr))
    }
}

/// Opens a channel that runs `command`, one of [`super::command`]'s, and
/// returns it once the command runs as root and said it is ready. What is
/// sent on the channel from then on reaches that command.
pub async fn open_elevated<H: Handler>(
    ssh: &Handle<H>,
    auth: &SudoAuth,
    command: &str,
) -> Result<Channel<Msg>, String> {
    let mut channel = ssh
        .channel_open_session()
        .await
        .map_err(|err| format!("Failed to open SSH channel: {err}"))?;
    let started = async {
        channel.exec(true, command).await?;
        if let SudoAuth::Password(password) = auth {
            channel.data(password_line(password).as_slice()).await?;
        }
        Ok::<(), russh::Error>(())
    }
    .await;
    if let Err(err) = started {
        let _ = channel.close().await;
        return Err(format!("Failed to run sudo on the server: {err}"));
    }
    match wait_until_ready(&mut channel).await {
        Ok(()) => Ok(channel),
        Err(error) => {
            let _ = channel.close().await;
            Err(error)
        }
    }
}

/// What the channel said before it was ready.
#[derive(Default)]
struct Startup {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    exit_status: Option<u32>,
}

impl Startup {
    fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }

    fn failure(&self) -> String {
        errors::elevation_failed(self.exit_status, &self.stderr())
    }
}

/// Reads the channel up to the ready line. A wrong password fails as soon
/// as sudo asks again, since no second try ever comes.
async fn wait_until_ready(channel: &mut Channel<Msg>) -> Result<(), String> {
    let marker = format!("{READY_MARKER}\n");
    let mut startup = Startup::default();
    loop {
        let Some(message) = channel.wait().await else {
            return Err(startup.failure());
        };
        match message {
            ChannelMsg::Data { data } => {
                startup.stdout.extend_from_slice(&data);
                if startup.stdout.ends_with(marker.as_bytes()) {
                    return Ok(());
                }
                if startup.stdout.len() > READY_LIMIT {
                    return Err(format!(
                        "The host printed something unexpected before the file manager could \
                         start as root: {}",
                        String::from_utf8_lossy(&startup.stdout).trim()
                    ));
                }
            }
            ChannelMsg::ExtendedData { data, .. } => {
                let room = STDERR_LIMIT.saturating_sub(startup.stderr.len());
                startup
                    .stderr
                    .extend_from_slice(&data[..data.len().min(room)]);
                if errors::is_wrong_password(&startup.stderr()) {
                    return Err(errors::wrong_password());
                }
            }
            ChannelMsg::ExitStatus { exit_status } => startup.exit_status = Some(exit_status),
            ChannelMsg::Failure | ChannelMsg::Close => return Err(startup.failure()),
            _ => {}
        }
    }
}
