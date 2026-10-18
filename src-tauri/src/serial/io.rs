use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, PoisonError};

/// Where a port's input goes instead of the terminal while a file transfer
/// has the port: the transfer's end of a channel.
pub(crate) type SerialTap = Arc<Mutex<Option<mpsc::Sender<Vec<u8>>>>>;

/// Whether an I/O error only means the port's timeout ran out.
fn timed_out(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
    )
}

/// Reads a serial port for a tab's reader thread. A read that times out is no
/// end of the stream, only a chance to see whether the tab is closing, which
/// ends the stream; a port that reads nothing without timing out was unplugged.
/// What a transfer takes through the tap never reaches the terminal.
pub(super) struct SerialReader<R> {
    port: R,
    closed: Arc<AtomicBool>,
    tap: SerialTap,
}

impl<R> SerialReader<R> {
    pub(super) fn new(port: R, closed: Arc<AtomicBool>, tap: SerialTap) -> Self {
        Self { port, closed, tap }
    }

    /// Hands `bytes` to the transfer that has the port; `false` when none has.
    fn divert(&self, bytes: &[u8]) -> bool {
        let tap = self.tap.lock().unwrap_or_else(PoisonError::into_inner);
        tap.as_ref()
            .is_some_and(|sender| sender.send(bytes.to_vec()).is_ok())
    }
}

impl<R: Read> Read for SerialReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.closed.load(Ordering::Relaxed) {
                return Ok(0);
            }
            match self.port.read(buf) {
                Ok(0) => return Err(disconnected()),
                Ok(read) if self.divert(&buf[..read]) => continue,
                Err(err) if timed_out(&err) => continue,
                result => return result,
            }
        }
    }
}

/// How often a Windows port is checked for input while it has none.
#[cfg(windows)]
const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(2);

/// Reads a Windows port only once input is waiting. Windows runs one call at
/// a time on a port opened the way `serialport` opens it, so a read left
/// waiting for input held each keystroke's write back until it timed out,
/// and the reader, starting its next read at once, often took the port
/// first again: typing lagged by hundreds of milliseconds. Asking how much
/// input is waiting returns at once, so a write gets the port between checks.
#[cfg(windows)]
pub(super) struct PolledPort(pub(super) Box<dyn serialport::SerialPort>);

#[cfg(windows)]
impl Read for PolledPort {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let deadline = std::time::Instant::now() + super::open::READ_TIMEOUT;
        loop {
            if self.0.bytes_to_read()? > 0 {
                return self.0.read(buf);
            }
            if std::time::Instant::now() >= deadline {
                return Err(io::ErrorKind::TimedOut.into());
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }
}

/// Writes to a serial port, waiting out the timeouts a slow line or a device
/// holding off flow control cause instead of giving the write up.
pub(super) struct SerialWriter<W> {
    port: W,
    closed: Arc<AtomicBool>,
}

impl<W> SerialWriter<W> {
    pub(super) fn new(port: W, closed: Arc<AtomicBool>) -> Self {
        Self { port, closed }
    }

    #[cfg(test)]
    pub(super) fn into_inner(self) -> W {
        self.port
    }

    fn check_open(&self) -> io::Result<()> {
        if self.closed.load(Ordering::Relaxed) {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "The serial port is closed",
            ))
        } else {
            Ok(())
        }
    }
}

impl<W: Write> Write for SerialWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            self.check_open()?;
            match self.port.write(buf) {
                Ok(0) => continue,
                Err(err) if timed_out(&err) => continue,
                result => return result,
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        loop {
            self.check_open()?;
            match self.port.flush() {
                Err(err) if timed_out(&err) => continue,
                result => return result,
            }
        }
    }
}

fn disconnected() -> io::Error {
    io::Error::new(
        io::ErrorKind::NotConnected,
        "The serial device was disconnected",
    )
}
