//! Shell integration for SSH sessions, which have it unless their connection
//! or the setting turns it off: the remote shell reports its working directory (OSC 7), which the SFTP drawer
//! can follow, and marks its commands (OSC 133) unless command marks are off
//! in the settings. Before the interactive shell starts, a separate exec
//! channel runs `exec sh -s` and is fed [`install_script`], which writes
//! tTerm's scripts for bash, zsh and fish to
//! `~/.cache/tterm/shell-integration/<version>` (once per version), removes
//! versions unused for a month, and replies with the directory. The
//! interactive channel then runs `unix/start.sh` from there in place of a
//! plain shell; it prints the login banner sshd shows only for plain shells.
//! A host without a POSIX `sh` or with another login shell, and any failure
//! along the way, gets the plain shell, and the connection header says why.
//!
//! Exec commands are run by the user's login shell (`$SHELL -c`), so they
//! hold nothing beyond `exec`, `env` with a plain assignment, `sh`, one
//! single-quoted path and a plain word, which bash, zsh, fish, csh and ksh
//! all read the same way.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use russh::client::{self, Msg};
use russh::{Channel, ChannelMsg};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::terminal::shell_scripts::{lf, UNIX_SCRIPTS};

/// How long installing the scripts, or starting the shell with them, may
/// take before the session falls back to a plain shell.
const TIMEOUT: Duration = Duration::from_secs(5);

const REPLY_PREFIX: &str = "TTERM-SHELL-INTEGRATION ";
const HEREDOC_END: &str = "TTERM_SCRIPT_END";
/// Installed versions no session has started from for this many days are removed.
const KEEP_UNUSED_DAYS: u32 = 30;

/// How shell integration went for a session, shown in its connection header.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum ShellIntegrationStatus {
    Active {
        /// The host's name (`uname -n`), which tells its directory reports
        /// apart from those of a host reached from it with `ssh`.
        #[serde(skip_serializing_if = "Option::is_none")]
        hostname: Option<String>,
    },
    /// The session got a plain shell.
    Unavailable { reason: UnavailableReason },
}

/// Why a session got a plain shell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum UnavailableReason {
    /// The login shell is not bash, zsh or fish.
    Shell {
        shell: String,
    },
    /// Nothing ran the installer: no POSIX `sh` (a network device, a Windows
    /// server) or a forced command.
    NoReply,
    /// The scripts could not be written to the cache directory.
    WriteFailed,
    /// The cache directory's path holds a single quote.
    Path,
    /// The server refused to run the start script.
    Rejected,
    TimedOut,
    /// The SSH channel failed.
    Failed,
}

impl UnavailableReason {
    /// An answer about the host itself, which a reconnect would get again.
    fn is_lasting(&self) -> bool {
        matches!(
            self,
            Self::Shell { .. } | Self::NoReply | Self::WriteFailed | Self::Path
        )
    }
}

/// Names the install directory, so a new tTerm with changed scripts installs
/// next to an older one instead of over it while it may be in use.
fn version() -> String {
    let mut hasher = Sha256::new();
    for (path, content) in UNIX_SCRIPTS {
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update(lf(content).as_bytes());
        hasher.update([0]);
    }
    hasher
        .finalize()
        .iter()
        .take(6)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Head of [`install_script`]: only bash, zsh and fish get the scripts.
const INSTALL_HEAD: &str = r#"umask 077
case "${SHELL##*/}" in
    bash | zsh | fish) ;;
    *) echo "@REPLY@shell ${SHELL##*/}"; exit 0 ;;
esac
tterm_base=${XDG_CACHE_HOME:-$HOME/.cache}/tterm/shell-integration
tterm_dir=$tterm_base/@VERSION@
tterm_write_failed() {
    rm -rf "$tterm_tmp"
    echo "@REPLY@write-failed"
    exit 0
}
if [ ! -f "$tterm_dir/.complete" ]; then
tterm_tmp=$tterm_base/.tmp.$$
rm -rf "$tterm_tmp"
mkdir -p @DIRECTORIES@ || tterm_write_failed
"#;

