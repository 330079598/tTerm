//! Runs shell scripts on an SFTP connection's host, for the work the SFTP
//! protocol cannot do itself, such as making an archive or searching a tree
//! with `find`.

use russh::client::{Handle, Handler, Msg};
use russh::{Channel, ChannelMsg, Sig};
use tokio::sync::watch;

use crate::sftp::internal::launcher::ChannelLauncher;

/// Enough of a failing command's standard error to say why it failed; a
/// command that fails on every file, such as `find` meeting thousands of
/// unreadable folders, would otherwise grow it without bound.
const STDERR_LIMIT: usize = 8 * 1024;

/// Quotes `value` as one word for a POSIX shell.
pub fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Quotes a path for a POSIX shell so that a command cannot read it as an
/// option: a relative path starting with `-` gets a `./` in front.
pub fn shell_path(path: &str) -> String {
    if path.starts_with('-') {
        shell_quote(&format!("./{path}"))
    } else {
        shell_quote(path)
    }
}

/// Resolves once `cancel` reads true. A sender that is gone can no longer
/// cancel, so the future then never resolves.
pub async fn cancelled(cancel: &mut watch::Receiver<bool>) {
    while !*cancel.borrow_and_update() {
        if cancel.changed().await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

/// A script running on the host. Its standard output is read with
/// [`RemoteScript::next_output`]; its standard error and exit status are
/// kept for [`RemoteScript::finish`].
pub struct RemoteScript {
    channel: Channel<Msg>,
    stderr: Vec<u8>,
    exit_status: Option<u32>,
    done: bool,
}

/// How a script that ran to the end finished.
pub struct ScriptOutcome {
    /// None when the server closed the channel without reporting a status.
    pub exit_status: Option<u32>,
    pub stderr: String,
}

impl ScriptOutcome {
    pub fn succeeded(&self) -> bool {
        self.exit_status == Some(0)
    }

    /// A message for a script that failed: its standard error when it said
    /// anything, else its exit status.
    pub fn failure_message(&self, action: &str) -> String {
        let detail = self.stderr.trim();
        let status = self.exit_status.map_or_else(
            || "no exit status".to_string(),
            |code| format!("exit status {code}"),
        );
        if detail.is_empty() {
            format!("{action} failed with {status}")
        } else {
            format!("{action} failed with {status}: {detail}")
        }
    }
}

impl RemoteScript {
    /// Starts `script` under `sh` on its own channel. The login shell only
    /// runs `exec sh -s`, which bash, zsh, fish and csh all understand, and
    /// the script goes in on standard input, so its quoting is POSIX's
    /// whichever shell the account logs in with. The script must keep its
    /// commands off standard input (`</dev/null`), or they would read the
    /// rest of the script.
    pub async fn start<H: Handler>(ssh: &Handle<H>, script: &str) -> Result<Self, String> {
        let channel = ssh
            .channel_open_session()
            .await
            .map_err(|err| format!("Failed to open SSH channel: {err}"))?;
        Self::start_on(channel, script).await
    }

    /// Starts `script` as [`RemoteScript::start`] does, on a channel the
    /// caller opened, for one that times the opening.
    pub async fn start_on(channel: Channel<Msg>, script: &str) -> Result<Self, String> {
        let started = async {
            channel.exec(true, "exec sh -s").await?;
            channel.data(script.as_bytes()).await?;
            channel.eof().await
        }
        .await;
        if let Err(err) = started {
            let _ = channel.close().await;
            return Err(format!("Failed to run a command on the server: {err}"));
        }
        Ok(Self {
            channel,
            stderr: Vec::new(),
            exit_status: None,
            done: false,
        })
    }

    /// The next piece of standard output, or None once the script is done.
    /// Cancel-safe, so it can race a cancellation in `tokio::select!`.
    pub async fn next_output(&mut self) -> Option<Vec<u8>> {
        while !self.done {
            let Some(message) = self.channel.wait().await else {
                self.done = true;
                break;
            };
            match message {
                ChannelMsg::Data { data } => return Some(data.to_vec()),
                ChannelMsg::ExtendedData { data, .. } => {
                    let room = STDERR_LIMIT.saturating_sub(self.stderr.len());
                    self.stderr.extend_from_slice(&data[..data.len().min(room)]);
                }
                ChannelMsg::ExitStatus { exit_status } => self.exit_status = Some(exit_status),
                ChannelMsg::Failure | ChannelMsg::Close => self.done = true,
                _ => {}
            }
        }
        None
    }

    /// Reads the script to its end and reports how it finished.
    pub async fn finish(mut self) -> ScriptOutcome {
        while self.next_output().await.is_some() {}
        let _ = self.channel.close().await;
        ScriptOutcome {
            exit_status: self.exit_status,
            stderr: String::from_utf8_lossy(&self.stderr).into_owned(),
        }
    }

    /// Stops the script before its end. Closing the channel alone leaves a
    /// command that never writes to it, such as `tar` writing an archive,
    /// running on the host, so it is sent SIGTERM first; servers that do not
    /// support signals ignore that.
    pub async fn stop(self) {
        let _ = self.channel.signal(Sig::TERM).await;
        let _ = self.channel.close().await;
    }
}

/// Runs `script` to its end, ignoring its standard output. Returns None when
/// `cancel` stopped it first.
pub async fn run_to_end<H: Handler>(
    launcher: &ChannelLauncher,
    ssh: &Handle<H>,
    script: &str,
    cancel: &mut watch::Receiver<bool>,
) -> Result<Option<ScriptOutcome>, String> {
    if *cancel.borrow() {
        return Ok(None);
    }
    let mut running = launcher.start_script(ssh, script).await?;
    loop {
        tokio::select! {
            () = cancelled(cancel) => {
                running.stop().await;
                return Ok(None);
            }
            output = running.next_output() => {
                if output.is_none() {
                    return Ok(Some(running.finish().await));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_single_quotes_inside_a_word() {
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
        assert_eq!(shell_quote(""), "''");
    }

    #[test]
    fn keeps_paths_from_reading_as_options() {
        assert_eq!(shell_path("-rf"), "'./-rf'");
        assert_eq!(shell_path("/tmp/-x"), "'/tmp/-x'");
        assert_eq!(shell_path("a b"), "'a b'");
    }

    #[test]
    fn failure_message_prefers_standard_error() {
        let outcome = ScriptOutcome {
            exit_status: Some(2),
            stderr: "tar: nope\n".to_string(),
        };
        assert_eq!(
            outcome.failure_message("Compressing"),
            "Compressing failed with exit status 2: tar: nope"
        );
        let silent = ScriptOutcome {
            exit_status: None,
            stderr: String::new(),
        };
        assert_eq!(
            silent.failure_message("Compressing"),
            "Compressing failed with no exit status"
        );
    }
}
