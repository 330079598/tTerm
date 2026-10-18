use super::io::{SerialReader, SerialWriter};
use super::open::describe_open_error;
use super::ports::{port_to_open, visible_ports};
use super::session::encode_line_ending;
use super::{SerialLineEnding, SerialOptions, SerialPortEntry, SerialUsbId};
use serialport::{SerialPortInfo, SerialPortType, UsbPortInfo};
use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn options(port: &str, usb: Option<SerialUsbId>) -> SerialOptions {
    let mut options: SerialOptions =
        serde_json::from_value(serde_json::json!({ "port": port })).unwrap();
    options.usb = usb;
    options
}

fn usb(vid: u16, pid: u16, serial_number: Option<&str>) -> SerialUsbId {
    SerialUsbId {
        vid,
        pid,
        serial_number: serial_number.map(str::to_string),
    }
}

fn entry(path: &str, usb: Option<SerialUsbId>) -> SerialPortEntry {
    SerialPortEntry {
        path: path.to_string(),
        kind: if usb.is_some() { "usb" } else { "pci" },
        usb,
        manufacturer: None,
        product: None,
    }
}

fn info(path: &str, port_type: SerialPortType) -> SerialPortInfo {
    SerialPortInfo {
        port_name: path.to_string(),
        port_type,
    }
}

#[test]
fn options_default_to_115200_8n1() {
    let options = options(" COM3 ", None).validated().unwrap();
    assert_eq!(options.port, "COM3");
    assert_eq!(options.baud_rate, 115_200);
    assert_eq!((options.data_bits, options.stop_bits), (8, 1));
    assert_eq!(options.line_ending, SerialLineEnding::Cr);
}

#[test]
fn options_reject_what_no_port_opens_with() {
    assert!(options("  ", None).validated().is_err());
    let mut bad = options("COM3", None);
    bad.data_bits = 9;
    assert!(bad.validated().is_err());
    let mut bad = options("COM3", None);
    bad.stop_bits = 3;
    assert!(bad.validated().is_err());
    let mut bad = options("COM3", None);
    bad.baud_rate = 0;
    assert!(bad.validated().is_err());
}

#[test]
fn ports_sort_by_their_number_and_hide_macos_dial_in_twins() {
    let ports = visible_ports(
        vec![
            info("COM10", SerialPortType::PciPort),
            info("/dev/tty.usbserial-1", SerialPortType::Unknown),
            info("COM2", SerialPortType::PciPort),
            info("/dev/cu.usbserial-1", SerialPortType::Unknown),
        ],
        true,
    );
    let paths: Vec<&str> = ports.iter().map(|port| port.path.as_str()).collect();
    assert_eq!(paths, ["/dev/cu.usbserial-1", "COM2", "COM10"]);
}

#[test]
fn usb_ports_carry_their_device() {
    let ports = visible_ports(
        vec![info(
            "/dev/ttyUSB0",
            SerialPortType::UsbPort(UsbPortInfo {
                vid: 0x1a86,
                pid: 0x7523,
                serial_number: Some(" ".to_string()),
                manufacturer: Some("QinHeng".to_string()),
                product: Some("CH340".to_string()),
            }),
        )],
        false,
    );
    assert_eq!(ports[0].kind, "usb");
    assert_eq!(ports[0].usb, Some(usb(0x1a86, 0x7523, None)));
    assert_eq!(ports[0].product.as_deref(), Some("CH340"));
}

#[test]
fn a_connection_without_a_device_opens_its_port_by_name() {
    assert_eq!(
        port_to_open(&options("/dev/pts/4", None), &[]).as_deref(),
        Some("/dev/pts/4")
    );
}

#[test]
fn a_device_is_found_under_its_new_name() {
    let saved = options("/dev/ttyUSB0", Some(usb(1, 2, Some("A1"))));
    let ports = [
        entry("/dev/ttyUSB1", Some(usb(1, 2, Some("A1")))),
        entry("/dev/ttyS0", None),
    ];
    assert_eq!(
        port_to_open(&saved, &ports).as_deref(),
        Some("/dev/ttyUSB1")
    );
}