/// Tail of [`install_script`], after the chain of file writes.
const INSTALL_TAIL: &str = r#": > "$tterm_tmp/.complete" || tterm_write_failed
# Left behind by an install that stopped half way.
[ -f "$tterm_dir/.complete" ] || rm -rf "$tterm_dir"
if [ -d "$tterm_dir" ]; then rm -rf "$tterm_tmp"; else mv "$tterm_tmp" "$tterm_dir" || tterm_write_failed; fi
fi
# Marks this version as used, then removes the ones unused for a while.
touch "$tterm_dir"
find "$tterm_base" -mindepth 1 -maxdepth 1 -mtime +@KEEP_DAYS@ -exec rm -rf {} \; 2>/dev/null
echo "@REPLY@host $(uname -n 2>/dev/null)"
echo "@REPLY@ok $tterm_dir"
"#;

/// The `sh` script that installs the scripts and replies with
/// `TTERM-SHELL-INTEGRATION ok <dir>`, after a `host <name>` line, or with
/// why not. The files are
/// written to a temporary directory first and renamed into place, so another
/// tab installing at the same moment never sees half of them.
pub(crate) fn install_script() -> String {
    let mut directories: Vec<&str> = UNIX_SCRIPTS
        .iter()
        .filter_map(|(path, _)| path.rsplit_once('/').map(|(dir, _)| dir))
        .collect();
    directories.sort_unstable();
    directories.dedup();
    let directories: Vec<String> = directories
        .iter()
        .map(|dir| format!("\"$tterm_tmp/{dir}\""))
        .collect();

    let mut script = INSTALL_HEAD
        .replace("@VERSION@", &version())
        .replace("@DIRECTORIES@", &directories.join(" "))
        .replace("@REPLY@", REPLY_PREFIX);
    // One `&&` chain, so a failed write skips the rest.
    for (path, content) in UNIX_SCRIPTS {
        script.push_str(&format!(
            "cat > \"$tterm_tmp/{path}\" <<'{HEREDOC_END}' &&\n{content}{HEREDOC_END}\n",
            content = lf(content),
        ));
    }
    script.push_str(
        &INSTALL_TAIL
            .replace("@KEEP_DAYS@", &KEEP_UNUSED_DAYS.to_string())
            .replace("@REPLY@", REPLY_PREFIX),
    );
    script
}

/// What the installer answered.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Reply<'a> {
    Installed(&'a str),
    /// The login shell is not one the scripts are for.
    Shell(&'a str),
    WriteFailed,
}

/// The installer's answer in its output, which may follow whatever the login
/// shell's startup files print.
pub(crate) fn parse_reply(output: &str) -> Option<Reply<'_>> {
    output.lines().find_map(|line| {
        let answer = line.trim_end_matches('\r').strip_prefix(REPLY_PREFIX)?;
        if let Some(dir) = answer.strip_prefix("ok ") {
            return dir.starts_with('/').then_some(Reply::Installed(dir));
        }
        if let Some(shell) = answer.strip_prefix("shell") {
            return Some(Reply::Shell(shell.trim()));
        }
        (answer == "write-failed").then_some(Reply::WriteFailed)
    })
}

/// The host name the installer reported, if any.
pub(crate) fn parse_hostname(output: &str) -> Option<&str> {
    output.lines().find_map(|line| {
        let name = line
            .trim_end_matches('\r')
            .strip_prefix(REPLY_PREFIX)?
            .strip_prefix("host ")?
            .trim();
        (!name.is_empty()).then_some(name)
    })
}

/// How to start a session's shell with shell integration.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Prepared {
    /// The exec command that starts the shell.
    pub command: String,
    /// The host's name, for [`ShellIntegrationStatus::Active`].
    pub hostname: Option<String>,
}

/// The exec command that starts the shell from `dir`, or `None` for a path
/// that cannot be single-quoted. Without `marks` the shell only reports its
/// working directory.
pub(crate) fn launch_command(dir: &str, marks: bool) -> Option<String> {
    if dir.contains('\'') || dir.chars().any(char::is_control) {
        return None;
    }
    let env = if marks { "" } else { "env tterm_marks=0 " };
    Some(format!("exec {env}sh '{dir}/unix/start.sh' --motd"))
}

