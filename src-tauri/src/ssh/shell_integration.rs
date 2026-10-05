//! Shell integration for SSH sessions: command marks (OSC 133) from the
//! remote shell, for connections that turn it on. Before the interactive
//! shell starts, a separate exec channel runs `exec sh -s` and is fed
//! [`install_script`], which writes tTerm's scripts for bash, zsh and fish to
//! `~/.cache/tterm/shell-integration/<version>` (once per version) and
//! replies with that directory. The interactive channel then runs
//! `unix/start.sh` from there in place of a plain shell. A host without a
//! POSIX `sh` or with another login shell, and any failure along the way, gets
//! the plain shell.
//!
//! Exec commands are run by the user's login shell (`$SHELL -c`), so they
//! hold nothing beyond `exec sh` and one single-quoted path, which bash, zsh,
//! fish, csh and ksh all read the same way.

use std::collections::HashSet;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use russh::client::{self, Msg};
use russh::{Channel, ChannelMsg};
use sha2::{Digest, Sha256};

use crate::terminal::shell_scripts::{lf, UNIX_SCRIPTS};

/// How long installing the scripts, or starting the shell with them, may
/// take before the session falls back to a plain shell.
const TIMEOUT: Duration = Duration::from_secs(5);

const REPLY_PREFIX: &str = "TTERM-SHELL-INTEGRATION ok ";
const HEREDOC_END: &str = "TTERM_SCRIPT_END";

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
    *) exit 0 ;;
esac
tterm_base=${XDG_CACHE_HOME:-$HOME/.cache}/tterm/shell-integration
tterm_dir=$tterm_base/@VERSION@
if [ ! -f "$tterm_dir/.complete" ]; then
tterm_tmp=$tterm_base/.tmp.$$
rm -rf "$tterm_tmp"
mkdir -p @DIRECTORIES@ || exit 0
"#;

/// Tail of [`install_script`], after the chain of file writes.
const INSTALL_TAIL: &str = r#": > "$tterm_tmp/.complete" || exit 0
# Left behind by an install that stopped half way.
[ -f "$tterm_dir/.complete" ] || rm -rf "$tterm_dir"
if [ -d "$tterm_dir" ]; then rm -rf "$tterm_tmp"; else mv "$tterm_tmp" "$tterm_dir" || exit 0; fi
fi
[ -f "$tterm_dir/.complete" ] && echo "@REPLY@$tterm_dir"
"#;

/// The `sh` script that installs the scripts and replies with
/// `TTERM-SHELL-INTEGRATION ok <dir>`; no reply means the plain shell. The
/// files are written to a temporary directory first and renamed into place,
/// so another tab installing at the same moment never sees half of them.
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
        .replace("@DIRECTORIES@", &directories.join(" "));
    // One `&&` chain, so a failed write skips the rest.
    for (path, content) in UNIX_SCRIPTS {
        script.push_str(&format!(
            "cat > \"$tterm_tmp/{path}\" <<'{HEREDOC_END}' &&\n{content}{HEREDOC_END}\n",
            content = lf(content),
        ));
    }
    script.push_str(&INSTALL_TAIL.replace("@REPLY@", REPLY_PREFIX));
    script
}

/// The install directory from the installer's output, which may follow
/// whatever the login shell's startup files print.
pub(crate) fn parse_reply(output: &str) -> Option<&str> {
    output
        .lines()
        .filter_map(|line| line.trim_end_matches('\r').strip_prefix(REPLY_PREFIX))
        .find(|dir| dir.starts_with('/'))
}

/// The exec command that starts the shell from `dir`, or `None` for a path
/// that cannot be single-quoted.
pub(crate) fn launch_command(dir: &str) -> Option<String> {
    if dir.contains('\'') || dir.chars().any(char::is_control) {
        return None;
    }
    Some(format!("exec sh '{dir}/unix/start.sh'"))
}

/// Hosts (`user@host:port`) whose installer answered without a directory, so
/// reconnects go straight to the plain shell until tTerm restarts.
fn unsupported() -> MutexGuard<'static, HashSet<String>> {
    static UNSUPPORTED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    UNSUPPORTED
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Installs the scripts on `host` and returns the command that starts the
/// shell with them, or `None` for a plain shell.
pub(crate) async fn prepare<H: client::Handler>(
    session: &client::Handle<H>,
    host: &str,
) -> Option<String> {
    if unsupported().contains(host) {
        return None;
    }
    match tokio::time::timeout(TIMEOUT, install(session)).await {
        Ok(Ok(Some(dir))) => {
            let command = launch_command(&dir);
            if command.is_none() {
                unsupported().insert(host.to_string());
            }
            command
        }
        Ok(Ok(None)) => {
            unsupported().insert(host.to_string());
            None
        }
        // A slow or failing connection may do better on the next attempt.
        Ok(Err(err)) => {
            eprintln!("Shell integration install failed: {err}");
            None
        }
        Err(_) => {
            eprintln!("Shell integration install timed out");
            None
        }
    }
}

