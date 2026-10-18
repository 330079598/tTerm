//! Serial connections: the settings a serial tab opens its port with, the
//! ports this machine has, and the commands behind a serial tab's controls.

pub(crate) mod history;
mod io;
mod open;
mod output;
pub(crate) mod ports;
pub(crate) mod reconnect;
pub(crate) mod session;
#[cfg(test)]
mod tests;
pub(crate) mod xmodem;

use crate::core::blocking::run_blocking;
use crate::core::state::{ActiveSession, PtyMap, ZmodemMap};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, State};

/// How long a serial pulse holds its line before releasing it, unless asked.
const DEFAULT_PULSE_MS: u64 = 100;
/// The longest pulse or break a tab's controls may ask for.
const MAX_PULSE_MS: u64 = 5_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SerialParity {
    #[default]
    None,
    Odd,
    Even,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SerialFlowControl {
    #[default]
    None,
    /// XON/XOFF.
    Software,
    /// RTS/CTS.
    Hardware,
}

/// What the Enter key sends: xterm.js types a carriage return.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SerialLineEnding {
    #[default]
    Cr,
    Lf,
    Crlf,
}

/// How a serial tab completes a line break the device sends only half of,
/// for devices that end their lines, or echo Enter, with a lone CR or LF.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SerialReceiveNewline {
    /// Shows line breaks as the device sends them.
    #[default]
    None,
    /// A CR also starts a new line.
    Cr,
    /// An LF also returns to the start of the line.
    Lf,
}

/// The USB device behind a port, kept with a saved connection so it finds
/// its device again when the port's name changes: after a replug, or on
/// another machine the connection was synced to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerialUsbId {
    pub vid: u16,
    pub pid: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial_number: Option<String>,
}

/// A serial connection's settings, as a tab and a saved profile carry them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerialOptions {
    /// `COM3`, `/dev/ttyUSB0`, `/dev/cu.usbserial-1410`.
    #[serde(default)]
    pub port: String,
    #[serde(default = "default_baud_rate")]
    pub baud_rate: u32,
    /// 5 to 8.
    #[serde(default = "default_data_bits")]
    pub data_bits: u8,
    #[serde(default)]
    pub parity: SerialParity,
    /// 1 or 2.
    #[serde(default = "default_stop_bits")]
    pub stop_bits: u8,
    #[serde(default)]
    pub flow_control: SerialFlowControl,
    #[serde(default)]
    pub line_ending: SerialLineEnding,
    #[serde(default)]
    pub receive_newline: SerialReceiveNewline,
    /// The frontend echoes typed text into the terminal, for devices that
    /// don't echo it back.
    #[serde(default)]
    pub local_echo: bool,
    /// The frontend starts each line of output with the time it arrived.
    #[serde(default)]
    pub timestamps: bool,
    /// Received bytes show in hex instead of as text.
    #[serde(default)]
    pub hex_view: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usb: Option<SerialUsbId>,
}

fn default_baud_rate() -> u32 {
    115_200
}

fn default_data_bits() -> u8 {
    8
}

fn default_stop_bits() -> u8 {
    1
}

impl SerialOptions {
    /// Checks the settings a port is opened with, trimming the port's name.
    pub fn validated(mut self) -> Result<Self, String> {
        self.port = self.port.trim().to_string();
        if self.port.is_empty() {
            return Err("Choose a serial port to connect to".to_string());
        }
        if self.baud_rate == 0 {
            return Err("The baud rate must be greater than 0".to_string());
        }
        if !(5..=8).contains(&self.data_bits) {
            return Err(format!(
                "Data bits must be 5, 6, 7 or 8, not {}",
                self.data_bits
            ));
        }
        if !matches!(self.stop_bits, 1 | 2) {
            return Err(format!("Stop bits must be 1 or 2, not {}", self.stop_bits));
        }
        Ok(self)
    }
}

/// How a serial tab shows what it receives, which its controls change while
/// it runs. Shared by every attempt of the tab, so it outlasts a reconnect.
#[derive(Debug, Default)]
pub struct SerialView {
    hex: AtomicBool,
}

impl SerialView {
    pub fn hex(&self) -> bool {
        self.hex.load(Ordering::Relaxed)
    }

    pub fn set_hex(&self, hex: bool) {
        self.hex.store(hex, Ordering::Relaxed);
    }
}

/// A serial session's checked settings and its view.
#[derive(Debug, Clone)]
pub struct SerialPlan {
    pub options: SerialOptions,
    pub view: Arc<SerialView>,
}

impl SerialPlan {
    pub fn new(options: SerialOptions) -> Self {
        let view = SerialView::default();
        view.set_hex(options.hex_view);
        Self {
            options,
            view: Arc::new(view),
        }
    }
}

