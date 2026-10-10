use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Input is written in pieces of this size, so a Ctrl+C stops a paste
/// between them.
const INPUT_CHUNK_BYTES: usize = 1024;
/// Input at least this long is taken for a paste.
const PASTE_BYTES: usize = 1024;
/// How far a paste may run ahead of the shell's output at first.
const PASTE_WINDOW_BYTES: u64 = 4096;
/// A shell silent for this long while a paste is ahead of it is taken to
/// read without echoing (zsh holds a bracketed paste until its end), and the
/// paste may then run twice as far ahead.
const PASTE_STALL: Duration = Duration::from_millis(100);
const INTERRUPT: &[u8] = b"\x03";

pub struct ActivePty {
    /// Writes straight to the PTY and blocks until the shell takes the bytes,
    /// which ZMODEM relies on to pace a file upload.
    pub writer: PtyWriter,
    /// Typed and pasted input, written in order on a thread of its own.
    pub input: PtyInput,
    pub master: Box<dyn portable_pty::MasterPty + Send>,
    pub child: Box<dyn portable_pty::Child + Send>,
}

impl ActivePty {
    pub fn new(
        writer: Box<dyn Write + Send>,
        master: Box<dyn portable_pty::MasterPty + Send>,
        child: Box<dyn portable_pty::Child + Send>,
    ) -> Self {
        let writer = PtyWriter(Arc::new(Mutex::new(writer)));
        Self {
            input: PtyInput::spawn(writer.clone(), child.process_id().unwrap_or(0)),
            writer,
            master,
            child,
        }
    }
}

/// The PTY's writer, shared by its owner and the input thread.
#[derive(Clone)]
pub struct PtyWriter(Arc<Mutex<Box<dyn Write + Send>>>);

impl PtyWriter {
    fn lock(&self) -> std::sync::MutexGuard<'_, Box<dyn Write + Send>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Write for PtyWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.lock().write(buf)
    }

    // Holds the lock for the whole buffer so writes from other threads
    // never land in the middle of it.
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.lock().write_all(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.lock().flush()
    }
}

/// Hands input to a thread that writes it to the PTY. A shell takes a large
/// paste only as fast as it reads it, and once the PTY's input buffer is full
/// a write blocks until then; `write_pty` runs on the main thread, so writing
/// there froze the whole window for as long.
///
/// A paste is written only as fast as the shell answers it with output, so
/// what the shell has yet to take stays queued here. Written at once, a
/// console shell takes all of it off the PTY (PSReadLine into its own
/// memory), shows nothing for seconds and then works through it past any
/// Ctrl+C. A Ctrl+C drops whatever input is still queued ahead of it.
pub struct PtyInput {
    tx: mpsc::Sender<(u64, Vec<u8>)>,
    /// Counts the Ctrl+Cs sent; input queued before the latest is dropped.
    interrupts: Arc<AtomicU64>,
    /// Bytes the shell has written to the PTY.
    output: Arc<AtomicU64>,
}

impl PtyInput {
    fn spawn(mut writer: PtyWriter, pid: u32) -> Self {
        let (tx, rx) = mpsc::channel::<(u64, Vec<u8>)>();
        let interrupts = Arc::new(AtomicU64::new(0));
        let output = Arc::new(AtomicU64::new(0));
        let current = interrupts.clone();
        let shell_output = output.clone();
        // Ends once the session drops its sender, or the PTY stops taking input.
        thread::spawn(move || {
            let mut pasted = false;
            for (interrupt, bytes) in rx {
                let is_paste = bytes.len() >= PASTE_BYTES;
                if bytes == INTERRUPT {
                    if pasted {
                        discard_unread_input(pid);
                    }
                    pasted = false;
                } else if is_paste {
                    pasted = true;
                }
                let interrupted = || interrupt != current.load(Ordering::SeqCst);
                let mut pace = is_paste.then(|| Pace::new(&shell_output));
                for chunk in bytes.chunks(INPUT_CHUNK_BYTES) {
                    if let Some(pace) = &mut pace {
                        pace.wait(&shell_output, &interrupted);
                    }
                    if interrupted() {
                        break;
                    }
                    if writer.write_all(chunk).is_err() {
                        return;
                    }
                    if let Some(pace) = &mut pace {
                        pace.written += chunk.len() as u64;
                    }
                }
            }
        });
        Self {
            tx,
            interrupts,
            output,
        }
    }

    /// Wraps the PTY's reader so pastes are paced by the shell's output.
    pub fn track_output(&self, reader: Box<dyn Read + Send>) -> Box<dyn Read + Send> {
        Box::new(OutputCounter {
            reader,
            output: self.output.clone(),
        })
    }

    pub fn send(&self, bytes: Vec<u8>) -> io::Result<()> {
        let interrupt = if bytes == INTERRUPT {
            self.interrupts.fetch_add(1, Ordering::SeqCst) + 1
        } else {
            self.interrupts.load(Ordering::SeqCst)
        };
        self.tx
            .send((interrupt, bytes))
            .map_err(|_| io::Error::other("PTY input is closed"))
    }
}

