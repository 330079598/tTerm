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
    match parse_reply(&String::from_utf8_lossy(&output.stdout)) {
        Some(Reply::Installed(dir)) => Some(dir.to_string()),
        _ => None,
    }
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

/// A shell started through `unix/start.sh` on a pty, and what it printed.
struct StartedShell {
    shell: PathBuf,
    output: Arc<Mutex<Vec<u8>>>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

impl StartedShell {
    /// Starts `shell` from the scripts in `dir` for a user whose home is
    /// `home`, marking commands when `marks` is set.
    fn spawn(dir: &str, home: &Path, shell: &Path, marks: bool) -> Self {
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
        command.env("HOME", home);
        command.env("SHELL", shell);
        command.env("TERM", "xterm-256color");
        command.env("tterm_marks", if marks { "1" } else { "0" });
        command.env_remove("ZDOTDIR");
        command.env_remove("HISTFILE");
        command.cwd(home);
        let child = pair.slave.spawn_command(command).expect("spawn start.sh");
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
        let writer = pair.master.take_writer().expect("pty writer");
        Self {
            shell: shell.to_path_buf(),
            output,
            writer,
            child,
        }
    }

    fn expect(&self, needle: &str) {
        expect_output(&self.output, needle, &self.shell);
    }

    fn run(&mut self, line: &str) {
        self.writer
            .write_all(format!("{line}\r").as_bytes())
            .unwrap();
    }

    fn printed(&self, needle: &str) -> bool {
        String::from_utf8_lossy(&self.output.lock().unwrap()).contains(needle)
    }

    fn exit(mut self) {
        self.run("exit");
        let _ = self.child.kill();
    }
}

/// bash, zsh and fish on PATH.
fn shells() -> Vec<PathBuf> {
    let shells: Vec<PathBuf> = ["bash", "zsh", "fish"]
        .into_iter()
        .filter_map(which)
        .collect();
    assert!(!shells.is_empty(), "none of bash, zsh or fish is on PATH");
    shells
}

/// macOS's own bash: version 3.2, and patched to read the login files in
/// POSIX mode.
fn is_macos_bash(shell: &Path) -> bool {
    cfg!(target_os = "macos") && shell == Path::new("/bin/bash")
}

/// A home for `shell` with the scripts installed, and their directory.
fn home_with_scripts(shell: &Path) -> (TempDir, String) {
    let home = TempDir::new();
    // Keeps zsh from offering its new-user setup.
    std::fs::write(home.path.join(".zshrc"), "").unwrap();
    // A profile that replaces PROMPT_COMMAND must not drop tTerm's hook.
    // macOS's own bash still gets the hook before the profile, so this is
    // left out there.
    if !is_macos_bash(shell) {
        std::fs::write(home.path.join(".bash_profile"), "PROMPT_COMMAND=true\n").unwrap();
    }
    let dir = install_with_sh(&home.path, shell).expect("installer reply");
    (home, dir)
}

#[test]
#[ignore = "runs the real sh and the bash, zsh and fish found on PATH"]
fn real_shell_integration_marks_commands_in_each_shell() {
    for shell in shells() {
        let (home, dir) = home_with_scripts(&shell);
        assert!(
            dir.ends_with(&format!("/.cache/tterm/shell-integration/{}", version())),
            "{dir}"
        );
        // Installing again finds the scripts in place.
        assert_eq!(install_with_sh(&home.path, &shell).as_deref(), Some(&*dir));

        let mut started = StartedShell::spawn(&dir, &home.path, &shell, true);
        started.expect("\x1b]133;A");
        started.run("false");
        // bash has no hook that sees the command line before it runs.
        if !shell.ends_with("bash") {
            started.expect("\x1b]633;E;false\x1b\\");
        }
        // bash before 4.4 has no PS0 to mark where output starts.
        if !is_macos_bash(&shell) {
            started.expect("\x1b]133;C");
        }
        started.expect("\x1b]133;D;1");
        started.run("cd /");
        started.expect("\x1b]7;file:///\x1b\\");
        started.exit();
    }
}

#[test]
#[ignore = "runs the real sh and the bash, zsh and fish found on PATH"]
fn real_shell_integration_reports_the_directory_with_marks_off() {
    for shell in shells() {
        let (home, dir) = home_with_scripts(&shell);
        let mut started = StartedShell::spawn(&dir, &home.path, &shell, false);
        started.run("cd /");
        started.expect("\x1b]7;file:///\x1b\\");
        assert!(
            !started.printed("\x1b]133;"),
            "{}: marked commands with marks off",
            shell.display()
        );
        started.exit();
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

    let command = prepare(&session, "real-sshd-test", true)
        .await
        .expect("the login shell must be bash, zsh or fish");
    assert!(command.ends_with(" --motd"), "{command}");
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

    // The first prompt's D carries the startup files' status, which may be 1
    // as well, so the status that counts is the one after the output starts.
    let failed = |output: &[u8]| {
        let text = String::from_utf8_lossy(output);
        text.split_once("\x1b]133;C")
            .is_some_and(|(_, after)| after.contains("\x1b]133;D;1"))
    };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while !failed(&output) {
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
        text.contains("\x1b]133;A") && text.contains("\x1b]7;file:///"),
        "{text}"
    );
    let _ = channel.data(&b"exit\n"[..]).await;
}