#[test]
fn another_device_under_the_saved_name_is_not_opened() {
    let saved = options("COM3", Some(usb(1, 2, Some("A1"))));
    let ports = [entry("COM3", Some(usb(1, 2, Some("B2"))))];
    assert_eq!(port_to_open(&saved, &ports), None);
}

#[test]
fn two_alike_devices_without_serial_numbers_keep_the_saved_name() {
    let saved = options("COM4", Some(usb(1, 2, None)));
    let ports = [
        entry("COM3", Some(usb(1, 2, None))),
        entry("COM4", Some(usb(1, 2, None))),
    ];
    assert_eq!(port_to_open(&saved, &ports).as_deref(), Some("COM4"));
    let gone = options("COM5", Some(usb(1, 2, None)));
    assert_eq!(port_to_open(&gone, &ports), None);
}

#[test]
fn an_unplugged_device_is_tried_under_its_saved_name() {
    let saved = options("COM3", Some(usb(1, 2, None)));
    assert_eq!(port_to_open(&saved, &[]).as_deref(), Some("COM3"));
}

#[test]
fn enter_sends_the_chosen_line_ending() {
    assert_eq!(&*encode_line_ending(SerialLineEnding::Cr, b"ls\r"), b"ls\r");
    assert_eq!(
        &*encode_line_ending(SerialLineEnding::Lf, b"a\rb\r"),
        b"a\nb\n"
    );
    assert_eq!(
        &*encode_line_ending(SerialLineEnding::Crlf, b"at\r"),
        b"at\r\n"
    );
    assert_eq!(&*encode_line_ending(SerialLineEnding::Crlf, b"x"), b"x");
}

#[test]
fn open_errors_say_what_to_do() {
    let busy = serialport::Error::new(serialport::ErrorKind::NoDevice, "busy");
    assert!(describe_open_error("COM3", &busy, true, false).contains("in use"));
    assert!(describe_open_error("COM3", &busy, false, false).contains("not found"));
    let denied = serialport::Error::new(
        serialport::ErrorKind::Io(io::ErrorKind::PermissionDenied),
        "denied",
    );
    assert!(describe_open_error("/dev/ttyUSB0", &denied, true, true).contains("dialout"));
    assert!(!describe_open_error("/dev/cu.x", &denied, true, false).contains("dialout"));
}

/// Plays back reads and records writes, the way a port with a timeout behaves.
struct FakePort {
    reads: VecDeque<io::Result<Vec<u8>>>,
    written: Vec<u8>,
    write_timeouts: usize,
}

impl Read for FakePort {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.reads.pop_front() {
            Some(Ok(bytes)) => {
                buf[..bytes.len()].copy_from_slice(&bytes);
                Ok(bytes.len())
            }
            Some(Err(err)) => Err(err),
            None => Err(io::Error::new(io::ErrorKind::TimedOut, "timed out")),
        }
    }
}

