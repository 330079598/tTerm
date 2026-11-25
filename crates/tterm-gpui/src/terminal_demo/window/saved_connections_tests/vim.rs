//! Opt-in SSH acceptance with the installed Vim, a disposable PTY and no desktop.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use gpui_kit::TestAppContext;
use portable_pty::{Child, CommandBuilder, PtySize, native_pty_system};
use russh::{Channel, ChannelMsg, server::Msg};
use tterm_terminal::mode::Modes;

use super::server::{ShellScript, ShellServer};
use super::{direct_launch, harness, output, pump_until, trust_server};

type PtyEnds = (Box<dyn Read + Send>, Box<dyn Write + Send>);

/// The test owns the child; exactly one authenticated shell can take its PTY.
#[derive(Clone)]
pub(super) struct Bridge {
    ends: Arc<Mutex<Option<PtyEnds>>>,
    pub(super) password: String,
}

impl Bridge {
    pub(super) fn attach(&self, channel: Channel<Msg>) {
        let (mut pty_rx, mut pty_tx) = self.ends.lock().unwrap().take().unwrap();
        let (mut ssh_rx, ssh_tx) = channel.split();
        let runtime = tokio::runtime::Handle::current();
        std::thread::spawn(move || {
            let mut buffer = [0u8; 4096];
            while let Ok(count) = pty_rx.read(&mut buffer) {
                if count == 0 || runtime.block_on(ssh_tx.data(&buffer[..count])).is_err() {
                    break;
                }
            }
        });
        tokio::spawn(async move {
            while let Some(message) = ssh_rx.wait().await {
                match message {
                    ChannelMsg::Data { data } => {
                        if pty_tx.write_all(&data).is_err() {
                            break;
                        }
                    }
                    ChannelMsg::Eof | ChannelMsg::Close => break,
                    _ => {}
                }
            }
        });
    }
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct VimChild(Box<dyn Child + Send + Sync>);

impl Drop for VimChild {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn vim_session() -> (Scratch, VimChild, Bridge) {
    let path = std::env::temp_dir().join(format!("tterm-ssh-vim-{:032x}", rand::random::<u128>()));
    std::fs::create_dir(&path).unwrap();
    let scratch = Scratch(path);
    std::fs::write(scratch.0.join("buffer.txt"), "before\n").unwrap();
    let pair = native_pty_system().openpty(PtySize::default()).unwrap();
    let mut command = CommandBuilder::new("/usr/bin/vim");
    command.cwd(&scratch.0);
    command.env_clear();
    command.env("HOME", &scratch.0);
    command.env("TERM", "xterm-256color");
    command.env("LC_ALL", "C");
    command.args([
        "-Z",
        "-N",
        "-u",
        "NONE",
        "-i",
        "NONE",
        "-n",
        "--noplugin",
        "--",
        "buffer.txt",
    ]);
    let child = VimChild(
        pair.slave
            .spawn_command(command)
            .expect("/usr/bin/vim with a PTY"),
    );
    let bridge = Bridge {
        ends: Arc::new(Mutex::new(Some((
            pair.master.try_clone_reader().unwrap(),
            pair.master.take_writer().unwrap(),
        )))),
        password: format!("{:032x}", rand::random::<u128>()),
    };
    (scratch, child, bridge)
}

#[gpui_kit::test]
#[ignore = "needs /usr/bin/vim, a Unix PTY and a loopback listener"]
fn vim_edits_saves_and_restores_the_primary_screen_over_ssh(cx: &mut TestAppContext) {
    let (scratch, mut vim, bridge) = vim_session();
    let harness = harness(cx);
    let mut script = ShellScript::new(Vec::new());
    script.vim = Some(bridge.clone());
    let server = ShellServer::start_scripted(harness._runtime._runtime.handle(), script);
    trust_server("vim-loopback", "Vim loopback", &server);
    let mut launch = direct_launch("vim-loopback", "Vim loopback", &server);
    launch.connection.password = Some(bridge.password);
    let model = harness.visual.update(|window, cx| {
        harness.root.update(cx, |workspace, cx| {
            let id = workspace
                .open_saved_profile_launch(launch, window, cx)
                .unwrap();
            workspace
                .tabs
                .get(id)
                .unwrap()
                .binding
                .as_ref()
                .unwrap()
                .session()
                .unwrap()
                .model()
        })
    });
    pump_until(harness.visual, "Vim to draw the real buffer", |_| {
        model.snapshot().modes().contains(Modes::ALT_SCREEN) && output(&model).contains("before")
    });
    harness.visual.simulate_keystrokes("i s s h space escape");
    pump_until(harness.visual, "Vim to render the edit", |_| {
        output(&model).contains("ssh before")
    });
    harness.visual.simulate_keystrokes("shift-z shift-z");
    pump_until(harness.visual, "Vim to exit the alternate screen", |_| {
        !model.snapshot().modes().contains(Modes::ALT_SCREEN)
            && vim.0.try_wait().unwrap().is_some_and(|exit| exit.success())
    });
    assert_eq!(
        std::fs::read(scratch.0.join("buffer.txt")).unwrap(),
        b"ssh before\n"
    );
    assert_eq!((server.connections(), server.shells()), (1, 1));
    harness.visual.update(|window, _| window.remove_window());
}