/// How far one paste has got ahead of the shell.
struct Pace {
    output_before: u64,
    written: u64,
    window: u64,
}

impl Pace {
    fn new(output: &AtomicU64) -> Self {
        Self {
            output_before: output.load(Ordering::SeqCst),
            written: 0,
            window: PASTE_WINDOW_BYTES,
        }
    }

    /// Waits until the shell's output since the paste began is within the
    /// window of what was written, or the shell stays silent.
    fn wait(&mut self, output: &AtomicU64, interrupted: &dyn Fn() -> bool) {
        loop {
            let answered = output.load(Ordering::SeqCst) - self.output_before;
            if self.written < answered + self.window {
                return;
            }
            let silent_until = Instant::now() + PASTE_STALL;
            while output.load(Ordering::SeqCst) - self.output_before == answered {
                if interrupted() {
                    return;
                }
                if Instant::now() >= silent_until {
                    self.window *= 2;
                    return;
                }
                thread::sleep(Duration::from_millis(1));
            }
        }
    }
}

/// Counts what the shell writes, for [`Pace`].
struct OutputCounter {
    reader: Box<dyn Read + Send>,
    output: Arc<AtomicU64>,
}

impl Read for OutputCounter {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.reader.read(buf)?;
        self.output.fetch_add(read as u64, Ordering::SeqCst);
        Ok(read)
    }
}

/// Discards the paste the console behind a Windows shell took off the PTY
/// but the shell has not read yet. Elsewhere the kernel holds only a few
/// kilobytes of it and the rest is still queued here.
fn discard_unread_input(pid: u32) {
    #[cfg(target_os = "windows")]
    if pid != 0 {
        if let Err(err) = super::console_reset::flush_console_input(pid) {
            eprintln!("Failed to discard pasted input: {err}");
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = pid;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Records what reaches the PTY, holding every write until `gate` opens.
    struct GatedWriter {
        written: Arc<Mutex<Vec<u8>>>,
        entered: mpsc::Sender<()>,
        gate: mpsc::Receiver<()>,
    }

    impl Write for GatedWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            let _ = self.entered.send(());
            let _ = self.gate.recv();
            self.written.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn ctrl_c_drops_the_rest_of_a_paste() {
        let written = Arc::new(Mutex::new(Vec::new()));
        let (entered_tx, entered) = mpsc::channel();
        let (open, gate) = mpsc::channel();
        let writer = PtyWriter(Arc::new(Mutex::new(Box::new(GatedWriter {
            written: written.clone(),
            entered: entered_tx,
            gate,
        }))));
        let input = PtyInput::spawn(writer, 0);

        input.send(vec![b'a'; INPUT_CHUNK_BYTES * 3]).unwrap();
        // The first piece of the paste is being written when Ctrl+C comes.
        entered.recv_timeout(Duration::from_secs(5)).unwrap();
        input.send(INTERRUPT.to_vec()).unwrap();
        input.send(b"ls\r".to_vec()).unwrap();
        for _ in 0..3 {
            open.send(()).unwrap();
        }

        let mut expected = vec![b'a'; INPUT_CHUNK_BYTES];
        expected.extend_from_slice(b"\x03ls\r");
        for _ in 0..500 {
            if written.lock().unwrap().len() >= expected.len() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(*written.lock().unwrap(), expected);
    }

    /// Records what reaches the PTY and answers every write with output.
    struct EchoWriter {
        written: Arc<Mutex<Vec<u8>>>,
        output: Arc<std::sync::OnceLock<Arc<AtomicU64>>>,
    }

    impl Write for EchoWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.written.lock().unwrap().extend_from_slice(buf);
            if let Some(output) = self.output.get() {
                output.fetch_add(buf.len() as u64, Ordering::SeqCst);
            }
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn paste_duration(echo: bool) -> Duration {
        let written = Arc::new(Mutex::new(Vec::new()));
        let output = Arc::new(std::sync::OnceLock::new());
        let writer = PtyWriter(Arc::new(Mutex::new(Box::new(EchoWriter {
            written: written.clone(),
            output: output.clone(),
        }))));
        let input = PtyInput::spawn(writer, 0);
        if echo {
            output.set(input.output.clone()).unwrap();
        }

        let paste = vec![b'a'; PASTE_WINDOW_BYTES as usize * 2];
        let started = Instant::now();
        input.send(paste.clone()).unwrap();
        while written.lock().unwrap().len() < paste.len() {
            assert!(started.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(1));
        }
        started.elapsed()
    }

    #[test]
    fn a_paste_runs_ahead_of_the_shell_only_by_a_window() {
        // A shell echoing what it reads takes the paste without a pause.
        assert!(paste_duration(true) < PASTE_STALL);
        // A silent one gets the window doubled once it stays quiet.
        assert!(paste_duration(false) >= PASTE_STALL);
    }
}
