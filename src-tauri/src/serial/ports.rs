use super::{SerialOptions, SerialPortEntry, SerialUsbId};
use serialport::{SerialPortInfo, SerialPortType};

/// The serial ports this machine has, in the order a person counts them.
pub fn list_ports() -> Result<Vec<SerialPortEntry>, String> {
    let ports = serialport::available_ports()
        .map_err(|err| format!("Failed to list serial ports: {}", err.description))?;
    Ok(visible_ports(ports, cfg!(target_os = "macos")))
}

/// Turns what the OS reports into the dialog's list. macOS lists every device
/// twice, as `/dev/tty.*` for answering calls and `/dev/cu.*` for placing
/// them; a terminal places them, so only the `cu` twin is kept.
pub(super) fn visible_ports(ports: Vec<SerialPortInfo>, macos: bool) -> Vec<SerialPortEntry> {
    let mut entries: Vec<SerialPortEntry> = ports
        .into_iter()
        .filter(|port| !(macos && port.port_name.starts_with("/dev/tty.")))
        .map(entry_from)
        .collect();
    entries.sort_by(|a, b| natural_key(&a.path).cmp(&natural_key(&b.path)));
    entries.dedup_by(|a, b| a.path == b.path);
    entries
}

fn entry_from(port: SerialPortInfo) -> SerialPortEntry {
    let (kind, usb, manufacturer, product) = match port.port_type {
        SerialPortType::UsbPort(info) => (
            "usb",
            Some(SerialUsbId {
                vid: info.vid,
                pid: info.pid,
                serial_number: info.serial_number.filter(|value| !value.trim().is_empty()),
            }),
            info.manufacturer,
            info.product,
        ),
        SerialPortType::PciPort => ("pci", None, None, None),
        SerialPortType::BluetoothPort => ("bluetooth", None, None, None),
        SerialPortType::Unknown => ("unknown", None, None, None),
    };
    SerialPortEntry {
        path: port.port_name,
        kind,
        usb,
        manufacturer,
        product,
    }
}

/// Sorts `COM2` before `COM10` and `ttyUSB2` before `ttyUSB10`.
fn natural_key(path: &str) -> (String, u64, String) {
    let digits_start = path
        .rfind(|c: char| !c.is_ascii_digit())
        .map_or(0, |index| index + 1);
    let (stem, digits) = path.split_at(digits_start);
    (
        stem.to_ascii_lowercase(),
        digits.parse().unwrap_or(0),
        path.to_string(),
    )
}

/// Whether a port's device is the saved one. A saved serial number must match
/// too; one the device doesn't report can't tell two adapters apart.
fn is_device(saved: &SerialUsbId, port: &SerialPortEntry) -> bool {
    port.usb.as_ref().is_some_and(|usb| {
        usb.vid == saved.vid
            && usb.pid == saved.pid
            && (saved.serial_number.is_none() || usb.serial_number == saved.serial_number)
    })
}

/// The port a serial connection opens: the saved device wherever it is now,
/// else the saved port, unless another device has taken that name. `None`
/// when neither is to be found.
///
/// A port the OS doesn't list, such as a pseudo terminal a test pairs with
/// `socat`, is still opened by its name.
pub fn port_to_open(options: &SerialOptions, ports: &[SerialPortEntry]) -> Option<String> {
    let listed = ports.iter().find(|port| port.path == options.port);
    let Some(saved) = options.usb.as_ref() else {
        return Some(options.port.clone());
    };
    let devices: Vec<&SerialPortEntry> =
        ports.iter().filter(|port| is_device(saved, port)).collect();
    match listed {
        // A device that reports nothing about itself can't be told apart.
        Some(port) if port.usb.is_none() || is_device(saved, port) => Some(port.path.clone()),
        None if devices.is_empty() => Some(options.port.clone()),
        _ => match devices.as_slice() {
            [device] => Some(device.path.clone()),
            _ => None,
        },
    }
}