async fn install<H: client::Handler>(
    session: &client::Handle<H>,
) -> Result<Option<String>, russh::Error> {
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
    Ok(parse_reply(&String::from_utf8_lossy(&output)).map(str::to_string))
}

/// Runs `command` on a channel that has its PTY. On success returns what the
/// shell printed before the server confirmed the request; on failure the
/// channel is closed and the caller opens another for a plain shell.
pub(crate) async fn start(channel: &mut Channel<Msg>, command: &str) -> Option<Vec<u8>> {
    let started = tokio::time::timeout(TIMEOUT, async {
        channel.exec(true, command).await.ok()?;
        let mut early_output = Vec::new();
        loop {
            match channel.wait().await? {
                ChannelMsg::Success => return Some(early_output),
                ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. } => {
                    early_output.extend_from_slice(&data)
                }
                ChannelMsg::Failure
                | ChannelMsg::Eof
                | ChannelMsg::Close
                | ChannelMsg::ExitStatus { .. } => return None,
                _ => {}
            }
        }
    })
    .await
    .ok()
    .flatten();
    if started.is_none() {
        let _ = channel.close().await;
    }
    started
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installs_every_script_once_under_a_stable_version() {
        let script = install_script();
        for (path, content) in UNIX_SCRIPTS {
            assert_eq!(
                script.matches(&format!("\"$tterm_tmp/{path}\" <<")).count(),
                1,
                "{path}"
            );
            assert!(
                content.ends_with('\n') && !content.lines().any(|line| line == HEREDOC_END),
                "{path} must end with a line break and not end the heredoc early"
            );
        }
        assert!(!script.contains('\r'));
        assert_eq!(version(), version());
        assert_eq!(version().len(), 12);
        assert!(script.contains(&format!("tterm_dir=$tterm_base/{}\n", version())));
    }

    #[test]
    fn reads_the_directory_after_startup_noise() {
        assert_eq!(
            parse_reply("Welcome!\r\nTTERM-SHELL-INTEGRATION ok /home/me/.cache/t\r\n"),
            Some("/home/me/.cache/t")
        );
        assert_eq!(
            parse_reply("TTERM-SHELL-INTEGRATION ok /home/my files/x\n"),
            Some("/home/my files/x")
        );
        assert_eq!(parse_reply(""), None);
        assert_eq!(parse_reply("TTERM-SHELL-INTEGRATION ok relative\n"), None);
        assert_eq!(parse_reply("sh: not found\n"), None);
    }

    #[test]
    fn quotes_the_start_script_path() {
        assert_eq!(
            launch_command("/home/me/.cache/tterm/x").as_deref(),
            Some("exec sh '/home/me/.cache/tterm/x/unix/start.sh'")
        );
        assert_eq!(
            launch_command("/home/my files/x").as_deref(),
            Some("exec sh '/home/my files/x/unix/start.sh'")
        );
        assert_eq!(launch_command("/home/o'neil/x"), None);
        assert_eq!(launch_command("/home/me\n/x"), None);
    }
}

/// The installer and `unix/start.sh` with real shells. `#[ignore]`d: they
/// need `sh` plus bash, zsh or fish, and the second a real `sshd`; run with
/// `cargo test --lib -- --ignored real_shell_integration`.
#[cfg(all(test, unix))]
mod real_shell_integration_tests {
    use super::*;
    use crate::ssh::test_sshd::{current_user, TempDir, TestSshd};
    use std::io::{Read, Write};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::sync::Arc;
    use std::time::Instant;

