//! Serial connection settings and port descriptions, from `src/lib/serial.ts`.

use super::{
    SerialFlowControl, SerialLineEnding, SerialOptions, SerialParity, SerialPortEntry,
    SerialReceiveNewline, SerialUsbId,
};
use crate::js::trim_js;
use serde_json::Value;
use tterm_terminal::theme::js::js_number;

/// The baud rates the connection dialog offers.
pub const SERIAL_BAUD_RATES: [u32; 20] = [
    300, 1_200, 2_400, 4_800, 9_600, 14_400, 19_200, 38_400, 57_600, 74_880, 115_200, 230_400,
    250_000, 460_800, 500_000, 921_600, 1_000_000, 1_500_000, 2_000_000, 3_000_000,
];

/// The data-bit counts a serial port accepts.
pub const SERIAL_DATA_BITS: [u8; 4] = [5, 6, 7, 8];

/// The stop-bit counts a serial port accepts.
pub const SERIAL_STOP_BITS: [u8; 2] = [1, 2];

/// The settings used when none were saved.
pub fn default_serial_options() -> SerialOptions {
    SerialOptions {
        port: String::new(),
        baud_rate: 115_200,
        data_bits: 8,
        parity: SerialParity::None,
        stop_bits: 1,
        flow_control: SerialFlowControl::None,
        line_ending: SerialLineEnding::Cr,
        receive_newline: SerialReceiveNewline::None,
        local_echo: false,
        timestamps: false,
        hex_view: false,
        usb: None,
    }
}

fn json_number(value: Option<&Value>) -> f64 {
    match value {
        Some(Value::Number(number)) => number.as_f64().unwrap_or(f64::NAN),
        Some(Value::String(text)) => js_number(text),
        Some(Value::Bool(value)) => u8::from(*value).into(),
        Some(Value::Null) | None => 0.0,
        _ => f64::NAN,
    }
}

fn integer_u8(value: Option<&Value>) -> Option<u8> {
    let number = json_number(value);
    if number.is_finite() && number.fract() == 0.0 && (0.0..=255.0).contains(&number) {
        Some(number as u8)
    } else {
        None
    }
}

fn integer_u16(value: Option<&Value>) -> Option<u16> {
    let number = json_number(value);
    if number.is_finite() && number.fract() == 0.0 && (0.0..=65535.0).contains(&number) {
        Some(number as u16)
    } else {
        None
    }
}

fn one_of_data_bits(value: Option<&Value>) -> u8 {
    integer_u8(value)
        .filter(|value| SERIAL_DATA_BITS.contains(value))
        .unwrap_or(8)
}

fn one_of_stop_bits(value: Option<&Value>) -> u8 {
    integer_u8(value)
        .filter(|value| SERIAL_STOP_BITS.contains(value))
        .unwrap_or(1)
}

fn parity(value: Option<&Value>) -> SerialParity {
    match value.and_then(Value::as_str) {
        Some("odd") => SerialParity::Odd,
        Some("even") => SerialParity::Even,
        _ => SerialParity::None,
    }
}

fn flow_control(value: Option<&Value>) -> SerialFlowControl {
    match value.and_then(Value::as_str) {
        Some("software") => SerialFlowControl::Software,
        Some("hardware") => SerialFlowControl::Hardware,
        _ => SerialFlowControl::None,
    }
}

fn line_ending(value: Option<&Value>) -> SerialLineEnding {
    match value.and_then(Value::as_str) {
        Some("lf") => SerialLineEnding::Lf,
        Some("crlf") => SerialLineEnding::Crlf,
        _ => SerialLineEnding::Cr,
    }
}

fn receive_newline(value: Option<&Value>) -> SerialReceiveNewline {
    match value.and_then(Value::as_str) {
        Some("cr") => SerialReceiveNewline::Cr,
        Some("lf") => SerialReceiveNewline::Lf,
        _ => SerialReceiveNewline::None,
    }
}

fn normalize_usb(value: Option<&Value>) -> Option<SerialUsbId> {
    let Value::Object(value) = value? else {
        return None;
    };
    Some(SerialUsbId {
        vid: integer_u16(value.get("vid"))?,
        pid: integer_u16(value.get("pid"))?,
        serial_number: value
            .get("serialNumber")
            .and_then(Value::as_str)
            .filter(|serial| !serial.is_empty())
            .map(str::to_owned),
    })
}

