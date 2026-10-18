use super::io::{SerialReader, SerialWriter};
use super::{SerialLine, SerialLineEnding, SerialOptions};
use crate::core::supervisor::TabConnection;
use crate::terminal::types::{PtyInput, PtyWriter};
use serialport::SerialPort;
use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

/// A serial tab's open port.
pub struct ActiveSerial {
    /// Writes straight to the port, for ZMODEM.
    pub writer: PtyWriter,
    /// Typed and pasted input, written in order on a thread of its own.
    pub input: PtyInput,
    /// The port that was opened, which may differ from the saved one when the
    /// device came back under another name.
    pub path: String,
    line_ending: SerialLineEnding,
    control: SerialControl,
    /// Ends the reader thread and fails writes once the tab lets go of the port.
    closed: Arc<AtomicBool>,
}

impl ActiveSerial {
    /// Typed text as the device expects it: Enter sends the line ending.
    pub fn encode_input<'a>(&self, input: &'a [u8]) -> Cow<'a, [u8]> {
        encode_line_ending(self.line_ending, input)
    }

    pub(super) fn control(&self) -> SerialControl {
        self.control.clone()
    }
}

impl Drop for ActiveSerial {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::Relaxed);
    }
}

/// Replaces each carriage return xterm.js types for Enter with `ending`.
pub(super) fn encode_line_ending(ending: SerialLineEnding, input: &[u8]) -> Cow<'_, [u8]> {
    let replacement: &[u8] = match ending {
        SerialLineEnding::Cr => return Cow::Borrowed(input),
        SerialLineEnding::Lf => b"\n",
        SerialLineEnding::Crlf => b"\r\n",
    };
    if !input.contains(&b'\r') {
        return Cow::Borrowed(input);
    }
    let mut encoded = Vec::with_capacity(input.len() + input.len() / 8);
    for &byte in input {
        if byte == b'\r' {
            encoded.extend_from_slice(replacement);
        } else {
            encoded.push(byte);
        }
    }
    Cow::Owned(encoded)
}

/// The modem control lines of an open port, which the port's driver keeps at
/// the levels last set; tTerm leaves them alone until the user sets one.
#[derive(Clone)]
pub(super) struct SerialControl(Arc<Mutex<Box<dyn SerialPort>>>);

impl SerialControl {
    fn port(&self) -> std::sync::MutexGuard<'_, Box<dyn SerialPort>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(super) fn set_signals(&self, dtr: Option<bool>, rts: Option<bool>) -> Result<(), String> {
        let mut port = self.port();
        if let Some(level) = dtr {
            set_line(port.as_mut(), SerialLine::Dtr, level)?;
        }
        if let Some(level) = rts {
            set_line(port.as_mut(), SerialLine::Rts, level)?;
        }
        Ok(())
    }

    /// Asserts `line` for `duration`, then releases it.
    pub(super) fn pulse(&self, line: SerialLine, duration: Duration) -> Result<(), String> {
        let mut port = self.port();
        set_line(port.as_mut(), line, true)?;
        std::thread::sleep(duration);
        set_line(port.as_mut(), line, false)
    }

    pub(super) fn send_break(&self, duration: Duration) -> Result<(), String> {
        let port = self.port();
        port.set_break()
            .map_err(|err| format!("Failed to send a break: {}", err.description))?;
        std::thread::sleep(duration);
        port.clear_break()
            .map_err(|err| format!("Failed to end the break: {}", err.description))
    }
}

fn set_line(port: &mut dyn SerialPort, line: SerialLine, level: bool) -> Result<(), String> {
    let (name, result) = match line {
        SerialLine::Dtr => ("DTR", port.write_data_terminal_ready(level)),
        SerialLine::Rts => ("RTS", port.write_request_to_send(level)),
    };
    result.map_err(|err| format!("Failed to set {name}: {}", err.description))
}

/// The settings of a serial tab's plan.
pub(crate) fn serial_options(tab: &TabConnection) -> Result<&SerialOptions, String> {
    tab.plan
        .serial
        .as_ref()
        .ok_or_else(|| "The serial connection has no settings".to_string())
}

/// Opens the port at `path` for a tab and starts reading it into the tab.
pub(crate) fn spawn_serial_attempt(
    tab: &TabConnection,
    path: &str,
) -> Result<ActiveSerial, String> {
    let options = serial_options(tab)?;
    let port = super::open::open_port(options, path)?;
    let clone = |port: &dyn SerialPort| {
        port.try_clone()
            .map_err(|err| format!("Failed to share {path}: {}", err.description))
    };
    let reader_port = clone(port.as_ref())?;
    let writer_port = clone(port.as_ref())?;

    let closed = Arc::new(AtomicBool::new(false));
    let writer = PtyWriter::new(Box::new(SerialWriter::new(writer_port, closed.clone())));
    let input = PtyInput::over_writer(writer.clone());
    let reader = input.track_output(Box::new(SerialReader::new(reader_port, closed.clone())));
    let sender = crate::terminal::io_batcher::TerminalOutputSender::spawn(
        &tab.channel.tab_id,
        tab.output_channel.clone(),
    );
    crate::terminal::reader::spawn_reader_thread(
        reader,
        tab.channel.clone(),
        tab.exit_tx.clone(),
        Some(sender),
        crate::zmodem::session::ZmodemSettings::load(),
    );

    Ok(ActiveSerial {
        writer,
        input,
        path: path.to_string(),
        line_ending: options.line_ending,
        control: SerialControl(Arc::new(Mutex::new(port))),
        closed,
    })
}

/// Opens a tab's port for its first attempt: the saved device wherever it
/// is now, else the saved port.
pub(crate) fn spawn_first_serial_attempt(tab: &TabConnection) -> Result<ActiveSerial, String> {
    let options = serial_options(tab)?;
    let ports = super::ports::list_ports().unwrap_or_default();
    let path = super::ports::port_to_open(options, &ports).ok_or_else(|| {
        format!(
            "The device saved with this connection is not connected, and {} is now another device.",
            options.port
        )
    })?;
    spawn_serial_attempt(tab, &path)
}