/// Hosts (`user@host:port`) that gave a lasting reason for a plain shell, so
/// reconnects go straight to it until tTerm restarts.
fn lasting_reasons() -> MutexGuard<'static, HashMap<String, UnavailableReason>> {
    static REASONS: OnceLock<Mutex<HashMap<String, UnavailableReason>>> = OnceLock::new();
    REASONS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Installs the scripts on `host` and returns how to start the shell with
/// them, marking commands when `marks` is set, or why the session gets a
/// plain shell.
pub(crate) async fn prepare<H: client::Handler>(
    session: &client::Handle<H>,
    host: &str,
    marks: bool,
) -> Result<Prepared, UnavailableReason> {
    if let Some(reason) = lasting_reasons().get(host) {
        return Err(reason.clone());
    }
    let result = match tokio::time::timeout(TIMEOUT, install(session)).await {
        Ok(Ok(output)) => match parse_reply(&output) {
            Some(Reply::Installed(dir)) => launch_command(dir, marks)
                .map(|command| Prepared {
                    command,
                    hostname: parse_hostname(&output).map(str::to_string),
                })
                .ok_or(UnavailableReason::Path),
            Some(Reply::Shell(shell)) => Err(UnavailableReason::Shell {
                shell: shell.to_string(),
            }),
            Some(Reply::WriteFailed) => Err(UnavailableReason::WriteFailed),
            None => Err(UnavailableReason::NoReply),
        },
        Ok(Err(err)) => {
            eprintln!("Shell integration install failed: {err}");
            Err(UnavailableReason::Failed)
        }
        Err(_) => Err(UnavailableReason::TimedOut),
    };
    if let Err(reason) = &result {
        if reason.is_lasting() {
            lasting_reasons().insert(host.to_string(), reason.clone());
        }
    }
    result
}

/// Runs the installer and returns everything it printed.
async fn install<H: client::Handler>(session: &client::Handle<H>) -> Result<String, russh::Error> {
    let mut channel = session.channel_open_session().await?;
    channel.exec(true, "exec sh -s").await?;
    channel.data(install_script().as_bytes()).await?;
    channel.eof().await?;
    let mut output = Vec::new();
    while let Some(message) = channel.wait().await {
        match message {
            ChannelMsg::Data { data } => output.extend_from_slice(&data),
            ChannelMsg::Failure | ChannelMsg::Close => break,
            _ => {}
        }
    }
    let _ = channel.close().await;
    Ok(String::from_utf8_lossy(&output).into_owned())
}

/// Runs `command` on a channel that has its PTY. On success returns what the
/// shell printed before the server confirmed the request; on failure the
/// channel is closed and the caller opens another for a plain shell.
pub(crate) async fn start(
    channel: &mut Channel<Msg>,
    command: &str,
) -> Result<Vec<u8>, UnavailableReason> {
    let started = tokio::time::timeout(TIMEOUT, async {
        channel
            .exec(true, command)
            .await
            .map_err(|_| UnavailableReason::Failed)?;
        let mut early_output = Vec::new();
        loop {
            match channel.wait().await {
                Some(ChannelMsg::Success) => return Ok(early_output),
                Some(ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. }) => {
                    early_output.extend_from_slice(&data)
                }
                Some(
                    ChannelMsg::Failure
                    | ChannelMsg::Eof
                    | ChannelMsg::Close
                    | ChannelMsg::ExitStatus { .. },
                ) => return Err(UnavailableReason::Rejected),
                Some(_) => {}
                None => return Err(UnavailableReason::Failed),
            }
        }
    })
    .await
    .unwrap_or(Err(UnavailableReason::TimedOut));
    if started.is_err() {
        let _ = channel.close().await;
    }
    started
}

#[cfg(test)]
mod tests;

/// The installer and `unix/start.sh` with real shells. `#[ignore]`d: they
/// need `sh` plus bash, zsh or fish, and the second a real `sshd`; run with
/// `cargo test --lib -- --ignored real_shell_integration`.
#[cfg(all(test, unix))]
mod real_shell_integration_tests;
