//! Runs a proxy command, OpenSSH's `ProxyCommand`: a local program whose
//! standard input and output carry the SSH connection to the first host.

use crate::ssh::types::SshConnectError;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::process::{ExitStatus, Stdio};
use std::task::{ready, Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, ReadBuf};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::task::JoinHandle;

/// How much of the command's error output is kept for the error message.
const STDERR_TAIL: usize = 4096;
/// How long to wait for the command's exit status once its output ended.
const EXIT_WAIT: Duration = Duration::from_secs(2);
/// Characters a shell would treat as syntax, which a host or user name put
/// into the command line must not contain. OpenSSH refuses the same.
const SHELL_SYNTAX: &[char] = &[
    '\'', '"', '`', '$', '\\', ';', '&', '<', '>', '|', '(', ')', '{', '}', '*', '?', '[', ']',
    '!', '#', '~', '^',
];

/// What the command's `%` tokens stand for.
pub(crate) struct CommandTarget<'a> {
    pub host: &'a str,
    pub port: u16,
    pub username: &'a str,
}

/// The command line with its tokens replaced, as OpenSSH does: `%h` and `%n`
/// the host, `%p` the port, `%r` the user and `%%` a percent sign.
pub(crate) fn expand(template: &str, target: &CommandTarget<'_>) -> Result<String, String> {
    let mut command = String::with_capacity(template.len() + target.host.len());
    let mut chars = template.chars();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            command.push(ch);
            continue;
        }
        match chars.next() {
            Some('%') => command.push('%'),
            Some('h' | 'n') => command.push_str(shell_safe("host name", target.host)?),
            Some('p') => command.push_str(&target.port.to_string()),
            Some('r') => command.push_str(shell_safe("user name", target.username)?),
            Some(other) => {
                return Err(format!(
                "The proxy command uses %{other}, which tTerm does not know. Use %h, %p, %r or %%."
            ))
            }
            None => {
                return Err(
                    "The proxy command ends with a lone %. Write %% for a percent sign."
                        .to_string(),
                )
            }
        }
    }
    Ok(command)
}

fn shell_safe<'a>(what: &str, value: &'a str) -> Result<&'a str, String> {
    let unsafe_char = value
        .chars()
        .find(|ch| SHELL_SYNTAX.contains(ch) || ch.is_whitespace() || ch.is_control());
    if let Some(ch) = unsafe_char {
        return Err(format!(
            "The {what} '{value}' contains '{ch}', which cannot be passed to a proxy command."
        ));
    }
    if value.starts_with('-') {
        return Err(format!(
            "The {what} '{value}' starts with '-', which a proxy command would read as an option."
        ));
    }
    Ok(value)
}

/// Starts `command_line` through the system shell, ready to carry SSH.
pub(crate) async fn spawn(command_line: &str) -> Result<CommandStream, SshConnectError> {
    let mut command = shell_command(command_line).await;
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(home) = crate::profiles::parser::paths::home_dir() {
        command.current_dir(home);
    }
    let mut child = command.spawn().map_err(|error| {
        SshConnectError::Permanent(format!("Could not start the proxy command: {error}"))
    })?;
    let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
        return Err(SshConnectError::Permanent(
            "Could not connect to the proxy command's input and output.".to_string(),
        ));
    };
    let stderr = child
        .stderr
        .take()
        .map(|stderr| tokio::spawn(read_tail(stderr)));
    Ok(CommandStream {
        stdin: Some(stdin),
        stdout,
        child: Some(child),
        stderr,
        exit: None,
        ended: false,
    })
}

#[cfg(unix)]
async fn shell_command(command_line: &str) -> Command {
    let mut command = Command::new("/bin/sh");
    // `exec` replaces the shell, so the program itself is stopped when the
    // session ends.
    command.arg("-c").arg(format!("exec {command_line}"));
    // tTerm opened from the Dock has a bare PATH, while the command was
    // written for the user's shell, where Homebrew and the like are on it.
    let login_path =
        tokio::task::spawn_blocking(|| crate::core::login_shell::variable("PATH")).await;
    if let Ok(Some(path)) = login_path {
        command.env("PATH", path);
    }
    command
}

#[cfg(windows)]
async fn shell_command(command_line: &str) -> Command {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut command = Command::new("cmd.exe");
    // With `/S`, cmd removes only the outer quotes and runs the rest as written.
    command
        .raw_arg(format!("/D /S /C \"{command_line}\""))
        .creation_flags(CREATE_NO_WINDOW);
    command
}