    fn which(name: &str) -> Option<PathBuf> {
        let output = Command::new("which").arg(name).output().ok()?;
        output
            .status
            .success()
            .then(|| PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()))
    }

    /// Runs the installer as the SSH probe does, for a user whose home and
    /// login shell are `home` and `shell`.
    fn install_with_sh(home: &Path, shell: &Path) -> Option<String> {
        let mut child = Command::new("sh")
            .arg("-s")
            .env("HOME", home)
            .env("SHELL", shell)
            .env_remove("XDG_CACHE_HOME")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn sh");
        child
            .stdin
            .take()
            .unwrap()
            .write_all(install_script().as_bytes())
            .expect("feed the installer");
        let output = child.wait_with_output().expect("run the installer");
        parse_reply(&String::from_utf8_lossy(&output.stdout)).map(str::to_string)
    }

    /// Waits for `needle` in the shell's output, failing with what arrived.
    fn expect_output(output: &Mutex<Vec<u8>>, needle: &str, shell: &Path) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while Instant::now() < deadline {
            if String::from_utf8_lossy(&output.lock().unwrap()).contains(needle) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!(
            "{}: no {needle:?} in {:?}",
            shell.display(),
            String::from_utf8_lossy(&output.lock().unwrap())
        );
    }

    #[test]
    #[ignore = "runs the real sh and the bash, zsh and fish found on PATH"]
    fn real_shell_integration_marks_commands_in_each_shell() {
        let shells: Vec<PathBuf> = ["bash", "zsh", "fish"]
            .into_iter()
            .filter_map(which)
            .collect();
        assert!(!shells.is_empty(), "none of bash, zsh or fish is on PATH");

        for shell in shells {
            let home = TempDir::new();
            // Keeps zsh from offering its new-user setup.
            std::fs::write(home.path.join(".zshrc"), "").unwrap();
            let dir = install_with_sh(&home.path, &shell).expect("installer reply");
            assert!(
                dir.ends_with(&format!("/.cache/tterm/shell-integration/{}", version())),
                "{dir}"
            );
            // Installing again finds the scripts in place.
            assert_eq!(install_with_sh(&home.path, &shell).as_deref(), Some(&*dir));

            let pair = portable_pty::native_pty_system()
                .openpty(portable_pty::PtySize {
                    rows: 24,
                    cols: 80,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .expect("open pty");
            let mut command = portable_pty::CommandBuilder::new("sh");
            command.arg(format!("{dir}/unix/start.sh"));
            command.env("HOME", &home.path);
            command.env("SHELL", &shell);
            command.env("TERM", "xterm-256color");
            command.env_remove("ZDOTDIR");
            command.cwd(&home.path);
            let mut child = pair.slave.spawn_command(command).expect("spawn start.sh");
            drop(pair.slave);

            let output = Arc::new(Mutex::new(Vec::new()));
            let mut reader = pair.master.try_clone_reader().expect("pty reader");
            let collected = output.clone();
            std::thread::spawn(move || {
                let mut buffer = [0u8; 4096];
                while let Ok(read) = reader.read(&mut buffer) {
                    if read == 0 {
                        break;
                    }
                    collected.lock().unwrap().extend_from_slice(&buffer[..read]);
                }
            });
            let mut writer = pair.master.take_writer().expect("pty writer");

            expect_output(&output, "\x1b]133;A", &shell);
            writer.write_all(b"false\r").unwrap();
            expect_output(&output, "\x1b]133;C", &shell);
            expect_output(&output, "\x1b]133;D;1", &shell);
            writer.write_all(b"exit\r").unwrap();
            let _ = child.kill();
        }
    }

    #[tokio::test]
    #[ignore = "spawns a real sshd; run with --ignored real_shell_integration"]
    async fn real_shell_integration_over_sshd_marks_commands_in_the_login_shell() {
        use crate::ssh::auth::{authenticate, AuthPrompter, AuthTarget};

        let cache = TempDir::new();
        let sshd = TestSshd::spawn(
            12331,
            &["-t", "ed25519"],
            &format!("SetEnv XDG_CACHE_HOME={}\n", cache.path.display()),
        )
        .await;
        let username = current_user();
        let mut session = sshd.connect().await;
        authenticate(
            &mut session,
            &AuthTarget {
                host: "127.0.0.1",
                port: sshd.port,
                username: &username,
                hop: None,
            },
            sshd.key_method(),
            &AuthPrompter::Unavailable,
        )
        .await
        .expect("authenticate");

        let command = prepare(&session, "real-sshd-test")
            .await
            .expect("the login shell must be bash, zsh or fish");
        assert!(
            command.contains(&*cache.path.to_string_lossy()),
            "{command}"
        );

        let mut channel = session.channel_open_session().await.expect("channel");
        channel
            .request_pty(false, "xterm-256color", 80, 24, 0, 0, &[])
            .await
            .expect("pty");
        let mut output = start(&mut channel, &command).await.expect("start");
        channel.data(&b"false\n"[..]).await.expect("type a command");

        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        while !String::from_utf8_lossy(&output).contains("\x1b]133;D;1") {
            let message = tokio::time::timeout_at(deadline, channel.wait())
                .await
                .unwrap_or_else(|_| {
                    panic!("no failed status in {:?}", String::from_utf8_lossy(&output))
                });
            match message {
                Some(ChannelMsg::Data { data }) => output.extend_from_slice(&data),
                Some(ChannelMsg::Close) | None => panic!("the shell exited"),
                _ => {}
            }
        }
        let text = String::from_utf8_lossy(&output);
        assert!(
            text.contains("\x1b]133;A") && text.contains("\x1b]133;C"),
            "{text}"
        );
        let _ = channel.data(&b"exit\n"[..]).await;
    }
}
