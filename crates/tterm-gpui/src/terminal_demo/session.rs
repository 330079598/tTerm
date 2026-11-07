use portable_pty::MasterPty;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

pub enum WriteMessage {
    Bytes(Vec<u8>),
    Close,
}

pub struct Session {
    pub(crate) child: Option<Arc<Mutex<Box<dyn portable_pty::Child + Send + Sync>>>>,
    pub(crate) master: Option<Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>>,
    pub(crate) writer: Option<mpsc::Sender<WriteMessage>>,
    pub(crate) reader: Option<std::thread::JoinHandle<()>>,
    pub(crate) parser: Option<std::thread::JoinHandle<()>>,
    pub(crate) writer_thread: Option<std::thread::JoinHandle<()>>,
    pub(crate) child_monitor: Option<std::thread::JoinHandle<()>>,
    pub(crate) alive: Arc<AtomicBool>,
}

impl Session {
    pub fn shutdown(mut self) {
        self.alive.store(false, Ordering::Release);
        if let Some(child) = self.child.as_ref() {
            if let Ok(mut child) = child.lock() {
                let _ = child.kill();
            }
        }
        if let Some(master) = self.master.take() {
            if let Ok(mut master) = master.lock() {
                master.take();
            }
        }
        if let Some(writer) = self.writer.take() {
            let _ = writer.send(WriteMessage::Close);
        }
        let child = self.child.take();
        std::thread::spawn(move || {
            if let Some(child) = child {
                if let Ok(mut child) = child.lock() {
                    let _ = child.wait();
                }
            }
            self.join_threads();
        });
    }

    fn join_threads(mut self) {
        if let Some(thread) = self.reader.take() {
            let _ = thread.join();
        }
        if let Some(thread) = self.parser.take() {
            let _ = thread.join();
        }
        if let Some(thread) = self.writer_thread.take() {
            let _ = thread.join();
        }
        if let Some(thread) = self.child_monitor.take() {
            let _ = thread.join();
        }
    }
}