impl Write for FakePort {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.write_timeouts > 0 {
            self.write_timeouts -= 1;
            return Err(io::Error::new(io::ErrorKind::TimedOut, "timed out"));
        }
        let n = buf.len().min(2);
        self.written.extend_from_slice(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn fake(reads: Vec<io::Result<Vec<u8>>>) -> FakePort {
    FakePort {
        reads: reads.into(),
        written: Vec::new(),
        write_timeouts: 0,
    }
}

#[test]
fn the_reader_waits_out_timeouts() {
    let timeout = || Err(io::Error::new(io::ErrorKind::TimedOut, "timed out"));
    let port = fake(vec![timeout(), timeout(), Ok(b"hi".to_vec())]);
    let mut reader = SerialReader::new(port, Arc::new(AtomicBool::new(false)), Default::default());
    let mut buf = [0u8; 8];
    assert_eq!(reader.read(&mut buf).unwrap(), 2);
    assert_eq!(&buf[..2], b"hi");
}

#[test]
fn the_reader_ends_once_the_tab_lets_go() {
    let closed = Arc::new(AtomicBool::new(false));
    let mut reader = SerialReader::new(fake(Vec::new()), closed.clone(), Default::default());
    closed.store(true, Ordering::Relaxed);
    assert_eq!(reader.read(&mut [0u8; 8]).unwrap(), 0);
}

#[test]
fn a_port_that_reads_nothing_was_unplugged() {
    let mut reader = SerialReader::new(
        fake(vec![Ok(Vec::new())]),
        Arc::new(AtomicBool::new(false)),
        Default::default(),
    );
    let err = reader.read(&mut [0u8; 8]).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::NotConnected);
}

#[test]
fn the_writer_waits_out_timeouts_and_short_writes() {
    let mut port = fake(Vec::new());
    port.write_timeouts = 3;
    let mut writer = SerialWriter::new(port, Arc::new(AtomicBool::new(false)));
    writer.write_all(b"hello").unwrap();
    assert_eq!(writer.into_inner().written, b"hello");
}

#[test]
fn the_writer_fails_once_the_tab_lets_go() {
    let closed = Arc::new(AtomicBool::new(true));
    let mut writer = SerialWriter::new(fake(Vec::new()), closed);
    assert_eq!(
        writer.write_all(b"x").unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
}

#[test]
fn a_saved_profile_keeps_its_serial_settings() {
    let profile: crate::profiles::types::SavedProfile = serde_json::from_value(serde_json::json!({
        "id": "board", "name": "ESP32", "connection_type": "serial",
        "host": null, "port": null, "username": null, "auth_method": null,
        "private_key_path": null,
        "serial": { "port": "COM3", "baudRate": 921600, "lineEnding": "crlf",
                    "usb": { "vid": 4292, "pid": 60000, "serialNumber": "0001" } },
    }))
    .unwrap();
    let serial = profile.serial.clone().unwrap();
    assert_eq!(serial.baud_rate, 921_600);
    assert_eq!(serial.line_ending, SerialLineEnding::Crlf);
    let saved = serde_json::to_value(&profile).unwrap();
    assert_eq!(saved["serial"]["usb"]["serialNumber"], "0001");
    assert_eq!(saved["serial"]["parity"], "none");
}

#[test]
fn a_serial_connection_plans_a_serial_session() {
    let connection: crate::core::session::types::PtyConnectionOptions =
        serde_json::from_value(serde_json::json!({
            "type": "serial",
            "serial": { "port": "/dev/ttyUSB0", "baudRate": 9600 },
        }))
        .unwrap();
    let plan = crate::core::session::normalize_connection(Some(connection)).unwrap();
    assert_eq!(plan.kind, crate::core::state::SessionKind::Serial);
    assert_eq!(plan.profile_name, "/dev/ttyUSB0");
    assert!(plan.reconnect_enabled);
    assert_eq!(plan.serial.unwrap().options.baud_rate, 9600);
}

#[test]
fn typed_text_goes_out_in_the_device_charset() {
    let gbk = crate::terminal::encoding::TerminalEncoding::from_label(Some("gbk"));
    let sent = super::session::encode_serial_input(SerialLineEnding::Crlf, gbk, "中\r".as_bytes());
    assert_eq!(&*sent, b"\xd6\xd0\r\n");
    let utf8 = crate::terminal::encoding::TerminalEncoding::default();
    let sent = super::session::encode_serial_input(SerialLineEnding::Cr, utf8, b"ls\r");
    assert!(matches!(sent, std::borrow::Cow::Borrowed(_)));
}

#[test]
fn output_in_another_charset_shows_as_utf8_even_split_across_reads() {
    use crate::terminal::reader::OutputFilter;
    let gbk = crate::terminal::encoding::TerminalEncoding::from_label(Some("gbk"));
    let mut output = super::output::SerialOutput::new(gbk, Default::default());
    let first = output.filter(b"ok \xd6".to_vec());
    let second = output.filter(b"\xd0\r\n".to_vec());
    let shown = [first.display, second.display].concat();
    assert_eq!(String::from_utf8(shown).unwrap(), "ok 中\r\n");
}

fn hex_output() -> (super::output::SerialOutput, Arc<super::SerialView>) {
    let view = Arc::new(super::SerialView::default());
    view.set_hex(true);
    let gbk = crate::terminal::encoding::TerminalEncoding::from_label(Some("gbk"));
    (super::output::SerialOutput::new(gbk, view.clone()), view)
}

#[test]
fn the_hex_view_shows_the_bytes_sent_sixteen_a_line() {
    let (mut output, _) = hex_output();
    let now = std::time::Instant::now();
    let shown = output.filter_at((0u8..18).collect(), now);
    assert_eq!(
        String::from_utf8(shown.display).unwrap(),
        "00 01 02 03 04 05 06 07 08 09 0A 0B 0C 0D 0E 0F \r\n10 11 "
    );
    // The log keeps the text, decoded, not the hex.
    let gbk = output.filter_at(b"\xd6\xd0".to_vec(), now);
    assert_eq!(String::from_utf8(gbk.display).unwrap(), "D6 D0 ");
    assert_eq!(String::from_utf8(gbk.log.unwrap()).unwrap(), "中");
}

#[test]
fn a_pause_starts_a_new_hex_line() {
    let (mut output, _) = hex_output();
    let now = std::time::Instant::now();
    output.filter_at(b"AT".to_vec(), now);
    let later = now + std::time::Duration::from_millis(80);
    let shown = output.filter_at(b"OK".to_vec(), later);
    assert_eq!(String::from_utf8(shown.display).unwrap(), "\r\n4F 4B ");
    let soon = later + std::time::Duration::from_millis(5);
    let shown = output.filter_at(b"!".to_vec(), soon);
    assert_eq!(String::from_utf8(shown.display).unwrap(), "21 ");
}

#[test]
fn switching_views_starts_a_new_line() {
    let (mut output, view) = hex_output();
    let now = std::time::Instant::now();
    output.filter_at(b"A".to_vec(), now);
    view.set_hex(false);
    let text = output.filter_at(b"boot".to_vec(), now);
    assert_eq!(text.display, b"\r\nboot");
    view.set_hex(true);
    let hex = output.filter_at(b"B".to_vec(), now);
    assert_eq!(String::from_utf8(hex.display).unwrap(), "\r\n42 ");
}

/// A port whose second read finds the transfer gone, as when one ends
/// between two reads.
struct EndingTransfer {
    tap: super::io::SerialTap,
    reads: usize,
}

impl Read for EndingTransfer {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.reads += 1;
        let bytes: &[u8] = if self.reads == 1 {
            b"C"
        } else {
            *self.tap.lock().unwrap() = None;
            b"hi"
        };
        buf[..bytes.len()].copy_from_slice(bytes);
        Ok(bytes.len())
    }
}

#[test]
fn a_transfer_takes_the_input_until_it_lets_go() {
    let tap = super::io::SerialTap::default();
    let (sender, incoming) = std::sync::mpsc::channel();
    *tap.lock().unwrap() = Some(sender);
    let port = EndingTransfer {
        tap: tap.clone(),
        reads: 0,
    };
    let mut reader = SerialReader::new(port, Arc::new(AtomicBool::new(false)), tap);
    let mut buf = [0u8; 8];
    let read = reader.read(&mut buf).unwrap();
    assert_eq!(&buf[..read], b"hi");
    assert_eq!(incoming.try_recv().unwrap(), b"C");
}
