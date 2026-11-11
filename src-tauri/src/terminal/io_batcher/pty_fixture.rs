//! A real PTY fixture shared by correctness tests and the ignored benchmark.
use portable_pty::{native_pty_system, Child, CommandBuilder, PtySize};
use std::io::Read;
use std::time::Duration;

pub(super) struct PtyFixture {
    pub reader: Box<dyn Read + Send>,
    pub child: Box<dyn Child + Send + Sync>,
    _master: Box<dyn portable_pty::MasterPty + Send>,
    watchdog_cancel: Option<std::sync::mpsc::Sender<()>>,
    watchdog: Option<std::thread::JoinHandle<()>>,
}

impl PtyFixture {
    pub fn spawn(command: &str) -> Self {
        let pair = native_pty_system().openpty(PtySize::default()).unwrap();
        let mut cmd = CommandBuilder::new("/bin/bash");
        cmd.args(["--noprofile", "--norc", "-c", command]);
        let child = pair.slave.spawn_command(cmd).unwrap();
        drop(pair.slave);
        let reader = pair.master.try_clone_reader().unwrap();
        let mut killer = child.clone_killer();
        let (cancel, rx) = std::sync::mpsc::channel();
        let watchdog = std::thread::spawn(move || {
            if rx.recv_timeout(Duration::from_secs(60)).is_err() {
                let _ = killer.kill();
            }
        });
        Self {
            reader,
            child,
            _master: pair.master,
            watchdog_cancel: Some(cancel),
            watchdog: Some(watchdog),
        }
    }

    pub fn reader(&mut self) -> Box<dyn Read + Send> {
        std::mem::replace(&mut self.reader, Box::new(std::io::empty()))
    }
}

impl Drop for PtyFixture {
    fn drop(&mut self) {
        if let Some(cancel) = self.watchdog_cancel.take() {
            let _ = cancel.send(());
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(watchdog) = self.watchdog.take() {
            let _ = watchdog.join();
        }
    }
}