/// The last [`STDERR_TAIL`] bytes the command wrote to its error output.
async fn read_tail(mut stderr: ChildStderr) -> Vec<u8> {
    let mut tail = Vec::new();
    let mut buffer = [0u8; 1024];
    while let Ok(count @ 1..) = stderr.read(&mut buffer).await {
        tail.extend_from_slice(&buffer[..count]);
        if tail.len() > STDERR_TAIL {
            tail.drain(..tail.len() - STDERR_TAIL);
        }
    }
    tail
}

type ExitFuture = Pin<Box<dyn Future<Output = io::Result<()>> + Send>>;

/// The running command as the stream SSH reads and writes. When the command
/// fails, the read or write that notices reports its exit status and the end
/// of its error output instead of a bare end of stream.
pub struct CommandStream {
    /// Dropped on shutdown, which closes the command's input; tokio's own
    /// shutdown of a child's input only flushes it.
    stdin: Option<ChildStdin>,
    stdout: ChildStdout,
    /// Killed when the stream is dropped, which ends the command with the session.
    child: Option<Child>,
    stderr: Option<JoinHandle<Vec<u8>>>,
    /// Set once the command closed its output or input: how it ended.
    exit: Option<ExitFuture>,
    ended: bool,
}

impl CommandStream {
    /// Polls how the command ended, starting the wait the first time.
    fn poll_exit(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if self.ended {
            return Poll::Ready(Ok(()));
        }
        let exit = self
            .exit
            .get_or_insert_with(|| Box::pin(exit_report(self.child.take(), self.stderr.take())));
        let result = ready!(exit.as_mut().poll(cx));
        self.exit = None;
        self.ended = true;
        Poll::Ready(result)
    }
}

impl AsyncRead for CommandStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.exit.is_none() && !this.ended {
            let before = buf.filled().len();
            ready!(Pin::new(&mut this.stdout).poll_read(cx, buf))?;
            if buf.filled().len() > before || buf.remaining() == 0 {
                return Poll::Ready(Ok(()));
            }
        }
        // The output ended: a failed command reports why, otherwise it is
        // the end of the stream.
        this.poll_exit(cx)
    }
}

impl AsyncWrite for CommandStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let Some(stdin) = this.stdin.as_mut() else {
            return Poll::Ready(Err(io::ErrorKind::BrokenPipe.into()));
        };
        if this.exit.is_none() && !this.ended {
            match ready!(Pin::new(stdin).poll_write(cx, data)) {
                Err(error) if error.kind() == io::ErrorKind::BrokenPipe => {}
                result => return Poll::Ready(result),
            }
        }
        // The command closed its input, usually because it exited.
        match ready!(this.poll_exit(cx)) {
            Err(error) => Poll::Ready(Err(error)),
            Ok(()) => Poll::Ready(Err(io::ErrorKind::BrokenPipe.into())),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut().stdin.as_mut() {
            Some(stdin) => Pin::new(stdin).poll_flush(cx),
            None => Poll::Ready(Ok(())),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if let Some(stdin) = this.stdin.as_mut() {
            ready!(Pin::new(stdin).poll_flush(cx))?;
        }
        this.stdin = None;
        Poll::Ready(Ok(()))
    }
}

/// `Ok` when the command exited cleanly or is still running, otherwise an
/// error with its exit status and what it last printed to its error output.
async fn exit_report(child: Option<Child>, stderr: Option<JoinHandle<Vec<u8>>>) -> io::Result<()> {
    let Some(mut child) = child else {
        return Ok(());
    };
    let status = match tokio::time::timeout(EXIT_WAIT, child.wait()).await {
        Ok(Ok(status)) if !status.success() => status,
        _ => return Ok(()),
    };
    let tail = match stderr {
        Some(task) => match tokio::time::timeout(EXIT_WAIT, task).await {
            Ok(Ok(tail)) => tail,
            _ => Vec::new(),
        },
        None => Vec::new(),
    };
    Err(io::Error::other(exit_message(
        status,
        &String::from_utf8_lossy(&tail),
    )))
}

fn exit_message(status: ExitStatus, stderr: &str) -> String {
    let mut message = match status.code() {
        Some(code) => format!("The proxy command exited with status {code}"),
        None => format!("The proxy command was stopped ({status})"),
    };
    let detail = last_lines(stderr, 3);
    if !detail.is_empty() {
        message.push_str(": ");
        message.push_str(&detail);
    }
    // 127 is the shell's "command not found", 9009 is cmd's.
    if matches!(status.code(), Some(127 | 9009)) {
        message.push_str(if cfg!(windows) {
            ". Check that its program is installed and on the PATH."
        } else {
            ". Check that its program is installed; tTerm runs it with the PATH of your login shell."
        });
    }
    message
}

/// The last `count` non-empty lines of `text`, joined into one line.
fn last_lines(text: &str, count: usize) -> String {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    lines[lines.len().saturating_sub(count)..].join(" / ")
}

#[cfg(test)]
mod tests;