/// Saved or typed settings with values a serial port cannot open replaced by
/// their defaults.
pub fn normalize_serial_settings(value: &Value) -> SerialOptions {
    let raw = match value {
        Value::Object(value) => Some(value),
        _ => None,
    };
    let baud = json_number(raw.and_then(|raw| raw.get("baudRate")));
    let baud_rate =
        if baud.is_finite() && baud.fract() == 0.0 && baud > 0.0 && baud <= f64::from(u32::MAX) {
            baud as u32
        } else {
            115_200
        };
    SerialOptions {
        port: raw
            .and_then(|raw| raw.get("port"))
            .and_then(Value::as_str)
            .map(trim_js)
            .unwrap_or_default()
            .to_owned(),
        baud_rate,
        data_bits: one_of_data_bits(raw.and_then(|raw| raw.get("dataBits"))),
        parity: parity(raw.and_then(|raw| raw.get("parity"))),
        stop_bits: one_of_stop_bits(raw.and_then(|raw| raw.get("stopBits"))),
        flow_control: flow_control(raw.and_then(|raw| raw.get("flowControl"))),
        line_ending: line_ending(raw.and_then(|raw| raw.get("lineEnding"))),
        receive_newline: receive_newline(raw.and_then(|raw| raw.get("receiveNewline"))),
        local_echo: raw
            .and_then(|raw| raw.get("localEcho"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        timestamps: raw
            .and_then(|raw| raw.get("timestamps"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        hex_view: raw
            .and_then(|raw| raw.get("hexView"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        usb: normalize_usb(raw.and_then(|raw| raw.get("usb"))),
    }
}

/// The framing in the usual shorthand: `8N1`, `7E2`.
pub fn serial_framing(settings: &SerialOptions) -> String {
    let parity = match settings.parity {
        SerialParity::None => "N",
        SerialParity::Odd => "O",
        SerialParity::Even => "E",
    };
    format!("{}{}{}", settings.data_bits, parity, settings.stop_bits)
}

/// `115200 8N1`.
pub fn serial_summary(settings: &SerialOptions) -> String {
    format!("{} {}", settings.baud_rate, serial_framing(settings))
}

/// `1A86:7523`, as device managers show a USB device.
pub fn format_usb_id(usb: &SerialUsbId) -> String {
    format!("{:04X}:{:04X}", usb.vid, usb.pid)
}

/// What a port is, for the dialog's list: its product, else its maker.
pub fn serial_port_description(entry: &SerialPortEntry) -> Option<String> {
    entry
        .product
        .as_deref()
        .map(trim_js)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            entry
                .manufacturer
                .as_deref()
                .map(trim_js)
                .filter(|value| !value.is_empty())
        })
        .map(str::to_owned)
}

/// Settings pointed at `entry`, remembering its USB device.
pub fn with_serial_port(mut settings: SerialOptions, entry: &SerialPortEntry) -> SerialOptions {
    settings.port = entry.path.clone();
    settings.usb = entry.usb.clone();
    settings
}

#[cfg(test)]
mod tests {
    use super::{
        default_serial_options, format_usb_id, normalize_serial_settings, serial_port_description,
        serial_summary, with_serial_port,
    };
    use crate::serial::{SerialParity, SerialPortEntry, SerialUsbId};
    use serde_json::json;

    #[test]
    fn fills_in_default_framing_for_missing_settings() {
        let mut expected = default_serial_options();
        expected.port = "COM3".to_owned();
        assert_eq!(
            normalize_serial_settings(&json!({ "port": " COM3 " })),
            expected
        );
    }

    #[test]
    fn replaces_settings_no_port_can_open_with() {
        let mut expected = default_serial_options();
        expected.port = "COM3".to_owned();
        assert_eq!(
            normalize_serial_settings(&json!({
                "port": "COM3",
                "baudRate": -5,
                "dataBits": 9,
                "parity": "mark",
                "stopBits": 3,
                "flowControl": "dsr",
                "lineEnding": "lfcr",
                "receiveNewline": "crlf",
                "usb": { "vid": "x", "pid": 1 }
            })),
            expected
        );
    }

    #[test]
    fn keeps_a_saved_device() {
        let settings = normalize_serial_settings(&json!({
            "port": "/dev/ttyUSB0",
            "baudRate": 921600,
            "parity": "even",
            "receiveNewline": "cr",
            "usb": { "vid": 0x1a86, "pid": 0x7523, "serialNumber": "" }
        }));
        assert_eq!(settings.baud_rate, 921_600);
        assert_eq!(settings.parity, SerialParity::Even);
        assert_eq!(
            settings.receive_newline,
            crate::serial::SerialReceiveNewline::Cr
        );
        assert_eq!(
            settings.usb,
            Some(SerialUsbId {
                vid: 0x1a86,
                pid: 0x7523,
                serial_number: None,
            })
        );
        assert_eq!(serial_summary(&settings), "921600 8E1");
    }

    #[test]
    fn shows_a_usb_device_as_device_managers_do() {
        assert_eq!(
            format_usb_id(&SerialUsbId {
                vid: 0x1a86,
                pid: 0x7523,
                serial_number: None,
            }),
            "1A86:7523"
        );
    }

    #[test]
    fn describes_a_port_by_product_else_maker() {
        let port = |product: Option<&str>, manufacturer: Option<&str>| SerialPortEntry {
            path: "COM3".to_owned(),
            kind: "usb",
            usb: None,
            manufacturer: manufacturer.map(str::to_owned),
            product: product.map(str::to_owned),
        };
        assert_eq!(
            serial_port_description(&port(Some("CH340"), None)).as_deref(),
            Some("CH340")
        );
        assert_eq!(
            serial_port_description(&port(None, Some("FTDI"))).as_deref(),
            Some("FTDI")
        );
        assert_eq!(
            serial_port_description(&SerialPortEntry {
                path: "COM1".to_owned(),
                kind: "pci",
                usb: None,
                manufacturer: None,
                product: None,
            }),
            None
        );
    }

    #[test]
    fn remembers_a_chosen_ports_device_and_forgets_it_without_one() {
        let usb = SerialUsbId {
            vid: 1,
            pid: 2,
            serial_number: Some("A".to_owned()),
        };
        let chosen = with_serial_port(
            default_serial_options(),
            &SerialPortEntry {
                path: "COM4".to_owned(),
                kind: "usb",
                usb: Some(usb.clone()),
                manufacturer: None,
                product: None,
            },
        );
        assert_eq!(chosen.port, "COM4");
        assert_eq!(chosen.usb, Some(usb));
        assert_eq!(
            with_serial_port(
                chosen,
                &SerialPortEntry {
                    path: "COM1".to_owned(),
                    kind: "pci",
                    usb: None,
                    manufacturer: None,
                    product: None,
                }
            )
            .usb,
            None
        );
    }
}
