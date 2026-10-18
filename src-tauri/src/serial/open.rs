use super::{SerialFlowControl, SerialOptions, SerialParity};
use serialport::{DataBits, ErrorKind, FlowControl, Parity, SerialPort, StopBits};
use std::time::Duration;

/// How long one read waits for data before the reader checks whether its tab
/// is closing, and how long one write waits on a held-off line before it
/// tries again.
pub(super) const READ_TIMEOUT: Duration = Duration::from_millis(20);

/// Opens the port at `path` with the connection's settings.
pub(super) fn open_port(
    options: &SerialOptions,
    path: &str,
) -> Result<Box<dyn SerialPort>, String> {
    serialport::new(path, options.baud_rate)
        .data_bits(data_bits(options.data_bits))
        .parity(match options.parity {
            SerialParity::None => Parity::None,
            SerialParity::Odd => Parity::Odd,
            SerialParity::Even => Parity::Even,
        })
        .stop_bits(if options.stop_bits == 2 {
            StopBits::Two
        } else {
            StopBits::One
        })
        .flow_control(match options.flow_control {
            SerialFlowControl::None => FlowControl::None,
            SerialFlowControl::Software => FlowControl::Software,
            SerialFlowControl::Hardware => FlowControl::Hardware,
        })
        .timeout(READ_TIMEOUT)
        .open()
        .map_err(|err| {
            // Both a missing port and one another program holds come back as
            // `NoDevice` on Windows; the port list tells them apart.
            let listed = super::ports::list_ports()
                .is_ok_and(|ports| ports.iter().any(|port| port.path == path));
            describe_open_error(path, &err, listed, cfg!(target_os = "linux"))
        })
}

fn data_bits(bits: u8) -> DataBits {
    match bits {
        5 => DataBits::Five,
        6 => DataBits::Six,
        7 => DataBits::Seven,
        _ => DataBits::Eight,
    }
}

/// Says why a port didn't open and what to do about it.
pub(super) fn describe_open_error(
    path: &str,
    err: &serialport::Error,
    listed: bool,
    linux: bool,
) -> String {
    match err.kind() {
        ErrorKind::Io(std::io::ErrorKind::PermissionDenied) if linux => format!(
            "You have no permission to open {path}. Add your user to the dialout group \
             (sudo usermod -aG dialout $USER), then log out and back in."
        ),
        ErrorKind::Io(std::io::ErrorKind::PermissionDenied) => {
            format!("You have no permission to open {path}.")
        }
        ErrorKind::NoDevice if listed => format!(
            "{path} is in use by another program, such as a serial monitor or an IDE. \
             Close it and connect again."
        ),
        ErrorKind::NoDevice | ErrorKind::Io(std::io::ErrorKind::NotFound) => {
            format!("{path} was not found. Check that the device is plugged in.")
        }
        ErrorKind::InvalidInput => format!(
            "{path} does not support these settings: {}",
            err.description
        ),
        _ => format!("Failed to open {path}: {}", err.description),
    }
}
