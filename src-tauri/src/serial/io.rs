use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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
pub(super) struct SerialReader<R> {
    port: R,
    closed: Arc<AtomicBool>,
}

impl<R> SerialReader<R> {
    pub(super) fn new(port: R, closed: Arc<AtomicBool>) -> Self {
        Self { port, closed }
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
                Err(err) if timed_out(&err) => continue,
                result => return result,
            }
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