/// One port this machine has, as the connection dialog lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SerialPortEntry {
    pub path: String,
    /// `usb`, `pci`, `bluetooth` or `unknown`.
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usb: Option<SerialUsbId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manufacturer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product: Option<String>,
}

/// A modem control line a serial tab can drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SerialLine {
    Dtr,
    Rts,
}

#[tauri::command]
pub async fn list_serial_ports() -> Result<Vec<SerialPortEntry>, String> {
    run_blocking(ports::list_ports).await
}

/// Sets a serial tab's DTR and RTS lines; a line left out keeps its level.
#[tauri::command]
pub async fn set_serial_signals(
    tab_id: String,
    session_nonce: u32,
    dtr: Option<bool>,
    rts: Option<bool>,
    state: State<'_, PtyMap>,
) -> Result<(), String> {
    let control = serial_control(&state, &tab_id, session_nonce).await?;
    run_blocking(move || control.set_signals(dtr, rts)).await
}

/// Inverts one line for a moment and restores it, the way boards such as the
/// ESP32 or Arduino are reset from their USB serial adapter.
#[tauri::command]
pub async fn pulse_serial_line(
    tab_id: String,
    session_nonce: u32,
    line: SerialLine,
    duration_ms: Option<u64>,
    state: State<'_, PtyMap>,
) -> Result<(), String> {
    let control = serial_control(&state, &tab_id, session_nonce).await?;
    let duration = pulse_duration(duration_ms);
    run_blocking(move || control.pulse(line, duration)).await
}

/// Sends bytes to a serial tab's port as they are, without the line ending or
/// charset typed text gets: what its send bar's hex mode sends.
#[tauri::command]
pub async fn write_serial_bytes(
    app: AppHandle,
    tab_id: String,
    session_nonce: u32,
    data: Vec<u8>,
    state: State<'_, PtyMap>,
    zmodem_state: State<'_, ZmodemMap>,
) -> Result<(), String> {
    let transferring = zmodem_state
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains_key(&tab_id);
    if transferring {
        return Err("Wait for the file transfer on this port to end".to_string());
    }
    with_serial(&state, &tab_id, session_nonce, |serial| {
        serial.input.send(data.clone())
    })
    .await?
    .map_err(|err| format!("Failed to write to the serial port: {err}"))?;
    crate::session_log::record_input(&app, &tab_id, &data);
    Ok(())
}

/// Shows what a serial tab receives in hex, or as text again, from now on
/// or from when its device comes back.
#[tauri::command]
pub async fn set_serial_view(
    tab_id: String,
    session_nonce: u32,
    hex: bool,
    state: State<'_, PtyMap>,
) -> Result<(), String> {
    let map = state.read().await;
    map.get(&tab_id)
        .filter(|session| session.session_nonce == session_nonce)
        .and_then(|session| session.serial_view.as_ref())
        .ok_or_else(|| "This tab is not an open serial connection".to_string())?
        .set_hex(hex);
    Ok(())
}

#[tauri::command]
pub async fn send_serial_break(
    tab_id: String,
    session_nonce: u32,
    duration_ms: Option<u64>,
    state: State<'_, PtyMap>,
) -> Result<(), String> {
    let control = serial_control(&state, &tab_id, session_nonce).await?;
    let duration = pulse_duration(duration_ms);
    run_blocking(move || control.send_break(duration)).await
}

fn pulse_duration(duration_ms: Option<u64>) -> std::time::Duration {
    std::time::Duration::from_millis(
        duration_ms
            .unwrap_or(DEFAULT_PULSE_MS)
            .clamp(1, MAX_PULSE_MS),
    )
}

/// The control lines of a tab's open port.
async fn serial_control(
    state: &PtyMap,
    tab_id: &str,
    session_nonce: u32,
) -> Result<session::SerialControl, String> {
    with_serial(state, tab_id, session_nonce, session::ActiveSerial::control).await
}

/// Runs `f` on a tab's open port.
async fn with_serial<T>(
    state: &PtyMap,
    tab_id: &str,
    session_nonce: u32,
    f: impl FnOnce(&session::ActiveSerial) -> T,
) -> Result<T, String> {
    let map = state.read().await;
    let session = map
        .get(tab_id)
        .filter(|session| session.session_nonce == session_nonce)
        .ok_or_else(|| "This tab's serial session has ended".to_string())?;
    let active = session.active.lock().await;
    match active.as_ref() {
        Some(ActiveSession::Serial(serial)) => Ok(f(serial)),
        Some(_) => Err("This tab is not a serial connection".to_string()),
        None => Err("The serial device is disconnected".to_string()),
    }
}
